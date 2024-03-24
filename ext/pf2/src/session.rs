pub mod configuration;
mod new_thread_watcher;
pub mod ruby_object;

use std::collections::HashSet;
use std::ffi::{c_int, CStr};
use std::str::FromStr as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use rb_sys::*;

use self::configuration::Configuration;
use self::new_thread_watcher::NewThreadWatcher;
use crate::profile::Profile;
use crate::scheduler::Scheduler;
use crate::signal_scheduler::SignalScheduler;
use crate::timer_thread_scheduler::TimerThreadScheduler;
use crate::util::*;

pub struct Session {
    pub configuration: Configuration,
    pub scheduler: Arc<dyn Scheduler>,
    pub profile: Arc<RwLock<Profile>>,
    pub running: Arc<AtomicBool>,
    pub new_thread_watcher: Option<NewThreadWatcher>,
}

impl Session {
    pub fn new_from_rb_initialize(argc: c_int, argv: *const VALUE, rbself: VALUE) -> Self {
        // Parse arguments
        let kwargs: VALUE = Qnil.into();
        unsafe {
            rb_scan_args(argc, argv, cstr!(":"), &kwargs);
        };
        let mut kwargs_values: [VALUE; 4] = [Qnil.into(); 4];
        unsafe {
            rb_get_kwargs(
                kwargs,
                [
                    rb_intern(cstr!("interval_ms")),
                    rb_intern(cstr!("threads")),
                    rb_intern(cstr!("time_mode")),
                    rb_intern(cstr!("scheduler")),
                ]
                .as_mut_ptr(),
                0,
                4,
                kwargs_values.as_mut_ptr(),
            );
        };

        let interval = Self::parse_option_interval_ms(kwargs_values[0]);
        let threads = Self::parse_option_threads(kwargs_values[1]);
        let time_mode = Self::parse_option_time_mode(kwargs_values[2]);
        let scheduler = Self::parse_option_scheduler(kwargs_values[3]);

        let configuration = Configuration {
            scheduler,
            interval,
            target_ruby_threads: threads.clone(),
            time_mode,
        };

        // Store configuration as a Ruby Hash for convenience
        unsafe {
            rb_iv_set(rbself, cstr!("@configuration"), configuration.to_rb_hash());
        }

        // Create a new Profile
        let profile = Arc::new(RwLock::new(Profile::new()));

        // Initialize the specified Scheduler
        let scheduler: Arc<dyn Scheduler> = match configuration.scheduler {
            configuration::Scheduler::Signal => {
                Arc::new(SignalScheduler::new(&configuration, Arc::clone(&profile)))
            }
            configuration::Scheduler::TimerThread => Arc::new(TimerThreadScheduler::new(
                &configuration,
                Arc::clone(&profile),
            )),
        };

        let new_thread_watcher = match threads {
            configuration::Threads::All => {
                let scheduler = Arc::clone(&scheduler);
                Some(NewThreadWatcher::watch(move |thread: VALUE| {
                    log::debug!("New Ruby thread detected: {:?}", thread);
                    scheduler.on_new_thread(thread);
                }))
            }
            configuration::Threads::Targeted(_) => None,
        };

        Session {
            configuration,
            scheduler,
            profile,
            running: Arc::new(AtomicBool::new(false)),
            new_thread_watcher,
        }
    }

    fn parse_option_interval_ms(value: VALUE) -> Duration {
        if value == Qundef as VALUE {
            // Return default
            return configuration::DEFAULT_INTERVAL;
        }

        let interval_ms = unsafe { rb_num2long(value) };
        Duration::from_millis(interval_ms.try_into().unwrap_or_else(|_| {
            eprintln!(
                "[Pf2] Warning: Specified interval ({}) is not valid. Using default value (49ms).",
                interval_ms
            );
            49
        }))
    }

    fn parse_option_threads(value: VALUE) -> configuration::Threads {
        if (value == Qundef as VALUE)
            || (value == Qnil as VALUE)
            || (value == unsafe { rb_id2sym(rb_intern(cstr!("all"))) })
        {
            return configuration::Threads::All;
        }

        let mut set: HashSet<VALUE> = HashSet::new();
        unsafe {
            for i in 0..RARRAY_LEN(value) {
                set.insert(rb_ary_entry(value, i));
            }
        }
        configuration::Threads::Targeted(set)
    }

    fn parse_option_time_mode(value: VALUE) -> configuration::TimeMode {
        if value == Qundef as VALUE {
            // Return default
            return configuration::DEFAULT_TIME_MODE;
        }

        let specified_mode = unsafe {
            let mut str = rb_funcall(value, rb_intern(cstr!("to_s")), 0);
            let ptr = rb_string_value_ptr(&mut str);
            CStr::from_ptr(ptr).to_str().unwrap()
        };
        configuration::TimeMode::from_str(specified_mode).unwrap_or_else(|_| {
            // Raise an ArgumentError if the mode is invalid
            unsafe {
                rb_raise(
                    rb_eArgError,
                    cstr!("Invalid time mode. Valid values are 'cpu' and 'wall'."),
                )
            }
        })
    }

    fn parse_option_scheduler(value: VALUE) -> configuration::Scheduler {
        if value == Qundef as VALUE {
            // Return default
            return configuration::DEFAULT_SCHEDULER;
        }

        let specified_scheduler = unsafe {
            let mut str = rb_funcall(value, rb_intern(cstr!("to_s")), 0);
            let ptr = rb_string_value_ptr(&mut str);
            CStr::from_ptr(ptr).to_str().unwrap()
        };
        configuration::Scheduler::from_str(specified_scheduler).unwrap_or_else(|_| {
            // Raise an ArgumentError if the mode is invalid
            unsafe {
                rb_raise(
                    rb_eArgError,
                    cstr!("Invalid scheduler. Valid values are ':signal' and ':timer_thread'."),
                )
            }
        })
    }

    pub fn start(&mut self) -> VALUE {
        self.running.store(true, Ordering::Relaxed);
        self.start_profile_buffer_flusher_thread();
        self.scheduler.start()
    }

    fn start_profile_buffer_flusher_thread(&self) {
        let profile = Arc::clone(&self.profile);
        let running = Arc::clone(&self.running);
        log::debug!("flusher: Starting");
        thread::spawn(move || loop {
            if !running.load(Ordering::Relaxed) {
                log::debug!("flusher: Exiting");
                break;
            }

            log::trace!("flusher: Flushing temporary sample buffer");
            match profile.try_write() {
                Ok(mut profile) => {
                    profile.flush_temporary_sample_buffer();
                }
                Err(_) => {
                    log::debug!("flusher: Failed to acquire profile lock");
                }
            }
            thread::sleep(Duration::from_millis(500));
        });
    }

    pub fn stop(&mut self) -> VALUE {
        self.running.store(false, Ordering::Relaxed);
        self.scheduler.stop()
    }

    pub fn dmark(&self) {
        self.scheduler.dmark()
    }
}
