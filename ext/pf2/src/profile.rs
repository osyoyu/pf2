use std::time::{Instant, SystemTime};
use std::{collections::HashSet, ptr::null_mut};

use rb_sys::*;

use backtrace_sys2::backtrace_create_state;

use super::backtrace::{Backtrace, BacktraceState};
use super::ringbuffer::Ringbuffer;
use super::sample::Sample;

// Capacity large enough to hold 1 second worth of samples for 16 threads
// 16 threads * 20 samples per second * 1 second = 320
const DEFAULT_RINGBUFFER_CAPACITY: usize = 320;

#[derive(Debug)]
pub struct Profile {
    pub start_timestamp: SystemTime,
    pub start_instant: Instant,
    pub end_instant: Option<Instant>,
    pub samples: Vec<Sample>,
    pub temporary_sample_buffer: Ringbuffer,
    pub backtrace_state: BacktraceState,
    known_values: HashSet<VALUE>,
}

impl Profile {
    pub fn new() -> Self {
        let backtrace_state = unsafe {
            let ptr = backtrace_create_state(
                null_mut(),
                1,
                Some(Backtrace::backtrace_error_callback),
                null_mut(),
            );
            BacktraceState::new(ptr)
        };

        Self {
            start_timestamp: SystemTime::now(),
            start_instant: Instant::now(),
            end_instant: None,
            samples: vec![],
            temporary_sample_buffer: Ringbuffer::new(DEFAULT_RINGBUFFER_CAPACITY),
            backtrace_state,
            known_values: HashSet::new(),
        }
    }

    pub fn flush_temporary_sample_buffer(&mut self) {
        while let Some(sample) = self.temporary_sample_buffer.pop() {
            self.known_values.insert(sample.ruby_thread);
            for frame in sample.frames.iter() {
                if frame == &0 {
                    break;
                }
                self.known_values.insert(*frame);
            }
            self.samples.push(sample);
        }
    }

    pub unsafe fn dmark(&self) {
        for value in self.known_values.iter() {
            rb_gc_mark(*value);
        }
        self.temporary_sample_buffer.dmark();
    }
}
