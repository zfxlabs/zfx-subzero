use std::collections::HashMap;

use rand::seq::SliceRandom;

/// A `Map` data structure which helps to get a list with random elements.
#[derive(Debug, Clone)]
pub struct SampleableMap<K: Eq + std::hash::Hash + Clone, V: Clone> {
    /// Elements in the map
    map: HashMap<K, V>,
    /// A queue of elements from `map` which is used in getting random elements upon request
    queue: Vec<(K, V)>,
}

impl<K, V> std::ops::Deref for SampleableMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    type Target = HashMap<K, V>;

    fn deref(&self) -> &'_ Self::Target {
        &self.map
    }
}

impl<K, V> std::ops::DerefMut for SampleableMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.map
    }
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> SampleableMap<K, V> {
    /// Create new instance with empty elements.
    pub fn new() -> Self {
        Self { map: HashMap::default(), queue: vec![] }
    }

    /// Returns a list of random `k`-elements.
    ///
    /// When `queue` is empty, it assigns elements from `map` of `self`
    /// and shuffles itself
    ///
    /// ## Parameters:
    /// * `k` - size limit for the resulting list of elements
    pub fn sample(&mut self, k: usize) -> Vec<(K, V)> {
        let mut i = 0;
        let mut result = vec![];
        loop {
            if i >= k {
                break;
            } else {
                match self.queue.pop() {
                    Some(val) => {
                        result.push(val);
                        i += 1;
                    }
                    None => {
                        let mut rng = rand::thread_rng();
                        self.queue = self.next_queue();
                        if self.queue.len() > 0 {
                            self.queue.shuffle(&mut rng);
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        result
    }

    fn next_queue(&self) -> Vec<(K, V)> {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<(K, V)>>()
    }
}
