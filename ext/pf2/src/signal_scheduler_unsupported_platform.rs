use std::sync::{Arc, RwLock};

use crate::profile::Profile;
use crate::scheduler::Scheduler;
use crate::session::configuration::Configuration;

pub struct SignalScheduler {}

impl Scheduler for SignalScheduler {
    fn start(&self) -> rb_sys::VALUE {
        unimplemented!()
    }

    fn stop(&self) -> rb_sys::VALUE {
        unimplemented!()
    }

    fn on_new_thread(&self, thread: rb_sys::VALUE) {
        unimplemented!()
    }

    fn dmark(&self) {
        unimplemented!()
    }

    fn dfree(&self) {
        unimplemented!()
    }

    fn dsize(&self) -> rb_sys::size_t {
        unimplemented!()
    }
}

impl SignalScheduler {
    pub fn new(configuration: &Configuration, profile: Arc<RwLock<Profile>>) -> Self {
        unimplemented!()
    }
}
