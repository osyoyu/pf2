#![deny(unsafe_op_in_unsafe_fn)]

use rb_sys::*;

use crate::session::ruby_object::SessionRubyObject;
use crate::util::*;

#[allow(non_snake_case)]
#[no_mangle]
extern "C" fn Init_pf2() {
    #[cfg(feature = "debug")]
    {
        env_logger::builder()
            .format_timestamp(None)
            .format_module_path(false)
            .init();
    }

    unsafe {
        let rb_mPf2: VALUE = rb_define_module(cstr!("Pf2"));

        let rb_mPf2_Session = rb_define_class_under(rb_mPf2, cstr!("Session"), rb_cObject);
        rb_define_alloc_func(rb_mPf2_Session, Some(SessionRubyObject::rb_alloc));
        rb_define_method(
            rb_mPf2_Session,
            cstr!("initialize"),
            Some(to_ruby_cfunc_with_args(SessionRubyObject::rb_initialize)),
            -1,
        );
        rb_define_method(
            rb_mPf2_Session,
            cstr!("start"),
            Some(to_ruby_cfunc_with_no_args(SessionRubyObject::rb_start)),
            0,
        );
        rb_define_method(
            rb_mPf2_Session,
            cstr!("stop"),
            Some(to_ruby_cfunc_with_no_args(SessionRubyObject::rb_stop)),
            0,
        );
    }
}
