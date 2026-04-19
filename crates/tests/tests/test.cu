#include <cuda_runtime.h>
#include <stdio.h>

int main() {
  int hostarr[10] = {};
  for (int i = 0; i < 10; i++) {
    hostarr[i] = i;
  }
  
  for (int i = 0; i < 10; i++) {
    printf("Host %d\n", hostarr[i]);
  }
  printf("\n");
  
  int *devarr = nullptr;
  cudaMalloc(&devarr, 10 * sizeof(int));
  cudaMemcpy(devarr, hostarr, 10 * sizeof(int), cudaMemcpyHostToDevice);
  
  int finalarr[10] = {};
  cudaMemcpy(finalarr, devarr, 10 * sizeof(int), cudaMemcpyDeviceToHost);
  for (int i = 0; i < 10; i++) {
    printf("Final %d\n", finalarr[i]);
  }
  printf("\n");
  
  cudaFree(devarr);
  
  return 0;
}
