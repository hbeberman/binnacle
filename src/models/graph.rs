//! Graph algorithms for task dependency analysis.
//!
//! This module provides graph-related data structures and algorithms for analyzing
//! the task dependency graph, including component detection and connectivity analysis.

use std::collections::HashMap;

/// Union-Find (Disjoint Set Union) data structure for detecting connected components.
///
/// This implementation uses path compression and union by rank for optimal performance.
/// Time complexity: O(α(n)) per operation, where α is the inverse Ackermann function
/// (effectively constant for all practical purposes).
#[derive(Debug, Clone)]
pub struct UnionFind {
    /// Maps each element to its parent in the union-find forest
    parent: HashMap<String, String>,

    /// Ranks for union by rank heuristic (approximate tree height)
    rank: HashMap<String, usize>,
}

impl UnionFind {
    /// Create a new empty Union-Find structure.
    pub fn new() -> Self {
        Self {
            parent: HashMap::new(),
            rank: HashMap::new(),
        }
    }

    /// Add a new element to the structure.
    ///
    /// If the element already exists, this is a no-op.
    pub fn make_set(&mut self, x: String) {
        if !self.parent.contains_key(&x) {
            self.parent.insert(x.clone(), x.clone());
            self.rank.insert(x, 0);
        }
    }

    /// Find the representative (root) of the set containing `x`.
    ///
    /// Uses path compression to flatten the tree structure for future lookups.
    /// Returns `None` if the element doesn't exist in the structure.
    pub fn find(&mut self, x: &str) -> Option<String> {
        if !self.parent.contains_key(x) {
            return None;
        }

        // Path compression: make all nodes point directly to root
        let parent = self.parent.get(x).unwrap().clone();
        if parent != x {
            if let Some(root) = self.find(&parent) {
                self.parent.insert(x.to_string(), root.clone());
                return Some(root);
            }
        }
        Some(parent)
    }

    /// Union the sets containing `x` and `y`.
    ///
    /// Uses union by rank to keep trees balanced.
    /// Returns `true` if the sets were merged, `false` if they were already in the same set
    /// or if either element doesn't exist.
    pub fn union(&mut self, x: &str, y: &str) -> bool {
        let root_x = match self.find(x) {
            Some(r) => r,
            None => return false,
        };
        let root_y = match self.find(y) {
            Some(r) => r,
            None => return false,
        };

        if root_x == root_y {
            return false; // Already in same set
        }

        // Union by rank
        let rank_x = *self.rank.get(&root_x).unwrap_or(&0);
        let rank_y = *self.rank.get(&root_y).unwrap_or(&0);

        if rank_x < rank_y {
            self.parent.insert(root_x, root_y);
        } else if rank_x > rank_y {
            self.parent.insert(root_y, root_x);
        } else {
            self.parent.insert(root_y, root_x.clone());
            self.rank.insert(root_x, rank_x + 1);
        }

        true
    }

    /// Check if `x` and `y` are in the same connected component.
    ///
    /// Returns `false` if either element doesn't exist.
    pub fn connected(&mut self, x: &str, y: &str) -> bool {
        match (self.find(x), self.find(y)) {
            (Some(root_x), Some(root_y)) => root_x == root_y,
            _ => false,
        }
    }

    /// Get all connected components.
    ///
    /// Returns a vector of components, where each component is a vector of element IDs.
    /// Elements within a component are in the same connected set.
    pub fn components(&mut self) -> Vec<Vec<String>> {
        let mut component_map: HashMap<String, Vec<String>> = HashMap::new();

        // Group elements by their root
        let elements: Vec<String> = self.parent.keys().cloned().collect();
        for elem in elements {
            if let Some(root) = self.find(&elem) {
                component_map.entry(root).or_default().push(elem);
            }
        }

        // Convert to vector of components
        component_map.into_values().collect()
    }

    /// Get the number of elements in the structure.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Check if the structure is empty.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Get the number of distinct components.
    pub fn num_components(&mut self) -> usize {
        self.components().len()
    }
}

