use crate::warp::Warp;

pub struct SM {
    pub warps: Vec<Warp>,
}

impl SM {
    pub fn new(quantity: usize) -> Self {
        Self {
            warps: vec![Warp::new(); quantity],
        }
    }
    
    pub fn is_full(&self) -> bool {
        self.warps.iter().all(|warp| warp.is_occupied())
    }
    
    pub fn get_free_warps(&mut self, count: usize) -> Option<Vec<&mut Warp>> {
        let res: Vec<&mut Warp> = self.warps.iter_mut().filter(|warp| !warp.is_occupied()).take(count).collect();
        if res.len() == count {
            Some(res)
        } else {
            None
        }
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