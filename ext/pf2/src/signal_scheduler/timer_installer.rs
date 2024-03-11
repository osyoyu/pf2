use std::ffi::c_void;
use std::mem;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};

use rb_sys::*;

use super::configuration::Configuration;
use crate::profile::Profile;
use crate::ruby_internal_apis::rb_thread_getcpuclockid;
use crate::signal_scheduler::{cstr, SignalHandlerArgs};

pub struct TimerInstaller {
    internal: Box<Mutex<Internal>>,
}

struct Internal {
    configuration: Configuration,
    pub profile: Arc<RwLock<Profile>>,
}

impl TimerInstaller {
    // Register a callback that gets called when a Ruby Thread is resumed.
    // The callback should create a timer for the thread.
    pub fn install_timer_to_ruby_threads(
        configuration: Configuration,
        profile: Arc<RwLock<Profile>>,
    ) {
        let registrar = Self {
            internal: Box::new(Mutex::new(Internal {
                configuration: configuration.clone(),
                profile,
            })),
        };

        for ruby_thread in configuration.target_ruby_threads.iter() {
            let ruby_thread: VALUE = *ruby_thread;
            registrar.register_timer_to_ruby_thread(ruby_thread);
        }

        if configuration.track_new_threads {
            let ptr = Box::into_raw(registrar.internal);
            unsafe {
                rb_internal_thread_add_event_hook(
                    Some(Self::on_thread_start),
                    RUBY_INTERNAL_THREAD_EVENT_STARTED,
                    ptr as *mut c_void,
                );
            };
        }
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
        internal
            .configuration
            .target_ruby_threads
            .insert(current_ruby_thread);
    }

    fn register_timer_to_ruby_thread(&self, ruby_thread: VALUE) {
        let internal = self.internal.lock().unwrap();

        // NOTE: This Box is never dropped
        let signal_handler_args = Box::new(SignalHandlerArgs {
            profile: Arc::clone(&internal.profile),
            context_ruby_thread: ruby_thread,
        });

        // Create a signal event
        let mut sigevent: libc::sigevent = unsafe { mem::zeroed() };
        // Note: SIGEV_THREAD_ID is Linux-specific. In other platforms, we would need to
        // "tranpoline" the signal as any pthread can receive the signal.
        sigevent.sigev_notify = libc::SIGEV_THREAD_ID;
        sigevent.sigev_notify_thread_id = i32::try_from(unsafe {
            rb_num2int(rb_funcall(
                ruby_thread,
                rb_intern(cstr!("native_thread_id")), // kernel thread ID
                0,
            ))
        })
        .unwrap();
        sigevent.sigev_signo = libc::SIGALRM;
        // Pass required args to the signal handler
        sigevent.sigev_value.sival_ptr = Box::into_raw(signal_handler_args) as *mut c_void;

        // Create and configure timer to fire every _interval_ ms of CPU time
        let mut timer: libc::timer_t = unsafe { mem::zeroed() };
        let clockid = match internal.configuration.time_mode {
            crate::signal_scheduler::TimeMode::CpuTime => unsafe {
                rb_thread_getcpuclockid(ruby_thread)
            },
            crate::signal_scheduler::TimeMode::WallTime => libc::CLOCK_MONOTONIC,
        };
        let err = unsafe { libc::timer_create(clockid, &mut sigevent, &mut timer) };
        if err != 0 {
            panic!("timer_create failed: {}", err);
        }
        let itimerspec = Self::duration_to_itimerspec(&internal.configuration.interval);
        let err = unsafe { libc::timer_settime(timer, 0, &itimerspec, null_mut()) };
        if err != 0 {
            panic!("timer_settime failed: {}", err);
        }

        log::debug!("timer registered for thread {}", ruby_thread);
    }

    fn duration_to_itimerspec(duration: &std::time::Duration) -> libc::itimerspec {
        let nanos = duration.as_nanos();
        let seconds_part: i64 = (nanos / 1_000_000_000).try_into().unwrap();
        let nanos_part: i64 = (nanos % 1_000_000_000).try_into().unwrap();

        let mut its: libc::itimerspec = unsafe { mem::zeroed() };
        its.it_interval.tv_sec = seconds_part;
        its.it_interval.tv_nsec = nanos_part;
        its.it_value.tv_sec = seconds_part;
        its.it_value.tv_nsec = nanos_part;
        its
    }
}
