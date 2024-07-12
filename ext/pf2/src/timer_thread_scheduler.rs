#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_void, CString};
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use rb_sys::*;

use crate::profile::Profile;
use crate::profile_serializer::ProfileSerializer;
use crate::sample::Sample;
use crate::scheduler::Scheduler;
use crate::serialization::serializer::ProfileSerializer2;
use crate::session::configuration::{self, Configuration};
use crate::util::*;

#[derive(Clone, Debug)]
pub struct TimerThreadScheduler {
    configuration: Arc<Configuration>,
    profile: Arc<RwLock<Profile>>,
    stop_requested: Arc<AtomicBool>,
}

#[derive(Debug)]
struct PostponedJobArgs {
    configuration: Arc<Configuration>,
    profile: Arc<RwLock<Profile>>,
}

impl Scheduler for TimerThreadScheduler {
    fn start(&self) -> VALUE {
        // Register the Postponed Job which does the actual work of collecting samples
        let postponed_job_args: Box<PostponedJobArgs> = Box::new(PostponedJobArgs {
            configuration: Arc::clone(&self.configuration),
            profile: Arc::clone(&self.profile),
        });
        let postponed_job_handle: rb_postponed_job_handle_t = unsafe {
            rb_postponed_job_preregister(
                0,
                Some(Self::postponed_job),
                Box::into_raw(postponed_job_args) as *mut c_void, // FIXME: leak
            )
        };

        // Start a timer thread that periodically triggers postponed jobs based on configuration
        let configuration = Arc::clone(&self.configuration);
        let stop_requested = Arc::clone(&self.stop_requested);
        thread::spawn(move || {
            Self::thread_main_loop(configuration, stop_requested, postponed_job_handle)
        });

        Qtrue.into()
    }

    fn stop(&self) -> VALUE {
        // Stop the collector thread
        self.stop_requested.store(true, Ordering::Relaxed);

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

        let serialized = if self.configuration.use_experimental_serializer {
            ProfileSerializer2::new().serialize(&profile)
        } else {
            ProfileSerializer::serialize(&profile)
        };
        let serialized = CString::new(serialized).unwrap();
        unsafe { rb_str_new_cstr(serialized.as_ptr()) }
    }

    fn on_new_thread(&self, _thread: VALUE) {
        todo!();
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
        std::mem::size_of::<TimerThreadScheduler>() as size_t
    }
}

impl TimerThreadScheduler {
    pub fn new(configuration: &Configuration, profile: Arc<RwLock<Profile>>) -> Self {
        Self {
            configuration: Arc::new(configuration.clone()),
            profile,
            stop_requested: Arc::new(AtomicBool::new(false)),
        }

        // cstr!("TimerThreadScheduler only supports :wall mode."),
    }

    fn thread_main_loop(
        configuration: Arc<Configuration>,
        stop_requested: Arc<AtomicBool>,
        postponed_job_handle: rb_postponed_job_handle_t,
    ) {
        loop {
            if stop_requested.fetch_and(true, Ordering::Relaxed) {
                break;
            }
            unsafe {
                log::trace!("Triggering postponed job");
                rb_postponed_job_trigger(postponed_job_handle);
            }

            thread::sleep(configuration.interval);
        }
    }

    unsafe extern "C" fn postponed_job(ptr: *mut c_void) {
        unsafe {
            rb_gc_disable();
        }
        let args = unsafe { ManuallyDrop::new(Box::from_raw(ptr as *mut PostponedJobArgs)) };

        let mut profile = match args.profile.try_write() {
            Ok(profile) => profile,
            Err(_) => {
                // FIXME: Do we want to properly collect GC samples? I don't know yet.
                log::trace!("Failed to acquire profile lock (garbage collection possibly in progress). Dropping sample.");
                return;
            }
        };

        // Collect stack information from specified Ruby Threads
        match &args.configuration.target_ruby_threads {
            configuration::Threads::All => todo!(),
            configuration::Threads::Targeted(threads) => {
                for ruby_thread in threads.iter() {
                    // Check if the thread is still alive
                    if unsafe { rb_funcall(*ruby_thread, rb_intern(cstr!("status")), 0) }
                        == Qfalse as u64
                    {
                        continue;
                    }

                    let sample = Sample::capture(*ruby_thread, &profile.backtrace_state);
                    if profile.temporary_sample_buffer.push(sample).is_err() {
                        log::debug!("Temporary sample buffer full. Dropping sample.");
                    }
                }
            }
        }
        unsafe {
            rb_gc_enable();
        }
    }
}
