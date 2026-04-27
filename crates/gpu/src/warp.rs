use nvtypes::dim3;

use crate::thread::Thread;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WarpState {
    Active,
    Stalled,
    InActive
}

impl Default for WarpState {
    fn default() -> Self {
        Self::InActive
    }
}

#[derive(Default, Clone)]
pub struct Warp {
    pub threads: Vec<Thread>,
    pub state: WarpState,
}

impl Warp {
    pub fn new() -> Self {
        Self {
            threads: std::iter::repeat_with(Thread::default).take(32).collect::<Vec<_>>(),
            state: WarpState::default(),
        }
    }

    pub fn is_occupied(&self) -> bool {
        self.state != WarpState::InActive
    }
    
    pub fn set_state(&mut self, state: WarpState) {
        self.state = state;
    }
    
    pub fn set_coords(&mut self, block_dim: (u32, u32, u32), thread_dim: Vec<(u32, u32, u32)>) {
        let block_dim3 = dim3 { x: block_dim.0, y: block_dim.1, z: block_dim.2 };
        for (thread, coord) in self.threads.iter_mut().zip(thread_dim.iter()) {
            let coord3 = dim3 { x: coord.0, y: coord.1, z: coord.2 };
            thread.set_grid_pos(block_dim3);
            thread.set_threads_pos(coord3);
        }
    }
}

impl std::fmt::Display for Warp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for thread in &self.threads {
            write!(f, "\t\t{}", thread)?;
        }
        Ok(())
    }
}
