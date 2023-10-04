#include <errno.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <sys/syscall.h>

#include <ruby.h>
#include <ruby/debug.h>
#include <ruby/thread.h>

#define MAX_BUFFER_SIZE 3000

struct pf2_buffer_t {
    VALUE framebuffer[MAX_BUFFER_SIZE];
    int linebuffer[MAX_BUFFER_SIZE];
};

struct gvl_record {
    pid_t thread_id; // native_thread_id
    int event_type; // -1: unknown, 0: waiting, 1: resumed
    unsigned long long time_ns; // nanoseconds passed since the profiler started
};

// Ruby functions
void Init_pf2(void);
VALUE rb_start(VALUE self, VALUE debug);
VALUE rb_stop(VALUE self);

static void pf2_start(void);
static void pf2_stop(void);
static void pf2_signal_handler(int signo);
static void pf2_postponed_job(void *_);
static void pf2_gvl_hook_register();
static void pf2_gvl_event_handler(rb_event_flag_t event, const rb_internal_thread_event_data_t *_, void *__);

static void pf2_record(struct pf2_buffer_t *buffer);
static VALUE find_or_create_thread_results(VALUE results, pid_t thread_id);

// Buffer to record rb_profile_frames() results
struct pf2_buffer_t buffer;
// The time when the profiler started
struct timespec initial_time;
// Debug print?
bool _debug = false;

rb_internal_thread_event_hook_t *gvl_hook = NULL;

struct gvl_record gvl_records[30000];
int gvl_record_cnt = 0;

void
Init_pf2(void)
{
    VALUE rb_mPf2 = rb_define_module("Pf2");
    rb_define_module_function(rb_mPf2, "start", rb_start, 1);
    rb_define_module_function(rb_mPf2, "stop", rb_stop, 0);
}

VALUE
rb_start(VALUE self, VALUE debug) {
    _debug = RTEST(debug);

    /**
     * {
     *   sequence: 0,
     *   threads: {},
     * }
     */
    VALUE results = rb_hash_new();
    rb_hash_aset(results, ID2SYM(rb_intern_const("sequence")), INT2FIX(0));
    rb_hash_aset(results, ID2SYM(rb_intern_const("threads")), rb_hash_new());

    rb_iv_set(self, "@results", results);

    pf2_start();

    if (_debug) {
        rb_funcall(rb_mKernel, rb_intern("puts"), 1, rb_str_new_cstr("[debug] Pf2 started"));
    }

    return results;
}

VALUE
rb_stop(VALUE self) {
    pf2_stop();

    if (_debug) {
        rb_funcall(rb_mKernel, rb_intern("puts"), 1, rb_str_new_cstr("[debug] Pf2 stopped"));
    }

    return rb_iv_get(self, "@results");
}

static void
pf2_start(void)
{
    clock_gettime(CLOCK_MONOTONIC, &initial_time);

    // Configure timer for every 10 ms
    // TODO: Make interval configurable
    struct itimerval timer;
    timer.it_value.tv_sec = 1;
    timer.it_value.tv_usec = 0;
    timer.it_interval.tv_sec = 0;
    timer.it_interval.tv_usec = 10 * 1000; // 10 ms
    if (signal(SIGALRM, pf2_signal_handler) == SIG_ERR) {
        rb_syserr_fail(errno, "Failed to configure profiling timer");
    };
    if (setitimer(ITIMER_REAL, &timer, NULL) == -1) {
        rb_syserr_fail(errno, "Failed to configure profiling timer");
    };

    // Register GVL event hook
    pf2_gvl_hook_register();
}

