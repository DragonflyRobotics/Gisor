#include <stdio.h>
#include <cuda_runtime.h>

int main()
{
    float *d_ptr;

    printf("Setting device 0\n");
    cudaSetDevice(0);
    cudaMalloc(&d_ptr, 1024);
    cudaFree(d_ptr);

    printf("Setting device 1\n");
    cudaSetDevice(1);
    cudaMalloc(&d_ptr, 1024);
    cudaFree(d_ptr);

    printf("Done\n");
    return 0;
}