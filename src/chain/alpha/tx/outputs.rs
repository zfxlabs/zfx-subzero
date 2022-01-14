use std::collections::HashSet;

use std::ops::{Deref, DerefMut};
use std::hash::{Hash, Hasher};
use std::cmp::{Ord, Eq, PartialEq, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outputs<O: Eq + Hash> {
    pub outputs: HashSet<O>,
}

impl<O: Eq + Hash> Deref for Outputs<O>
{
    type Target = HashSet<O>;

    fn deref(&self) -> &'_ Self::Target {
        &self.outputs
    }
}

impl<O: Eq + Hash> DerefMut for Outputs<O>
{
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.outputs
    }
}

// Assumption: We can hash the outputs because as long as the ordering is preserved
// changing them constitutes a distinct transaction and thus must be handled as a
// separate entity within the hypegraph.
impl<O: Eq + Hash + Ord + Clone> Hash for Outputs<O>
{
    fn hash<H: Hasher>(&self, state: &mut H) {
	let mut v: Vec<O> = self.iter().cloned().collect();
	v.sort();
	v.hash(state);
    }
}

impl<O: Eq + Hash + Ord + Clone> Eq for Outputs<O> {}

impl<O: Eq + Hash + Ord + Clone> PartialEq for Outputs<O>
{
    fn eq(&self, other: &Self) -> bool {
	let mut self_v: Vec<O> = self.iter().cloned().collect();
	let mut other_v: Vec<O> = other.iter().cloned().collect();
	self_v.sort();
	other_v.sort();
	self_v == other_v
    }
}

impl<O: Eq + Hash + Ord + Clone> Ord for Outputs<O>
{
    fn cmp(&self, other: &Self) -> Ordering {
	let mut self_v: Vec<O> = self.iter().cloned().collect();
	let mut other_v: Vec<O> = other.iter().cloned().collect();
	self_v.sort();
	other_v.sort();
	self_v.cmp(&other_v)
    }
}

impl<O: Eq + Hash + Ord + Clone> PartialOrd for Outputs<O> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
	let mut self_v: Vec<O> = self.iter().cloned().collect();
	let mut other_v: Vec<O> = other.iter().cloned().collect();
	self_v.sort();
	other_v.sort();
	Some(self_v.cmp(&other_v))
    }
}

impl<O: Eq + Hash + Ord + Clone> Outputs<O>
{
    pub fn new(outputs: Vec<O>) -> Self {
	Outputs { outputs: outputs.iter().cloned().collect() }
    }
}
