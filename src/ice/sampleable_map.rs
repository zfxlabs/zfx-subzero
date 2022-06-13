use std::collections::HashMap;

use rand::seq::SliceRandom;

#[derive(Debug, Clone)]
pub struct SampleableMap<K: Eq + std::hash::Hash + Clone, V: Clone> {
    map: HashMap<K, V>,
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
    pub fn new() -> Self {
        Self { map: HashMap::default(), queue: vec![] }
    }

    fn next_queue(&self) -> Vec<(K, V)> {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<(K, V)>>()
    }

    pub fn sample(&mut self, k: usize) -> Vec<(K, V)> {
        let mut i = 0;
        let mut s = vec![];
        loop {
            if i >= k {
                break;
            } else {
                match self.queue.pop() {
                    Some(val) => {
                        s.push(val);
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
        s
    }
}
