use std::collections::HashMap;

use jeriya_shared::log::trace;

pub struct ClusterGraph {
    cluster_indices: Vec<Vec<usize>>,
    nodes: Vec<Node>,
    unique_index_mapping: HashMap<usize, usize>, // map from unique index to node index
}

impl ClusterGraph {
    pub fn new() -> Self {
        Self {
            cluster_indices: Vec::new(),
            nodes: Vec::new(),
            unique_index_mapping: HashMap::new(),
        }
    }

    pub fn push_cluster(&mut self, unique_index: usize, cluster: Vec<usize>, neighbor_unique_indices: Vec<usize>) {
        let cluster_index = self.cluster_indices.len();
        self.cluster_indices.push(cluster);
        self.unique_index_mapping.insert(unique_index, cluster_index);
        self.nodes.push(Node {
            unique_index,
            cluster_index,
            neighbor_unique_indices,
        });
    }

    /// Checks whether the node with the given unique index has the given neighbor.
    pub fn has_node_neighbor(&self, node_unique_index: usize, neighbor_unique_index: usize) -> Option<bool> {
        let node_index = self.unique_index_mapping.get(&node_unique_index)?;
        let node = self.nodes.get(*node_index)?;
        Some(node.neighbor_unique_indices.contains(&neighbor_unique_index))
    }

    /// Returns true when all neighbors are in the graph.
    pub fn validate(&self) -> bool {
        for node in &self.nodes {
            for neighbor_unique_index in &node.neighbor_unique_indices {
                if !self.has_node_neighbor(*neighbor_unique_index, node.unique_index).unwrap_or(false) {
                    trace!(
                        "Node {} has neighbor {} but not vice versa.",
                        node.unique_index,
                        neighbor_unique_index
                    );
                    return false;
                }
            }
        }
        true
    }
}

pub struct Node {
    unique_index: usize,
    cluster_index: usize,
    neighbor_unique_indices: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut cluster_graph = ClusterGraph::new();
        cluster_graph.push_cluster(500, vec![0, 1, 2], vec![501, 502]);
        cluster_graph.push_cluster(501, vec![3, 4], vec![500]);
        cluster_graph.push_cluster(502, vec![5], vec![500]);
        assert!(cluster_graph.validate());
        assert!(cluster_graph.has_node_neighbor(500, 501).unwrap());
        assert!(cluster_graph.has_node_neighbor(500, 502).unwrap());
        assert!(cluster_graph.has_node_neighbor(501, 500).unwrap());
        assert!(cluster_graph.has_node_neighbor(502, 500).unwrap());
    }
}
