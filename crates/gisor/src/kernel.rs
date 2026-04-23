use std::{
    env, ffi::{CStr, c_char}, os::raw::c_void
};

use cpp_demangle::Symbol;
use gpu::basegpu::{BasicGPU, GPU0};
use memory::MemoryAddress;
use nvtypes::{CUresult, CUstream, dim3, uint3};
use ptx_parser::{parse, parse_c_signature};


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
        // for i in 0..gpu.num_args.unwrap()-1 {
        //     println!("Offset: {}", i as isize);
        //     let arg = *(*(args.offset(i as isize)) as *const usize);
        //     args_vec.push(arg);
        // }
        let d_a = *(*(args.offset(0)) as *const u64);
        let d_b = *(*(args.offset(1)) as *const u64);
        let d_c = *(*(args.offset(2)) as *const u64);
        let n = *(*(args.offset(3)) as *const i32);
        args_vec.push(d_a as usize);
        args_vec.push(d_b as usize);
        args_vec.push(d_c as usize);
        args_vec.push(n as usize);
        // panic!("");
        
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
    let ptx = env::var("GISOR_PTX").unwrap_or_default();
    println!("PTX: {}", ptx);
    unsafe {
        println!("Host function: {:?}", CStr::from_ptr(deviceFun));
        let parsed = parse(ptx.as_str());
        match parsed {
            Ok(parsed) => {
                gpu.kernels.insert(CStr::from_ptr(deviceFun).to_string_lossy().to_string(), parsed.instructions);
            }
            Err(err) => {
                panic!("Parse error: {:?}", err);
            }
        }
        gpu.select_kernel(CStr::from_ptr(deviceFun).to_string_lossy().to_string());
        let sym = Symbol::new(CStr::from_ptr(deviceFun).to_str().unwrap()).unwrap();
        println!(
            "Device demangled function: {:?}",
            sym.demangle().unwrap().to_string()
        );
        let csig = parse_c_signature(sym.demangle().unwrap().as_str());
        match csig {
            Ok(csig) => {
                gpu.num_args = Some(csig.params.len());
            }
            Err(err) => {
                panic!("Parse error: {:?}", err);
            }
        }
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
