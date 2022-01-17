use std::collections::HashSet;

use std::cmp::{Eq, Ord, Ordering, PartialEq};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

// Assumption: We can hash the outputs because as long as the ordering is preserved
// changing them constitutes a distinct transaction and thus must be handled as a
// separate entity within the hypegraph.
impl<O: Eq + Hash + Ord + Clone> Hash for Outputs<O> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut v: Vec<O> = self.outputs.clone();
        v.sort();
        v.hash(state);
    }
}

impl<O: Eq + Hash + Ord + Clone> Outputs<O> {
    pub fn new(outputs: Vec<O>) -> Self {
        let mut sorted = outputs.clone();
        sorted.sort();
        Outputs { outputs: sorted }
    }
}
