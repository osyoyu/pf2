use std::time::Instant;

use rb_sys::*;

use super::ringbuffer::Ringbuffer;
use super::sample::Sample;

#[derive(Debug)]
pub struct Profile {
    pub start_timestamp: Instant,
    pub samples: Vec<Sample>,
    pub temporary_sample_buffer: Ringbuffer,
}

impl Profile {
    pub fn new() -> Self {
        Self {
            start_timestamp: Instant::now(),
            samples: vec![],
            temporary_sample_buffer: Ringbuffer::new(10000),
        }
    }

    pub fn flush_temporary_sample_buffer(&mut self) {
        while let Some(sample) = self.temporary_sample_buffer.pop() {
            self.samples.push(sample);
        }
    }

    pub unsafe fn dmark(&self) {
        self.samples.iter().for_each(|sample| {
            sample.dmark();
        });
        self.temporary_sample_buffer.dmark();
    }
}
