use std::time::Instant;

use rb_sys::*;

#[derive(Debug)]
pub struct Sample {
    pub ruby_thread: VALUE,
    pub timestamp: Instant,
    pub line_count: i32,
    pub frames: [VALUE; 2000],
    pub linenos: [i32; 2000],
}

impl Sample {
    // Nearly async-signal-safe
    // (rb_profile_thread_frames isn't defined as a-s-s)
    pub fn capture(ruby_thread: VALUE) -> Self {
        let mut sample = Sample {
            ruby_thread,
            timestamp: Instant::now(),
            line_count: 0,
            frames: [0; 2000],
            linenos: [0; 2000],
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
}
