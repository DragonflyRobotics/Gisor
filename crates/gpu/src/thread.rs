use nvtypes::dim3;

#[derive(Default, Copy, Clone)]
pub struct Thread {
    pub grid_pos: dim3,
    pub threads_pos: dim3,
}

impl Thread {
    pub fn new() -> Self {
        Self {
            grid_pos: dim3 { x: 0, y: 0, z: 0 },
            threads_pos: dim3 { x: 0, y: 0, z: 0 },
        }
    }
    
    pub fn set_grid_pos(&mut self, grid_pos: dim3) {
        self.grid_pos = grid_pos;
    }
    
    pub fn set_threads_pos(&mut self, threads_pos: dim3) {
        self.threads_pos = threads_pos;
    }
}

impl std::fmt::Display for Thread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "grid_pos: {:?}, threads_pos: {:?}", self.grid_pos, self.threads_pos)
    }
}
