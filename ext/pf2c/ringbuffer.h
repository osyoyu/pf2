#ifndef PF2_RINGBUFFER_H
#define PF2_RINGBUFFER_H

#include <stdatomic.h>
#include <stdbool.h>

#include "sample.h"

// A lock-free ringbuffer for storing pf2_sample structs.
// Thread safe for single-producer single-consumer (SPSC) use.
struct pf2_ringbuffer {
    int size;
    atomic_int head;
    atomic_int tail;
    struct pf2_sample *samples;
};

struct pf2_ringbuffer * pf2_ringbuffer_new(int size);
void pf2_ringbuffer_free(struct pf2_ringbuffer *ringbuf);
// async-signal-safe
bool pf2_ringbuffer_push(struct pf2_ringbuffer *ringbuf, struct pf2_sample *sample);
bool pf2_ringbuffer_pop(struct pf2_ringbuffer *ringbuf, struct pf2_sample *out);

#endif // RINGBUFFER_H
