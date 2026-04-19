use std::{collections::HashMap, sync::Mutex};

use memory::{Memory, MemoryAddress, MemoryElement};
use nvtypes::dim3;
use once_cell::sync::Lazy;
use utils::triple_zip;

use crate::{
    sm::SM,
    warp::{self, Warp, WarpState},
};

pub trait BasicGPU {
    fn malloc(&mut self, size: usize) -> (MemoryAddress, usize);
    fn free(&mut self, addr: MemoryAddress);
    fn load_ptx(&mut self, ptx: String);
    fn select_kernel(&mut self, kernel: String);
    fn set_launch_params(&mut self, grid: dim3, threads: dim3);
}

pub struct LaunchParams {
    pub grid: (u32, u32, u32),
    pub block: (u32, u32, u32),
}

pub struct GPU {
    pub memory: Memory,
    pub sms: Vec<SM>,
    kernel_symbol: Option<String>,
    launch_params: Option<LaunchParams>,
    raw_ptx: Option<String>,
}

impl BasicGPU for GPU {
    fn malloc(&mut self, size: usize) -> (MemoryAddress, usize) {
        let addr = MemoryAddress::new();
        self.memory
            .data
            .insert(addr, MemoryElement::new_empty(size));
        (addr, size)
    }

    fn free(&mut self, addr: MemoryAddress) {
        self.memory.data.remove(&addr);
    }

    fn load_ptx(&mut self, ptx: String) {
        self.raw_ptx = Some(ptx);
    }

    fn select_kernel(&mut self, kernel: String) {
        self.kernel_symbol = Some(kernel);
    }

    fn set_launch_params(&mut self, grid: dim3, threads: dim3) {
        self.launch_params = Some(LaunchParams {
            grid: (grid.x, grid.y, grid.z),
            block: (threads.x, threads.y, threads.z),
        });

        let gridx = 0..grid.x;
        let gridy = 0..grid.y;
        let gridz = 0..grid.z;

        let grid_zip = triple_zip(gridx, gridy, gridz); // len grid is num blocks

        let threadsx = 0..threads.x;
        let threadsy = 0..threads.y;
        let threadsz = 0..threads.z;

        let threads_zip = triple_zip(threadsx, threadsy, threadsz);
        let warps_needed: usize =
            ((threads.x as f32 * threads.y as f32 * threads.z as f32) / 32.0).ceil() as usize;

        for block in grid_zip {
            // find suitable SM
            let mut selected_warps: Option<Vec<&mut Warp>> = None;
            for sm in self.sms.iter_mut() {
                let candidate = sm.get_free_warps(warps_needed);
                if candidate.is_some() {
                    selected_warps = candidate;
                    break;
                }
            }
            if let Some(mut warps) = selected_warps {
                for (warp_threads, warp) in threads_zip.chunks(32).zip(warps.iter_mut()) {
                    warp.set_state(WarpState::Active);
                    warp.set_coords(
                        block,
                        warp_threads
                            .iter()
                            .map(|t| (t.0, t.1, t.2))
                            .collect::<Vec<_>>(),
                    );
                }
            } else {
                println!("No free warps available for block {:?}", block);
            }
        }
        println!("GPU: {}\n", self);
    }
}

impl std::fmt::Display for GPU {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, sm) in self.sms.iter().enumerate() {
            writeln!(f, "SM {}:\n{}", i, sm)?;
        }
        Ok(())
    }
}

pub static GPU0: Lazy<Mutex<GPU>> = Lazy::new(|| {
    Mutex::new(GPU {
        memory: Memory {
            data: HashMap::new(),
        },
        sms: vec![
            SM::new(10),
            SM::new(10),
            SM::new(10),
            SM::new(10),
            SM::new(10),
        ],
        kernel_symbol: None,
        launch_params: None,
        raw_ptx: None,
    })
});
