#include <ruby.h>
#include <ruby/debug.h>
#include <ruby/thread.h>

#include "sampler.h"

void Init_pf2(void);
VALUE rb_start(VALUE self, VALUE debug);
VALUE rb_stop(VALUE self);

// Debug print?
bool _debug = false;

void
Init_pf2(void)
{
    VALUE rb_mPf2 = rb_define_module("Pf2");
    rb_define_module_function(rb_mPf2, "start", rb_start, 1);
    rb_define_module_function(rb_mPf2, "stop", rb_stop, 0);
}

VALUE
rb_start(VALUE self, VALUE debug) {
    _debug = RTEST(debug);

    /**
     * {
     *   sequence: 0,
     *   threads: {},
     * }
     */
    VALUE results = rb_hash_new();
    rb_hash_aset(results, ID2SYM(rb_intern_const("sequence")), INT2FIX(0));
    rb_hash_aset(results, ID2SYM(rb_intern_const("threads")), rb_hash_new());

    rb_iv_set(self, "@results", results);

    pf2_start();

    if (_debug) {
        rb_funcall(rb_mKernel, rb_intern("puts"), 1, rb_str_new_cstr("[debug] Pf2 started"));
    }

    return results;
}

VALUE
rb_stop(VALUE self) {
    pf2_stop();

    if (_debug) {
        rb_funcall(rb_mKernel, rb_intern("puts"), 1, rb_str_new_cstr("[debug] Pf2 stopped"));
    }

    return rb_iv_get(self, "@results");
}
