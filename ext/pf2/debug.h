#ifndef PF2_DEBUG_H
#define PF2_DEBUG_H

#include <stdio.h>

#ifdef PF2_DEBUG
    #define PF2_DEBUG_LOG(format, ...) printf(format, ##__VA_ARGS__)
#else
    #define PF2_DEBUG_LOG(format, ...) ((void)0)
#endif

#endif // PF2_DEBUG_H
