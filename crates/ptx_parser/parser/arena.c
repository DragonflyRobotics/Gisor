#include "arena.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static size_t arena_align(size_t n) {
    return (n+7) & ~(size_t)7;
}

Arena arena_create(size_t cap) {
    Arena a;
    a.buffer = (char*) malloc(cap);
    a.cap = cap;
    a.used = 0;
    return a;
}


void* arena_alloc(Arena* a, size_t size) {
    size_t start = arena_align(a->used);
    size_t end = start + size;
    if (end > a->cap) {
        fprintf(stderr, "Not enough capacity for arena alloc");
        abort();
    }

    void* ptr = a->buffer + start;
    a->used = end;
    return ptr;
}

void arena_destroy(Arena* a) {
    free(a->buffer);
    a->buffer = NULL;
    a->used = 0;
    a->cap = 0;
}


char* arena_copy(Arena* a, const char* src, size_t len) {
    char* dst = (char*)arena_alloc(a, len+ 1);
    memcpy(dst, src, len);
    dst[len] = '\0';
    return dst;
}


