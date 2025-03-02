#include <time.h>
#include <stdint.h>
#include <string.h>

#include <ruby.h>
#include <ruby/debug.h>

#include <backtrace.h>

#include "backtrace_state.h"
#include "serializer.h"
#include "session.h"
#include "sample.h"

static struct pf2_ser_function extract_function_from_ruby_frame(VALUE frame);
static struct pf2_ser_function extract_function_from_native_pc(uintptr_t pc);
// static int backtrace_pcinfo_callback(void *data, uintptr_t pc, const char *filename, int lineno, const char *function);
static void pf2_backtrace_syminfo_callback(void *data, uintptr_t pc, const char *symname, uintptr_t symval, uintptr_t symsize);
static int function_index_for(struct pf2_ser *serializer, struct pf2_ser_function *function);
static int location_index_for(struct pf2_ser *serializer, int function_index, int32_t lineno);
static void ensure_samples_capacity(struct pf2_ser *serializer);
static void ensure_locations_capacity(struct pf2_ser *serializer);
static void ensure_functions_capacity(struct pf2_ser *serializer);

struct pf2_ser *
pf2_ser_new(void) {
    struct pf2_ser *ser = malloc(sizeof(struct pf2_ser));
    if (ser == NULL) { goto err; }
    ser->start_timestamp_ns = 0;
    ser->duration_ns = 0;

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

    free(serializer);
}

void
pf2_ser_prepare(struct pf2_ser *serializer, struct pf2_session *session) {
    // Set metadata
    serializer->start_timestamp_ns =
        (uint64_t)session->start_time_realtime.tv_sec * 1000000000ULL +
        (uint64_t)session->start_time_realtime.tv_nsec;
    serializer->duration_ns = session->duration_ns;

    // Process samples
    for (size_t i = 0; i < session->samples_index; i++) {
        struct pf2_sample *sample = &session->samples[i];
        ensure_samples_capacity(serializer);

        struct pf2_ser_sample *ser_sample = &serializer->samples[serializer->samples_count++];
        ser_sample->ruby_thread_id = 0; // TODO: Add thread ID support
        ser_sample->elapsed_ns = sample->timestamp_ns - serializer->start_timestamp_ns;

        // Copy and process Ruby stack frames
        ser_sample->stack = malloc(sizeof(size_t) * sample->depth);
        ser_sample->stack_count = sample->depth;
        for (int j = 0; j < sample->depth; j++) {
            VALUE frame = sample->cmes[j];
            int32_t lineno = sample->linenos[j];

            struct pf2_ser_function func = extract_function_from_ruby_frame(frame);
            size_t function_index = function_index_for(serializer, &func);
            size_t location_index = location_index_for(serializer, function_index, lineno);

            ser_sample->stack[j] = location_index;
        }

        // Copy and process native stack frames, if any
        if (sample->native_stack_depth > 0) {
            ser_sample->native_stack = malloc(sizeof(size_t) * sample->native_stack_depth);
            ser_sample->native_stack_count = sample->native_stack_depth;

            for (size_t j = 0; j < sample->native_stack_depth; j++) {
                struct pf2_ser_function func = extract_function_from_native_pc(sample->native_stack[j]);
                size_t function_index = function_index_for(serializer, &func);
                size_t location_index = location_index_for(serializer, function_index, 0);

                ser_sample->native_stack[j] = location_index;
            }
        } else {
            ser_sample->native_stack = NULL;
            ser_sample->native_stack_count = 0;
        }

    }
}

VALUE
pf2_ser_to_ruby_hash(struct pf2_ser *serializer) {
    VALUE hash = rb_hash_new();

    // Add metadata
    rb_hash_aset(hash, ID2SYM(rb_intern("start_timestamp_ns")), ULL2NUM(serializer->start_timestamp_ns));
    rb_hash_aset(hash, ID2SYM(rb_intern("duration_ns")), ULL2NUM(serializer->duration_ns));

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
                rb_ary_push(native_stack, ULL2NUM(sample->native_stack[j]));
            }
        }
        rb_hash_aset(sample_hash, ID2SYM(rb_intern("native_stack")), native_stack);

        // Add thread ID and elapsed time
        rb_hash_aset(
            sample_hash,
            ID2SYM(rb_intern("ruby_thread_id")),
            sample->ruby_thread_id ? SIZET2NUM(sample->ruby_thread_id) : Qnil
        );
        rb_hash_aset(sample_hash, ID2SYM(rb_intern("elapsed_ns")), ULL2NUM(sample->elapsed_ns));

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

