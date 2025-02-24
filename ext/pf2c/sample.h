#ifndef PF2_SAMPLE_H
#define PF2_SAMPLE_H

#include <ruby.h>

struct pf2_sample {
    int depth;
    VALUE cmes[200];
    int linenos[200];
    long consumed_time_ns;
};

#endif // PF2_SAMPLE_H
