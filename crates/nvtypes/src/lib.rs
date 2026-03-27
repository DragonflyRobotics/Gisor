use std::os::raw::{c_uint, c_void};

#[repr(C)]
#[derive(Debug)]
pub enum CudaError {
    Success = 0,
    MemoryAllocation = 2,
}

// https://docs.nvidia.com/cuda/cuda-runtime-api/group__CUDART__EXECUTION.html#group__CUDART__EXECUTION_1g5064cdf5d8e6741ace56fd8be951783c
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct dim3 {
    pub x: c_uint,
    pub y: c_uint,
    pub z: c_uint,
}

pub type cudaStream_t = *mut c_void;