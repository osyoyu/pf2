pub mod configuration;
pub mod ruby_object;

use std::collections::HashSet;
use std::ffi::{c_int, CStr};
use std::str::FromStr as _;
use std::time::Duration;

use rb_sys::*;

use self::configuration::Configuration;
use crate::scheduler::Scheduler;
use crate::signal_scheduler::SignalScheduler;
use crate::timer_thread_scheduler::TimerThreadScheduler;
use crate::util::*;

pub struct Session {
    pub configuration: Configuration,
    pub scheduler: Box<dyn Scheduler>,
}

impl Session {
    pub fn new_from_rb_initialize(argc: c_int, argv: *const VALUE, rbself: VALUE) -> Self {
        // Parse arguments
        let kwargs: VALUE = Qnil.into();
        unsafe {
            rb_scan_args(argc, argv, cstr!(":"), &kwargs);
        };
        let mut kwargs_values: [VALUE; 5] = [Qnil.into(); 5];
        unsafe {
            rb_get_kwargs(
                kwargs,
                [
                    rb_intern(cstr!("interval_ms")),
                    rb_intern(cstr!("threads")),
                    rb_intern(cstr!("time_mode")),
                    rb_intern(cstr!("track_all_threads")),
                    rb_intern(cstr!("scheduler")),
                ]
                .as_mut_ptr(),
                0,
                5,
                kwargs_values.as_mut_ptr(),
            );
        };

        let interval = Self::parse_option_interval_ms(kwargs_values[0]);
        let threads = Self::parse_option_threads(kwargs_values[1]);
        let time_mode = Self::parse_option_time_mode(kwargs_values[2]);
        let track_all_threads = Self::parse_option_track_all_threads(kwargs_values[3]);
        let scheduler = Self::parse_option_scheduler(kwargs_values[4]);

        let configuration = Configuration {
            scheduler,
            interval,
            target_ruby_threads: threads.clone(),
            time_mode,
            track_all_threads,
        };

        // Store configuration as a Ruby Hash for convenience
        unsafe {
            rb_iv_set(rbself, cstr!("@configuration"), configuration.to_rb_hash());
        }

        let scheduler: Box<dyn Scheduler> = match configuration.scheduler {
            configuration::Scheduler::Signal => Box::new(SignalScheduler::new(&configuration)),
            configuration::Scheduler::TimerThread => {
                Box::new(TimerThreadScheduler::new(&configuration))
            }
        };

        Session {
            configuration,
            scheduler,
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

    fn parse_option_threads(value: VALUE) -> HashSet<VALUE> {
        let threads = if value == Qundef as VALUE {
            // Use Thread.list (all active Threads)
            unsafe { rb_funcall(rb_cThread, rb_intern(cstr!("list")), 0) }
        } else {
            value
        };

        let mut set: HashSet<VALUE> = HashSet::new();
        unsafe {
            for i in 0..RARRAY_LEN(threads) {
                set.insert(rb_ary_entry(threads, i));
            }
        }
        set
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

    fn parse_option_track_all_threads(value: VALUE) -> bool {
        if value == Qundef as VALUE {
            // Return default
            return false;
        }
        todo!("Implement track_all_threads");
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
        self.scheduler.start()
    }

    pub fn stop(&mut self) -> VALUE {
        self.scheduler.stop()
    }

    pub fn dmark(&self) {
        self.scheduler.dmark()
    }
}
