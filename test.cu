#include <stdio.h>
#include <stdint.h> // for uintptr_t
#include <cuda_runtime.h>

int main() {
    size_t size = 1024;
    void* ptr = NULL;
    cudaError_t err = cudaMalloc(&ptr, size);

    if (!ptr) {
        printf("Got null pointer!\n");
    } else {
        printf("Allocated memory at %p\n", ptr);
    }

    return 0;
}