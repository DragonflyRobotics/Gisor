use std::os::raw::{c_int, c_void};

use nvtypes::{CudaError, cudaStream_t, dim3};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaMalloc(dev_ptr: *mut *mut c_void, size: usize) -> c_int {
    println!("Intercepted cudaMalloc, size = {}", size);

    CudaError::MemoryAllocation as c_int
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaFree(dev_ptr: *mut c_void) -> c_int {
    println!("Intercepted cudaFree, dev_ptr = {:?}", dev_ptr);
    CudaError::Success as c_int
}

// https://docs.nvidia.com/cuda/cuda-runtime-api/group__CUDART__EXECUTION.html#group__CUDART__EXECUTION_1g5064cdf5d8e6741ace56fd8be951783c
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaLaunchKernel(
    func: *const c_void,
    gridDim: dim3,
    blockDim: dim3,
    args: *mut *mut c_void,
    sharedMem: usize,
    stream: cudaStream_t,
) -> CudaError {
    println!(
        "Intercepted cudaLaunchKernel! grid=({},{},{}), block=({},{},{})",
        gridDim.x, gridDim.y, gridDim.z, blockDim.x, blockDim.y, blockDim.z
    );
    CudaError::Success
}
