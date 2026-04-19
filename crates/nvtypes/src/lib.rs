use std::{ffi::c_void, os::raw::c_uint};

#[repr(C)]
#[derive(Debug)]
pub enum CudaError {
    Success = 0,
    MemoryAllocation = 2,
}

pub struct CUctx_st {
    _junk: [u8; 0]
}

pub struct CUmod_st {
    _junk: [u8; 0]
}
pub struct CUfunc_st {
    _junk: [u8; 0]
}

pub struct CUstream_st {
    _junk: [u8; 0]
}

pub struct cudaKernel_t{
    _junk: [u8; 0]
}

pub type CUstream = *mut CUstream_st;

pub type CUresult = c_uint;
pub type CUdevice = c_uint;
pub type CUcontext = *mut CUctx_st;
pub type CUctxCreateParams= c_void;
pub type CUmodule = *mut CUmod_st;
pub type CUfunction = *mut CUfunc_st;
pub type cudaKernel = *mut cudaKernel_t;

#[repr(C)]
pub struct uint3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl std::fmt::Debug for uint3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "uint3 {{ x: {}, y: {}, z: {} }}", self.x, self.y, self.z)
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct dim3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}
impl std::fmt::Debug for dim3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dim3 {{ x: {}, y: {}, z: {} }}", self.x, self.y, self.z)
    }
}

#[repr(C)]
pub enum CUmemcpyKind {
    HostToHost = 0,
    HostToDevice = 1,
    DeviceToHost = 2,
    DeviceToDevice = 3,
}