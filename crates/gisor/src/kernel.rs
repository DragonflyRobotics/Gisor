use std::{
    ffi::{CStr, c_char},
    os::raw::c_void,
};

use cpp_demangle::Symbol;
use gpu::basegpu::{BasicGPU, GPU0};
use memory::MemoryAddress;
use nvtypes::{CUresult, CUstream, dim3, uint3};


#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaLaunchKernel(
    kernel: *mut c_void,
    gridDim: dim3,
    blockDim: dim3,
    args: *mut *mut c_void,
    sharedMemBytes: usize,
    stream: CUstream,
) -> CUresult {
    println!("__cudaLaunchKernel called");
    println!(
        "gridDim: {:?}, blockDim: {:?}, args: {:?}, sharedMemBytes: {}",
        gridDim, blockDim, args, sharedMemBytes
    );
    let mut args_vec: Vec<usize> = vec![];
    let mut gpu = GPU0.lock().unwrap();
    unsafe {
        // for i in 0..gpu.num_args.unwrap() {
        //     println!("Offset: {}", i as isize);
        //     let arg = *(*(args.offset(i as isize)) as *const usize);
        //     args_vec.push(arg);
        // }
        let d_a = *(*(args.offset(0)) as *const u64);
        let d_b = *(*(args.offset(1)) as *const u64);
        let d_c = *(*(args.offset(2)) as *const u64);
        let n = *(*(args.offset(3)) as *const i32);
        panic!("");
        
        // println!("+++ Kernel arguments:");
        // println!("+++   d_a = 0x{:x}", d_a);
        // println!("+++   d_b = 0x{:x}", d_b);
        // println!("+++   d_c = 0x{:x}", d_c);
        // println!("+++   n = {}", n);
        
    }
    gpu.execute(args_vec);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaGetKernel() -> CUresult {
    println!("__cudaGetKernel called");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaPopCallConfiguration() -> CUresult {
    println!("__cudaPopCallConfiguration called");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaRegisterFunction(
    fatCubinHandle: *mut *mut c_void,
    hostFun: *const c_char,
    deviceFun: *mut c_char,
    thread_limit: i32,
    tid: *mut uint3,
    bid: *mut uint3,
    bDim: *mut dim3,
    gDim: *mut dim3,
    wSize: *mut i32,
) -> CUresult {
    println!(
        "__cudaRegisterFunction called with thread_limit: {}",
        thread_limit
    );
    let mut gpu = GPU0.lock().unwrap();
    unsafe {
        println!("Host function: {:?}", CStr::from_ptr(deviceFun));
        let sym = Symbol::new(CStr::from_ptr(deviceFun).to_str().unwrap()).unwrap();
        println!(
            "Device demangled function: {:?}",
            sym.demangle().unwrap().to_string()
        );
        gpu.num_args = Some(4);
        // println!("TID: {:?}", *tid);
        // println!("BID: {:?}", *bid);
        // println!("Block Dim: {:?}", *bDim);
        // println!("Grid Dim: {:?}", *gDim);
        // println!("WSize: {:?}", *wSize);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaRegisterFatBinary() -> CUresult {
    println!("__cudaRegisterFatBinary called");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaUnregisterFatBinary() -> CUresult {
    println!("__cudaUnregisterFatBinary called");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaRegisterFatBinaryEnd() -> CUresult {
    println!("__cudaRegisterFatBinaryEnd called");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cudaPushCallConfiguration(
    gridDim: dim3,
    blockDim: dim3,
    sharedMemBytes: usize,
    stream: CUstream,
) -> CUresult {
    println!("__cudaPushCallConfiguration called");
    println!(
        "gridDim: {:?}, blockDim: {:?}, sharedMemBytes: {}",
        gridDim, blockDim, sharedMemBytes
    );
    let mut gpu = GPU0.lock().unwrap();
    gpu.set_launch_params(gridDim, blockDim);


    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cudaLaunchKernel() -> CUresult {
    println!("cudaLaunchKernel called");
    0
}
