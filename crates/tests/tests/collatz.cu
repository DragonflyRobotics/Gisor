#include <stdio.h>
#include <cuda_runtime.h>

__global__ void collatz(int *a, int n)
{
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        if (a[i] % 2 == 0) {
            a[i] = a[i] / 2;
        } else {
            a[i] = a[i] * 3 + 1;
        }
    }
}

int main()
{
    const int n = 27;
    const int bytes = n * sizeof(float);

    int *h_collatz = (int*)malloc(bytes);

    // init
    for (int i = 0; i < n; i++) {
        h_collatz[i] = i;
    }

    int *d_collatz;

    cudaMalloc(&d_collatz, bytes);

    cudaMemcpy(d_collatz, h_collatz, bytes, cudaMemcpyHostToDevice);

    int blockSize = 256;
    int gridSize = (n + blockSize - 1) / blockSize;

    collatz<<<gridSize, blockSize>>>(d_collatz, n);

    cudaDeviceSynchronize();

    cudaMemcpy(h_collatz, d_collatz, bytes, cudaMemcpyDeviceToHost);

    // verify
    for (int i = 0; i < 5; i++) {
        printf("%d\n", h_collatz[i]);
    }

    cudaFree(d_collatz);

    free(h_collatz);

    return 0;
}