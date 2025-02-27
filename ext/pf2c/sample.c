#include <stdbool.h>
#include <time.h>

#include <ruby.h>
#include <ruby/debug.h>

#include "sample.h"

// Capture a sample from the current thread.
bool
pf2_sample_capture(struct pf2_sample *sample)
{
    // Record the current time
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    sample->timestamp_ns = (uint64_t)now.tv_sec * 1000000000ULL + (uint64_t)now.tv_nsec;

    // Obtain the current stack from Ruby
    sample->depth = rb_profile_frames(0, 200, sample->cmes, sample->linenos);

    return true;
}
