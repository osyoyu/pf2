#include <stdbool.h>
#include <stdlib.h>

#include "ringbuffer.h"

struct pf2_ringbuffer *
pf2_ringbuffer_new(int size) {
    if (size <= 0) { return NULL; }

    struct pf2_ringbuffer *ringbuf = malloc(sizeof(struct pf2_ringbuffer));
    if (!ringbuf) { goto err; }
    ringbuf->size = size + 1; // One extra slot is required to distinguish full from empty
    ringbuf->head = 0;
    ringbuf->tail = 0;
    ringbuf->samples = malloc(ringbuf->size * sizeof(struct pf2_sample));
    if (!ringbuf->samples) { goto err_free_ringbuf; }
    return ringbuf;

err_free_ringbuf:
    free(ringbuf);
err:
    return NULL;
}

void
pf2_ringbuffer_free(struct pf2_ringbuffer *ringbuf) {
    free(ringbuf->samples);
    free(ringbuf);
}

// Returns 0 on success, 1 on failure (buffer full).
bool
pf2_ringbuffer_push(struct pf2_ringbuffer *ringbuf, struct pf2_sample *sample) {
    // Tail is only modified by the producer thread (us), so relaxed ordering is sufficient
    const int current_tail = atomic_load_explicit(&ringbuf->tail, memory_order_relaxed);
    const int next_tail = (current_tail + 1) % ringbuf->size;

    // Check head to see if buffer is full. If next_tail == head, the buffer is full.
    // Use acquire ordering to synchronize with the head update in pf2_ringbuffer_pop().
    // This ensures we see the latest head value.
    if (next_tail == atomic_load_explicit(&ringbuf->head, memory_order_acquire)) {
        return false;  // Buffer full
    }

    // Copy the sample from the provided input pointer to the buffer.
    ringbuf->samples[current_tail] = *sample;

    // Use release ordering when updating tail to ensure the sample write is visible
    // to the consumer before they see the new tail value
    atomic_store_explicit(&ringbuf->tail, next_tail, memory_order_release);
    return true;
}

// Returns 0 on success, 1 on failure (buffer empty).
bool
pf2_ringbuffer_pop(struct pf2_ringbuffer *ringbuf, struct pf2_sample *out) {
    // Head won't be modifed by the producer thread. It is safe to use relaxed ordering.
    const int current_head = atomic_load_explicit(&ringbuf->head, memory_order_relaxed);

    // Check tail to see if buffer is empty. If head == tail, the buffer is empty.
    // Use acquire ordering to synchronize with the tail update in pf2_ringbuffer_push().
    // This ensures we see the latest tail value.
    if (current_head == atomic_load_explicit(&ringbuf->tail, memory_order_acquire)) {
        return false;  // Buffer empty
    }

    // Copy the sample from the buffer to the provided output pointer.
    *out = ringbuf->samples[current_head];

    // Use release ordering when updating head to ensure the sample read is complete
    // before the producer sees the new head value
    atomic_store_explicit(&ringbuf->head, (current_head + 1) % ringbuf->size, memory_order_release);
    return true;
}
