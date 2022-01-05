use std::collections::HashSet;

#[derive(Clone)]
pub struct ConflictSet<T> {
    conflicts: HashSet<T>,
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
	ConflictSet {
	    conflicts,
	    pref: t.clone(),
	    last: t,
	    cnt: 0,
	}
    }

    pub fn is_equivalent(&self, hs: HashSet<T>) -> bool {
	self.conflicts == hs
    }

    pub fn is_preferred(&self, t: T) -> bool {
	self.pref == t
    }

    pub fn set_conflicts(&mut self, conflicts: HashSet<T>) {
	self.conflicts = conflicts;
    }
}
