use std::collections::HashSet;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ConflictSet<T: Eq + std::hash::Hash> {
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


    /// Remove an element from the conflict set.
    /// `pref` and `last need to be changed if they were the removed element.
    pub fn remove_from_conflict_set(&mut self, elt: &T) {
        if self.conflicts.len() <= 1 {
            return;
        }
        let _ = self.conflicts.remove(elt);
        let mut next = elt.clone();
        for n in self.conflicts.iter() {
            if n != elt {
                next = n.clone();
                break;
            }
        }

        if self.pref == *elt {
            self.pref = next.clone();
            self.cnt = 0;
        }

        if self.last == *elt {
            self.last = next.clone();
            // not sure here
            // self.cnt = 0;
        }
    }
}