static void
pf2_stop(void)
{
    // Stop timer
    struct itimerval timer = { 0 };
    setitimer(ITIMER_REAL, &timer, NULL);
    // Remove GVL event hook
    rb_internal_thread_remove_event_hook(gvl_hook);

    // Append GVL timing information to per-Thread results
    VALUE rb_mPf2 = rb_const_get(rb_cObject, rb_intern("Pf2"));
    VALUE results = rb_iv_get(rb_mPf2, "@results");
    for (int i = 0; i < gvl_record_cnt; i++) {
        struct gvl_record gr = gvl_records[i];

        VALUE thread_results = find_or_create_thread_results(results, gr.thread_id);
        VALUE gvl_timings = rb_hash_aref(thread_results, ID2SYM(rb_intern_const("gvl_timings")));

        VALUE gvl_record = rb_hash_new();
        VALUE record_type;
        if (gr.event_type == 0) {
            record_type = ID2SYM(rb_intern_const("waiting"));
        } else if (gr.event_type == 1) {
            record_type = ID2SYM(rb_intern_const("resumed"));
        } else {
            record_type = ID2SYM(rb_intern_const("unknown"));
        }
        rb_hash_aset(gvl_record, ID2SYM(rb_intern_const("type")), record_type);
        rb_hash_aset(gvl_record, ID2SYM(rb_intern_const("time")), ULL2NUM(gr.time_ns));

        rb_ary_push(gvl_timings, gvl_record);
    }
}

// async-signal-safe
static void
pf2_signal_handler(int signo)
{
    rb_postponed_job_register_one(0, pf2_postponed_job, 0);
}

static void
pf2_postponed_job(void *_) {
    pf2_record(&buffer);
};

static void
pf2_gvl_hook_register(void)
{
    gvl_hook = rb_internal_thread_add_event_hook(
        pf2_gvl_event_handler,
        (
            RUBY_INTERNAL_THREAD_EVENT_READY |
            RUBY_INTERNAL_THREAD_EVENT_RESUMED
        ),
        NULL
    );
}

static void
pf2_gvl_event_handler(rb_event_flag_t event, const rb_internal_thread_event_data_t *_, void *__)
{
    if (gvl_record_cnt >= 29999) { return; }

    // get the current time
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);

    pid_t current_thread_id = (pid_t)syscall(SYS_gettid);

    int event_type;
    if (event == RUBY_INTERNAL_THREAD_EVENT_READY) {
        event_type = 0;
    } else if (event == RUBY_INTERNAL_THREAD_EVENT_RESUMED) {
        event_type = 1;
    } else {
        event_type = -1;
    }

    unsigned long long nsec = (ts.tv_sec - initial_time.tv_sec) * 1000000000 + ts.tv_nsec - initial_time.tv_nsec;

    struct gvl_record gr;
    gr.thread_id = current_thread_id;
    gr.event_type = event_type;
    gr.time_ns = nsec;

    gvl_records[gvl_record_cnt++] = gr;
}

