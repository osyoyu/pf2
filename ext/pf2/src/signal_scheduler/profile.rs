use std::time::Instant;

use super::sample::Sample;

const TEMPORARY_SAMPLE_BUFFER_CAPACITY: usize = 100000;

#[derive(Debug)]
pub struct Profile {
    pub start_timestamp: Instant,
    pub samples: Vec<Sample>,
    pub temporary_sample_buffer: TemporarySampleBuffer,
}

impl Profile {
    pub fn new() -> Self {
        Self {
            start_timestamp: Instant::now(),
            samples: vec![],
            temporary_sample_buffer: TemporarySampleBuffer {
                buffer: unsafe { std::mem::zeroed() },
                index: 0,
            },
        }
    }

    pub fn flush_temporary_sample_buffer(&mut self) {
        for i in 0..self.temporary_sample_buffer.index {
            self.samples
                .push(self.temporary_sample_buffer.buffer[i].take().unwrap());
        }
        self.temporary_sample_buffer.index = 0;
    }
}

#[derive(Debug)]
pub struct TemporarySampleBuffer {
    buffer: [Option<Sample>; TEMPORARY_SAMPLE_BUFFER_CAPACITY],
    index: usize,
}

impl TemporarySampleBuffer {
    // async-signal-safe
    pub fn push(&mut self, sample: Sample) {
        if self.index == TEMPORARY_SAMPLE_BUFFER_CAPACITY {
            panic!("TemporarySampleBuffer is full");
        }
        self.buffer[self.index] = Some(sample);
        self.index += 1;
    }
}
