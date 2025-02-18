use std::time::Instant;

use rb_sys::*;

#[derive(Debug)]
pub struct Marker {
    pub ruby_thread: VALUE,
    pub timestamp: Instant,
    pub tag: String,
}

impl Marker {
    pub fn new(ruby_thread: VALUE, tag: String) -> Self {
        Marker {
            ruby_thread,
            timestamp: Instant::now(),
            tag,
        }
    }

    pub unsafe fn dmark(&self) {
        rb_gc_mark(self.ruby_thread);
    }
}