// Buffer structure
static void
pf2_record(struct pf2_buffer_t *buffer)
{
    // get the current time
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);

    VALUE rb_mPf2 = rb_const_get(rb_cObject, rb_intern("Pf2"));
    VALUE results = rb_iv_get(rb_mPf2, "@results");

    // Iterate over all Threads
    VALUE threads = rb_iv_get(rb_mPf2, "@@threads");
    for (int i = 0; i < RARRAY_LEN(threads); i++) {
        VALUE thread = rb_ary_entry(threads, i);
        VALUE thread_status = rb_funcall(thread, rb_intern("status"), 0);
        if (NIL_P(thread) || thread_status == Qfalse) {
            // Thread is dead, just ignore
            continue;
        }

        pid_t thread_id = NUM2INT(rb_funcall(thread, rb_intern("native_thread_id"), 0));
        VALUE thread_results = find_or_create_thread_results(results, thread_id);
        assert(!NIL_P(thread_results));

        // The actual querying
        int stack_depth = rb_profile_thread_frames(thread, 0, MAX_BUFFER_SIZE, buffer->framebuffer, buffer->linebuffer);

        // TODO: Reimplement Pf2-internal data structures without CRuby
        // (which will allow us to release the GVL at this point)
        // rb_thread_call_without_gvl(...);

        VALUE frames_table = rb_hash_lookup(thread_results, ID2SYM(rb_intern_const("frames")));
        assert(!NIL_P(frames_table));
        VALUE samples = rb_hash_lookup(thread_results, ID2SYM(rb_intern_const("samples")));
        assert(!NIL_P(samples));

        // Dig down the stack (top of call stack -> bottom (root))
        VALUE stack_tree_p = rb_hash_lookup(thread_results, ID2SYM(rb_intern_const("stack_tree")));
        for (int i = stack_depth - 1; i >= 0; i--) {
            assert(NIL_P(buffer->framebuffer[i]));

            // Collect & record frame information
            VALUE frame_obj_id = rb_obj_id(buffer->framebuffer[i]);
            VALUE frame_table_entry = rb_hash_aref(frames_table, frame_obj_id);
            if (NIL_P(frame_table_entry)) {
                frame_table_entry = rb_hash_new();
                rb_hash_aset(frame_table_entry, ID2SYM(rb_intern_const("full_label")), rb_profile_frame_full_label(buffer->framebuffer[i]));
                rb_hash_aset(frames_table, frame_obj_id, frame_table_entry);
            }

            VALUE children = rb_hash_aref(stack_tree_p, ID2SYM(rb_intern_const("children")));
            VALUE next_node = rb_hash_lookup(children, frame_obj_id);
            // If this is the first time we see this frame, register it to the stack tree
            if (NIL_P(next_node)) { // not found
                next_node = rb_hash_new();

                // Increment sequence
                VALUE next =
                    rb_funcall(
                        rb_hash_lookup(results, ID2SYM(rb_intern_const("sequence"))),
                        rb_intern("+"),
                        1,
                        INT2FIX(1)
                    );
                rb_hash_aset(results, ID2SYM(rb_intern_const("sequence")), next);

                rb_hash_aset(next_node, ID2SYM(rb_intern_const("node_id")), INT2FIX(next));
                rb_hash_aset(next_node, ID2SYM(rb_intern_const("frame_id")), frame_obj_id);
                rb_hash_aset(next_node, ID2SYM(rb_intern_const("full_label")), rb_profile_frame_full_label(buffer->framebuffer[i]));
                rb_hash_aset(next_node, ID2SYM(rb_intern_const("children")), rb_hash_new());

                rb_hash_aset(children, frame_obj_id, next_node);
            }

            VALUE stack_tree_id = rb_hash_aref(next_node, ID2SYM(rb_intern_const("node_id")));

            // If on leaf
            if (i == 0) {
                // Record sample
                VALUE sample = rb_hash_new();
                rb_hash_aset(sample, ID2SYM(rb_intern_const("stack_tree_id")), stack_tree_id);
                unsigned long long nsec = (ts.tv_sec - initial_time.tv_sec) * 1000000000 + ts.tv_nsec - initial_time.tv_nsec;
                rb_hash_aset(sample, ID2SYM(rb_intern_const("timestamp")), ULL2NUM(nsec));
                rb_ary_push(samples, sample);
            }

            stack_tree_p = next_node;
        }
    }
}

static VALUE
find_or_create_thread_results(VALUE results, pid_t thread_id) {
    assert(!NIL_P(results));
    assert(!NIL_P(thread));

    VALUE threads = rb_hash_aref(results, ID2SYM(rb_intern_const("threads")));
    VALUE thread_results = rb_hash_aref(threads, INT2NUM(thread_id));
    if (NIL_P(thread_results)) {
        /**
         * {
         *   thread_id: 1,
         *   frames: [],
         *   stack_tree: {
         *     node_id: ...,
         *     children: {}
         *   },
         *   samples: [],
         * }
         */
        thread_results = rb_hash_new();
        rb_hash_aset(thread_results, ID2SYM(rb_intern_const("thread_id")), INT2NUM(thread_id));

        rb_hash_aset(thread_results, ID2SYM(rb_intern_const("frames")), rb_hash_new());
        VALUE stack_tree = rb_hash_aset(thread_results, ID2SYM(rb_intern_const("stack_tree")), rb_hash_new());
        rb_hash_aset(stack_tree, ID2SYM(rb_intern_const("node_id")), ID2SYM(rb_intern_const("root")));
        rb_hash_aset(stack_tree, ID2SYM(rb_intern_const("children")), rb_hash_new());
        rb_hash_aset(thread_results, ID2SYM(rb_intern_const("samples")), rb_ary_new());
        rb_hash_aset(thread_results, ID2SYM(rb_intern_const("gvl_timings")), rb_ary_new());

        rb_hash_aset(threads, INT2NUM(thread_id), thread_results);
    }
    return thread_results;
}
