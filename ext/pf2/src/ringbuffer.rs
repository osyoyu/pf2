use crate::sample::Sample;

#[derive(Debug)]
pub struct Ringbuffer {
    capacity: usize,
    buffer: Vec<Option<Sample>>,
    read_index: usize,
    write_index: usize,
}

#[derive(Debug, PartialEq)]
pub enum RingbufferError {
    Full,
}

impl Ringbuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: std::iter::repeat_with(|| None)
                .take(capacity + 1)
                .collect::<Vec<_>>(),
            read_index: 0,
            write_index: 0,
        }
    }

    // async-signal-safe
    pub fn push(&mut self, sample: Sample) -> Result<(), RingbufferError> {
        let next = (self.write_index + 1) % (self.capacity + 1);
        if next == self.read_index {
            return Err(RingbufferError::Full);
        }
        self.buffer[self.write_index] = Some(sample);
        self.write_index = next;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<Sample> {
        if self.read_index == self.write_index {
            return None;
        }
        let sample = self.buffer[self.read_index].take();
        self.read_index = (self.read_index + 1) % (self.capacity + 1);
        sample
    }

    // This will call rb_gc_mark() for capacity * Sample::MAX_STACK_DEPTH * 2 times, which is a lot!
    pub fn dmark(&self) {
        for sample in self.buffer.iter().flatten() {
            unsafe {
                sample.dmark();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_ringbuffer() {
        let mut ringbuffer = Ringbuffer::new(2);
        assert_eq!(ringbuffer.pop(), None);

        let sample1 = Sample {
            ruby_thread: 1,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 500],
            linenos: [0; 500],
            c_backtrace_pcs: [0; 1001],
        };
        let sample2 = Sample {
            ruby_thread: 2,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 500],
            linenos: [0; 500],
            c_backtrace_pcs: [0; 1001],
        };

        ringbuffer.push(sample1).unwrap();
        ringbuffer.push(sample2).unwrap();

        assert_eq!(ringbuffer.pop().unwrap().ruby_thread, 1);
        assert_eq!(ringbuffer.pop().unwrap().ruby_thread, 2);
        assert_eq!(ringbuffer.pop(), None);
    }

    #[test]
    fn test_ringbuffer_full() {
        let mut ringbuffer = Ringbuffer::new(1);
        let sample1 = Sample {
            ruby_thread: 1,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 500],
            linenos: [0; 500],
            c_backtrace_pcs: [0; 1001],
        };
        let sample2 = Sample {
            ruby_thread: 2,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 500],
            linenos: [0; 500],
            c_backtrace_pcs: [0; 1001],
        };

        ringbuffer.push(sample1).unwrap();
        assert_eq!(ringbuffer.push(sample2), Err(RingbufferError::Full));
    }

    #[test]
    fn test_ringbuffer_write_a_lot() {
        let mut ringbuffer = Ringbuffer::new(2);
        let sample1 = Sample {
            ruby_thread: 1,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 500],
            linenos: [0; 500],
            c_backtrace_pcs: [0; 1001],
        };
        let sample2 = Sample {
            ruby_thread: 2,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 500],
            linenos: [0; 500],
            c_backtrace_pcs: [0; 1001],
        };
        let sample3 = Sample {
            ruby_thread: 3,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 500],
            linenos: [0; 500],
            c_backtrace_pcs: [0; 1001],
        };

        ringbuffer.push(sample1).unwrap();
        ringbuffer.pop().unwrap();
        ringbuffer.push(sample2).unwrap();
        ringbuffer.pop().unwrap();
        ringbuffer.push(sample3).unwrap();
        assert_eq!(ringbuffer.pop().unwrap().ruby_thread, 3);
    }
}
