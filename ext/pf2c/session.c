#include <bits/time.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <signal.h>
#include <time.h>
#include <pthread.h>

#include <ruby.h>
#include <ruby/debug.h>

#include <backtrace.h>

#include "backtrace_state.h"
#include "sample.h"
#include "session.h"
#include "serializer.h"

static void *sample_collector_thread(void *arg);
static void sigprof_handler(int sig, siginfo_t *info, void *ucontext);
bool ensure_sample_capacity(struct pf2_session *session);

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

    // Configure signal handler
    struct sigaction sa;
    sa.sa_sigaction = sigprof_handler;
    sigemptyset(&sa.sa_mask);
    sigaddset(&sa.sa_mask, SIGPROF); // Mask SIGPROFs when handler is running
    sa.sa_flags = SA_SIGINFO | SA_RESTART;
    if (sigaction(SIGPROF, &sa, NULL) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to install signal handler");
    }

    // Configure a timer to send SIGPROF every 10 ms of CPU time
    struct sigevent sev;
    sev.sigev_notify = SIGEV_SIGNAL;
    sev.sigev_signo = SIGPROF;
    sev.sigev_value.sival_ptr = session; // Passed as info->si_value.sival_ptr
    if (timer_create(CLOCK_PROCESS_CPUTIME_ID, &sev, &session->timer) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to create timer");
    }
    struct itimerspec its = {
        .it_value = {
            .tv_sec = 0,
            .tv_nsec = 10 * 1000000, // 10 ms
        },
        .it_interval = {
            .tv_sec = 0,
            .tv_nsec = 10 * 1000000, // 10 ms
        },
    };
    if (timer_settime(session->timer, 0, &its, NULL) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to start timer");
    }

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
#ifdef PF2_DEBUG
                printf("Failed to expand sample buffer. Dropping sample\n");
#endif
                break;
            }

            session->samples[session->samples_index++] = sample;
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

    struct pf2_session *session = info->si_value.sival_ptr;

    // If garbage collection is in progress, don't collect samples.
    if (atomic_load_explicit(&session->is_marking, memory_order_acquire)) {
#ifdef PF2_DEBUG
        printf("Dropping sample: Garbage collection is in progress\n");
#endif
        return;
    }

    struct pf2_sample sample = { 0 };

    if (pf2_sample_capture(&sample) == false) {
#ifdef PF2_DEBUG
        printf("Dropping sample: Failed to capture sample\n");
#endif
        return;
    }

    // Copy the sample to the ringbuffer
    if (pf2_ringbuffer_push(session->rbuf, &sample) == false) {
        // Copy failed. The sample buffer is full.
#ifdef PF2_DEBUG
        printf("Dropping sample: Sample buffer is full\n");
#endif
        return;
    }

#ifdef PF2_DEBUG
    struct timespec sig_end_time;
    clock_gettime(CLOCK_MONOTONIC, &sig_end_time);

    // Calculate elapsed time in nanoseconds
    sample.consumed_time_ns =
        (sig_end_time.tv_sec - sig_start_time.tv_sec) * 1000000000L +
        (sig_end_time.tv_nsec - sig_start_time.tv_nsec);

    printf("sigprof_handler: consumed_time_ns: %lu\n", sample.consumed_time_ns);
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

    // Calculate duration
    struct timespec end_time;
    clock_gettime(CLOCK_MONOTONIC, &end_time);
    uint64_t start_ns = (uint64_t)session->start_time.tv_sec * 1000000000ULL + (uint64_t)session->start_time.tv_nsec;
    uint64_t end_ns = (uint64_t)end_time.tv_sec * 1000000000ULL + (uint64_t)end_time.tv_nsec;
    session->duration_ns = end_ns - start_ns;

    // Disarm and delete the timer.
    if (timer_delete(session->timer) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to delete timer");
    }

    // Terminate the collector thread
    session->is_running = false;
    pthread_join(*session->collector_thread, NULL);

    // Create serializer and serialize
    struct pf2_ser *serializer = pf2_ser_new();
    pf2_ser_prepare(serializer, session);
    VALUE result = pf2_ser_to_ruby_hash(serializer);
    pf2_ser_free(serializer);

    return result;
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

    session->rbuf = pf2_ringbuffer_new(1000);
    if (session->rbuf == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    atomic_store_explicit(&session->is_marking, false, memory_order_relaxed);
    session->collector_thread = malloc(sizeof(pthread_t));
    if (session->collector_thread == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    session->duration_ns = 0;

    session->samples_index = 0;
    session->samples_capacity = 500; // 10 seconds worth of samples at 50 Hz
    session->samples = malloc(sizeof(struct pf2_sample) * session->samples_capacity);
    if (session->samples == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

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
    int head = atomic_load_explicit(&rbuf->head, memory_order_acquire);
    int tail = atomic_load_explicit(&rbuf->tail, memory_order_acquire);
    while (head != tail) {
        sample = &rbuf->samples[head];
        // TODO: Move this to mark function in pf2_sample
        for (int i = 0; i < sample->depth; i++) {
            rb_gc_mark(sample->cmes[i]);
        }
        head = (head + 1) % rbuf->size;
    }

    // Iterate over all samples in the samples array and mark them
    for (size_t i = 0; i < session->samples_index; i++) {
        sample = &session->samples[i];
        for (int i = 0; i < sample->depth; i++) {
            rb_gc_mark(sample->cmes[i]);
        }
    }

    // Allow sample collection
    atomic_store_explicit(&session->is_marking, false, memory_order_release);
}

void
pf2_session_dfree(void *sess)
{
    // TODO: Ensure the uninstall process is complete before freeing the session
    struct pf2_session *session = sess;
    pf2_ringbuffer_free(session->rbuf);
    free(session->samples);
    free(session->collector_thread);
    free(session);
}

size_t
pf2_session_dsize(const void *sess)
{
    const struct pf2_session *session = sess;
    return (
        sizeof(struct pf2_session)
        + sizeof(struct pf2_sample) * session->samples_capacity
        + sizeof(struct pf2_sample) * session->rbuf->size
    );
}
