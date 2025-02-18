#include <ruby.h>

VALUE rb_mPf2c;

RUBY_FUNC_EXPORTED void
Init_pf2c(void)
{
    rb_mPf2c = rb_define_module("Pf2c");
}
