#ifndef PF2_INDEXED_MAPS_H
#define PF2_INDEXED_MAPS_H

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "serializer.h"

struct pf2_function_key {
    enum function_implementation implementation;
    int32_t start_lineno;
    size_t start_address;
    uint64_t name_hash;
    uint64_t filename_hash;
    const char *name;
    const char *filename;
};

struct pf2_location_key {
    size_t function_index;
    int32_t lineno;
    size_t address;
};

struct pf2_stack_key {
    uintptr_t ruby_thread_id;
    size_t *stack;
    size_t stack_count;
    uint64_t hash;
};

static inline uint64_t
pf2_hash_mix(uint64_t value)
{
    value ^= value >> 33;
    value *= 0xff51afd7ed558ccdULL;
    value ^= value >> 33;
    value *= 0xc4ceb9fe1a85ec53ULL;
    value ^= value >> 33;
    return value;
}

static inline uint64_t
pf2_hash_combine(uint64_t seed, uint64_t value)
{
    return pf2_hash_mix(seed ^ value);
}

static inline uint64_t
pf2_hash_cstr(const char *str)
{
    if (str == NULL) {
        return 0;
    }
    uint64_t hash = 1469598103934665603ULL;
    const unsigned char *ptr = (const unsigned char *)str;
    while (*ptr) {
        hash ^= (uint64_t)*ptr++;
        hash *= 1099511628211ULL;
    }
    return hash;
}

static inline bool
pf2_str_equal(const char *left, const char *right)
{
    if (left == NULL || right == NULL) {
        return left == right;
    }
    return strcmp(left, right) == 0;
}

static inline struct pf2_function_key
pf2_function_key_build(const struct pf2_ser_function *function)
{
    struct pf2_function_key key;
    key.implementation = function->implementation;
    key.start_lineno = function->start_lineno;
    key.start_address = function->start_address;
    key.name = function->name;
    key.filename = function->filename;
    key.name_hash = pf2_hash_cstr(function->name);
    key.filename_hash = pf2_hash_cstr(function->filename);
    return key;
}

static inline uint64_t
pf2_function_key_hash(struct pf2_function_key key)
{
    uint64_t hash = pf2_hash_mix((uint64_t)key.implementation);
    hash = pf2_hash_combine(hash, (uint64_t)(uint32_t)key.start_lineno);
    hash = pf2_hash_combine(hash, (uint64_t)key.start_address);
    hash = pf2_hash_combine(hash, key.name_hash);
    hash = pf2_hash_combine(hash, key.filename_hash);
    return hash;
}

static inline bool
pf2_function_key_equal(struct pf2_function_key left, struct pf2_function_key right)
{
    if (left.implementation != right.implementation) {
        return false;
    }
    if (left.start_lineno != right.start_lineno || left.start_address != right.start_address) {
        return false;
    }
    if (left.name_hash != right.name_hash || left.filename_hash != right.filename_hash) {
        return false;
    }
    return pf2_str_equal(left.name, right.name) && pf2_str_equal(left.filename, right.filename);
}

static inline uint64_t
pf2_location_key_hash(struct pf2_location_key key)
{
    uint64_t hash = pf2_hash_mix((uint64_t)key.function_index);
    hash = pf2_hash_combine(hash, (uint64_t)(uint32_t)key.lineno);
    hash = pf2_hash_combine(hash, (uint64_t)key.address);
    return hash;
}

static inline bool
pf2_location_key_equal(struct pf2_location_key left, struct pf2_location_key right)
{
    return left.function_index == right.function_index
        && left.lineno == right.lineno
        && left.address == right.address;
}

static inline uint64_t
pf2_stack_hash(const size_t *stack, size_t stack_count, uintptr_t thread_id)
{
    uint64_t hash = pf2_hash_mix((uint64_t)thread_id);
    hash = pf2_hash_combine(hash, (uint64_t)stack_count);
    for (size_t i = 0; i < stack_count; i++) {
        hash = pf2_hash_combine(hash, (uint64_t)stack[i]);
    }
    return hash;
}

static inline uint64_t
pf2_stack_key_hash(struct pf2_stack_key key)
{
    return key.hash;
}

static inline bool
pf2_stack_key_equal(struct pf2_stack_key left, struct pf2_stack_key right)
{
    if (left.ruby_thread_id != right.ruby_thread_id) {
        return false;
    }
    if (left.stack_count != right.stack_count) {
        return false;
    }
    if (left.stack_count > 0 && memcmp(left.stack, right.stack, left.stack_count * sizeof(size_t)) != 0) {
        return false;
    }
    return true;
}

#define NAME pf2_function_map
#define KEY_TY struct pf2_function_key
#define VAL_TY size_t
#define HASH_FN pf2_function_key_hash
#define CMPR_FN pf2_function_key_equal
#include "verstable.h"
#undef NAME
#undef KEY_TY
#undef VAL_TY
#undef HASH_FN
#undef CMPR_FN

#define NAME pf2_location_map
#define KEY_TY struct pf2_location_key
#define VAL_TY size_t
#define HASH_FN pf2_location_key_hash
#define CMPR_FN pf2_location_key_equal
#include "verstable.h"
#undef NAME
#undef KEY_TY
#undef VAL_TY
#undef HASH_FN
#undef CMPR_FN

#define NAME pf2_stack_map
#define KEY_TY struct pf2_stack_key
#define VAL_TY size_t
#define HASH_FN pf2_stack_key_hash
#define CMPR_FN pf2_stack_key_equal
#include "verstable.h"
#undef NAME
#undef KEY_TY
#undef VAL_TY
#undef HASH_FN
#undef CMPR_FN

#endif // PF2_INDEXED_MAPS_H
