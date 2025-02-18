#include <stdio.h>
#include <stdlib.h>
#include <signal.h>
#include <time.h>

#include <ruby.h>
#include <ruby/debug.h>

#include "pf2.h"

static void sigprof_handler(int sig, siginfo_t *info, void *ucontext);

VALUE rb_mPf2c;

VALUE
rb_pf2_session_start(VALUE self)
{
    struct pf2_session *session;
    TypedData_Get_Struct(self, struct pf2_session, &pf2_session_type, session);

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
    sev.sigev_value.sival_ptr = session->timer;
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

// async-signal-safe
static void
sigprof_handler(int sig, siginfo_t *info, void *ucontext)
{
#ifdef PF2_DEBUG
    struct timespec sig_start_time;
    clock_gettime(CLOCK_MONOTONIC, &sig_start_time);
#endif

    VALUE buff[200];
    int lines[200] = {0};
    rb_profile_frames(0, 200, buff, lines);

#ifdef PF2_DEBUG
    struct timespec sig_end_time;
    clock_gettime(CLOCK_MONOTONIC, &sig_end_time);

    // Calculate elapsed time in nanoseconds
    long elapsed_ns =
      (sig_end_time.tv_sec - sig_start_time.tv_sec) * 1000000000L +
      (sig_end_time.tv_nsec - sig_start_time.tv_nsec);

    // TODO: Store signal handler execution time somewhere
    printf("Signal handler execution time: %ld ns\n", elapsed_ns);
#endif
}

VALUE
rb_pf2_session_stop(VALUE self)
{
    struct pf2_session *session;
    TypedData_Get_Struct(self, struct pf2_session, &pf2_session_type, session);

    if (timer_delete(session->timer) == -1) {
        rb_raise(rb_eRuntimeError, "Failed to delete timer");
    }

    return Qtrue;
}

VALUE
pf2_session_alloc(VALUE self)
{
    struct pf2_session *session = malloc(sizeof(struct pf2_session));
    if (session == NULL) {
        rb_raise(rb_eNoMemError, "Failed to allocate memory for session");
    }
    return TypedData_Wrap_Struct(self, &pf2_session_type, session);
}

RUBY_FUNC_EXPORTED void
Init_pf2c(void)
{
    rb_mPf2c = rb_define_module("Pf2c");
    VALUE rb_mPf2c_cSession = rb_define_class_under(rb_mPf2c, "Session", rb_cObject);
    rb_define_alloc_func(rb_mPf2c_cSession, pf2_session_alloc);
    rb_define_method(rb_mPf2c_cSession, "start", rb_pf2_session_start, 0);
    rb_define_method(rb_mPf2c_cSession, "stop", rb_pf2_session_stop, 0);
}
