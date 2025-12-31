#ifndef PF2_SAMPLE_H
#define PF2_SAMPLE_H

#include <pthread.h>

#include <ruby.h>

#define PF2_SAMPLE_MAX_RUBY_DEPTH 1024
#define PF2_SAMPLE_MAX_NATIVE_DEPTH 512

struct pf2_sample {
    pthread_t context_pthread;

    int depth;
    VALUE cmes[PF2_SAMPLE_MAX_RUBY_DEPTH];
    int linenos[PF2_SAMPLE_MAX_RUBY_DEPTH];

    size_t native_stack_depth;
    uintptr_t native_stack[PF2_SAMPLE_MAX_NATIVE_DEPTH];

    uint64_t consumed_time_ns;
    uint64_t timestamp_ns;
};

bool pf2_sample_capture(struct pf2_sample *sample);

#endif // PF2_SAMPLE_H
