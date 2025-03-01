#ifndef PF2_SAMPLE_H
#define PF2_SAMPLE_H

#include <ruby.h>

extern const int PF2_SAMPLE_MAX_NATIVE_DEPTH;

struct pf2_sample {
    int depth;
    VALUE cmes[200];
    int linenos[200];

    size_t native_stack_depth;
    uintptr_t native_stack[200];

    uint64_t consumed_time_ns;
    uint64_t timestamp_ns;
};

bool pf2_sample_capture(struct pf2_sample *sample);

#endif // PF2_SAMPLE_H