static struct pf2_ser_function
extract_function_from_ruby_frame(VALUE frame) {
    struct pf2_ser_function func;

    VALUE frame_full_label = rb_profile_frame_full_label(frame);
    if (RTEST(frame_full_label)) {
        const char *label = StringValueCStr(frame_full_label);
        func.name = strdup(label);
    } else {
        func.name = NULL;
    }

    VALUE frame_path = rb_profile_frame_path(frame);
    if (RTEST(frame_path)) {
        const char *path = StringValueCStr(frame_path);
        func.filename = strdup(path);
    } else {
        func.filename = NULL;
    }

    VALUE frame_first_lineno = rb_profile_frame_first_lineno(frame);
    if (RTEST(frame_first_lineno)) {
        func.start_lineno = NUM2INT(frame_first_lineno);
    } else {
        func.start_lineno = -1;
    }

    func.implementation = IMPLEMENTATION_RUBY;
    func.start_address = 0; // For C functions

    return func;
}

static struct pf2_ser_function
extract_function_from_native_pc(uintptr_t pc) {
    struct pf2_ser_function func;
    func.implementation = IMPLEMENTATION_NATIVE;

    func.start_address = 0;
    func.name = NULL;
    func.filename = NULL;
    func.start_lineno = 0;

    // Use libbacktrace to get function details
    struct backtrace_state *state = global_backtrace_state;
    assert(state != NULL);
    backtrace_syminfo(state, pc, pf2_backtrace_syminfo_callback, pf2_backtrace_print_error, &func);

    // TODO: backtrace_pcinfo could give us more information, such as filenames and linenos.
    // Maybe try pcinfo first, then use syminfo as a fallback?

    return func;
}

// Full callback for syminfo
static void
pf2_backtrace_syminfo_callback(void *data, uintptr_t pc, const char *symname, uintptr_t symval, uintptr_t symsize) {
    struct pf2_ser_function *func = (struct pf2_ser_function *)data;

    if (symname != NULL) {
        func->name = strdup(symname);
    }
    func->start_address = symval;

    return;
}

// static int
// backtrace_pcinfo_callback(void *data, uintptr_t pc, const char *filename, int lineno, const char *function) {
//     struct pf2_ser_function *func = (struct pf2_ser_function *)data;

//     if (function) {
//         func->name = strdup(function);
//     }
//     func->start_lineno = lineno;
//     if (filename) {
//         func->filename = strdup(filename);
//     }

//     // Return non-zero to stop after first match
//     return 1;
// }

// Returns the index of the function in `functions`.
// Calling this method will modify `serializer->profile` in place.
static int
function_index_for(struct pf2_ser *serializer, struct pf2_ser_function *function) {
    for (size_t i = 0; i < serializer->functions_count; i++) {
        struct pf2_ser_function *existing = &serializer->functions[i];
        if (
            (existing->name == NULL && function->name == NULL) ||
            (existing->name != NULL && function->name != NULL && strcmp(existing->name, function->name) == 0)
        ) {
            if (
                (existing->filename == NULL && function->filename == NULL) ||
                (existing->filename != NULL && function->filename != NULL && strcmp(existing->filename, function->filename) == 0)
            ) {
                if (existing->start_lineno == function->start_lineno) {
                    return i;
                }
            }
        }
    }

    // No matching function was found, add the function to the profile.
    ensure_functions_capacity(serializer);
    size_t new_index = serializer->functions_count++;
    serializer->functions[new_index] = *function;
    return new_index;
}

// Returns the index of the location in `locations`.
// Calling this method will modify `self.profile` in place.
static int
location_index_for(struct pf2_ser *serializer, int function_index, int32_t lineno) {
    for (size_t i = 0; i < serializer->locations_count; i++) {
        struct pf2_ser_location *existing = &serializer->locations[i];
        if (existing->function_index == function_index && existing->lineno == lineno) {
            return i;
        }
    }

    ensure_locations_capacity(serializer);
    size_t new_index = serializer->locations_count++;
    serializer->locations[new_index].function_index = function_index;
    serializer->locations[new_index].lineno = lineno;
    serializer->locations[new_index].address = 0;  // Not handling addresses in this version
    return new_index;
}

static void
ensure_samples_capacity(struct pf2_ser *serializer) {
    if (serializer->samples_count >= serializer->samples_capacity) {
        size_t new_capacity = serializer->samples_capacity == 0 ? 16 : serializer->samples_capacity * 2;
        serializer->samples = realloc(serializer->samples, new_capacity * sizeof(struct pf2_ser_sample));
        serializer->samples_capacity = new_capacity;
    }
}

static void
ensure_locations_capacity(struct pf2_ser *serializer) {
    if (serializer->locations_count >= serializer->locations_capacity) {
        size_t new_capacity = serializer->locations_capacity == 0 ? 16 : serializer->locations_capacity * 2;
        serializer->locations = realloc(serializer->locations, new_capacity * sizeof(struct pf2_ser_location));
        serializer->locations_capacity = new_capacity;
    }
}

static void
ensure_functions_capacity(struct pf2_ser *serializer) {
    if (serializer->functions_count >= serializer->functions_capacity) {
        size_t new_capacity = serializer->functions_capacity == 0 ? 16 : serializer->functions_capacity * 2;
        serializer->functions = realloc(serializer->functions, new_capacity * sizeof(struct pf2_ser_function));
        serializer->functions_capacity = new_capacity;
    }
}
