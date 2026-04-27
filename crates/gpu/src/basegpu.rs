use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    collections::HashMap,
    sync::Mutex,
};

use csv::Writer;
use memory::{Memory, MemoryAddress, MemoryElement};
use nvtypes::dim3;
use once_cell::sync::Lazy;
use serde::Serialize;
use utils::triple_zip;

use crate::{
    execute_unit::ExecuteUnitClass,
    inst_info::inst_info,
    sm::SM,
    warp::{Warp, WarpState},
};

#[derive(PartialEq, Eq, Clone, Debug)]
struct WorkItem {
    priority: usize,
    sm_id: usize,
    warp_id: usize,
}

impl Ord for WorkItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}
impl PartialOrd for WorkItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

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
            .unwrap()
            .clone();
        self.num_args = Some(args.len());
        let mut work_queue: BinaryHeap<WorkItem> = BinaryHeap::new();

        let mut active_warps = self.prepare_active_warps(&insts);

        while !active_warps.is_empty() {
            let mut ran_warp = false;

            for unit_class in [ExecuteUnitClass::Special, ExecuteUnitClass::Memory, ExecuteUnitClass::Generic] {
                if self.execute_best_class_warp(&active_warps, unit_class, &args) {
                    ran_warp = true;
                    break;
                }
            }

            if !ran_warp {
                break;
            }

            active_warps.retain(|&(smi, warpi)| self.sms[smi].warps[warpi].state == WarpState::Active);
        }
    }
}

impl GPU {
    fn prepare_active_warps(&mut self, insts: &Vec<inst_info>) -> Vec<(usize, usize)> {
        let mut active_warps: Vec<(usize, usize)> = Vec::new();
        let launch_params = self.launch_params.as_ref().unwrap();

        for (smi, sm) in self.sms.iter_mut().enumerate() {
            for (warpi, warp) in sm.warps.iter_mut().enumerate() {
                if warp.state == WarpState::Active {
                    for thread in warp.threads.iter_mut() {
                        thread.execute_unit.set_execute_id(
                            thread.threads_pos.x,
                            thread.threads_pos.y,
                            thread.threads_pos.z,
                            thread.grid_pos.x,
                            thread.grid_pos.y,
                            thread.grid_pos.z,
                            launch_params.block.0,
                            launch_params.block.1,
                            launch_params.block.2,
                            launch_params.grid.0,
                            launch_params.grid.1,
                            launch_params.grid.2,
                        );
                        thread.execute_unit.import_inst(insts.clone());
                    }
                    active_warps.push((smi, warpi));
                }
            }
        }
        while !work_queue.is_empty() {
            println!("Work queue: {:?}", work_queue.len());
            let work_item = work_queue.pop().unwrap();
            let warp = &mut self.sms[work_item.sm_id].warps[work_item.warp_id];

        active_warps
    }

    fn execute_best_class_warp( &mut self, active_warps: &[(usize, usize)], unit_class: ExecuteUnitClass, args: &Vec<usize>,) -> bool 
    {
        let mut scored: Vec<(usize, usize, usize, usize)> = Vec::new();
        let class_pri = if unit_class == ExecuteUnitClass::Special {
            0usize
        } else if unit_class == ExecuteUnitClass::Memory {
            1usize
        } else {
            2usize
        };

        for &(smi, warpi) in active_warps {

            let warp = &self.sms[smi].warps[warpi];
            if warp.next_execute_unit_class() != Some(unit_class) {
                continue;
            }

            scored.push((smi, warpi, class_pri, warp.divergence_score()));
        }

        if scored.is_empty() {
            return false;
        }

        warp_scheduler::prioritize(&mut scored);

        let (smi, warpi, _, _) = scored[0];

            let warp = &mut self.sms[smi].warps[warpi];
            let mut threads_state: [bool; 32] = [false; 32];
            for (i, thread) in warp.threads.iter_mut().enumerate() {
                let done_t = thread
                    .execute_unit
                    .execute_clock(&mut self.memory, args.clone());
                threads_state[i] = done_t;
            }

            if threads_state.iter().all(|&t| t) {
                warp.state = WarpState::InActive;
                active_warps.retain(|&(a, b)| !(a == smi && b == warpi));
            }
        }

        true
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
        sms: std::iter::repeat_with(|| SM::new(1000)).take(120).collect(),
        kernel_symbol: None,
        launch_params: None,
        raw_ptx: None,
        num_args: None,
        kernels: HashMap::new(),
    })
});
