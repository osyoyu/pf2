use rb_sys::{size_t, VALUE};

pub trait Scheduler {
    fn start(&mut self) -> VALUE;
    fn stop(&mut self) -> VALUE;
    fn dmark(&self);
    fn dfree(&self);
    fn dsize(&self) -> size_t;
}
