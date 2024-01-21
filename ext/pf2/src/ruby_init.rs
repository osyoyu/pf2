#![deny(unsafe_op_in_unsafe_fn)]

use rb_sys::*;

#[cfg(target_os = "linux")]
use crate::signal_scheduler::SignalScheduler;
use crate::timer_thread_scheduler::TimerThreadScheduler;
use crate::util::*;

#[allow(non_snake_case)]
#[no_mangle]
extern "C" fn Init_pf2() {
    unsafe {
        let rb_mPf2: VALUE = rb_define_module(cstr!("Pf2"));

        #[cfg(target_os = "linux")]
        {
            let rb_mPf2_SignalScheduler =
                rb_define_class_under(rb_mPf2, cstr!("SignalScheduler"), rb_cObject);
            rb_define_alloc_func(rb_mPf2_SignalScheduler, Some(SignalScheduler::rb_alloc));
            rb_define_method(
                rb_mPf2_SignalScheduler,
                cstr!("start"),
                Some(to_ruby_cfunc2(SignalScheduler::rb_start)),
                1,
            );
            rb_define_method(
                rb_mPf2_SignalScheduler,
                cstr!("stop"),
                Some(to_ruby_cfunc1(SignalScheduler::rb_stop)),
                0,
            );
        }

        let rb_mPf2_TimerThreadScheduler =
            rb_define_class_under(rb_mPf2, cstr!("TimerThreadScheduler"), rb_cObject);
        rb_define_alloc_func(
            rb_mPf2_TimerThreadScheduler,
            Some(TimerThreadScheduler::rb_alloc),
        );
        rb_define_method(
            rb_mPf2_TimerThreadScheduler,
            cstr!("start"),
            Some(to_ruby_cfunc2(TimerThreadScheduler::rb_start)),
            1,
        );
        rb_define_method(
            rb_mPf2_TimerThreadScheduler,
            cstr!("stop"),
            Some(to_ruby_cfunc1(TimerThreadScheduler::rb_stop)),
            0,
        );
    }
}
