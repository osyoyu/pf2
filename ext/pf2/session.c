#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>

#include <ruby.h>
#include <ruby/thread.h>
#include <ruby/debug.h>

#include <backtrace.h>

#include "backtrace_state.h"
#include "configuration.h"
#include "debug.h"
#include "sample.h"
#include "session.h"
#include "serializer.h"

// Pointer to current active session, for access from signal handlers
static struct pf2_session *global_current_session = NULL;

static VALUE sample_collector_thread(void *arg);
static void *pf2_sleep_no_gvl(void *arg);
static void drain_ringbuffer(struct pf2_session *session);
static void sigprof_handler(int sig, siginfo_t *info, void *ucontext);
static bool ensure_samples_capacity(struct pf2_session *session);
static bool ensure_locations_capacity(struct pf2_session *session);
static bool ensure_functions_capacity(struct pf2_session *session);
static void pf2_ser_sample_cleanup(struct pf2_ser_sample *sample);
static bool function_index_for(struct pf2_session *session, struct pf2_ser_function *function, size_t *out_index);
static bool location_index_for(
    struct pf2_session *session,
    size_t function_index,
    int32_t lineno,
    size_t address,
    size_t *out_index
);
static bool find_sample_by_ruby_stack(
    struct pf2_session *session,
    const struct pf2_ser_sample *sample,
    size_t *out_index
);
static struct pf2_ser_function extract_function_from_ruby_frame(VALUE frame);
static struct pf2_ser_function extract_function_from_native_pc(uintptr_t pc);
static void pf2_backtrace_syminfo_callback(
    void *data,
    uintptr_t pc,
    const char *symname,
    uintptr_t symval,
    uintptr_t symsize
);
static void pf2_session_stop(struct pf2_session *session);

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
        rb_intern("_test_no_install_timer")
    };
    VALUE *kwarg_values = NULL;
    rb_get_kwargs(kwargs, kwarg_labels, 0, 3, kwarg_values);

    session->configuration = pf2_configuration_new_from_options_hash(kwargs);

    return self;
}

VALUE
rb_pf2_session_start(VALUE self)
{
    struct pf2_session *session;
    TypedData_Get_Struct(self, struct pf2_session, &pf2_session_type, session);

    // Store pointer to current session for access from signal handlers
    global_current_session = session;

    session->is_running = true;

    // Record start time
    clock_gettime(CLOCK_REALTIME, &session->start_time_realtime);
    clock_gettime(CLOCK_MONOTONIC, &session->start_time);
    session->start_time_ns =
        (uint64_t)session->start_time.tv_sec * 1000000000ULL +
        (uint64_t)session->start_time.tv_nsec;

    // Spawn a collector thread which periodically wakes up and collects samples
    session->collector_thread = rb_thread_create(sample_collector_thread, (void *)session);

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

    global_current_session = session;

    if (!session->configuration->_test_no_install_timer) {
#ifdef HAVE_TIMER_CREATE
        // Configure a kernel timer to send SIGPROF periodically
        struct sigevent sev;
        sev.sigev_notify = SIGEV_SIGNAL;
        sev.sigev_signo = SIGPROF;
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
    } // if !__test_no_install_timer

    return Qtrue;
}

static VALUE
sample_collector_thread(void *arg)
{
    struct pf2_session *session = (struct pf2_session *)arg;

    while (session->is_running == true) {
        // Take samples from the ring buffer
        drain_ringbuffer(session);

        // Sleep for 100 ms
        // TODO: Replace with high watermark callback
        struct timespec ts = { .tv_sec = 0, .tv_nsec = 10 * 1000000, }; // 10 ms
        rb_thread_call_without_gvl(pf2_sleep_no_gvl, &ts, RUBY_UBF_IO, NULL);
    }

    return Qnil;
}

static void *
pf2_sleep_no_gvl(void *arg)
{
    struct timespec *ts = arg;
    nanosleep(ts, NULL);
    return NULL;
}

