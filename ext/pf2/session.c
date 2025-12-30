#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>

#include <ruby.h>
#include <ruby/debug.h>

#include <backtrace.h>

#include "backtrace_state.h"
#include "configuration.h"
#include "debug.h"
#include "sample.h"
#include "session.h"
#include "serializer.h"

#ifndef HAVE_TIMER_CREATE
// Global session pointer for setitimer fallback
static struct pf2_session *global_current_session = NULL;
#endif

static void *sample_collector_thread(void *arg);
static void sigprof_handler(int sig, siginfo_t *info, void *ucontext);
bool ensure_sample_capacity(struct pf2_session *session);
static void pf2_session_stop(struct pf2_session *session);
static bool pf2_sample_deep_copy(struct pf2_sample *dest, const struct pf2_sample *src);

VALUE
rb_pf2_session_initialize(int argc, VALUE *argv, VALUE self)
{
    struct pf2_session *session;
    TypedData_Get_Struct(self, struct pf2_session, &pf2_session_type, session);

    // Create configuration from options hash
    VALUE kwargs = Qnil;
    rb_scan_args(argc, argv, ":", &kwargs);
    ID kwarg_labels[] = {
        rb_intern("interval_ms"),
        rb_intern("time_mode"),
        rb_intern("max_depth"),
        rb_intern("max_native_depth")
    };
    VALUE *kwarg_values = NULL;
    rb_get_kwargs(kwargs, kwarg_labels, 0, 4, kwarg_values);

    session->configuration = pf2_configuration_new_from_options_hash(kwargs);
    session->rbuf = pf2_ringbuffer_new(
        1000,
        session->configuration->max_ruby_depth,
        session->configuration->max_native_depth
    );
    if (session->rbuf == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    session->samples_index = 0;
    session->samples_capacity = 500; // 10 seconds worth of samples at 50 Hz
    session->samples = malloc(sizeof(struct pf2_sample) * session->samples_capacity);
    if (session->samples == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    return self;
}

VALUE
rb_pf2_session_start(VALUE self)
{
    struct pf2_session *session;
    TypedData_Get_Struct(self, struct pf2_session, &pf2_session_type, session);

    session->is_running = true;

    // Record start time
    clock_gettime(CLOCK_REALTIME, &session->start_time_realtime);
    clock_gettime(CLOCK_MONOTONIC, &session->start_time);

    // Spawn a collector thread which periodically wakes up and collects samples
    if (pthread_create(session->collector_thread, NULL, sample_collector_thread, session) != 0) {
        rb_raise(rb_eRuntimeError, "Failed to spawn sample collector thread");
    }

    // Install signal handler for SIGPROF
    struct sigaction sa;
    sa.sa_sigaction = sigprof_handler;
    sigemptyset(&sa.sa_mask);
    sigaddset(&sa.sa_mask, SIGPROF); // Mask SIGPROFs when handler is running
    sa.sa_flags = SA_SIGINFO | SA_RESTART;
    if (sigaction(SIGPROF, &sa, NULL) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to install SIGPROF handler");
    }

#ifndef HAVE_TIMER_CREATE
    // Install signal handler for SIGALRM if using wall time mode with setitimer
    if (session->configuration->time_mode != PF2_TIME_MODE_CPU_TIME) {
        sigaddset(&sa.sa_mask, SIGALRM);
        if (sigaction(SIGALRM, &sa, NULL) == -1) {
            rb_raise(rb_eRuntimeError, "Failed to install SIGALRM handler");
        }
    }
#endif

#ifdef HAVE_TIMER_CREATE
    // Configure a kernel timer to send SIGPROF periodically
    struct sigevent sev;
    sev.sigev_notify = SIGEV_SIGNAL;
    sev.sigev_signo = SIGPROF;
    sev.sigev_value.sival_ptr = session; // Passed as info->si_value.sival_ptr
    if (timer_create(
        session->configuration->time_mode == PF2_TIME_MODE_CPU_TIME
            ? CLOCK_PROCESS_CPUTIME_ID
            : CLOCK_MONOTONIC,
        &sev,
        &session->timer
    ) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to create timer");
    }
    struct itimerspec its = {
        .it_value = {
            .tv_sec = 0,
            .tv_nsec = session->configuration->interval_ms * 1000000,
        },
        .it_interval = {
            .tv_sec = 0,
            .tv_nsec = session->configuration->interval_ms * 1000000,
        },
    };
    if (timer_settime(session->timer, 0, &its, NULL) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to start timer");
    }
#else
    // Use setitimer as fallback
    // Some platforms (e.g. macOS) do not have timer_create(3).
    // setitimer(3) can be used as a alternative, but has limited functionality.
    global_current_session = session;

    struct itimerval itv = {
        .it_value = {
            .tv_sec = 0,
            .tv_usec = session->configuration->interval_ms * 1000,
        },
        .it_interval = {
            .tv_sec = 0,
            .tv_usec = session->configuration->interval_ms * 1000,
        },
    };
    int which_timer = session->configuration->time_mode == PF2_TIME_MODE_CPU_TIME
        ? ITIMER_PROF  // CPU time (sends SIGPROF)
        : ITIMER_REAL; // Wall time (sends SIGALRM)

    if (setitimer(which_timer, &itv, NULL) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to start timer");
    }
#endif

    return Qtrue;
}

static void *
sample_collector_thread(void *arg)
{
    struct pf2_session *session = arg;

    while (session->is_running == true) {
        // Take samples from the ring buffer
        struct pf2_sample sample;
        while (pf2_ringbuffer_pop(session->rbuf, &sample) == true) {
            // Ensure we have capacity before adding a new sample
            if (!ensure_sample_capacity(session)) {
                // Failed to expand buffer
                PF2_DEBUG_LOG("Failed to expand sample buffer. Dropping sample\n");
                break;
            }

            struct pf2_sample *dest = &session->samples[session->samples_index];
            if (!pf2_sample_deep_copy(dest, &sample)) {
                PF2_DEBUG_LOG("Failed to copy sample. Dropping sample\n");
                break;
            }
            session->samples_index++;
        }

        // Sleep for 100 ms
        // TODO: Replace with high watermark callback
        struct timespec ts = { .tv_sec = 0, .tv_nsec = 10 * 1000000, }; // 10 ms
        nanosleep(&ts, NULL);
    }

    return NULL;
}

// async-signal-safe
static void
sigprof_handler(int sig, siginfo_t *info, void *ucontext)
{
#ifdef PF2_DEBUG
    struct timespec sig_start_time;
    clock_gettime(CLOCK_MONOTONIC, &sig_start_time);
#endif

    struct pf2_session *session;
#ifdef HAVE_TIMER_CREATE
    session = info->si_value.sival_ptr;
#else
    session = global_current_session;
#endif

    // If garbage collection is in progress, don't collect samples.
    if (atomic_load_explicit(&session->is_marking, memory_order_acquire)) {
        PF2_DEBUG_LOG("Dropping sample: Garbage collection is in progress\n");
        return;
    }

    struct pf2_sample *slot = NULL;
    int next_tail = 0;
    if (pf2_ringbuffer_reserve(session->rbuf, &slot, &next_tail) == false) {
        PF2_DEBUG_LOG("Dropping sample: Sample buffer is full\n");
        return;
    }

    if (pf2_sample_capture(slot, session->configuration) == false) {
        PF2_DEBUG_LOG("Dropping sample: Failed to capture sample\n");
        return;
    }

    // Copy the sample to the ringbuffer
    pf2_ringbuffer_commit(session->rbuf, next_tail);

#ifdef PF2_DEBUG
    struct timespec sig_end_time;
    clock_gettime(CLOCK_MONOTONIC, &sig_end_time);

    // Calculate elapsed time in nanoseconds
    sample.consumed_time_ns =
        (sig_end_time.tv_sec - sig_start_time.tv_sec) * 1000000000L +
        (sig_end_time.tv_nsec - sig_start_time.tv_nsec);

    PF2_DEBUG_LOG("sigprof_handler: consumed_time_ns: %lu\n", sample.consumed_time_ns);
#endif
}

// Ensures that the session's sample array has capacity for at least one more sample
// Returns true if successful, false if memory allocation failed
bool
ensure_sample_capacity(struct pf2_session *session)
{
    // Check if we need to expand
    if (session->samples_index < session->samples_capacity) {
        return true;
    }

    // Calculate new size (double the current size)
    size_t new_capacity = session->samples_capacity * 2;

    // Reallocate the array
    struct pf2_sample *new_samples = realloc(session->samples, new_capacity * sizeof(struct pf2_sample));
    if (new_samples == NULL) {
        return false;
    }

    session->samples = new_samples;
    session->samples_capacity = new_capacity;

    return true;
}

VALUE
rb_pf2_session_stop(VALUE self)
{
    struct pf2_session *session;
    TypedData_Get_Struct(self, struct pf2_session, &pf2_session_type, session);

    pf2_session_stop(session);

    // Create serializer and serialize
    struct pf2_ser *serializer = pf2_ser_new();
    pf2_ser_prepare(serializer, session);
    VALUE result = pf2_ser_to_ruby_hash(serializer);
    pf2_ser_free(serializer);

    return result;
}

static void
pf2_session_stop(struct pf2_session *session)
{
    // Calculate duration
    struct timespec end_time;
    clock_gettime(CLOCK_MONOTONIC, &end_time);
    uint64_t start_ns = (uint64_t)session->start_time.tv_sec * 1000000000ULL + (uint64_t)session->start_time.tv_nsec;
    uint64_t end_ns = (uint64_t)end_time.tv_sec * 1000000000ULL + (uint64_t)end_time.tv_nsec;
    session->duration_ns = end_ns - start_ns;

    // Disarm and delete the timer.
#ifdef HAVE_TIMER_CREATE
    if (timer_delete(session->timer) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to delete timer");
    }
#else
    struct itimerval zero_timer = {{0, 0}, {0, 0}};
    int which_timer = session->configuration->time_mode == PF2_TIME_MODE_CPU_TIME
        ? ITIMER_PROF
        : ITIMER_REAL;
    if (setitimer(which_timer, &zero_timer, NULL) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to stop timer");
    }
    global_current_session = NULL;
#endif

    // Terminate the collector thread
    session->is_running = false;
    pthread_join(*session->collector_thread, NULL);
}

VALUE
rb_pf2_session_configuration(VALUE self)
{
    struct pf2_session *session;
    TypedData_Get_Struct(self, struct pf2_session, &pf2_session_type, session);
    return pf2_configuration_to_ruby_hash(session->configuration);
}

VALUE
pf2_session_alloc(VALUE self)
{
    // Initialize state for libbacktrace
    if (global_backtrace_state == NULL) {
        global_backtrace_state = backtrace_create_state("pf2", 1, pf2_backtrace_print_error, NULL);
        if (global_backtrace_state == NULL) {
            rb_raise(rb_eRuntimeError, "Failed to initialize libbacktrace");
        }
    }

    struct pf2_session *session = malloc(sizeof(struct pf2_session));
    if (session == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    // is_running
    session->is_running = false;

    // timer
#ifdef HAVE_TIMER_CREATE
    session->timer = (timer_t)0;
#else
    session->timer = (struct itimerval){0};
#endif

    // rbuf (will be allocated in #initialize)
    session->rbuf = NULL;

    // is_marking
    atomic_store_explicit(&session->is_marking, false, memory_order_relaxed);

    // collector_thread
    session->collector_thread = malloc(sizeof(pthread_t));
    if (session->collector_thread == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    // samples, samples_index, samples_capacity
    session->samples_index = 0;
    session->samples_capacity = 0;
    session->samples = NULL;

    // start_time_realtime, start_time
    session->start_time_realtime = (struct timespec){0};
    session->start_time = (struct timespec){0};

    // duration_ns
    session->duration_ns = 0;

    // configuration
    session->configuration = NULL;

    return TypedData_Wrap_Struct(self, &pf2_session_type, session);
}

void
pf2_session_dmark(void *sess)
{
    struct pf2_session *session = sess;

    // Disallow sample collection during marking
    atomic_store_explicit(&session->is_marking, true, memory_order_release);

    // Iterate over all samples in the ringbuffer and mark them
    struct pf2_ringbuffer *rbuf = session->rbuf;
    struct pf2_sample *sample;
    if (rbuf != NULL) {
        int head = atomic_load_explicit(&rbuf->head, memory_order_acquire);
        int tail = atomic_load_explicit(&rbuf->tail, memory_order_acquire);
        while (head != tail) {
            sample = &rbuf->samples[head];
            // TODO: Move this to mark function in pf2_sample
            if (sample->cmes != NULL) {
                for (int i = 0; i < sample->depth; i++) {
                    rb_gc_mark(sample->cmes[i]);
                }
            }
            head = (head + 1) % rbuf->size;
        }
    }

    // Iterate over all samples in the samples array and mark them
    for (size_t i = 0; i < session->samples_index; i++) {
        sample = &session->samples[i];
        if (sample->cmes != NULL) {
            for (int i = 0; i < sample->depth; i++) {
                rb_gc_mark(sample->cmes[i]);
            }
        }
    }

    // Allow sample collection
    atomic_store_explicit(&session->is_marking, false, memory_order_release);
}

void
pf2_session_dfree(void *sess)
{
    struct pf2_session *session = sess;

    assert(session->is_running == false || session->is_running == true);

    // Stop the session if it's still running
    if (session->is_running) {
        pf2_session_stop(session);
    }

    if (session->configuration != NULL) {
        pf2_configuration_free(session->configuration);
    }
    if (session->rbuf != NULL) {
        pf2_ringbuffer_free(session->rbuf);
    }
    for (size_t i = 0; i < session->samples_index; i++) {
        pf2_sample_free(&session->samples[i]);
    }
    free(session->samples);
    free(session->collector_thread);
    free(session);
}

size_t
pf2_session_dsize(const void *sess)
{
    const struct pf2_session *session = sess;
    size_t size = (
        sizeof(struct pf2_session)
        + sizeof(struct pf2_sample) * session->samples_capacity
        + (session->rbuf == NULL ? 0 : sizeof(struct pf2_sample) * session->rbuf->size)
    );

    if (session->rbuf != NULL) {
        size += (size_t)session->rbuf->size * (
            sizeof(VALUE) * session->configuration->max_ruby_depth
            + sizeof(int) * session->configuration->max_ruby_depth
            + sizeof(uintptr_t) * session->configuration->max_native_depth
        );
    }

    for (size_t i = 0; i < session->samples_index; i++) {
        const struct pf2_sample *sample = &session->samples[i];
        size += sizeof(VALUE) * sample->depth;
        size += sizeof(int) * sample->depth;
        size += sizeof(uintptr_t) * sample->native_stack_depth;
    }

    return size;
}

// Utility function to deep-copy a pf2_sample from src to dest.
// For cmes, linenos, and native_stack, this function allocates exactly the
// required amount of memory, rather than the configured maximum capacity.
static bool
pf2_sample_deep_copy(struct pf2_sample *dest, const struct pf2_sample *src)
{
    memset(dest, 0, sizeof(struct pf2_sample));

    dest->context_pthread = src->context_pthread;
    dest->depth = src->depth;
    dest->native_stack_depth = src->native_stack_depth;
    dest->consumed_time_ns = src->consumed_time_ns;
    dest->timestamp_ns = src->timestamp_ns;

    if (src->depth > 0) {
        dest->cmes = malloc(sizeof(VALUE) * src->depth);
        dest->linenos = malloc(sizeof(int) * src->depth);
        if (dest->cmes == NULL || dest->linenos == NULL) {
            goto err;
        }
        memcpy(dest->cmes, src->cmes, sizeof(VALUE) * src->depth);
        memcpy(dest->linenos, src->linenos, sizeof(int) * src->depth);
    }

    if (src->native_stack_depth > 0) {
        dest->native_stack = malloc(sizeof(uintptr_t) * src->native_stack_depth);
        if (dest->native_stack == NULL) {
            goto err;
        }
        memcpy(dest->native_stack, src->native_stack, sizeof(uintptr_t) * src->native_stack_depth);
    }

    return true;

err:
    free(dest->cmes);
    free(dest->linenos);
    free(dest->native_stack);
    dest->cmes = NULL;
    dest->linenos = NULL;
    dest->native_stack = NULL;
    return false;
}
