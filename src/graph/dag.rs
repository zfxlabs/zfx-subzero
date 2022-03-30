use super::{Error, Result};

use std::collections::VecDeque;
use std::collections::{hash_map::Entry, HashMap, HashSet};

#[derive(Debug)]
pub struct DAG<V> {
    /// `g` defines a directed acyclic graph by the outbound edges from a vertex
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

impl<V: Clone + Eq + std::hash::Hash + std::fmt::Debug> DAG<V> {
    pub fn new() -> Self {
        DAG { g: HashMap::default(), inv: HashMap::default(), chits: HashMap::default() }
    }

    /// Inserts a new vertex into the DAG.
    ///   Note: Edges are always inserted when the vertex is initially created
    ///     when suitable parents have been selected.
    pub fn insert_vx(&mut self, vx: V, edges: Vec<V>) -> Result<()> {
        // Insert the inversion of the edges
        match self.inv.entry(vx.clone()) {
            Entry::Occupied(_) => (),
            Entry::Vacant(v) => {
                let _ = v.insert(vec![]);
            }
        }
        for ivx in edges.iter() {
            match self.inv.entry(ivx.clone()) {
                Entry::Occupied(mut o) => {
                    let o = o.get_mut();
                    o.push(vx.clone());
                }
                Entry::Vacant(_) => return Err(Error::VacantEntry),
            }
        }
        // Insert DAG with all inbound edges
        match self.g.entry(vx.clone()) {
            Entry::Occupied(_) => return Err(Error::VertexExists),
            Entry::Vacant(v1) => {
                let _ = v1.insert(edges);
            }
        }
        // Insert a 0 chit for this vertex
        self.set_chit(vx, 0)
    }

