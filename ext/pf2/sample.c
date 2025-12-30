#include <stdbool.h>
#include <stdlib.h>
#include <time.h>
#include <pthread.h>

#include <backtrace.h>
#include <ruby.h>
#include <ruby/debug.h>

#include "backtrace_state.h"
#include "sample.h"

static int capture_native_backtrace(struct pf2_sample *sample, int max_native_depth);
static int backtrace_on_ok(void *data, uintptr_t pc);

void
pf2_sample_free(struct pf2_sample *sample)
{
    free(sample->cmes);
    free(sample->linenos);
    free(sample->native_stack);
    sample->cmes = NULL;
    sample->linenos = NULL;
    sample->native_stack = NULL;
}

// Capture a sample from the current thread.
bool
pf2_sample_capture(struct pf2_sample *sample, const struct pf2_configuration *config)
{
    if (sample->cmes == NULL || sample->linenos == NULL || sample->native_stack == NULL) {
        return false;
    }

    // Reset per-sample fields
    sample->depth = 0;
    sample->native_stack_depth = 0;
    sample->consumed_time_ns = 0;
    sample->timestamp_ns = 0;

    // Record the current time
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    sample->timestamp_ns = (uint64_t)now.tv_sec * 1000000000ULL + (uint64_t)now.tv_nsec;

    sample->context_pthread = pthread_self();

    // Obtain the current stack from Ruby
    sample->depth = rb_profile_frames(0, config->max_ruby_depth, sample->cmes, sample->linenos);

    // Capture C-level backtrace
    sample->native_stack_depth = capture_native_backtrace(sample, config->max_native_depth);

    return true;
}

// Struct to be passed to backtrace_on_ok
struct bt_data {
    struct pf2_sample *pf2_sample;
    int index;
    int max_native_depth;
};

static int
capture_native_backtrace(struct pf2_sample *sample, int max_native_depth)
{
    struct backtrace_state *state = global_backtrace_state;
    assert(state != NULL);

    struct bt_data data;
    data.pf2_sample = sample;
    data.index = 0;
    data.max_native_depth = max_native_depth;

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
    if (bt_data->index < bt_data->max_native_depth) {
        sample->native_stack[bt_data->index] = pc;
        bt_data->index++;
    }

    return 0; // Continue backtrace
}
