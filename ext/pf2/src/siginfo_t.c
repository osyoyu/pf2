#include <signal.h>

void *extract_si_value_sival_ptr(siginfo_t *siginfo) {
  return siginfo->si_value.sival_ptr;
}
