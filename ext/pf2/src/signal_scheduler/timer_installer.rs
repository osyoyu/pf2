use std::collections::HashMap;
use std::ffi::c_void;
use std::mem;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::sync::{Mutex, RwLock};
use std::{collections::HashSet, sync::Arc};

use rb_sys::*;

use crate::signal_scheduler::SignalHandlerArgs;

use super::configuration::Configuration;
use crate::profile::Profile;

// We could avoid deferring the timer creation by combining pthread_getcpuclockid(3) and timer_create(2) here,
// but we're not doing so since (1) Ruby does not expose the pthread_self() of a Ruby Thread
// (which is actually stored in th->nt->thread_id), and (2) pthread_getcpuclockid(3) is not portable
// in the first place (e.g. not available on macOS).
pub struct TimerInstaller {
    internal: Box<Mutex<Internal>>,
}

struct Internal {
    configuration: Configuration,
    target_ruby_threads: HashSet<VALUE>,
    registered_pthread_ids: HashSet<libc::pthread_t>,
    kernel_thread_id_to_ruby_thread_map: HashMap<libc::pid_t, VALUE>,
    profile: Arc<RwLock<Profile>>,
}

impl TimerInstaller {
    // Register a callback that gets called when a Ruby Thread is resumed.
    // The callback should create a timer for the thread.
    pub fn install_timer_to_ruby_threads(
        configuration: Configuration,
        ruby_threads: &HashSet<VALUE>,
        profile: Arc<RwLock<Profile>>,
        track_new_threads: bool,
    ) {
        let registrar = Self {
            internal: Box::new(Mutex::new(Internal {
                configuration,
                target_ruby_threads: ruby_threads.clone(),
                registered_pthread_ids: HashSet::new(),
                kernel_thread_id_to_ruby_thread_map: HashMap::new(),
                profile,
            })),
        };

        let ptr = Box::into_raw(registrar.internal);
        unsafe {
            rb_internal_thread_add_event_hook(
                Some(Self::on_thread_resume),
                RUBY_INTERNAL_THREAD_EVENT_RESUMED,
                ptr as *mut c_void,
            );
            // Spawn a no-op Thread to fire the event hook
            // (at least 2 Ruby Threads must be active for the RESUMED hook to be fired)
            rb_thread_create(Some(Self::do_nothing), null_mut());
        };

        if track_new_threads {
            unsafe {
                rb_internal_thread_add_event_hook(
                    Some(Self::on_thread_start),
                    RUBY_INTERNAL_THREAD_EVENT_STARTED,
                    ptr as *mut c_void,
                );
            };
        }
    }

    unsafe extern "C" fn do_nothing(_: *mut c_void) -> VALUE {
        Qnil.into()
    }

    // Thread resume callback
    unsafe extern "C" fn on_thread_resume(
        _flag: rb_event_flag_t,
        data: *const rb_internal_thread_event_data,
        custom_data: *mut c_void,
    ) {
        // The SignalScheduler (as a Ruby obj) should be passed as custom_data
        let internal =
            unsafe { ManuallyDrop::new(Box::from_raw(custom_data as *mut Mutex<Internal>)) };
        let mut internal = internal.lock().unwrap();

        // Check if the current thread is a target Ruby Thread
        let current_ruby_thread: VALUE = unsafe { (*data).thread };
        if !internal.target_ruby_threads.contains(&current_ruby_thread) {
            return;
        }

        // Check if the current thread is already registered
        let current_pthread_id = unsafe { libc::pthread_self() };
        if internal
            .registered_pthread_ids
            .contains(&current_pthread_id)
        {
            return;
        }

        // Record the pthread ID of the current thread
        internal.registered_pthread_ids.insert(current_pthread_id);
        // Keep a mapping from kernel thread ID to Ruby Thread
        internal
            .kernel_thread_id_to_ruby_thread_map
            .insert(unsafe { libc::gettid() }, current_ruby_thread);

        Self::register_timer_to_current_thread(
            &internal.configuration,
            &internal.profile,
            &internal.kernel_thread_id_to_ruby_thread_map,
        );

        // TODO: Remove the hook when all threads have been registered
    }

    // Thread resume callback
    unsafe extern "C" fn on_thread_start(
        _flag: rb_event_flag_t,
        data: *const rb_internal_thread_event_data,
        custom_data: *mut c_void,
    ) {
        // The SignalScheduler (as a Ruby obj) should be passed as custom_data
        let internal =
            unsafe { ManuallyDrop::new(Box::from_raw(custom_data as *mut Mutex<Internal>)) };
        let mut internal = internal.lock().unwrap();

        let current_ruby_thread: VALUE = unsafe { (*data).thread };
        internal.target_ruby_threads.insert(current_ruby_thread);
    }

    // Creates a new POSIX timer which invocates sampling for the thread that called this function.
    fn register_timer_to_current_thread(
        configuration: &Configuration,
        profile: &Arc<RwLock<Profile>>,
        kernel_thread_id_to_ruby_thread_map: &HashMap<libc::pid_t, VALUE>,
    ) {
        let current_pthread_id = unsafe { libc::pthread_self() };
        let context_ruby_thread: VALUE = unsafe {
            *(kernel_thread_id_to_ruby_thread_map
                .get(&(libc::gettid()))
                .unwrap())
        };

        // NOTE: This Box is never dropped
        let signal_handler_args = Box::new(SignalHandlerArgs {
            profile: Arc::clone(profile),
            context_ruby_thread,
        });

        // Create a signal event
        let mut sigevent: libc::sigevent = unsafe { mem::zeroed() };
        // Note: SIGEV_THREAD_ID is Linux-specific. In other platforms, we would need to
        // "tranpoline" the signal as any pthread can receive the signal.
        sigevent.sigev_notify = libc::SIGEV_THREAD_ID;
        sigevent.sigev_notify_thread_id =
            unsafe { libc::syscall(libc::SYS_gettid).try_into().unwrap() }; // The kernel thread ID
        sigevent.sigev_signo = libc::SIGALRM;
        // Pass required args to the signal handler
        sigevent.sigev_value.sival_ptr = Box::into_raw(signal_handler_args) as *mut c_void;

        // Create and configure timer to fire every 10 ms of CPU time
        let mut timer: libc::timer_t = unsafe { mem::zeroed() };
        match configuration.time_mode {
            crate::signal_scheduler::TimeMode::CpuTime => {
                let err = unsafe {
                    libc::timer_create(libc::CLOCK_THREAD_CPUTIME_ID, &mut sigevent, &mut timer)
                };
                if err != 0 {
                    panic!("timer_create failed: {}", err);
                }
            }
            crate::signal_scheduler::TimeMode::WallTime => {
                todo!("WallTime is not supported yet");
            }
        };
        let mut its: libc::itimerspec = unsafe { mem::zeroed() };
        its.it_interval.tv_sec = 0;
        its.it_interval.tv_nsec = 10_000_000; // 10 ms
        its.it_value.tv_sec = 0;
        its.it_value.tv_nsec = 10_000_000;
        let err = unsafe { libc::timer_settime(timer, 0, &its, null_mut()) };
        if err != 0 {
            panic!("timer_settime failed: {}", err);
        }

        log::debug!("timer registered for thread {}", current_pthread_id);
    }
}
