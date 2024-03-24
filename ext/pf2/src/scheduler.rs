use rb_sys::{size_t, VALUE};

pub trait Scheduler {
    fn start(&self) -> VALUE;
    fn stop(&self) -> VALUE;
    fn dmark(&self);
    fn dfree(&self);
    fn dsize(&self) -> size_t;
}
