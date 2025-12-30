#include <stdbool.h>
#include <stdlib.h>

#include "ringbuffer.h"

static bool pf2_sample_init(struct pf2_sample *sample, int max_ruby_depth, int max_native_depth);

struct pf2_ringbuffer *
pf2_ringbuffer_new(int size, int max_ruby_depth, int max_native_depth) {
    if (size <= 0) { return NULL; }

    struct pf2_ringbuffer *ringbuf = malloc(sizeof(struct pf2_ringbuffer));
    if (!ringbuf) { goto err; }
    ringbuf->size = size + 1; // One extra slot is required to distinguish full from empty
    ringbuf->head = 0;
    ringbuf->tail = 0;

    // Allocate space for samples to be stored
    ringbuf->samples = malloc(ringbuf->size * sizeof(struct pf2_sample));
    if (!ringbuf->samples) { goto err_free_ringbuf; }
    for (int i = 0; i < ringbuf->size; i++) {
        if (!pf2_sample_init(&ringbuf->samples[i], max_ruby_depth, max_native_depth)) {
            for (int j = 0; j < i; j++) {
                pf2_sample_free(&ringbuf->samples[j]);
            }
            free(ringbuf->samples);
            goto err_free_ringbuf;
        }
    }
    return ringbuf;

err_free_ringbuf:
    free(ringbuf);
err:
    return NULL;
}

static bool
pf2_sample_init(struct pf2_sample *sample, int max_ruby_depth, int max_native_depth)
{
    sample->cmes = malloc(sizeof(VALUE) * max_ruby_depth);
    sample->linenos = malloc(sizeof(int) * max_ruby_depth);
    sample->native_stack = malloc(sizeof(uintptr_t) * max_native_depth);
    if (sample->cmes == NULL || sample->linenos == NULL || sample->native_stack == NULL) {
        free(sample->cmes);
        free(sample->linenos);
        free(sample->native_stack);
        sample->cmes = NULL;
        sample->linenos = NULL;
        sample->native_stack = NULL;
        return false;
    }

    sample->depth = 0;
    sample->native_stack_depth = 0;
    sample->consumed_time_ns = 0;
    sample->timestamp_ns = 0;

    return true;
}

void
pf2_ringbuffer_free(struct pf2_ringbuffer *ringbuf) {
    for (int i = 0; i < ringbuf->size; i++) {
        pf2_sample_free(&ringbuf->samples[i]);
    }
    free(ringbuf->samples);
    free(ringbuf);
}

// Returns 0 on success, 1 on failure (buffer full).
bool
pf2_ringbuffer_reserve(struct pf2_ringbuffer *ringbuf, struct pf2_sample **sample, int *next_tail) {
    // Tail is only modified by the producer thread (us), so relaxed ordering is sufficient
    const int current_tail = atomic_load_explicit(&ringbuf->tail, memory_order_relaxed);
    const int next_tail_local = (current_tail + 1) % ringbuf->size;

    // Check head to see if buffer is full. If next_tail == head, the buffer is full.
    // Use acquire ordering to synchronize with the head update in pf2_ringbuffer_pop().
    // This ensures we see the latest head value.
    if (next_tail_local == atomic_load_explicit(&ringbuf->head, memory_order_acquire)) {
        return false;  // Buffer full
    }

    // Expose the slot pointer without publishing it yet. The producer must fill this slot
    // fully before calling commit; until then, the consumer will not see it because tail
    // remains unchanged.
    *sample = &ringbuf->samples[current_tail];
    // Hand back the computed next tail so the producer can publish atomically after capture.
    *next_tail = next_tail_local;

    return true;
}

void
pf2_ringbuffer_commit(struct pf2_ringbuffer *ringbuf, int next_tail) {
    // Use release ordering when updating tail to ensure the sample write is visible
    // to the consumer before they see the new tail value
    atomic_store_explicit(&ringbuf->tail, next_tail, memory_order_release);
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