impl Default for UnionFind {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_set() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());
        uf.make_set("b".to_string());

        assert_eq!(uf.len(), 2);
        assert!(!uf.connected("a", "b"));
    }

    #[test]
    fn test_make_set_idempotent() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());
        uf.make_set("a".to_string());

        assert_eq!(uf.len(), 1);
    }

    #[test]
    fn test_find_nonexistent() {
        let mut uf = UnionFind::new();
        assert_eq!(uf.find("a"), None);
    }

    #[test]
    fn test_union_basic() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());
        uf.make_set("b".to_string());

        assert!(uf.union("a", "b"));
        assert!(uf.connected("a", "b"));
    }

    #[test]
    fn test_union_already_connected() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());
        uf.make_set("b".to_string());

        assert!(uf.union("a", "b"));
        assert!(!uf.union("a", "b")); // Already connected
    }

    #[test]
    fn test_union_nonexistent() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());

        assert!(!uf.union("a", "b")); // b doesn't exist
    }

    #[test]
    fn test_connected_basic() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());
        uf.make_set("b".to_string());
        uf.make_set("c".to_string());

        assert!(!uf.connected("a", "b"));

        uf.union("a", "b");
        assert!(uf.connected("a", "b"));
        assert!(!uf.connected("a", "c"));
    }

    #[test]
    fn test_transitive_connectivity() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());
        uf.make_set("b".to_string());
        uf.make_set("c".to_string());

        uf.union("a", "b");
        uf.union("b", "c");

        assert!(uf.connected("a", "c")); // Transitive
    }

    #[test]
    fn test_components_empty() {
        let mut uf = UnionFind::new();
        assert_eq!(uf.components().len(), 0);
        assert_eq!(uf.num_components(), 0);
    }

    #[test]
    fn test_components_singleton() {
        let mut uf = UnionFind::new();
        uf.make_set("a".to_string());

        let components = uf.components();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 1);
        assert!(components[0].contains(&"a".to_string()));
    }

    #[test]
    fn test_components_multiple() {
        let mut uf = UnionFind::new();

        // Component 1: a-b-c
        uf.make_set("a".to_string());
        uf.make_set("b".to_string());
        uf.make_set("c".to_string());
        uf.union("a", "b");
        uf.union("b", "c");

        // Component 2: d-e
        uf.make_set("d".to_string());
        uf.make_set("e".to_string());
        uf.union("d", "e");

        // Component 3: f (singleton)
        uf.make_set("f".to_string());

        let components = uf.components();
        assert_eq!(components.len(), 3);
        assert_eq!(uf.num_components(), 3);

        // Check component sizes
        let mut sizes: Vec<usize> = components.iter().map(|c| c.len()).collect();
        sizes.sort();
        assert_eq!(sizes, vec![1, 2, 3]);
    }

    #[test]
    fn test_path_compression() {
        let mut uf = UnionFind::new();

        // Create a chain: a -> b -> c -> d
        uf.make_set("a".to_string());
        uf.make_set("b".to_string());
        uf.make_set("c".to_string());
        uf.make_set("d".to_string());

        uf.union("a", "b");
        uf.union("b", "c");
        uf.union("c", "d");

        // All should have the same root
        let root_a = uf.find("a").unwrap();
        let root_b = uf.find("b").unwrap();
        let root_c = uf.find("c").unwrap();
        let root_d = uf.find("d").unwrap();

        assert_eq!(root_a, root_b);
        assert_eq!(root_b, root_c);
        assert_eq!(root_c, root_d);
    }

    #[test]
    fn test_task_graph_components() {
        let mut uf = UnionFind::new();

        // Simulate a task graph with dependencies
        // Component 1: bn-0001 -> bn-0002 -> bn-0003
        uf.make_set("bn-0001".to_string());
        uf.make_set("bn-0002".to_string());
        uf.make_set("bn-0003".to_string());
        uf.union("bn-0001", "bn-0002");
        uf.union("bn-0002", "bn-0003");

        // Component 2: bn-0004 -> bn-0005
        uf.make_set("bn-0004".to_string());
        uf.make_set("bn-0005".to_string());
        uf.union("bn-0004", "bn-0005");

        // Component 3: bn-0006 (isolated task)
        uf.make_set("bn-0006".to_string());

        assert_eq!(uf.num_components(), 3);

        // Verify connectivity within components
        assert!(uf.connected("bn-0001", "bn-0003"));
        assert!(uf.connected("bn-0004", "bn-0005"));

        // Verify non-connectivity across components
        assert!(!uf.connected("bn-0001", "bn-0004"));
        assert!(!uf.connected("bn-0003", "bn-0006"));
    }

    #[test]
    fn test_is_empty() {
        let mut uf = UnionFind::new();
        assert!(uf.is_empty());

        uf.make_set("a".to_string());
        assert!(!uf.is_empty());
    }
}
