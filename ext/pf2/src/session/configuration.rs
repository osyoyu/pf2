use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use rb_sys::*;

use crate::util::cstr;

pub const DEFAULT_SCHEDULER: Scheduler = Scheduler::Signal;
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(49);
pub const DEFAULT_TIME_MODE: TimeMode = TimeMode::CpuTime;

#[derive(Clone, Debug)]
pub struct Configuration {
    pub scheduler: Scheduler,
    pub interval: Duration,
    pub time_mode: TimeMode,
    pub target_ruby_threads: Threads,
}

#[derive(Clone, Debug)]
pub enum Scheduler {
    Signal,
    TimerThread,
}

impl FromStr for Scheduler {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "signal" => Ok(Self::Signal),
            "timer_thread" => Ok(Self::TimerThread),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TimeMode {
    CpuTime,
    WallTime,
}

impl FromStr for TimeMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cpu" => Ok(Self::CpuTime),
            "wall" => Ok(Self::WallTime),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Threads {
    All,
    Targeted(HashSet<VALUE>),
}

impl Configuration {
    pub fn to_rb_hash(&self) -> VALUE {
        let hash: VALUE = unsafe { rb_hash_new() };
        unsafe {
            rb_hash_aset(
                hash,
                rb_id2sym(rb_intern(cstr!("scheduler"))),
                rb_id2sym(rb_intern(match self.scheduler {
                    Scheduler::Signal => cstr!("signal"),
                    Scheduler::TimerThread => cstr!("timer_thread"),
                })),
            );
            rb_hash_aset(
                hash,
                rb_id2sym(rb_intern(cstr!("interval_ms"))),
                rb_int2inum(self.interval.as_millis().try_into().unwrap()),
            );
            rb_hash_aset(
                hash,
                rb_id2sym(rb_intern(cstr!("time_mode"))),
                rb_id2sym(rb_intern(match self.time_mode {
                    TimeMode::CpuTime => cstr!("cpu"),
                    TimeMode::WallTime => cstr!("wall"),
                })),
            );
        }
        hash
    }
}
