pub use super::output::*;

use std::hash::Hash;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Outputs<O: Eq + Hash> {
    pub outputs: Vec<O>,
}

impl<O: Eq + Hash> Deref for Outputs<O> {
    type Target = Vec<O>;

    fn deref(&self) -> &'_ Self::Target {
        &self.outputs
    }
}

impl<O: Eq + Hash> DerefMut for Outputs<O> {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.outputs
    }
}

impl<O: Eq + Hash + Ord + Clone> Outputs<O> {
    pub fn new(outputs: Vec<O>) -> Self {
        let mut sorted = outputs.clone();
        sorted.sort();
        Outputs { outputs: sorted }
    }
}
