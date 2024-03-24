use std::collections::HashSet;
use std::ffi::c_void;
use std::mem;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};

use rb_sys::*;

use crate::profile::Profile;
use crate::ruby_internal_apis::rb_thread_getcpuclockid;
use crate::session::configuration::Configuration;
use crate::signal_scheduler::{cstr, SignalHandlerArgs};

#[derive(Debug)]
pub struct TimerInstaller {
    inner: Box<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    configuration: Configuration,
    pub profile: Arc<RwLock<Profile>>,
    known_threads: HashSet<VALUE>,
}

impl TimerInstaller {
    // Register a callback that gets called when a Ruby Thread is resumed.
    // The callback should create a timer for the thread.
    pub fn install_timer_to_ruby_threads(
        configuration: Configuration,
        profile: Arc<RwLock<Profile>>,
    ) {
        let installer = Self {
            inner: Box::new(Mutex::new(Inner {
                configuration: configuration.clone(),
                profile,
                known_threads: HashSet::new(),
            })),
        };

        if let Ok(mut inner) = installer.inner.try_lock() {
            for ruby_thread in configuration.target_ruby_threads.iter() {
                let ruby_thread: VALUE = *ruby_thread;
                inner.known_threads.insert(ruby_thread);
                inner.register_timer_to_ruby_thread(ruby_thread);
            }
        }

        if configuration.track_all_threads {
            let ptr = Box::into_raw(installer.inner);
            unsafe {
                // TODO: Clean up this hook when the profiling session ends
                rb_internal_thread_add_event_hook(
                    Some(Self::on_thread_resume),
                    RUBY_INTERNAL_THREAD_EVENT_RESUMED,
                    ptr as *mut c_void,
                );
            };
        }
    }

    // Thread start callback
    unsafe extern "C" fn on_thread_resume(
        _flag: rb_event_flag_t,
        data: *const rb_internal_thread_event_data,
        custom_data: *mut c_void,
    ) {
        let ruby_thread: VALUE = unsafe { (*data).thread };

        // A pointer to Box<Inner> is passed as custom_data
        let inner = unsafe { ManuallyDrop::new(Box::from_raw(custom_data as *mut Mutex<Inner>)) };
        let mut inner = inner.lock().unwrap();

        if !inner.known_threads.contains(&ruby_thread) {
            inner.known_threads.insert(ruby_thread);
            // Install a timer for the thread
            inner.register_timer_to_ruby_thread(ruby_thread);
        }
    }
}

impl Inner {
    fn register_timer_to_ruby_thread(&self, ruby_thread: VALUE) {
        // NOTE: This Box is never dropped
        let signal_handler_args = Box::new(SignalHandlerArgs {
            profile: Arc::clone(&self.profile),
            context_ruby_thread: ruby_thread,
        });

        // rb_funcall deadlocks when called within a THREAD_EVENT_STARTED hook
        let kernel_thread_id: i32 = i32::try_from(unsafe {
            rb_num2int(rb_funcall(
                ruby_thread,
                rb_intern(cstr!("native_thread_id")), // kernel thread ID
                0,
            ))
        })
        .unwrap();

        // Create a signal event
        let mut sigevent: libc::sigevent = unsafe { mem::zeroed() };
        // Note: SIGEV_THREAD_ID is Linux-specific. In other platforms, we would need to
        // "trampoline" the signal as any pthread can receive the signal.
        sigevent.sigev_notify = libc::SIGEV_THREAD_ID;
        sigevent.sigev_notify_thread_id = kernel_thread_id;
        sigevent.sigev_signo = libc::SIGALRM;
        // Pass required args to the signal handler
        sigevent.sigev_value.sival_ptr = Box::into_raw(signal_handler_args) as *mut c_void;

        // Create and configure timer to fire every _interval_ ms of CPU time
        let mut timer: libc::timer_t = unsafe { mem::zeroed() };
        let clockid = match self.configuration.time_mode {
            crate::session::configuration::TimeMode::CpuTime => unsafe {
                rb_thread_getcpuclockid(ruby_thread)
            },
            crate::session::configuration::TimeMode::WallTime => libc::CLOCK_MONOTONIC,
        };
        let err = unsafe { libc::timer_create(clockid, &mut sigevent, &mut timer) };
        if err != 0 {
            panic!("timer_create failed: {}", err);
        }
        let itimerspec = Self::duration_to_itimerspec(&self.configuration.interval);
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