static void
drain_ringbuffer(struct pf2_session *session)
{
    struct pf2_sample sample;
    while (pf2_ringbuffer_pop(session->rbuf, &sample) == true) {
        bool ok = true;
        bool stored_in_session = false;
        struct pf2_ser_sample ser_sample = {0};
        ser_sample.ruby_thread_id = (uintptr_t)sample.context_pthread;
        ser_sample.elapsed_ns = sample.timestamp_ns - session->start_time_ns;
        ser_sample.count = 1;

        if (sample.depth > 0) {
            ser_sample.stack = malloc(sizeof(size_t) * (size_t)sample.depth);
            if (ser_sample.stack == NULL) {
                ok = false;
                goto sample_done;
            }
            ser_sample.stack_count = (size_t)sample.depth;

            for (int j = 0; j < sample.depth; j++) {
                VALUE frame = sample.cmes[j];
                int32_t lineno = sample.linenos[j];

                struct pf2_ser_function func = extract_function_from_ruby_frame(frame);
                size_t function_index = 0;
                if (!function_index_for(session, &func, &function_index)) {
                    ok = false;
                    goto sample_done;
                }

                size_t location_index = 0;
                if (!location_index_for(session, function_index, lineno, 0, &location_index)) {
                    ok = false;
                    goto sample_done;
                }

                ser_sample.stack[j] = location_index;
            }
        }

        if (sample.native_stack_depth > 0) {
            ser_sample.native_stack = malloc(sizeof(size_t) * sample.native_stack_depth);
            if (ser_sample.native_stack == NULL) {
                ok = false;
                goto sample_done;
            }
            ser_sample.native_stack_count = sample.native_stack_depth;

            for (size_t j = 0; j < sample.native_stack_depth; j++) {
                struct pf2_ser_function func = extract_function_from_native_pc(sample.native_stack[j]);
                size_t function_index = 0;
                if (!function_index_for(session, &func, &function_index)) {
                    ok = false;
                    goto sample_done;
                }

                size_t location_index = 0;
                if (!location_index_for(session, function_index, 0, 0, &location_index)) {
                    ok = false;
                    goto sample_done;
                }

                ser_sample.native_stack[j] = location_index;
            }
        }

        {
            size_t existing_index = 0;
            if (find_sample_by_ruby_stack(session, &ser_sample, &existing_index)) {
                session->samples[existing_index].count += 1;
                if (ser_sample.elapsed_ns > session->samples[existing_index].elapsed_ns) {
                    session->samples[existing_index].elapsed_ns = ser_sample.elapsed_ns;
                }
                goto sample_done;
            }

            if (!ensure_samples_capacity(session)) {
                ok = false;
                goto sample_done;
            }

            size_t sample_index = session->samples_count++;
            session->samples[sample_index] = ser_sample;
            stored_in_session = true;
        }

sample_done:
        if (!ok) {
            atomic_fetch_add_explicit(&session->dropped_sample_count, 1, memory_order_relaxed);
            PF2_DEBUG_LOG("Dropping sample during indexing\n");
        } else {
            atomic_fetch_add_explicit(&session->collected_sample_count, 1, memory_order_relaxed);
        }

        if (!stored_in_session) {
            pf2_ser_sample_cleanup(&ser_sample);
        }
    }
}

