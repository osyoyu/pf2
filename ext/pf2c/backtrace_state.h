#ifndef PF2_BACKTRACE_H
#define PF2_BACKTRACE_H

#include <backtrace.h>

extern struct backtrace_state *global_backtrace_state;

void pf2_backtrace_print_error(void *data, const char *msg, int errnum);

#endif // PF2_BACKTRACE_H
