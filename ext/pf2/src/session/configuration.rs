use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use rb_sys::VALUE;

pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(49);
pub const DEFAULT_TIME_MODE: TimeMode = TimeMode::CpuTime;

#[derive(Clone, Debug)]
pub struct Configuration {
    pub interval: Duration,
    pub time_mode: TimeMode,
    pub target_ruby_threads: HashSet<VALUE>,
    pub track_all_threads: bool,
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
