#![deny(unsafe_op_in_unsafe_fn)]

mod timer_installer;

use self::timer_installer::TimerInstaller;
use crate::profile::Profile;
use crate::profile_serializer::ProfileSerializer;
use crate::sample::Sample;
use crate::scheduler::Scheduler;
use crate::session::configuration::Configuration;

use core::panic;
use std::ffi::{c_int, CString};
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

        TimerInstaller::install_timer_to_ruby_threads(
            self.configuration.clone(),
            Arc::clone(&self.profile),
        );

        Qtrue.into()
    }

    fn stop(&self) -> VALUE {
        // Finalize
        match self.profile.try_write() {
            Ok(mut profile) => {
                profile.flush_temporary_sample_buffer();
            }
            Err(_) => {
                println!("[pf2 ERROR] stop: Failed to acquire profile lock.");
                return Qfalse.into();
            }
        }

        let profile = self.profile.try_read().unwrap();
        log::debug!("Number of samples: {}", profile.samples.len());

        let serialized = ProfileSerializer::serialize(&profile);
        let serialized = CString::new(serialized).unwrap();
        unsafe { rb_str_new_cstr(serialized.as_ptr()) }
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
        Self {
            configuration: configuration.clone(),
            profile,
        }
    }

    // Install signal handler for profiling events to the current process.
    fn install_signal_handler(&self) {
        let mut sa: libc::sigaction = unsafe { mem::zeroed() };
        sa.sa_sigaction = Self::signal_handler as usize;
        sa.sa_flags = libc::SA_SIGINFO;
        let err = unsafe { libc::sigaction(libc::SIGALRM, &sa, null_mut()) };
        if err != 0 {
            panic!("sigaction failed: {}", err);
        }
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
}
