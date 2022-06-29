pub use super::output::*;

use super::types::Capacity;

use std::hash::Hash;
use std::ops::{Deref, DerefMut};

/// An aggregated structure for storing a list of [Output]s.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Outputs {
    pub outputs: Vec<Output>,
}

impl std::fmt::Debug for Outputs {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.outputs.len() == 0 {
            write!(f, "[]")
        } else {
            let mut comma_separated = String::new();
            for output in &self.outputs[0..self.outputs.len() - 1] {
                comma_separated.push_str(&format!("{:?}", output));
                comma_separated.push_str(", ");
            }
            comma_separated.push_str(&format!("{:?}", &self.outputs[self.outputs.len() - 1]));
            write!(f, "[{}]", comma_separated)
        }
    }
}

impl std::fmt::Display for Outputs {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.outputs.len() == 0 {
            write!(f, "[]")
        } else {
            let mut comma_separated = String::new();
            for output in &self.outputs[0..self.outputs.len() - 1] {
                comma_separated.push_str(&format!("{}", output));
                comma_separated.push_str(", ");
            }
            comma_separated.push_str(&format!("{}", &self.outputs[self.outputs.len() - 1]));
            write!(f, "[{}]", comma_separated)
        }
    }
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
    /// Create new instance from a list of [Output]s.
    ///
    /// ## Parameters
    /// * `outputs` - list of [Output]s for assigning to a single Outputs
    pub fn new(outputs: Vec<Output>) -> Self {
        let mut sorted = outputs.clone();
        sorted.sort();
        Outputs { outputs: sorted }
    }

    /// Returns total capacity from all [Output]s.
    pub fn sum(&self) -> Capacity {
        let mut total = 0;
        for output in self.iter() {
            total += output.capacity;
        }
        total
    }
}
