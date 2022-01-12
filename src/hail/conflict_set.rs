use crate::chain::alpha::block::{Block, BlockHash};

use std::collections::HashSet;

#[derive(Clone)]
pub struct ConflictSet<T> {
    pub conflicts: HashSet<T>,
    pub pref: T,
    pub last: T,
    pub cnt: u8,
}

impl<T> std::ops::Deref for ConflictSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    type Target = HashSet<T>;

    fn deref(&self) -> &'_ Self::Target {
        &self.conflicts
    }
}

impl<T> std::ops::DerefMut for ConflictSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.conflicts
    }
}

impl<T> ConflictSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    pub fn new(t: T) -> Self {
        let mut conflicts = HashSet::new();
        conflicts.insert(t.clone());
        ConflictSet { conflicts, pref: t.clone(), last: t, cnt: 0 }
    }

    pub fn is_equivalent(&self, hs: HashSet<T>) -> bool {
        self.conflicts == hs
    }

    pub fn is_preferred(&self, t: T) -> bool {
        self.pref == t
    }

    pub fn is_singleton(&self) -> bool {
        self.conflicts.len() == 1
    }

    pub fn set_conflicts(&mut self, conflicts: HashSet<T>) {
        self.conflicts = conflicts;
    }

    // pub fn lowest_hash(&self) -> BlockHash {
    // 	let mut h = self.conflicts[0].hash();
    // 	for i in 1..self.conflicts.len() {
    //         let hi = self.conflicts[i].hash();
    //         if hi < h {
    // 		h = hi;
    //         }
    // 	}
    // 	h
    // }
}
