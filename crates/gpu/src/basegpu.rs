use std::{collections::HashMap, sync::Mutex};

use crate::inst_info::make_inst;
use crate::inst_type::InstType;
use csv::Writer;
use indicatif::{ProgressBar, ProgressStyle};
use memory::{Memory, MemoryAddress, MemoryElement};
use nvtypes::dim3;
use once_cell::sync::Lazy;
use serde::Serialize;
use utils::triple_zip;

use crate::{
    inst_info::inst_info,
    sm::SM,
    warp::{Warp, WarpState},
};

#[derive(Serialize)]
struct ThreadRecord {
    sm_id: usize,
    warp_id: usize,
    thread_id: usize,
    grid_x: u32,
    grid_y: u32,
    grid_z: u32,
    thread_x: u32,
    thread_y: u32,
    thread_z: u32,
    warp_state: u8,
}

pub trait BasicGPU {
    fn malloc(&mut self, size: usize) -> (MemoryAddress, usize);
    fn free(&mut self, addr: MemoryAddress);
    fn load_ptx(&mut self, ptx: String);
    fn select_kernel(&mut self, kernel: String);
    fn set_launch_params(&mut self, grid: dim3, threads: dim3);
    fn dump(&self, file_name: &str);
    fn execute(&mut self, args: Vec<usize>);
}

pub struct LaunchParams {
    pub grid: (u32, u32, u32),
    pub block: (u32, u32, u32),
}

pub struct GPU {
    pub memory: Memory,
    pub sms: Vec<SM>,
    pub kernel_symbol: Option<String>,
    launch_params: Option<LaunchParams>,
    raw_ptx: Option<String>,
    pub num_args: Option<usize>,
    pub kernels: HashMap<String, Vec<inst_info>>,
}

impl BasicGPU for GPU {
    fn malloc(&mut self, size: usize) -> (MemoryAddress, usize) {
        let addr = MemoryAddress::new();
        for offset in 0..size {
            self.memory.data.insert(addr + offset, MemoryElement::new());
        }
        self.memory.sizes.insert(addr, size);
        (addr, size)
    }

    fn free(&mut self, addr: MemoryAddress) {
        let size = self.memory.sizes.remove(&addr).unwrap();
        for offset in 0..size {
            self.memory.data.remove(&(addr + offset));
        }
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
        // println!("{}", self);
        self.dump("test.csv");
    }

    fn dump(&self, file_name: &str) {
        let mut w = Writer::from_path(file_name).unwrap();

        for (sm_idx, sm) in self.sms.iter().enumerate() {
            for (warp_idx, warp) in sm.warps.iter().enumerate() {
                for (thread_idx, thread) in warp.threads.iter().enumerate() {
                    let record = ThreadRecord {
                        sm_id: sm_idx,
                        warp_id: warp_idx,
                        thread_id: thread_idx,
                        grid_x: thread.grid_pos.x,
                        grid_y: thread.grid_pos.y,
                        grid_z: thread.grid_pos.z,
                        thread_x: thread.threads_pos.x,
                        thread_y: thread.threads_pos.y,
                        thread_z: thread.threads_pos.z,
                        warp_state: warp.state as u8,
                    };
                    w.serialize(record).expect("Failed to write")
                }
            }
        }
        w.flush().expect("Failed to flush writer");
    }

    fn execute(&mut self, args: Vec<usize>) {
        let insts = self
            .kernels
            .get(self.kernel_symbol.as_deref().unwrap())
            .unwrap();
        self.num_args = Some(args.len());
        for sm in self.sms.iter_mut() {
            let pb = ProgressBar::new(sm.warps.len() as u64);
            pb.set_style(ProgressStyle::default_bar());
            for warp in sm.warps.iter_mut() {
                if (warp.state == WarpState::Active) {
                    for thread in warp.threads.iter_mut() {
                        thread.execute_unit.set_execute_id(
                            thread.threads_pos.x,
                            thread.threads_pos.y,
                            thread.threads_pos.z,
                            thread.grid_pos.x,
                            thread.grid_pos.y,
                            thread.grid_pos.z,
                            self.launch_params.as_ref().unwrap().block.0,
                            self.launch_params.as_ref().unwrap().block.1,
                            self.launch_params.as_ref().unwrap().block.2,
                            self.launch_params.as_ref().unwrap().grid.0,
                            self.launch_params.as_ref().unwrap().grid.1,
                            self.launch_params.as_ref().unwrap().grid.2,
                        );
                        thread.execute_unit.import_inst(insts.clone());
                        thread
                            .execute_unit
                            .execute_all(&mut self.memory, args.clone());
                    }
                }
                pb.inc(1);
            }
        }
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
            sizes: HashMap::new(),
        },
        sms: std::iter::repeat_with(|| SM::new(280)).take(70).collect(),
        kernel_symbol: None,
        launch_params: None,
        raw_ptx: None,
        num_args: None,
        kernels: HashMap::new(),
    })
});
