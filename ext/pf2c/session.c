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

#include "session.h"
#include "serializer.h"

static void *sample_collector_thread(void *arg);
static void sigprof_handler(int sig, siginfo_t *info, void *ucontext);

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
            if (session->samples_index >= 2000) {
                // Samples buffer is full.
                // TODO: Expand the buffer
#ifdef PF2_DEBUG
                printf("Sample buffer is full. Dropping sample\n");
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

    // Record the current time
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    sample.timestamp_ns = (uint64_t)now.tv_sec * 1000000000ULL + (uint64_t)now.tv_nsec;

    // Obtain the current stack from Ruby
    sample.depth = rb_profile_frames(0, 200, sample.cmes, sample.linenos);
    // Copy the sample to the ringbuffer.
    if (pf2_ringbuffer_push(session->rbuf, &sample) == false) {
        // Copy failed. The sample buffer is full.
#ifdef PF2_DEBUG
        printf("Dropping sample: Sample buffer is full\n");
#endif
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
    struct pf2_session *session = malloc(sizeof(struct pf2_session));
    if (session == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory for session");
    }
    session->rbuf = pf2_ringbuffer_new(1000);
    atomic_store_explicit(&session->is_marking, false, memory_order_relaxed);
    session->collector_thread = malloc(sizeof(pthread_t));
    session->duration_ns = 0;

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
    for (int i = 0; i < session->samples_index; i++) {
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
    struct pf2_session *session = sess;
    pf2_ringbuffer_free(session->rbuf);
    free(session);
}

size_t
pf2_session_dsize(const void *sess)
{
    const struct pf2_session *session = sess;
    return (
        sizeof(struct pf2_session)
        + sizeof(struct pf2_sample) * session->rbuf->size
    );
}
