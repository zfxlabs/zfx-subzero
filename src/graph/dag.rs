use super::{Result, Error};

use std::collections::{HashMap, hash_map::Entry};
use std::collections::VecDeque;

#[derive(Debug)]
pub struct DAG<V> {
    /// `g` defines a directed acyclic graph with only inbound edges.
    g: HashMap<V, Vec<V>>,
    /// `inv` defines a directed acyclic graph with the inverted edges of `g`.
    inv: HashMap<V, Vec<V>>,
    /// `chits` defines a {0, 1} vote for a particular transaction.
    chits: HashMap<V, u8>,
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
	DAG {
	    g: HashMap::default(),
	    inv: HashMap::default(),
	    chits: HashMap::default(),
	}
    }

    /// Inserts a new vertex into the DAG.
    ///   Note: Edges are always inserted when the vertex is initially created
    ///     when suitable parents have been selected.
    pub fn insert_vx(&mut self, vx: V, edges: Vec<V>) -> Result<()> {
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
		return Err(Error::VertexExists),
	    Entry::Vacant(mut v1) => {
		let _ = v1.insert(edges);
	    },
	}
	// Insert a 0 chit for this vertex
	self.set_chit(vx, 0)
    }

    /// Gets the chit of a particular node.
    pub fn get_chit(&self, vx: V) -> Result<u8> {
	match self.chits.get(&vx) {
	    Some(chit) =>
		Ok(chit.clone()),
	    None =>
		Err(Error::UndefinedChit),
	}
    }

    /// Sets the chit of a particular node.
    pub fn set_chit(&mut self, vx: V, chit: u8) -> Result<()> {
	match self.chits.entry(vx) {
	    Entry::Occupied(mut o) => {
		let o = o.get_mut();
		if *o == 1 {
		    Err(Error::ChitReplace)
		} else {
		    *o = chit;
		    Ok(())
		}
	    },
	    Entry::Vacant(mut v) => {
		let _ = v.insert(chit);
		Ok(())
	    },
	}
    }

    /// Finds the conviction of a particular node which is the breadth-first-search of
    /// the progeny of a node, summing the chits.
    pub fn conviction(&self, vx: V) -> Result<u8> {
	// Mark all vertices as not visited (empty)
	let mut visited: HashMap<V, bool> = HashMap::default();
	// A queue for the breadth first search
	let mut queue = VecDeque::new();
	// Mark the current node as visited and enqueue it
	let _ = visited.insert(vx.clone(), true);
	queue.push_back(vx);

	// The resulting summation
	let mut sum = 0;
	loop {
	    if queue.len() == 0 {
		break;
	    }
	    let elt = queue.pop_front().unwrap();
	    let chit = self.get_chit(elt.clone())?;
	    sum += chit;

	    let adj = self.inv.get(&elt).unwrap();
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
	Ok(sum)
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

    /// Creates an iterator for depth-first traversal of vertices reachable from `vx`
    pub fn dfs<'a>(&'a self, vx: &'a V) -> DFS<'a, V> {
        DFS::new(self, vx)
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
	DAG { g: self.inv.clone(), inv: self.g.clone(), chits: self.chits.clone() }
    }
}

/// Iterator for depth-first traversal of the ancestors of a vertex
pub struct DFS<'a, V> {
    /// The underlying DAG
    dag: &'a DAG<V>,
    /// A stack for the depth first search
    stack: Vec<&'a V>,
    /// Nodes visited so far by the iterator
    visited: HashMap<&'a V, bool>,
}

impl<'a, V> DFS<'a, V>
where
    V: Clone + Eq + std::hash::Hash + std::fmt::Debug + 'a,
{
    fn new(dag: &'a DAG<V>, vx: &'a V) -> Self {
        let mut it = Self {
            dag,
            // Mark all vertices as not visited (empty)
            visited: HashMap::default(),
            stack: vec![],
        };
        // Start at `vx`
        it.stack.push(vx);
        it
    }
}

