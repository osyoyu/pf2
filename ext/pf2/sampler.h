#include <ruby.h>

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

void pf2_start(void);
void pf2_stop(void);
