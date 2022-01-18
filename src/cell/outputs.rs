pub use super::output::*;

use super::types::Capacity;

use std::hash::Hash;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Outputs {
    pub outputs: Vec<Output>,
}

impl Deref for Outputs {
    type Target = Vec<Output>;

    fn deref(&self) -> &'_ Self::Target {
        &self.outputs
    }
}

impl DerefMut for Outputs {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.outputs
    }
}

impl Outputs {
    pub fn new(outputs: Vec<Output>) -> Self {
        let mut sorted = outputs.clone();
        sorted.sort();
        Outputs { outputs: sorted }
    }

    pub fn sum(&self) -> Capacity {
        let mut total = 0;
        for output in self.iter() {
            total += output.capacity;
        }
        total
    }
}
