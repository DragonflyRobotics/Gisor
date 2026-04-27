// Made with the help of AI to understand CUDA syntax and the Mandelbrot set
#include <stdio.h>
#include <cuda_runtime.h>

#define WIDTH 640
#define HEIGHT 480
#define MAX_ITER 256

__global__ void mandelbrot_kernel(int *output, int width, int height, 
                                   float x_min, float x_max, 
                                   float y_min, float y_max)
{
    int px = blockIdx.x * blockDim.x + threadIdx.x;
    int py = blockIdx.y * blockDim.y + threadIdx.y;
    
    if (px >= width || py >= height) return;
    
    // Map pixel to complex plane
    float x0 = x_min + (x_max - x_min) * px / (float)width;
    float y0 = y_min + (y_max - y_min) * py / (float)height;
    
    float x = 0.0f;
    float y = 0.0f;
    int iteration = 0;
    
    // Iterate: z = z^2 + c
    while (x*x + y*y <= 4.0f && iteration < MAX_ITER) {
        float xtemp = x*x - y*y + x0;
        y = 2.0f*x*y + y0;
        x = xtemp;
        iteration++;
    }
    
    output[py * width + px] = iteration;
}

void save_pgm(const char *filename, int *data, int width, int height)
{
    FILE *fp = fopen(filename, "w");
    fprintf(fp, "P2\n%d %d\n255\n", width, height);
    
    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            int iter = data[y * width + x];
            // Map iteration count to grayscale
            int color = (iter * 255) / MAX_ITER;
            fprintf(fp, "%d ", color);
        }
        fprintf(fp, "\n");
    }
    
    fclose(fp);
}

int main()
{
    const int pixels = WIDTH * HEIGHT;
    const int bytes = pixels * sizeof(int);
    
    int *h_output = (int*)malloc(bytes);
    int *d_output;
    
    cudaMalloc(&d_output, bytes);
    
    // Classic Mandelbrot view
    float x_min = -2.5f;
    float x_max = 1.0f;
    float y_min = -1.0f;
    float y_max = 1.0f;
    
    dim3 blockSize(16, 16);
    dim3 gridSize((WIDTH + 15) / 16, (HEIGHT + 15) / 16);
    
    printf("Generating Mandelbrot set (%dx%d)...\n", WIDTH, HEIGHT);
    printf("Launching %d blocks with %d threads each\n", 
           gridSize.x * gridSize.y, blockSize.x * blockSize.y);
    
    mandelbrot_kernel<<<gridSize, blockSize>>>(d_output, WIDTH, HEIGHT,
                                                 x_min, x_max, y_min, y_max);
    
    cudaDeviceSynchronize();
    
    cudaMemcpy(h_output, d_output, bytes, cudaMemcpyDeviceToHost);
    
    save_pgm("mandelbrot.pgm", h_output, WIDTH, HEIGHT);
    printf("Saved mandelbrot.pgm\n");
    
    cudaFree(d_output);
    free(h_output);
    
    return 0;
}