impl<'a, V> Iterator for DFS<'a, V> where
    V: Clone + Eq + std::hash::Hash + std::fmt::Debug + 'a,
{
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stack.is_empty() {
            return None;
        }

        let next = self.stack.pop().unwrap();
        match self.visited.entry(&next) {
            Entry::Occupied(_) => (),
		    Entry::Vacant(mut v) => {
		        v.insert(true);
		    },
	    }
	    let adj = self.dag.get(&next).unwrap();
	    for edge in adj.iter() {
		    match self.visited.entry(edge) {
		        Entry::Occupied(_) => (),
		        Entry::Vacant(_) => self.stack.push(edge),
		    }
		}
	    Some(next)
	}
}

#[cfg(test)]
mod test {
    use super::DAG;

    #[actix_rt::test]
    async fn test_bfs() {
        let mut dag: DAG<u8> = DAG::new();

	// Insert the genesis vertex
	dag.insert_vx(0, vec![]).unwrap();
	dag.insert_vx(1, vec![0]).unwrap();
	dag.insert_vx(2, vec![0]).unwrap();
	dag.insert_vx(3, vec![1, 2]).unwrap();
	dag.insert_vx(4, vec![3, 1]).unwrap();
	// Ensure only reachable vertices are taken into account
	dag.insert_vx(5, vec![3, 2]).unwrap();

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
	dag.insert_vx(0, vec![]).unwrap();
	dag.insert_vx(1, vec![0]).unwrap();
	dag.insert_vx(2, vec![0]).unwrap();
	dag.insert_vx(3, vec![1, 2]).unwrap();
	// Ensure only reachable vertices are taken into account
	dag.insert_vx(4, vec![1, 2]).unwrap();
	dag.insert_vx(5, vec![3, 2]).unwrap();

	let r1: Vec<_> = dag.dfs(&4).cloned().collect();
	assert_eq!(r1, vec![4,2,0,1]);

	let g2 = dag.invert();
	let r2: Vec<_> = g2.dfs(&3).cloned().collect();
        assert_eq!(r2, vec![3,5]);

	let l = dag.leaves();
	if l != vec![4,5] && l != vec![5,4] {
	    assert!(false);
	}
    }

    fn make_dag(data: &[(u8, &[u8])]) -> DAG<u8> {
        let mut dag = DAG::<u8>::new();
        for (v, ps) in data {
            dag.insert(*v, ps.to_vec());
        }
        dag
    }

    #[actix_rt::test]
    #[rustfmt::skip]
    async fn test_dfs2() {
        let dag = make_dag(&[
             (0, &[]),
             (1, &[0]), (2, &[0]),
             (3, &[1]), (4, &[1]), (5, &[2]), (6, &[2]),
             (7, &[4,5]), (8, &[3,4]),
             (9, &[8,7,6]),
            ]);

        let r1: Vec<_> = dag.dfs(&8).cloned().collect();
        assert_eq!(r1, [8,4,1,0,3]);

        let r2: Vec<_> = dag.dfs(&7).cloned().collect();
        assert_eq!(r2, [7,5,2,0,4,1]);

        let r3: Vec<_> = dag.dfs(&9).cloned().collect();
        assert_eq!(r3, [
            9,6,2,0,
            7,5,
            4,1,
            8,3]);
    }

    #[actix_rt::test]
    async fn test_conviction() {
        let mut dag: DAG<u8> = DAG::new();

	// Insert the genesis vertex
	dag.insert_vx(0, vec![]).unwrap();
	dag.insert_vx(1, vec![0]).unwrap();
	dag.insert_vx(2, vec![0]).unwrap();
	dag.insert_vx(3, vec![1, 2]).unwrap();
	// Ensure only reachable vertices are taken into account
	dag.insert_vx(4, vec![1, 2]).unwrap();
	dag.insert_vx(5, vec![3, 2]).unwrap();

	dag.set_chit(0, 1).unwrap();
	dag.set_chit(1, 1).unwrap();
	dag.set_chit(4, 1).unwrap();

	assert_eq!(dag.conviction(0).unwrap(), 3);
	assert_eq!(dag.conviction(1).unwrap(), 2);
	assert_eq!(dag.conviction(4).unwrap(), 1);
	assert_eq!(dag.conviction(2).unwrap(), 1);
	assert_eq!(dag.conviction(3).unwrap(), 0);
	assert_eq!(dag.conviction(5).unwrap(), 0);
    }
}
