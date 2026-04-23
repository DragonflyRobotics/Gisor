use rand::RngExt;
use std::{collections::HashMap, ops::Add};

pub struct MemoryElement {
    pub value: u8,
}

impl MemoryElement {
    pub fn new() -> Self {
        Self { value: 0u8 }
    }
    
    pub fn as_byte(&self) -> u8 {
        self.value
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

impl Add<u64> for MemoryAddress {
    type Output = Self;

    fn add(self, other: u64) -> Self::Output {
        Self { address: self.address + other }
    }
}
impl Add<usize> for MemoryAddress {
    type Output = Self;

    fn add(self, other: usize) -> Self::Output {
        Self { address: self.address + other as u64 }
    }
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
    pub sizes: HashMap<MemoryAddress, usize>,
}
