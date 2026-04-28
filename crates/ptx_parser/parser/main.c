#include "output.h"
#include <stdio.h>
#include <stdlib.h>

/* Read an entire file into a heap-allocated NUL-terminated string.
 * Caller must free the result. */
static char* read_file(const char* path) {
    FILE* f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "could not open %s\n", path);
        exit(1);
    }

    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);

    char* buf = (char*)malloc(size + 1);
    if (!buf) {
        fprintf(stderr, "malloc failed\n");
        exit(1);
    }
    fread(buf, 1, size, f);
    buf[size] = '\0';
    fclose(f);
    return buf;
}

int main(int argc, char** argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: %s <file.ptx>\n", argv[0]);
        return 1;
    }

    char* source = read_file(argv[1]);
    ParsedKernel k = ptx_parse(source);
    free(source);  /* parser made its own copies of any strings it needed */

    printf("kernel name: %s\n", k.name);
    printf("params (%zu):\n", k.param_count);
    for (size_t i = 0; i < k.param_count; i++) {
        printf("  [%zu] %s (type=%d)\n", i,
               k.params[i].name, k.params[i].ptx_type);
    }
    printf("instructions (%zu):\n", k.instruction_count);
    for (size_t i = 0; i < k.instruction_count; i++) {
        InstInfo* x = &k.instructions[i];
        printf("  [%zu] opcode=%d args=[", i, x->opcode);
        for (size_t j = 0; j < x->arg_count; j++) {
            printf("%s%zu", j ? ", " : "", x->args[j]);
        }
        printf("]\n");
    }

    ptx_parse_destroy(&k);
    return 0;
}