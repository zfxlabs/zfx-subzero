//! Utility data structures to keep Sleet memory use bounded

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::ops::Deref;

/// A `HashSet` replacement with a maximum capacity, once full the oldest element gets removed
pub struct BoundedHashSet<T> {
    size: usize,
    elems: HashSet<T>,
    queue: VecDeque<T>,
}

impl<T: Clone + Eq + Hash> BoundedHashSet<T> {
    /// Creates a new instance with `size` as max allowed capacity.
    /// When it reaches the max capacity, the oldest elements must be removed upon insert.
    pub fn new(size: usize) -> Self {
        BoundedHashSet {
            size,
            elems: HashSet::with_capacity(size + 1),
            queue: VecDeque::with_capacity(size + 1),
        }
    }

    /// Insert an element into the hash set.
    /// When it reaches the max capacity, the first oldest element is removed on FIFO basis.
    pub fn insert(&mut self, elem: T) {
        let duplicate = !self.elems.insert(elem.clone());
        if duplicate {
            return;
        }
        if self.elems.len() >= self.size {
            let e = self.queue.pop_front().unwrap();
            let _ = self.elems.remove(&e);
        }
        self.queue.push_back(elem);
    }
}

impl<T: Clone + Eq + Hash> Deref for BoundedHashSet<T> {
    type Target = HashSet<T>;

    fn deref(&self) -> &'_ Self::Target {
        &self.elems
    }
}

/// A `HashMap` replacement with a maximum capacity, once full the oldest element gets removed
pub struct BoundedHashMap<K, V> {
    size: usize,
    elems: HashMap<K, V>,
    queue: VecDeque<K>,
}

impl<K: Clone + Eq + Hash, V> BoundedHashMap<K, V> {
    /// Creates a new instance with `size` as max allowed capacity.
    /// When it reaches the max capacity, the oldest elements must be removed upon insert.
    pub fn new(size: usize) -> Self {
        BoundedHashMap {
            size,
            elems: HashMap::with_capacity(size + 1),
            queue: VecDeque::with_capacity(size + 1),
        }
    }

    /// Insert an element into the hash map.
    /// When it reaches the max capacity, the first oldest element is removed on FIFO basis.
    pub fn insert(&mut self, k: K, v: V) {
        if let Some(_) = self.elems.insert(k.clone(), v) {
            return;
        }
        if self.elems.len() >= self.size {
            let e = self.queue.pop_front().unwrap();
            let _ = self.elems.remove(&e);
        }
        self.queue.push_back(k);
    }
}

impl<K: Clone + Eq + Hash, V> Deref for BoundedHashMap<K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &'_ Self::Target {
        &self.elems
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[actix_rt::test]
    async fn bounded_hashmap_test() {
        let mut h = BoundedHashMap::new(3);
        h.insert(1, 1);
        h.insert(2, 2);
        h.insert(3, 3);
        assert!(h.contains_key(&3));

        h.insert(4, 4);
        assert!(h.contains_key(&4));
        assert!(Some(&4) == h.get(&4));

        assert!(!h.contains_key(&1));
    }

    #[actix_rt::test]
    async fn bounded_hashset_test() {
        let mut h = BoundedHashSet::new(3);
        h.insert(1);
        h.insert(2);
        h.insert(3);
        assert!(h.contains(&3));

        h.insert(4);
        assert!(h.contains(&4));

        assert!(!h.contains(&1));
    }
}
