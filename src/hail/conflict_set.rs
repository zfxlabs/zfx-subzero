use crate::alpha::types::BlockHash;

use std::collections::HashSet;

#[derive(Clone)]
pub struct ConflictSet {
    pub conflicts: HashSet<BlockHash>,
    pub pref: BlockHash,
    pub last: BlockHash,
    pub cnt: u8,
}

impl std::ops::Deref for ConflictSet {
    type Target = HashSet<BlockHash>;

    fn deref(&self) -> &'_ Self::Target {
        &self.conflicts
    }
}

impl std::ops::DerefMut for ConflictSet {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.conflicts
    }
}

impl ConflictSet {
    pub fn new(t: BlockHash) -> Self {
        let mut conflicts = HashSet::new();
        conflicts.insert(t.clone());
        ConflictSet { conflicts, pref: t.clone(), last: t, cnt: 0 }
    }

    pub fn is_equivalent(&self, hs: HashSet<BlockHash>) -> bool {
        self.conflicts == hs
    }

    pub fn is_preferred(&self, t: BlockHash) -> bool {
        self.pref == t
    }

    pub fn is_singleton(&self) -> bool {
        self.conflicts.len() == 1
    }

    pub fn set_conflicts(&mut self, conflicts: HashSet<BlockHash>) {
        self.conflicts = conflicts;
    }

    pub fn is_lowest_hash(&self, hash: BlockHash) -> bool {
        match self.lowest_hash() {
            Some(h) => h == hash,
            None => true,
        }
    }

    fn lowest_hash(&self) -> Option<BlockHash> {
        if self.conflicts.len() == 0 {
            None
        } else {
            let hashes: Vec<BlockHash> = self.conflicts.iter().cloned().collect();
            let mut h = hashes[0];
            for i in 1..hashes.len() {
                let hi = hashes[i];
                if hi < h {
                    h = hi;
                }
            }
            Some(h)
        }
    }
}
