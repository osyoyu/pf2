#include <stdio.h>
#include "backtrace_state.h"

struct backtrace_state *global_backtrace_state = NULL;

void
pf2_backtrace_print_error(void *data, const char *msg, int errnum)
{
    printf("libbacktrace error callback: %s (errnum %d)\n", msg, errnum);
}