// async-signal-safe
static void
sigprof_handler(int sig, siginfo_t *info, void *ucontext)
{
#ifdef PF2_DEBUG
    struct timespec sig_start_time;
    clock_gettime(CLOCK_MONOTONIC, &sig_start_time);
#endif

    struct pf2_session *session = global_current_session;

    // If garbage collection is in progress, don't collect samples.
    if (atomic_load_explicit(&session->is_marking, memory_order_acquire)) {
        PF2_DEBUG_LOG("Dropping sample: Garbage collection is in progress\n");
        atomic_fetch_add_explicit(&session->dropped_sample_count, 1, memory_order_relaxed);
        return;
    }

    struct pf2_sample sample;

    if (pf2_sample_capture(&sample) == false) {
        PF2_DEBUG_LOG("Dropping sample: Failed to capture sample\n");
        atomic_fetch_add_explicit(&session->dropped_sample_count, 1, memory_order_relaxed);
        return;
    }

    // Copy the sample to the ringbuffer
    if (pf2_ringbuffer_push(session->rbuf, &sample) == false) {
        // Copy failed. The sample buffer is full.
        PF2_DEBUG_LOG("Dropping sample: Sample buffer is full\n");
        atomic_fetch_add_explicit(&session->dropped_sample_count, 1, memory_order_relaxed);
        return;
    }

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
static bool
ensure_samples_capacity(struct pf2_session *session)
{
    // Check if we need to expand
    if (session->samples_count < session->samples_capacity) {
        return true;
    }

    // Calculate new size (double the current size)
    size_t new_capacity = session->samples_capacity == 0 ? 16 : session->samples_capacity * 2;

    // Reallocate the array
    struct pf2_ser_sample *new_samples = realloc(
        session->samples,
        new_capacity * sizeof(struct pf2_ser_sample)
    );
    if (new_samples == NULL) {
        return false;
    }

    session->samples = new_samples;
    session->samples_capacity = new_capacity;

    return true;
}

static bool
ensure_functions_capacity(struct pf2_session *session)
{
    if (session->functions_count < session->functions_capacity) {
        return true;
    }

    size_t new_capacity = session->functions_capacity == 0 ? 16 : session->functions_capacity * 2;
    struct pf2_ser_function *new_functions = realloc(
        session->functions,
        new_capacity * sizeof(struct pf2_ser_function)
    );
    if (new_functions == NULL) {
        return false;
    }

    session->functions = new_functions;
    session->functions_capacity = new_capacity;
    return true;
}

static bool
ensure_locations_capacity(struct pf2_session *session)
{
    if (session->locations_count < session->locations_capacity) {
        return true;
    }

    size_t new_capacity = session->locations_capacity == 0 ? 16 : session->locations_capacity * 2;
    struct pf2_ser_location *new_locations = realloc(
        session->locations,
        new_capacity * sizeof(struct pf2_ser_location)
    );
    if (new_locations == NULL) {
        return false;
    }

    session->locations = new_locations;
    session->locations_capacity = new_capacity;
    return true;
}

static void
pf2_ser_sample_cleanup(struct pf2_ser_sample *sample)
{
    free(sample->stack);
    free(sample->native_stack);
    sample->stack = NULL;
    sample->native_stack = NULL;
    sample->stack_count = 0;
    sample->native_stack_count = 0;
}


static bool
find_sample_by_ruby_stack(
    struct pf2_session *session,
    const struct pf2_ser_sample *sample,
    size_t *out_index
)
{
    for (size_t i = 0; i < session->samples_count; i++) {
        struct pf2_ser_sample *existing = &session->samples[i];
        if (existing->ruby_thread_id != sample->ruby_thread_id) {
            continue;
        }
        if (existing->stack_count != sample->stack_count) {
            continue;
        }
        if (existing->stack_count == 0) {
            *out_index = i;
            return true;
        }
        if (memcmp(existing->stack, sample->stack, existing->stack_count * sizeof(size_t)) == 0) {
            *out_index = i;
            return true;
        }
    }

    return false;
}

static bool
function_index_for(struct pf2_session *session, struct pf2_ser_function *function, size_t *out_index)
{
    struct pf2_function_key key = pf2_function_key_build(function);
    pf2_function_map_itr itr = pf2_function_map_get(&session->function_map, key);
    if (!pf2_function_map_is_end(itr)) {
        free(function->name);
        free(function->filename);
        *out_index = itr.data->val;
        return true;
    }

    if (!ensure_functions_capacity(session)) {
        free(function->name);
        free(function->filename);
        return false;
    }

    size_t new_index = session->functions_count++;
    session->functions[new_index] = *function;

    struct pf2_function_key stored_key = pf2_function_key_build(&session->functions[new_index]);
    pf2_function_map_itr insert_itr = pf2_function_map_insert(&session->function_map, stored_key, new_index);
    if (pf2_function_map_is_end(insert_itr)) {
        session->functions_count--;
        free(function->name);
        free(function->filename);
        return false;
    }

    *out_index = new_index;
    return true;
}

static bool
location_index_for(
    struct pf2_session *session,
    size_t function_index,
    int32_t lineno,
    size_t address,
    size_t *out_index
)
{
    struct pf2_location_key key = {
        .function_index = function_index,
        .lineno = lineno,
        .address = address,
    };
    pf2_location_map_itr itr = pf2_location_map_get(&session->location_map, key);
    if (!pf2_location_map_is_end(itr)) {
        *out_index = itr.data->val;
        return true;
    }

    if (!ensure_locations_capacity(session)) {
        return false;
    }

    size_t new_index = session->locations_count++;
    session->locations[new_index].function_index = function_index;
    session->locations[new_index].lineno = lineno;
    session->locations[new_index].address = address;

    pf2_location_map_itr insert_itr = pf2_location_map_insert(&session->location_map, key, new_index);
    if (pf2_location_map_is_end(insert_itr)) {
        session->locations_count--;
        return false;
    }

    *out_index = new_index;
    return true;
}

static struct pf2_ser_function
extract_function_from_ruby_frame(VALUE frame)
{
    struct pf2_ser_function func;

    VALUE frame_full_label = rb_profile_frame_full_label(frame);
    if (RTEST(frame_full_label)) {
        const char *label = StringValueCStr(frame_full_label);
        func.name = strdup(label);
    } else {
        func.name = NULL;
    }

    VALUE frame_path = rb_profile_frame_path(frame);
    if (RTEST(frame_path)) {
        const char *path = StringValueCStr(frame_path);
        func.filename = strdup(path);
    } else {
        func.filename = NULL;
    }

    VALUE frame_first_lineno = rb_profile_frame_first_lineno(frame);
    if (RTEST(frame_first_lineno)) {
        func.start_lineno = NUM2INT(frame_first_lineno);
    } else {
        func.start_lineno = -1;
    }

    func.implementation = IMPLEMENTATION_RUBY;
    func.start_address = 0;

    return func;
}

static struct pf2_ser_function
extract_function_from_native_pc(uintptr_t pc)
{
    struct pf2_ser_function func;
    func.implementation = IMPLEMENTATION_NATIVE;

    func.start_address = 0;
    func.name = NULL;
    func.filename = NULL;
    func.start_lineno = 0;

    struct backtrace_state *state = global_backtrace_state;
    assert(state != NULL);
    backtrace_syminfo(state, pc, pf2_backtrace_syminfo_callback, pf2_backtrace_print_error, &func);

    return func;
}

static void
pf2_backtrace_syminfo_callback(
    void *data,
    uintptr_t pc,
    const char *symname,
    uintptr_t symval,
    uintptr_t symsize
)
{
    struct pf2_ser_function *func = (struct pf2_ser_function *)data;

    if (symname != NULL) {
        func->name = strdup(symname);
    }
    func->start_address = symval;
    (void)pc;
    (void)symsize;
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
    if (!session->configuration->_test_no_install_timer) {
        if (timer_delete(session->timer) == -1) {
            rb_raise(rb_eRuntimeError, "Failed to delete timer");
        }
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
    if (!NIL_P(session->collector_thread)) {
        rb_funcall(session->collector_thread, rb_intern("join"), 0);
    }
    drain_ringbuffer(session);
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
        global_backtrace_state = backtrace_create_state(NULL, 1, pf2_backtrace_print_error, NULL);
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

    // rbuf
    session->rbuf = pf2_ringbuffer_new(1000);
    if (session->rbuf == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    // is_marking
    atomic_store_explicit(&session->is_marking, false, memory_order_relaxed);

    // collector_thread
    session->collector_thread = Qnil;

    // samples, samples_count, samples_capacity
    session->samples_count = 0;
    session->samples_capacity = 256;
    session->samples = malloc(sizeof(struct pf2_ser_sample) * session->samples_capacity);
    if (session->samples == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    // functions, locations
    session->functions_count = 0;
    session->functions_capacity = 128;
    session->functions = malloc(sizeof(struct pf2_ser_function) * session->functions_capacity);
    if (session->functions == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    session->locations_count = 0;
    session->locations_capacity = 256;
    session->locations = malloc(sizeof(struct pf2_ser_location) * session->locations_capacity);
    if (session->locations == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory");
    }

    pf2_function_map_init(&session->function_map);
    pf2_location_map_init(&session->location_map);
    pf2_stack_map_init(&session->stack_map);

    // collected_sample_count, dropped_sample_count
    atomic_store_explicit(&session->collected_sample_count, 0, memory_order_relaxed);
    atomic_store_explicit(&session->dropped_sample_count, 0, memory_order_relaxed);

    // start_time_realtime, start_time, start_time_ns
    session->start_time_realtime = (struct timespec){0};
    session->start_time = (struct timespec){0};
    session->start_time_ns = 0;

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

    if (!NIL_P(session->collector_thread)) {
        rb_gc_mark(session->collector_thread);
    }

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

    pf2_configuration_free(session->configuration);
    pf2_ringbuffer_free(session->rbuf);
    for (size_t i = 0; i < session->samples_count; i++) {
        pf2_ser_sample_cleanup(&session->samples[i]);
    }
    free(session->samples);
    for (size_t i = 0; i < session->functions_count; i++) {
        free(session->functions[i].name);
        free(session->functions[i].filename);
    }
    free(session->functions);
    free(session->locations);
    pf2_function_map_cleanup(&session->function_map);
    pf2_location_map_cleanup(&session->location_map);
    pf2_stack_map_cleanup(&session->stack_map);
    free(session);
}

size_t
pf2_session_dsize(const void *sess)
{
    const struct pf2_session *session = sess;
    return (
        sizeof(struct pf2_session)
        + sizeof(struct pf2_ser_sample) * session->samples_capacity
        + sizeof(struct pf2_ser_function) * session->functions_capacity
        + sizeof(struct pf2_ser_location) * session->locations_capacity
        + sizeof(struct pf2_sample) * session->rbuf->size
    );
}
