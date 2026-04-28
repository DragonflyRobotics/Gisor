#ifndef ARENA_H
#define ARENA_H

#include <stddef.h>

typedef struct Arena {
    char* buffer;
    size_t cap;
    size_t used;
} Arena;

Arena arena_create(size_t cap);
void* arena_alloc(Arena* a, size_t size);
char* arena_copy(Arena* a, const char* src, size_t len);
void arena_destroy(Arena* a);

#endif