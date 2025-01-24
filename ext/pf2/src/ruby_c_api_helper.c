#include <ruby.h>

VALUE rb_ull2num(unsigned long long n) {
    if (POSFIXABLE(n)) return LONG2FIX((long)n);
    return rb_ull2inum(n);
}
