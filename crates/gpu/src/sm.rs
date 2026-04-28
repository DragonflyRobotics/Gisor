use std::collections::VecDeque;

use crate::warp::Warp;

pub struct SM {
    pub warps: Vec<Warp>,
    free_warps: VecDeque<usize>,
}

impl SM {
    pub fn new(quantity: usize) -> Self {
        Self {
            warps: vec![Warp::new(); quantity],
            free_warps: (0..quantity).collect(),
        }
    }
    
    pub fn is_full(&self) -> bool {
        self.free_warps.is_empty()
    }
    
    pub fn can_reserve_warps(&self, count: usize) -> bool {
        self.free_warps.len() >= count
    }

    pub fn reserve_free_warps(&mut self, count: usize) -> Option<Vec<usize>> {
        if !self.can_reserve_warps(count) {
            return None;
        }

        let mut selected = Vec::with_capacity(count);
        for _ in 0..count {
            selected.push(self.free_warps.pop_front().unwrap());
        }

        Some(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::SM;

    #[test]
    fn reserve_free_warps_returns_available_indices() {
        let mut sm = SM::new(4);

        let first = sm.reserve_free_warps(2).unwrap();
        assert_eq!(first, vec![0, 1]);
        assert!(sm.can_reserve_warps(2));

        let second = sm.reserve_free_warps(2).unwrap();
        assert_eq!(second, vec![2, 3]);
        assert!(sm.is_full());
    }
}

impl std::fmt::Display for SM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, warp) in self.warps.iter().enumerate() {
            write!(f, "\tWarp {}:\n{}", i, warp)?;
        }
        Ok(())
    }
}