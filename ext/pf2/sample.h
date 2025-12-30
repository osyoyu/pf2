#ifndef PF2_SAMPLE_H
#define PF2_SAMPLE_H

#include <pthread.h>

#include <ruby.h>

#include "configuration.h"

struct pf2_sample {
    pthread_t context_pthread;

    int depth;
    VALUE *cmes;
    int *linenos;

    size_t native_stack_depth;
    uintptr_t *native_stack;

    uint64_t consumed_time_ns;
    uint64_t timestamp_ns;
};

void pf2_sample_free(struct pf2_sample *sample);
bool pf2_sample_capture(struct pf2_sample *sample, const struct pf2_configuration *config);

#endif // PF2_SAMPLE_H
