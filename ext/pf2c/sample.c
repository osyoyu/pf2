#include <stdbool.h>
#include <time.h>
#include <pthread.h>

#include <backtrace.h>
#include <ruby.h>
#include <ruby/debug.h>

#include "backtrace_state.h"
#include "sample.h"

const int PF2_SAMPLE_MAX_NATIVE_DEPTH = 300;

static int capture_native_backtrace(struct pf2_sample *sample);
static int backtrace_on_ok(void *data, uintptr_t pc);

// Capture a sample from the current thread.
bool
pf2_sample_capture(struct pf2_sample *sample)
{
    // Initialize sample
    memset(sample, 0, sizeof(struct pf2_sample));

    // Record the current time
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    sample->timestamp_ns = (uint64_t)now.tv_sec * 1000000000ULL + (uint64_t)now.tv_nsec;

    sample->context_pthread = pthread_self();

    // Obtain the current stack from Ruby
    sample->depth = rb_profile_frames(0, 200, sample->cmes, sample->linenos);

    // Capture C-level backtrace
    sample->native_stack_depth = capture_native_backtrace(sample);

    return true;
}

// Struct to be passed to backtrace_on_ok
struct bt_data {
    struct pf2_sample *pf2_sample;
    int index;
};

static int
capture_native_backtrace(struct pf2_sample *sample)
{
    struct backtrace_state *state = global_backtrace_state;
    assert(state != NULL);

    struct bt_data data;
    data.pf2_sample = sample;
    data.index = 0;

    // Capture the current PC
    // Skip the first 2 frames (capture_native_backtrace, sigprof_handler)
    backtrace_simple(state, 2, backtrace_on_ok, pf2_backtrace_print_error, &data);

    return data.index;
}

static int
backtrace_on_ok(void *data, uintptr_t pc)
{
    struct bt_data *bt_data = (struct bt_data *)data;
    struct pf2_sample *sample = bt_data->pf2_sample;

    // Store the PC value
    if (bt_data->index < PF2_SAMPLE_MAX_NATIVE_DEPTH) {
        sample->native_stack[bt_data->index] = pc;
        bt_data->index++;
    }

    return 0; // Continue backtrace
}
