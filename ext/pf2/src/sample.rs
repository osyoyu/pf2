use std::time::Instant;

use rb_sys::*;

const MAX_STACK_DEPTH: usize = 500;

#[derive(Debug, PartialEq)]
pub struct Sample {
    pub ruby_thread: VALUE,
    pub timestamp: Instant,
    pub line_count: i32,
    pub frames: [VALUE; MAX_STACK_DEPTH],
    pub linenos: [i32; MAX_STACK_DEPTH],
}

impl Sample {
    // Nearly async-signal-safe
    // (rb_profile_thread_frames isn't defined as a-s-s)
    pub fn capture(ruby_thread: VALUE) -> Self {
        let mut sample = Sample {
            ruby_thread,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; MAX_STACK_DEPTH],
            linenos: [0; MAX_STACK_DEPTH],
        };
        unsafe {
            sample.line_count = rb_profile_thread_frames(
                ruby_thread,
                0,
                2000,
                sample.frames.as_mut_ptr(),
                sample.linenos.as_mut_ptr(),
            );
        };
        sample
    }

    pub unsafe fn dmark(&self) {
        rb_gc_mark(self.ruby_thread);
        for frame in self.frames.iter() {
            rb_gc_mark(*frame);
        }
    }
}
