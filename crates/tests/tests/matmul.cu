#include <stdio.h>
#include <cuda_runtime.h>
#include <stdlib.h>

// #define TILE_SIZE 16

// // Naive matrix multiplication kernel
// __global__ void matmul_naive(float *A, float *B, float *C, int N)
// {
//     int row = blockIdx.y * blockDim.y + threadIdx.y;
//     int col = blockIdx.x * blockDim.x + threadIdx.x;
//
//     if (row < N && col < N) {
//         float sum = 0.0f;
//         for (int k = 0; k < N; k++) {
//             sum += A[row * N + k] * B[k * N + col];
//         }
//         C[row * N + col] = sum;
//     }
// }
//
// // Optimized tiled matrix multiplication using shared memory
// __global__ void matmul_tiled(float *A, float *B, float *C, int N)
// {
//     __shared__ float As[TILE_SIZE][TILE_SIZE];
//     __shared__ float Bs[TILE_SIZE][TILE_SIZE];
//
//     int row = blockIdx.y * TILE_SIZE + threadIdx.y;
//     int col = blockIdx.x * TILE_SIZE + threadIdx.x;
//
//     float sum = 0.0f;
//
//     // Loop over tiles
//     for (int t = 0; t < (N + TILE_SIZE - 1) / TILE_SIZE; t++) {
//         // Load tile into shared memory
//         if (row < N && t * TILE_SIZE + threadIdx.x < N) {
//             As[threadIdx.y][threadIdx.x] = A[row * N + t * TILE_SIZE + threadIdx.x];
//         } else {
//             As[threadIdx.y][threadIdx.x] = 0.0f;
//         }
//
//         if (col < N && t * TILE_SIZE + threadIdx.y < N) {
//             Bs[threadIdx.y][threadIdx.x] = B[(t * TILE_SIZE + threadIdx.y) * N + col];
//         } else {
//             Bs[threadIdx.y][threadIdx.x] = 0.0f;
//         }
//
//         __syncthreads();
//
//         // Compute partial dot product
//         for (int k = 0; k < TILE_SIZE; k++) {
//             sum += As[threadIdx.y][k] * Bs[k][threadIdx.x];
//         }
//
//         __syncthreads();
//     }
//
//     if (row < N && col < N) {
//         C[row * N + col] = sum;
//     }
// }
//
// void init_matrix(float *mat, int N, int seed)
// {
//     srand(seed);
//     for (int i = 0; i < N * N; i++) {
//         mat[i] = (float)(rand() % 10);
//     }
// }
//
// void print_matrix(float *mat, int N, const char *name)
// {
//     printf("%s:\n", name);
//     int display_size = (N > 8) ? 8 : N;
//     for (int i = 0; i < display_size; i++) {
//         for (int j = 0; j < display_size; j++) {
//             printf("%6.1f ", mat[i * N + j]);
//         }
//         if (N > 8) printf("...");
//         printf("\n");
//     }
//     if (N > 8) printf("...\n");
//     printf("\n");
// }

int main()
{
    return 0;
    /*
    const int N = 64;  // Matrix size NxN
    const int bytes = N * N * sizeof(float);
    
    float *h_A = (float*)malloc(bytes);
    float *h_B = (float*)malloc(bytes);
    float *h_C = (float*)malloc(bytes);
    
    // Initialize matrices
    init_matrix(h_A, N, 123);
    init_matrix(h_B, N, 456);
    
    printf("Matrix multiplication: C = A × B\n");
    printf("Matrix size: %dx%d\n\n", N, N);
    
    print_matrix(h_A, N, "Matrix A (top-left)");
    print_matrix(h_B, N, "Matrix B (top-left)");
    
    float *d_A, *d_B, *d_C;
    cudaMalloc(&d_A, bytes);
    cudaMalloc(&d_B, bytes);
    cudaMalloc(&d_C, bytes);
    
    cudaMemcpy(d_A, h_A, bytes, cudaMemcpyHostToDevice);
    cudaMemcpy(d_B, h_B, bytes, cudaMemcpyHostToDevice);
    
    dim3 blockSize(TILE_SIZE, TILE_SIZE);
    dim3 gridSize((N + TILE_SIZE - 1) / TILE_SIZE, 
                  (N + TILE_SIZE - 1) / TILE_SIZE);
    
    printf("Launching kernel with:\n");
    printf("  Grid: %dx%d blocks\n", gridSize.x, gridSize.y);
    printf("  Block: %dx%d threads\n", blockSize.x, blockSize.y);
    printf("  Total threads: %d\n\n", 
           gridSize.x * gridSize.y * blockSize.x * blockSize.y);
    
    // Run tiled version
    matmul_tiled<<<gridSize, blockSize>>>(d_A, d_B, d_C, N);
    cudaDeviceSynchronize();
    
    cudaMemcpy(h_C, d_C, bytes, cudaMemcpyDeviceToHost);
    
    print_matrix(h_C, N, "Result C = A × B (top-left)");
    
    // Verify correctness with a few elements
    printf("Verification (checking element C[0][0]):\n");
    float expected = 0.0f;
    for (int k = 0; k < N; k++) {
        expected += h_A[0 * N + k] * h_B[k * N + 0];
    }
    printf("  Expected: %.1f\n", expected);
    printf("  Got:      %.1f\n", h_C[0]);
    printf("  %s\n\n", (fabs(expected - h_C[0]) < 0.01f) ? "PASS" : "FAIL");
    
    cudaFree(d_A);
    cudaFree(d_B);
    cudaFree(d_C);
    free(h_A);
    free(h_B);
    free(h_C);
    
    return 0;
    */
}
