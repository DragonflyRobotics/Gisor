use std::{ffi::c_void, os::raw::c_char};

use gpu::basegpu::{BasicGPU, GPU0};
use memory::MemoryAddress;
use nvtypes::{CUmemcpyKind, CUresult};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaSetDevice(device: i32) -> CUresult {
    println!("cudaSetDevice called, device: {}", device);
    0
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaGetErrorString() -> CUresult {
    println!("cudaGetErrorString called");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaMemcpy(
    dst: *mut c_void,
    src: *const c_void,
    count: usize,
    kind: CUmemcpyKind,
) -> CUresult {
    println!("cudaMemcpy called");
    match kind {
        CUmemcpyKind::HostToHost => {
            println!("cudaMemcpy: kind = HostToHost");
        }
        CUmemcpyKind::HostToDevice => {
            println!("cudaMemcpy: kind = HostToDevice");
            let mut gpu = GPU0.lock().unwrap();
            let gpu_loc = MemoryAddress::from_address(dst as usize as u64);
            println!("gpu_loc: {:x}", gpu_loc.address);
            for i in 0..count {
                let byte = unsafe { *(src.add(i) as *const c_char) };
                let temp_loc = gpu_loc + i;
                if let Some(val) = gpu.memory.data.get_mut(&temp_loc) {
                    val.value = byte as u8;
                }
            }
        }
        CUmemcpyKind::DeviceToHost => {
            println!("cudaMemcpy: kind = DeviceToHost");
            let mut gpu = GPU0.lock().unwrap();
            let gpu_loc = MemoryAddress::from_address(src as usize as u64);
            
            for i in 0..count {
                let temp_loc = gpu_loc + i;
                if let Some(val) = gpu.memory.data.get_mut(&temp_loc) {
                    unsafe {
                        *(dst.add(i) as *mut u8) = val.value;
                    }
                }
            }
        }
        CUmemcpyKind::DeviceToDevice => {
            println!("cudaMemcpy: kind = DeviceToDevice");
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaFree(src: *mut c_void) -> CUresult {
    println!("cudaFree called");
    let mut gpu = GPU0.lock().unwrap();
    let gpu_loc = MemoryAddress::from_address(src as usize as u64);
    gpu.free(gpu_loc);
    println!("Num elements left: {}", gpu.memory.data.len());
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaMalloc(devPtr: *mut *mut c_void, size: usize) -> CUresult {
    println!("cudaMalloc called");
    let mut gpu = GPU0.lock().unwrap();
    let (addr, _) = gpu.malloc(size);
    unsafe {
        *devPtr = addr.address as usize as *mut c_void;
        println!("cudaMalloc: addr = {:x}", addr.address)
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaDeviceSynchronize() -> CUresult {
    println!("cudaDeviceSynchronize called");
    0
}
