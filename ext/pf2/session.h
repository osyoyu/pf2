#ifndef PF2_SESSION_H
#define PF2_SESSION_H

#include <pthread.h>
#include <stdatomic.h>
#include <sys/time.h>

#include <ruby.h>

#include "configuration.h"
#include "ringbuffer.h"
#include "sample.h"

struct pf2_session {
    bool is_running;
#ifdef HAVE_TIMER_CREATE
    timer_t timer;
#else
    struct itimerval timer;
#endif
    struct pf2_ringbuffer *rbuf;
    atomic_bool is_marking; // Whether garbage collection is in progress
    pthread_t *collector_thread;

    struct pf2_sample *samples; // Dynamic array of samples
    size_t samples_index;
    size_t samples_capacity;    // Current capacity of the samples array

    struct timespec start_time_realtime;
    struct timespec start_time; // When profiling started
    uint64_t duration_ns; // Duration of profiling in nanoseconds

    atomic_uint_fast64_t collected_sample_count; // Number of samples copied out of the ringbuffer
    atomic_uint_fast64_t dropped_sample_count; // Number of samples dropped for any reason

    struct pf2_configuration *configuration;
};

VALUE rb_pf2_session_initialize(int argc, VALUE *argv, VALUE self);
VALUE rb_pf2_session_start(VALUE self);
VALUE rb_pf2_session_stop(VALUE self);
VALUE rb_pf2_session_configuration(VALUE self);
VALUE pf2_session_alloc(VALUE self);
void pf2_session_dmark(void *sess);
void pf2_session_dfree(void *sess);
size_t pf2_session_dsize(const void *sess);

static const rb_data_type_t pf2_session_type = {
    .wrap_struct_name = "Pf2::Session",
    .function = {
        .dmark = pf2_session_dmark,
        .dfree = pf2_session_dfree,
        .dsize = pf2_session_dsize,
    },
    .data = NULL,
    .flags = 0,
};

#endif // PF2_SESSION_H
