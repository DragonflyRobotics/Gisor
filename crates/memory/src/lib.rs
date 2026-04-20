use rand::RngExt;
use std::collections::HashMap;

pub struct MemoryElement {
    value: Vec<u8>,
}

impl MemoryElement {
    pub fn new_empty(count: usize) -> Self {
        Self { value: vec![0u8; count] }
    }
    
    pub fn copy_in(&mut self, src: *const u8, count: usize) {
        unsafe {
            let bytes: Vec<u8> = std::slice::from_raw_parts(src as *const u8, count).to_vec();
            self.value = bytes;
        }
    }
    
    pub fn as_bytes(&self) -> &[u8] {
        &self.value
    }
    
    pub fn get_ints(&self) -> Vec<i32> {
        self.value.chunks_exact(4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
    }
}

impl std::fmt::Debug for MemoryElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemoryElement({:?})", self.value)
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryAddress {
    pub address: u64,
}

impl MemoryAddress {
    pub fn new() -> Self {
        let mut rng = rand::rng();
        let x: u64 = rng.random::<u64>();
        Self { address: x }
    }
    
    pub fn from_address(address: u64) -> Self {
        Self { address }
    }
}

pub struct Memory {
    pub data: HashMap<MemoryAddress, MemoryElement>,
}
