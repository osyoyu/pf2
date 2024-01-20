use std::collections::HashSet;
use std::time::Instant;

use rb_sys::*;

use super::ringbuffer::Ringbuffer;
use super::sample::Sample;

// Capacity large enough to hold 1 second worth of samples for 16 threads
// 16 threads * 20 samples per second * 1 second = 320
const DEFAULT_RINGBUFFER_CAPACITY: usize = 320;

#[derive(Debug)]
pub struct Profile {
    pub start_timestamp: Instant,
    pub samples: Vec<Sample>,
    pub temporary_sample_buffer: Ringbuffer,
    known_values: HashSet<VALUE>,
}

impl Profile {
    pub fn new() -> Self {
        Self {
            start_timestamp: Instant::now(),
            samples: vec![],
            temporary_sample_buffer: Ringbuffer::new(DEFAULT_RINGBUFFER_CAPACITY),
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
