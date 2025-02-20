#ifndef PF2_H
#define PF2_H

#include <stdatomic.h>

#include <ruby.h>

VALUE rb_pf2_session_start(VALUE self);
VALUE rb_pf2_session_stop(VALUE self);
VALUE pf2_session_alloc(VALUE self);
void pf2_session_dmark(void *sess);
void pf2_session_dfree(void *sess);
size_t pf2_session_dsize(const void *sess);

struct pf2_sample {
    int depth;
    VALUE cmes[200];
    int linenos[200];
    long consumed_time_ns;
};

struct pf2_session {
    timer_t timer;
    struct pf2_ringbuffer *rbuf;
    atomic_bool is_marking; // Whether garbage collection is in progress
};

static const rb_data_type_t pf2_session_type = {
    .wrap_struct_name = "Pf2c::Session",
    .function = {
        .dmark = pf2_session_dmark,
        .dfree = pf2_session_dfree,
        .dsize = pf2_session_dsize,
    },
    .data = NULL,
    .flags = RUBY_TYPED_FREE_IMMEDIATELY,
};

#endif // PF2_H
