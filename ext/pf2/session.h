#ifndef PF2_SESSION_H
#define PF2_SESSION_H

#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <limits.h>
#include <sys/time.h>

#include <ruby.h>

#include "configuration.h"
#include "ringbuffer.h"
#include "sample.h"

#include "khashl.h"

// Maps for sample storage

// BEGIN generic helpers

static inline khint_t hash_size_t(size_t v)
{
#if SIZE_MAX == UINT_MAX
    return kh_hash_uint32((khint_t)v);
#else
    return kh_hash_uint64((khint64_t)v);
#endif
}
static inline int eq_size_t(size_t a, size_t b) { return a == b; }

// END generic helpers

// BEGIN location_table

struct pf2_location_key {
    VALUE cme;
    int lineno;
};
static inline khint_t hash_location_key(struct pf2_location_key key)
{
    khint_t h = hash_size_t((size_t)key.cme);
    h ^= (khint_t)key.lineno + 0x9e3779b9U + (h << 6) + (h >> 2);
    return h;
}
static inline int eq_location_key(struct pf2_location_key a, struct pf2_location_key b)
{
    return a.cme == b.cme && a.lineno == b.lineno;
}

// END location_table

// BEGIN stack_table (Ruby stack)

struct pf2_stack_key {
    const size_t *frames; // pointer to an immutable array of location_ids
    size_t depth;
};
static inline khint_t hash_stack_key(struct pf2_stack_key key)
{
    khint_t h = hash_size_t(key.depth);
    for (size_t i = 0; i < key.depth; i++) {
        h ^= hash_size_t(key.frames[i]) + 0x9e3779b9U + (h << 6) + (h >> 2);
    }
    return h;
}
static inline int eq_stack_key(struct pf2_stack_key a, struct pf2_stack_key b)
{
    if (a.depth != b.depth) return 0;
    for (size_t i = 0; i < a.depth; i++) {
        if (a.frames[i] != b.frames[i]) return 0;
    }
    return 1;
}

// END stack_table

// BEGIN native_stack_table (raw PCs)

struct pf2_native_stack_key {
    const uintptr_t *frames; // pointer to an immutable array of PCs
    size_t depth;
};
static inline khint_t hash_native_stack_key(struct pf2_native_stack_key key)
{
    khint_t h = hash_size_t(key.depth);
    for (size_t i = 0; i < key.depth; i++) {
        h ^= kh_hash_uint64((khint64_t)key.frames[i]) + 0x9e3779b9U + (h << 6) + (h >> 2);
    }
    return h;
}
static inline int eq_native_stack_key(struct pf2_native_stack_key a, struct pf2_native_stack_key b)
{
    if (a.depth != b.depth) return 0;
    for (size_t i = 0; i < a.depth; i++) {
        if (a.frames[i] != b.frames[i]) return 0;
    }
    return 1;
}

// END native_stack_table

// BEGIN combined_sample_table

struct pf2_combined_stack_key {
    size_t ruby_stack_id;
    size_t native_stack_id;
};
static inline khint_t hash_combined_stack_key(struct pf2_combined_stack_key key)
{
    khint_t h = hash_size_t(key.ruby_stack_id);
    h ^= hash_size_t(key.native_stack_id) + 0x9e3779b9U + (h << 6) + (h >> 2);
    return h;
}
static inline int eq_combined_stack_key(struct pf2_combined_stack_key a, struct pf2_combined_stack_key b)
{
    return a.ruby_stack_id == b.ruby_stack_id && a.native_stack_id == b.native_stack_id;
}

// END combined_sample_table

struct pf2_sample_stats {
    // The number of times this sample was observed.
    size_t count;
    // Timestamps which this sample was observed. This array's length = # of samples.
    // TODO: Make timestamp collection optional?
    uint64_t *timestamps;
    // Thread ids corresponding to each timestamp.
    uintptr_t *thread_ids;
    // timestamps.length
    size_t timestamps_count;
    size_t timestamps_capacity;
};

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-function"
// location table: key = (cme, lineno), val = location_id
KHASHL_MAP_INIT(static, pf2_location_table, pf2_location_table, struct pf2_location_key, size_t, hash_location_key, eq_location_key)
// stack table: key = array of location_ids, val = stack_id
KHASHL_MAP_INIT(static, pf2_stack_table, pf2_stack_table, struct pf2_stack_key, size_t, hash_stack_key, eq_stack_key)
// native stack table: key = array of PCs, val = native_stack_id
KHASHL_MAP_INIT(static, pf2_native_stack_table, pf2_native_stack_table, struct pf2_native_stack_key, size_t, hash_native_stack_key, eq_native_stack_key)
// sample table: key = (ruby_stack_id, native_stack_id), val = aggregated counts/timestamps
KHASHL_MAP_INIT(static, pf2_sample_table, pf2_sample_table, struct pf2_combined_stack_key, struct pf2_sample_stats, hash_combined_stack_key, eq_combined_stack_key)
#pragma GCC diagnostic pop

struct pf2_sess_sample {
    size_t *stack; // array of location_indexes
    size_t stack_count;
    size_t *native_stack; // array of location_indexes
    size_t native_stack_count;
    uintptr_t ruby_thread_id;
    uint64_t elapsed_ns;
};

struct pf2_sess_location {
    size_t function_index;
    int32_t lineno;
    size_t address;
};

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

    pf2_location_table *location_table;
    pf2_stack_table *stack_table;
    pf2_native_stack_table *native_stack_table;
    pf2_sample_table *sample_table;

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
