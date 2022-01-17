pub use super::input::*;

use std::collections::HashSet;

use std::cmp::{Eq, Ord, Ordering, PartialEq};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inputs<I: Eq + Hash> {
    pub inputs: HashSet<I>,
}

impl<I: Eq + Hash> Deref for Inputs<I> {
    type Target = HashSet<I>;

    fn deref(&self) -> &'_ Self::Target {
        &self.inputs
    }
}

impl<I: Eq + Hash> DerefMut for Inputs<I> {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.inputs
    }
}

// Note: We only use this to hash the inputs for equality of a `tx`, not in the
// hyperarc entries (otherwise conflict fails).
impl<I: Eq + Hash + Ord + Clone> Hash for Inputs<I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut v: Vec<I> = self.iter().cloned().collect();
        v.sort();
        v.hash(state);
    }
}

impl<I: Eq + Hash + Ord + Clone> Eq for Inputs<I> {}

impl<I: Eq + Hash + Ord + Clone> PartialEq for Inputs<I> {
    fn eq(&self, other: &Self) -> bool {
        let mut self_v: Vec<I> = self.iter().cloned().collect();
        let mut other_v: Vec<I> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v == other_v
    }
}

impl<I: Eq + Hash + Ord + Clone> Ord for Inputs<I> {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut self_v: Vec<I> = self.iter().cloned().collect();
        let mut other_v: Vec<I> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v.cmp(&other_v)
    }
}

impl<I: Eq + Hash + Ord + Clone> PartialOrd for Inputs<I> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_v: Vec<I> = self.iter().cloned().collect();
        let mut other_v: Vec<I> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        Some(self_v.cmp(&other_v))
    }
}

impl<I: Eq + Hash + Ord + Clone> Inputs<I> {
    pub fn new(inputs: Vec<I>) -> Self {
        Inputs {
            inputs: inputs.iter().cloned().collect(),
        }
    }
}
