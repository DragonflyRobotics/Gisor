use std::os::raw::{c_void, c_int, c_ulong};

#[repr(C)]
#[derive(Debug)]
pub enum CudaError {
    Success = 0,
    MemoryAllocation = 2,
    // Add others as needed
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaMalloc(dev_ptr: *mut *mut c_void, size: usize) -> c_int {
    println!("Intercepted cudaMalloc, size = {}", size);

    CudaError::MemoryAllocation as c_int
}