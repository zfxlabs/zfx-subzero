pub use super::input::*;

use serde::ser::{Serialize, SerializeSeq, Serializer};

use std::collections::HashSet;

use std::cmp::{Eq, Ord, Ordering, PartialEq};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Deserialize)]
pub struct Inputs {
    pub inputs: HashSet<Input>,
}

impl Deref for Inputs {
    type Target = HashSet<Input>;

    fn deref(&self) -> &'_ Self::Target {
        &self.inputs
    }
}

impl DerefMut for Inputs {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.inputs
    }
}

impl Serialize for Inputs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut v: Vec<Input> = self.iter().cloned().collect();
        v.sort();
        let mut seq = serializer.serialize_seq(Some(v.len()))?;
        for e in v.iter() {
            seq.serialize_element(e)?;
        }
        seq.end()
    }
}

// Note: We only use this to hash the inputs for equality of a `tx`, not in the
// hyperarc entries (otherwise conflict fails).
impl Hash for Inputs {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut v: Vec<Input> = self.iter().cloned().collect();
        v.sort();
        v.hash(state);
    }
}

impl Eq for Inputs {}

impl PartialEq for Inputs {
    fn eq(&self, other: &Self) -> bool {
        let mut self_v: Vec<Input> = self.iter().cloned().collect();
        let mut other_v: Vec<Input> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v == other_v
    }
}

impl Ord for Inputs {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut self_v: Vec<Input> = self.iter().cloned().collect();
        let mut other_v: Vec<Input> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v.cmp(&other_v)
    }
}

impl PartialOrd for Inputs {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_v: Vec<Input> = self.iter().cloned().collect();
        let mut other_v: Vec<Input> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        Some(self_v.cmp(&other_v))
    }
}

impl Inputs {
    pub fn new(inputs: Vec<Input>) -> Self {
        Inputs { inputs: inputs.iter().cloned().collect() }
    }
}
