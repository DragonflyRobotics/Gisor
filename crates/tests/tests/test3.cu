#include <stdio.h>
#include <cuda_runtime.h>

// Each thread computes: is (tid * 0.5) <= 2.0?
// tid=0 -> 0.0 <= 2.0 -> 1
// tid=1 -> 0.5 <= 2.0 -> 1
// tid=2 -> 1.0 <= 2.0 -> 1
// tid=3 -> 1.5 <= 2.0 -> 1
// tid=4 -> 2.0 <= 2.0 -> 1
// tid=5 -> 2.5 <= 2.0 -> 0
// tid=6 -> 3.0 <= 2.0 -> 0
// tid=7 -> 3.5 <= 2.0 -> 0
__global__ void test_setp_le_f32(int *out) {
    float val = (float)threadIdx.x * 0.5f;
    float threshold = 2.0f;
    int result = 0;
    if (val <= threshold) result = 1;
    out[threadIdx.x] = result;
}

int main() {
    int h[8], *d;
    cudaMalloc(&d, 32);
    test_setp_le_f32<<<1, 8>>>(d);
    cudaMemcpy(h, d, 32, cudaMemcpyDeviceToHost);
    printf("setp.le.f32: ");
    for (int i = 0; i < 8; i++) printf("%d ", h[i]);
    printf("\n(expect:     1 1 1 1 1 0 0 0)\n");
    cudaFree(d);
}