#include <time.h>
#include <stdint.h>
#include <stdatomic.h>

#include <ruby.h>
#include <ruby/debug.h>

#include "serializer.h"
#include "session.h"

struct pf2_ser *
pf2_ser_new(void) {
    struct pf2_ser *ser = malloc(sizeof(struct pf2_ser));
    if (ser == NULL) { goto err; }
    ser->start_timestamp_ns = 0;
    ser->duration_ns = 0;

    ser->collected_sample_count = 0;
    ser->dropped_sample_count = 0;
    ser->owns_data = true;

    ser->samples = NULL;
    ser->samples_count = 0;
    ser->samples_capacity = 0;

    ser->locations = NULL;
    ser->locations_count = 0;
    ser->locations_capacity = 0;

    ser->functions = NULL;
    ser->functions_count = 0;
    ser->functions_capacity = 0;

    return ser;

err:
    return NULL;
}

void
pf2_ser_free(struct pf2_ser *serializer) {
    if (serializer->owns_data) {
        // Free samples
        for (size_t i = 0; i < serializer->samples_count; i++) {
            free(serializer->samples[i].stack);
            free(serializer->samples[i].native_stack);
        }
        free(serializer->samples);

        // Free functions
        for (size_t i = 0; i < serializer->functions_count; i++) {
            free(serializer->functions[i].name); // strdup'ed
            free(serializer->functions[i].filename); // strdup'ed
        }
        free(serializer->functions);

        // Free locations
        free(serializer->locations);
    }

    free(serializer);
}

void
pf2_ser_prepare(struct pf2_ser *serializer, struct pf2_session *session) {
    // Set metadata
    serializer->start_timestamp_ns =
        (uint64_t)session->start_time_realtime.tv_sec * 1000000000ULL +
        (uint64_t)session->start_time_realtime.tv_nsec;
    serializer->duration_ns = session->duration_ns;
    serializer->collected_sample_count =
        atomic_load_explicit(&session->collected_sample_count, memory_order_relaxed);
    serializer->dropped_sample_count =
        atomic_load_explicit(&session->dropped_sample_count, memory_order_relaxed);
    serializer->owns_data = false;

    serializer->samples = session->samples;
    serializer->samples_count = session->samples_count;
    serializer->samples_capacity = session->samples_capacity;

    serializer->functions = session->functions;
    serializer->functions_count = session->functions_count;
    serializer->functions_capacity = session->functions_capacity;

    serializer->locations = session->locations;
    serializer->locations_count = session->locations_count;
    serializer->locations_capacity = session->locations_capacity;
}

VALUE
pf2_ser_to_ruby_hash(struct pf2_ser *serializer) {
    VALUE hash = rb_hash_new();

    // Add metadata
    rb_hash_aset(hash, ID2SYM(rb_intern("start_timestamp_ns")), ULL2NUM(serializer->start_timestamp_ns));
    rb_hash_aset(hash, ID2SYM(rb_intern("duration_ns")), ULL2NUM(serializer->duration_ns));
    rb_hash_aset(hash, ID2SYM(rb_intern("collected_sample_count")), ULL2NUM(serializer->collected_sample_count));
    rb_hash_aset(hash, ID2SYM(rb_intern("dropped_sample_count")), ULL2NUM(serializer->dropped_sample_count));

    // Add samples
    VALUE samples = rb_ary_new_capa(serializer->samples_count);
    for (size_t i = 0; i < serializer->samples_count; i++) {
        struct pf2_ser_sample *sample = &serializer->samples[i];
        VALUE sample_hash = rb_hash_new();

        // Add Ruby stack
        VALUE stack = rb_ary_new_capa(sample->stack_count);
        for (size_t j = 0; j < sample->stack_count; j++) {
            rb_ary_push(stack, SIZET2NUM(sample->stack[j]));
        }
        rb_hash_aset(sample_hash, ID2SYM(rb_intern("stack")), stack);

        // Add native stack frames
        VALUE native_stack = rb_ary_new_capa(sample->native_stack_count);
        if (sample->native_stack != NULL) {
            for (size_t j = 0; j < sample->native_stack_count; j++) {
                rb_ary_push(native_stack, SIZET2NUM(sample->native_stack[j]));
            }
        }
        rb_hash_aset(sample_hash, ID2SYM(rb_intern("native_stack")), native_stack);

        // Add thread ID and elapsed time
        rb_hash_aset(
            sample_hash,
            ID2SYM(rb_intern("ruby_thread_id")),
            sample->ruby_thread_id ? ULL2NUM(sample->ruby_thread_id) : Qnil
        );
        rb_hash_aset(sample_hash, ID2SYM(rb_intern("elapsed_ns")), ULL2NUM(sample->elapsed_ns));
        rb_hash_aset(sample_hash, ID2SYM(rb_intern("count")), ULL2NUM(sample->count));

        rb_ary_push(samples, sample_hash);
    }
    rb_hash_aset(hash, ID2SYM(rb_intern("samples")), samples);

    // Add locations
    VALUE locations = rb_ary_new_capa(serializer->locations_count);
    for (size_t i = 0; i < serializer->locations_count; i++) {
        struct pf2_ser_location *location = &serializer->locations[i];
        VALUE location_hash = rb_hash_new();

        rb_hash_aset(
            location_hash,
            ID2SYM(rb_intern("function_index")),
            SIZET2NUM(location->function_index)
        );
        rb_hash_aset(
            location_hash,
            ID2SYM(rb_intern("lineno")),
            INT2NUM(location->lineno)
        );
        rb_hash_aset(location_hash, ID2SYM(rb_intern("address")), Qnil); // TODO: C functions

        rb_ary_push(locations, location_hash);
    }
    rb_hash_aset(hash, ID2SYM(rb_intern("locations")), locations);

    // Add functions
    VALUE functions = rb_ary_new_capa(serializer->functions_count);
    for (size_t i = 0; i < serializer->functions_count; i++) {
        struct pf2_ser_function *function = &serializer->functions[i];
        VALUE function_hash = rb_hash_new();

        rb_hash_aset(
            function_hash,
            ID2SYM(rb_intern("implementation")),
            function->implementation == IMPLEMENTATION_RUBY ? ID2SYM(rb_intern("ruby")) : ID2SYM(rb_intern("native"))
        ); // TODO: C functions
        rb_hash_aset(
            function_hash,
            ID2SYM(rb_intern("name")),
            function->name ? rb_str_new_cstr(function->name) : Qnil
        );
        rb_hash_aset(
            function_hash,
            ID2SYM(rb_intern("filename")),
            function->filename ? rb_str_new_cstr(function->filename) : Qnil
        );
        rb_hash_aset(
            function_hash,
            ID2SYM(rb_intern("start_lineno")),
            function->start_lineno >= 0 ? INT2NUM(function->start_lineno) : Qnil
        );
        rb_hash_aset(function_hash, ID2SYM(rb_intern("start_address")), Qnil); // TODO: C functions

        rb_ary_push(functions, function_hash);
    }
    rb_hash_aset(hash, ID2SYM(rb_intern("functions")), functions);

    return hash;
}
