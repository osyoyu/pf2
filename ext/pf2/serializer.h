#ifndef PF2C_SERIALIZER_H
#define PF2C_SERIALIZER_H

#include <stdint.h>

#include <ruby.h>

#include "session.h"

struct pf2_ser_sample {
    size_t *stack; // array of location_indexes
    size_t stack_count;
    size_t *native_stack; // array of location_indexes
    size_t native_stack_count;
    uintptr_t ruby_thread_id;
    uint64_t elapsed_ns;
};

struct pf2_ser_location {
    size_t function_index;
    int32_t lineno;
    size_t address;
};

enum function_implementation {
    IMPLEMENTATION_RUBY,
    IMPLEMENTATION_NATIVE
};

struct pf2_ser_function {
    enum function_implementation implementation;
    char *name;
    char *filename;
    int32_t start_lineno;
    size_t start_address;
};

struct pf2_ser {
    uint64_t start_timestamp_ns;
    uint64_t duration_ns;

    struct pf2_ser_sample *samples;
    size_t samples_count;
    size_t samples_capacity;

    struct pf2_ser_location *locations;
    size_t locations_count;
    size_t locations_capacity;

    struct pf2_ser_function *functions;
    size_t functions_count;
    size_t functions_capacity;
};

struct pf2_ser *pf2_ser_new(void);
void pf2_ser_free(struct pf2_ser *serializer);
void pf2_ser_prepare(struct pf2_ser *serializer, struct pf2_session *session);
VALUE pf2_ser_to_ruby_hash(struct pf2_ser *serializer);

#endif // PF2C_SERIALIZER_H
