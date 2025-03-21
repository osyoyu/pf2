#include <ruby.h>

#include "session.h"

VALUE rb_mPf2c;

RUBY_FUNC_EXPORTED void
Init_pf2(void)
{
    rb_mPf2c = rb_define_module("Pf2c");
    VALUE rb_mPf2c_cSession = rb_define_class_under(rb_mPf2c, "Session", rb_cObject);
    rb_define_alloc_func(rb_mPf2c_cSession, pf2_session_alloc);
    rb_define_method(rb_mPf2c_cSession, "initialize", rb_pf2_session_initialize, -1);
    rb_define_method(rb_mPf2c_cSession, "start", rb_pf2_session_start, 0);
    rb_define_method(rb_mPf2c_cSession, "stop", rb_pf2_session_stop, 0);
    rb_define_method(rb_mPf2c_cSession, "configuration", rb_pf2_session_configuration, 0);
}
