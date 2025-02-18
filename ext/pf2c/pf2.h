#include <ruby.h>

VALUE rb_pf2_session_start(VALUE self);
VALUE rb_pf2_session_stop(VALUE self);
VALUE pf2_session_alloc(VALUE self);

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
