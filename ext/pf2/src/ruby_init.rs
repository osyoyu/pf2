#![deny(unsafe_op_in_unsafe_fn)]

use rb_sys::*;

use crate::sample_collector::SampleCollector;
use crate::timer_collector::TimerCollector;
use crate::util::*;

#[allow(non_snake_case)]
#[no_mangle]
extern "C" fn Init_pf2() {
    unsafe {
        let rb_mPf2: VALUE = rb_define_module(cstr!("Pf2"));

        let rb_mPf2_Collector_PostponedJobCollector =
            rb_define_class_under(rb_mPf2, cstr!("SampleCollector"), rb_cObject);
        rb_define_alloc_func(
            rb_mPf2_Collector_PostponedJobCollector,
            Some(SampleCollector::rb_alloc),
        );
        rb_define_method(
            rb_mPf2_Collector_PostponedJobCollector,
            cstr!("start"),
            Some(to_ruby_cfunc2(SampleCollector::rb_start)),
            1,
        );
        rb_define_method(
            rb_mPf2_Collector_PostponedJobCollector,
            cstr!("stop"),
            Some(to_ruby_cfunc1(SampleCollector::rb_stop)),
            0,
        );

        let rb_mPf2_TimerCollector =
            rb_define_class_under(rb_mPf2, cstr!("TimerCollector"), rb_cObject);
        rb_define_alloc_func(rb_mPf2_TimerCollector, Some(TimerCollector::rb_alloc));
        rb_define_method(
            rb_mPf2_TimerCollector,
            cstr!("start"),
            Some(to_ruby_cfunc2(TimerCollector::rb_start)),
            1,
        );
        rb_define_method(
            rb_mPf2_TimerCollector,
            cstr!("stop"),
            Some(to_ruby_cfunc1(TimerCollector::rb_stop)),
            0,
        );
        rb_define_method(
            rb_mPf2_TimerCollector,
            cstr!("install_to_current_thread"),
            Some(to_ruby_cfunc1(TimerCollector::rb_install_to_current_thread)),
            0,
        );
    }
}
