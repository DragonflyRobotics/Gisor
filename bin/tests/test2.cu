#include <stdio.h>
#include <cuda_runtime.h>

__global__ void set_value(int *d, int value) {
    int idx = threadIdx.x + blockIdx.x * blockDim.x;
    d[idx] = value;
}

int main() {
    size_t N = 32;  // small array for demo
    int *d_ptr = NULL;

    // Allocate memory on GPU
    cudaError_t err = cudaMalloc((void**)&d_ptr, N * sizeof(int));
    // if (err != cudaSuccess) {
    //     printf("cudaMalloc failed: %s\n", cudaGetErrorString(err));
    //     return 1;
    // }
    // printf("Allocated %zu bytes at %p on GPU\n", N * sizeof(int), d_ptr);

    // Launch kernel to set all elements to 42
    set_value<<<1, N>>>(d_ptr, 42);
    // cudaDeviceSynchronize();  // wait for kernel to finish

    // // Copy data back to host
    // int h_data[N];
    // cudaMemcpy(h_data, d_ptr, N * sizeof(int), cudaMemcpyDeviceToHost);

    // // Print results
    // for (int i = 0; i < N; i++) {
    //     printf("h_data[%d] = %d\n", i, h_data[i]);
    // }

    // // Free GPU memory
    // cudaFree(d_ptr);
    return 0;
}