    /// Check if the given (parent) vertices exist
    pub fn has_vertices(&self, vs: &Vec<V>) -> std::result::Result<(), Vec<V>> {
        let mut missing = vec![];
        for v in vs.iter() {
            if !self.g.contains_key(v) {
                missing.push(v.clone());
            }
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Removes a vertex from the DAG. Outgoing and incoming edges are removed as well.
    /// Returns the child vertices (for Sleet to take further action where necessary)
    pub fn remove_vx(&mut self, vx: &V) -> Result<HashSet<V>> {
        let mut children_of_vx = HashSet::new();

        // Remove the edge pointing to this vertex from the child vertices
        let children = self.inv.get(vx).ok_or(Error::UndefinedVertex)?;
        for child in children {
            let _ = children_of_vx.insert(child.clone());
            match self.g.entry(child.clone()) {
                Entry::Vacant(_) => return Err(Error::VacantEntry),
                Entry::Occupied(mut o) => {
                    let vec = o.get_mut();
                    vec.retain(|e| e != vx);
                }
            }
        }

        // Remove this vertex from its parents
        let parents = self.g.get(vx).ok_or(Error::UndefinedVertex)?;
        for parent in parents {
            match self.inv.entry(parent.clone()) {
                Entry::Vacant(_) => return Err(Error::VacantEntry),
                Entry::Occupied(mut o) => {
                    let vec = o.get_mut();
                    vec.retain(|e| e != vx);
                }
            }
        }
        let _ = self.g.remove(vx);
        let _ = self.inv.remove(vx);

        Ok(children_of_vx)
    }

    /// Gets the chit of a particular node.
    pub fn get_chit(&self, vx: V) -> Result<u8> {
        match self.chits.get(&vx) {
            Some(chit) => Ok(chit.clone()),
            None => Err(Error::UndefinedChit),
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
            }
            Entry::Vacant(v) => {
                let _ = v.insert(chit);
                Ok(())
            }
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
        let mut sum: u8 = 0;
        loop {
            if queue.len() == 0 {
                break;
            }
            let elt = queue.pop_front().unwrap();
            let chit = self.get_chit(elt.clone())?;
            match sum.checked_add(chit) {
                Some(n) => sum = n,
                None => return Err(Error::ChitOverflow),
            }

            let adj = self.inv.get(&elt).unwrap();
            for edge in adj.iter().cloned() {
                match visited.entry(edge.clone()) {
                    Entry::Occupied(_) => (),
                    Entry::Vacant(v) => {
                        let _ = v.insert(true);
                        queue.push_back(edge);
                    }
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
                    }
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
    pub fn inverse(&self) -> &HashMap<V, Vec<V>> {
        &self.inv
    }

    /// Turns all inbound edges into outbound edges and returns the new graph.
    /// NOTE: This is only for testing.
    pub fn invert(&self) -> DAG<V> {
        DAG { g: self.inv.clone(), inv: self.g.clone(), chits: self.chits.clone() }
    }

    /// Get all the ancestors, partially ordered (parents precede children)
    pub fn get_ancestors(&self, v: &V) -> Vec<V> {
        let mut result: Vec<V> = vec![];
        let mut visited = HashSet::new();
        let _ = visited.insert(v.clone());

        let mut parents = self.g.get(v).unwrap().clone();
        loop {
            let mut grandparents = vec![];
            for a in parents.iter() {
                if !visited.contains(a) {
                    result.push(a.clone());
                    let _ = visited.insert(a.clone());
                }
                grandparents.extend(self.g.get(a).unwrap().iter().cloned());
            }
            if grandparents.is_empty() {
                break;
            }
            parents = grandparents;
        }
        result.reverse();
        result
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

impl<'a, V> Iterator for DFS<'a, V>
where
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
            Entry::Vacant(v) => {
                let _ = v.insert(true);
            }
        }
        let adj = self.dag.get(&next).unwrap();
        for edge in adj.iter() {
            match self.visited.entry(edge) {
                Entry::Occupied(_) => (),
                Entry::Vacant(v) => {
                    self.stack.push(edge);
                    let _ = v.insert(true);
                }
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
        assert_eq!(r1, vec![4, 3, 1, 2, 0]);

        let g2 = dag.invert();
        let r2 = g2.bfs(3);
        if r2 != vec![3, 4, 5] && r2 != vec![3, 5, 4] {
            assert!(false);
        }

        let l = dag.leaves();
        if l != vec![4, 5] && l != vec![5, 4] {
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
        assert_eq!(r1, vec![4, 2, 0, 1]);

        let g2 = dag.invert();
        let r2: Vec<_> = g2.dfs(&3).cloned().collect();
        assert_eq!(r2, vec![3, 5]);

        let l = dag.leaves();
        if l != vec![4, 5] && l != vec![5, 4] {
            assert!(false);
        }
    }

    fn make_dag(data: &[(u8, &[u8])]) -> DAG<u8> {
        let mut dag = DAG::<u8>::new();
        for (v, ps) in data {
            dag.insert_vx(*v, ps.to_vec()).unwrap();
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
    async fn dfs3() {
        #[rustfmt::skip]
        let dag = make_dag(&[
            (0, &[]),
            (1, &[0]), (2, &[0]),
            (3, &[1]),
            (4, &[3,1]),
            (5, &[4,1]),
            (6, &[5,1]),
            (7, &[6,1]),
            (8, &[7,1]),
            (9, &[8,1]),
            (10, &[9,1]),
            (11, &[10,1]),
        ]);
        let res: Vec<_> = dag.dfs(&11).cloned().collect();

        assert_eq!(res, [11, 1, 0, 10, 9, 8, 7, 6, 5, 4, 3]);
    }

    #[actix_rt::test]
    async fn test_leaves() {
        #[rustfmt::skip]
        let dag = make_dag(&[
            (0, &[]),
            (1, &[0]),
            (2, &[1]),
            (3, &[2]),
            (4, &[3]),
            (5, &[4]),
            (6, &[5]),
        ]);
        let res: Vec<_> = dag.leaves();

        assert_eq!(res, [6]);
    }

    #[actix_rt::test]
    async fn dfs_with_arrays() {
        let a0 = [0; 32];
        let a1 = [1; 32];
        let a2 = [2; 32];

        let mut dag = DAG::new();
        dag.insert_vx(a0, vec![]).unwrap();
        dag.insert_vx(a1, vec![a0]).unwrap();
        dag.insert_vx(a2, vec![a0, a1]).unwrap();

        let res: Vec<[u8; 32]> = dag.dfs(&a2).cloned().collect();
        assert_eq!(res, [a2, a1, a0]);
    }

    #[actix_rt::test]
    async fn dfs_with_u8() {
        let a0 = 0u8;
        let a1 = 1u8;
        let a2 = 2u8;

        let mut dag = DAG::new();
        dag.insert_vx(a0, vec![]).unwrap();
        dag.insert_vx(a1, vec![a0]).unwrap();
        dag.insert_vx(a2, vec![a0, a1]).unwrap();

        let res: Vec<u8> = dag.dfs(&a2).cloned().collect();
        assert_eq!(res, [a2, a1, a0]);
    }

    #[actix_rt::test]
    async fn dfs_with_arrays2() {
        let a0 = [0; 32];
        let a1 = [1; 32];
        let a2 = [2; 32];

        let mut dag = DAG::new();
        dag.insert_vx(a0, vec![]).unwrap();
        dag.insert_vx(a1, vec![a0]).unwrap();
        dag.insert_vx(a2, vec![a1, a0]).unwrap();

        let res: Vec<[u8; 32]> = dag.dfs(&a2).cloned().collect();
        assert_eq!(res, [a2, a0, a1]);
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

    #[actix_rt::test]
    async fn test_conviction2() {
        #[rustfmt::skip]
        let mut dag = make_dag(&[
            (0, &[]),
            (1, &[0]), (2, &[0]),
            (3, &[1]),
            (4, &[3]),
            (5, &[4]),
            (6, &[5]),
            (7, &[6]),
            (8, &[7]),
            (9, &[8]),
            (10, &[9]),
            (11, &[10]),
        ]);
        dag.set_chit(0, 1).unwrap();
        dag.set_chit(1, 1).unwrap();
        for i in 3..=11 {
            dag.set_chit(i, 1).unwrap();
        }
        assert_eq!(dag.conviction(0).unwrap(), 11);
    }

    #[actix_rt::test]
    async fn test_has_vertices() {
        let mut dag: DAG<u8> = DAG::new();

        // Insert the genesis vertex
        dag.insert_vx(0, vec![]).unwrap();
        dag.insert_vx(1, vec![0]).unwrap();
        dag.insert_vx(2, vec![0]).unwrap();
        dag.insert_vx(3, vec![1, 2]).unwrap();

        assert!(Ok(()) == dag.has_vertices(&vec![1, 2, 3]));
        assert!(Err(vec![4]) == dag.has_vertices(&vec![1, 2, 3, 4]));
        assert!(Err(vec![4]) == dag.has_vertices(&vec![4, 1, 2, 3]));
        assert!(Ok(()) == dag.has_vertices(&vec![]));
        assert!(Err(vec![4]) == dag.has_vertices(&vec![4]));
    }

    #[actix_rt::test]
    async fn test_remove() {
        #[rustfmt::skip]
        let mut dag = make_dag(&[
            (0, &[]),
            (1, &[0]), (2, &[0]),
            (3, &[1, 2]),
            (4, &[3]), (5, &[3])
        ]);

        let ch = dag.remove_vx(&3).unwrap();
        let mut ch: Vec<_> = ch.iter().cloned().collect();
        ch.sort();

        assert_eq!(ch, [4, 5]);
        assert_eq!(dag.get(&4).unwrap().len(), 0);
        assert_eq!(dag.get(&5).unwrap().len(), 0);
        assert_eq!(dag.inv.get(&1).unwrap().len(), 0);
        assert_eq!(dag.inv.get(&2).unwrap().len(), 0);
    }

    #[actix_rt::test]
    async fn test_get_ancestors() {
        #[rustfmt::skip]
        let dag = make_dag(&[
            (0, &[]),
            (1, &[0]), (2, &[0]), (42, &[0,1]),
            (3, &[1]), (4, &[1]),
            (5, &[3,4,2])
        ]);

        let anc = dag.get_ancestors(&5);
        assert_eq!(anc[..2], [0, 1]);
        assert!(anc[2..].contains(&3));
        assert!(anc[2..].contains(&4));
        assert!(anc[2..].contains(&2));
    }
}
