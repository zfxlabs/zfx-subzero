//! [ConflictSet] maintains a set of conflicting transaction
use std::collections::HashSet;

/// `ConflictSet` represents a set of conflicting transaction in [`sleet`][crate::sleet]
///
/// It is used to determine whether a transaction can be accepted in face of
/// conflicts. For singleton conflict set [BETA1][crate::sleet::BETA1] confidence is needed.
/// If there are conflicts the preferred transaction will only be accepted after [BETA2][crate::sleet::BETA2]
/// successful queries (in the Sleet [DAG][crate::graph::DAG] a vote for a child node also raises the
/// confidence for its ancestors).
///
/// Note that `ConflictSet` is a relatively simple low-level datastructure, used through
/// the [`ConflictGraph`][crate::graph::conflict_graph::ConflictGraph] in [`sleet`][crate::sleet]
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ConflictSet<T: Eq + std::hash::Hash> {
    /// The set of conflicts
    pub conflicts: HashSet<T>,
    /// The preferred element
    pub pref: T,
    /// The last queried element
    pub last: T,
    /// Confidence count in the preferred elements
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
    /// Create a new singleton conflict set
    pub fn new(t: T) -> Self {
        let mut conflicts = HashSet::new();
        conflicts.insert(t.clone());
        ConflictSet { conflicts, pref: t.clone(), last: t, cnt: 0 }
    }

    /// Equivalence relation for conflict sets.
    ///
    /// It only checks if the list of contained elements are the same,  irrespective
    /// of other fields of the conflict set.
    pub fn is_equivalent(&self, hs: HashSet<T>) -> bool {
        self.conflicts == hs
    }

    /// Return if the given element is the preferred one
    pub fn is_preferred(&self, t: T) -> bool {
        self.pref == t
    }

    /// Return if the conflict set is a singleton, i.e., has only one element
    pub fn is_singleton(&self) -> bool {
        self.conflicts.len() == 1
    }

    /// Set the `conflicts` field of the conflict set
    pub fn set_conflicts(&mut self, conflicts: HashSet<T>) {
        self.conflicts = conflicts;
    }

    /// Remove an element from the conflict set.
    ///
    /// _Note that `pref` and `last' are left unchanged even if they were the removed element._
    pub fn remove_from_conflict_set(&mut self, elt: &T) {
        if self.conflicts.len() <= 1 {
            return;
        }
        let _ = self.conflicts.remove(elt);
    }
}
