use super::{Result, Error};

use std::collections::{HashMap, hash_map::Entry};
use std::collections::VecDeque;

#[derive(Debug)]
pub struct DAG<V> {
    /// `g` defines a directed acyclic graph with only inbound edges.
    g: HashMap<V, Vec<V>>,
    /// `inv` defines a directed acyclic graph with the inverted edges of `g`.
    inv: HashMap<V, Vec<V>>,
}

impl<V> std::ops::Deref for DAG<V>
where
    V: Eq + std::hash::Hash + Clone,
{
    type Target = HashMap<V, Vec<V>>;

    fn deref(&self) -> &'_ Self::Target {
        &self.g
    }
}

impl<V> std::ops::DerefMut for DAG<V>
where
    V: Eq + std::hash::Hash + Clone,
{
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.g
    }
}

impl <V: Clone + Eq + std::hash::Hash + std::fmt::Debug> DAG<V> {
    pub fn new() -> Self {
	DAG { g: HashMap::default(), inv: HashMap::default() }
    }

    /// Inserts a new vertex into the DAG.
    ///   Note: Edges are always inserted when the vertex is initially created
    ///     when suitable parents have been selected.
    pub fn insert_vx(&mut self, vx: V, edges: Vec<V>) -> Result<&mut Vec<V>> {
	// Insert the inversion of the edges
	match self.inv.entry(vx.clone()) {
	    Entry::Occupied(mut o) =>
		(),
	    Entry::Vacant(mut v) => {
		let _ = v.insert(vec![]);
	    },
	}
	for ivx in edges.iter() {
	    match self.inv.entry(ivx.clone()) {
		Entry::Occupied(mut o) => {
		    let o = o.get_mut();
		    o.push(vx.clone());
		},
		Entry::Vacant(mut v) =>
		    return Err(Error::VacantEntry),
	    }
	}
	// Insert DAG with all inbound edges
	match self.entry(vx.clone()) {
	    Entry::Occupied(_) =>
		Err(Error::VertexExists),
	    Entry::Vacant(mut v1) => {
		Ok(v1.insert(edges))
	    },
	}
    }

    /// Performs a breadth-first-search from some vertex `vx`.
    pub fn bfs(&self, vx: V) -> Vec<V> {
	// Mark all vertices as not visited (empty)
	let mut visited: HashMap<V, bool> = HashMap::default();
	// A queue for the breadth first search
	let mut queue = VecDeque::new();
	// Mark the current node as visited and enqueue it
	let _ = visited.insert(vx.clone(), true);
	queue.push_back(vx);

	// The result
	let mut result = vec![];
	loop {
	    if queue.len() == 0 {
		break;
	    }
	    let elt = queue.pop_front().unwrap();
	    result.push(elt.clone());
	    
	    let adj = self.get(&elt).unwrap();
	    for edge in adj.iter().cloned() {
		match visited.entry(edge.clone()) {
		    Entry::Occupied(_) => (),
		    Entry::Vacant(v) => {
			let _ = v.insert(true);
			queue.push_back(edge);
		    },
		}
	    }
	}
	result
    }

    /// Performs a depth-first-search from a starting vertex `vx`. This is here mainly as
    /// a reference for later implementing an `Iterator`.
    pub fn dfs(&self, vx: V) -> Vec<V> {
	// Mark all vertices as not visited (empty)
	let mut visited: HashMap<V, bool> = HashMap::default();
	// A stack for the depth first search
	let mut stack = vec![];
	stack.push(vx.clone());

	let mut result = vec![];
	loop {
	    if stack.len() == 0 {
		break;
	    }
	    let elt = stack.pop().unwrap();
	    match visited.entry(elt.clone()) {
		Entry::Occupied(_) => (),
		Entry::Vacant(mut v) => {
		    v.insert(true);
		    result.push(elt.clone());
		},
	    }
	    let adj = self.get(&elt).unwrap();
	    for edge in adj.iter().cloned() {
		match visited.entry(edge.clone()) {
		    Entry::Occupied(_) =>
			(),
		    Entry::Vacant(_) =>
			stack.push(edge),
		}
	    }
	}
	result
    }

    /// The leaves of the DAG are all the vertices of the inverse of `g` containing no
    /// outbound edges.
    pub fn leaves(&self) -> Vec<V> {
	let mut leaves = vec![];
	for (vx, edges) in self.inv.iter() {
	    if edges.len() == 0 {
		leaves.push(vx.clone())
	    }
	}
	leaves
    }

    /// Fetches the inverted adjacency list.
    pub fn inverse(&mut self) -> &mut HashMap<V, Vec<V>> {
	&mut self.inv
    }

    /// Turns all inbound edges into outbound edges and returns the new graph.
    /// NOTE: This is only for testing.
    pub fn invert(&self) -> DAG<V> {
	DAG { g: self.inv.clone(), inv: self.g.clone() }
    }
}

#[cfg(test)]
mod test {
    use super::DAG;

    #[actix_rt::test]
    async fn test_bfs() {
        let mut dag: DAG<u8> = DAG::new();

	// Insert the genesis vertex
	dag.insert_vx(0, vec![]);
	dag.insert_vx(1, vec![0]);
	dag.insert_vx(2, vec![0]);
	dag.insert_vx(3, vec![1, 2]);
	dag.insert_vx(4, vec![3, 1]);
	// Ensure only reachable vertices are taken into account
	dag.insert_vx(5, vec![3, 2]);

	let r1 = dag.bfs(4);
        assert_eq!(r1, vec![4,3,1,2,0]);

	let g2 = dag.invert();
	let r2 = g2.bfs(3);
	if r2 != vec![3,4,5] && r2 != vec![3,5,4] {
	    assert!(false);
	}
	
	let l = dag.leaves();
	if l != vec![4,5] && l != vec![5,4] {
	    assert!(false);
	}
    }

    #[actix_rt::test]
    async fn test_dfs() {
        let mut dag: DAG<u8> = DAG::new();

	// Insert the genesis vertex
	dag.insert_vx(0, vec![]);
	dag.insert_vx(1, vec![0]);
	dag.insert_vx(2, vec![0]);
	dag.insert_vx(3, vec![1, 2]);
	// Ensure only reachable vertices are taken into account
	dag.insert_vx(4, vec![1, 2]);
	dag.insert_vx(5, vec![3, 2]);

	let r1 = dag.dfs(4);
	assert_eq!(r1, vec![4,2,0,1]);

	let g2 = dag.invert();
	let r2 = g2.dfs(3);
        assert_eq!(r2, vec![3,5]);

	let l = dag.leaves();
	if l != vec![4,5] && l != vec![5,4] {
	    assert!(false);
	}
    }
}
