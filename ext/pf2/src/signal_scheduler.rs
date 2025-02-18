#![deny(unsafe_op_in_unsafe_fn)]

use crate::profile::Profile;
use crate::profile_serializer::ProfileSerializer;
use crate::ruby_internal_apis::rb_thread_getcpuclockid;
use crate::sample::Sample;
use crate::scheduler::Scheduler;
use crate::serialization::serializer::ProfileSerializer2;
use crate::session::configuration::{self, Configuration};

use core::panic;
use std::ffi::{c_int, c_void, CString};
use std::mem::ManuallyDrop;
use std::sync::{Arc, RwLock};
use std::{mem, ptr::null_mut};

use rb_sys::*;

use crate::util::*;

#[derive(Debug)]
pub struct SignalScheduler {
    configuration: Configuration,
    profile: Arc<RwLock<Profile>>,
}

pub struct SignalHandlerArgs {
    profile: Arc<RwLock<Profile>>,
    context_ruby_thread: VALUE,
}

impl Scheduler for SignalScheduler {
    fn start(&self) -> VALUE {
        self.install_signal_handler();

        if let configuration::Threads::Targeted(threads) = &self.configuration.target_ruby_threads {
            for ruby_thread in threads.iter() {
                self.install_timer_to_ruby_thread(*ruby_thread);
            }
        }

        Qtrue.into()
    }

    fn stop(&self) -> VALUE {
        // Finalize
        match self.profile.try_write() {
            Ok(mut profile) => {
                profile.flush_temporary_sample_buffer();
                profile.end_instant = Some(std::time::Instant::now());
            }
            Err(_) => {
                println!("[pf2 ERROR] stop: Failed to acquire profile lock.");
                return Qfalse.into();
            }
        }

        let profile = self.profile.try_read().unwrap();
        log::debug!("Number of samples: {}", profile.samples.len());

        if self.configuration.use_experimental_serializer {
            let mut ser = ProfileSerializer2::new();
            ser.serialize(&profile);
            ser.to_ruby_hash()
        } else {
            let serialized = ProfileSerializer::serialize(&profile);
            let string = CString::new(serialized).unwrap();
            unsafe { rb_str_new_cstr(string.as_ptr()) }
        }
    }

    fn on_new_thread(&self, thread: VALUE) {
        self.install_timer_to_ruby_thread(thread);
    }

    fn dmark(&self) {
        match self.profile.read() {
            Ok(profile) => unsafe {
                profile.dmark();
            },
            Err(_) => {
                panic!("[pf2 FATAL] dmark: Failed to acquire profile lock.");
            }
        }
    }

    fn dfree(&self) {
        // No-op
    }

    fn dsize(&self) -> size_t {
        // FIXME: Report something better
        mem::size_of::<Self>() as size_t
    }
}

impl SignalScheduler {
    pub fn new(configuration: &Configuration, profile: Arc<RwLock<Profile>>) -> Self {
        Self { configuration: configuration.clone(), profile }
    }

    // Install signal handler for profiling events to the current process.
    fn install_signal_handler(&self) {
        let mut sa: libc::sigaction = unsafe { mem::zeroed() };
        sa.sa_sigaction = Self::signal_handler as usize;
        sa.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
        let err = unsafe { libc::sigaction(libc::SIGALRM, &sa, null_mut()) };
        if err != 0 {
            panic!("sigaction failed: {}", err);
        }
        log::debug!("Signal handler installed");
    }

    // Respond to the signal and collect a sample.
    // This function is called when a timer fires.
    //
    // Expected to be async-signal-safe, but the current implementation is not.
    extern "C" fn signal_handler(
        _sig: c_int,
        info: *mut libc::siginfo_t,
        _ucontext: *mut libc::ucontext_t,
    ) {
        let args = unsafe {
            let ptr = extract_si_value_sival_ptr(info) as *mut SignalHandlerArgs;
            ManuallyDrop::new(Box::from_raw(ptr))
        };

        let mut profile = match args.profile.try_write() {
            Ok(profile) => profile,
            Err(_) => {
                // FIXME: Do we want to properly collect GC samples? I don't know yet.
                log::trace!("Failed to acquire profile lock (garbage collection possibly in progress). Dropping sample.");
                return;
            }
        };

        let sample = Sample::capture(args.context_ruby_thread, &profile.backtrace_state); // NOT async-signal-safe
        if profile.temporary_sample_buffer.push(sample).is_err() {
            log::debug!("Temporary sample buffer full. Dropping sample.");
        }
    }

    fn install_timer_to_ruby_thread(&self, ruby_thread: VALUE) {
        // NOTE: This Box never gets dropped
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
            configuration::TimeMode::CpuTime => unsafe { rb_thread_getcpuclockid(ruby_thread) },
            configuration::TimeMode::WallTime => libc::CLOCK_MONOTONIC,
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
