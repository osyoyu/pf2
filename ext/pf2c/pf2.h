#ifndef PF2_H
#define PF2_H

#include <ruby.h>

VALUE rb_pf2_session_start(VALUE self);
VALUE rb_pf2_session_stop(VALUE self);
VALUE pf2_session_alloc(VALUE self);

struct pf2_sample {
    int depth;
    VALUE cmes[200];
    int linenos[200];
    long consumed_time_ns;
};

struct pf2_session {
    timer_t timer;
};

static const rb_data_type_t pf2_session_type = {
    .wrap_struct_name = "Pf2c::Session",
    .function = {
        .dmark = NULL,
        .dfree = NULL,
        .dsize = NULL,
    },
    .data = NULL,
    .flags = RUBY_TYPED_FREE_IMMEDIATELY,
};

#endif // PF2_H
