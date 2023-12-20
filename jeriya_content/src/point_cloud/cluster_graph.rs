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

    /// Computes a new level of clusters and returns the indices into the clusters.
    pub fn compute_level(&mut self) -> Vec<usize> {
        todo!()
    }

    pub fn push_cluster(&mut self, unique_index: usize, cluster: Vec<usize>, neighbor_unique_indices: Vec<usize>) {
        let cluster_index = self.cluster_indices.len();
        self.cluster_indices.push(cluster);
        self.unique_index_mapping.insert(unique_index, cluster_index);
        self.nodes.push(Node {
            unique_index,
            cluster_index,
            neighbor_unique_indices,
            children_indices: Vec::new(),
            depth: 0,
        });
    }

    /// Checks whether the node with the given unique index has the given neighbor.
    pub fn has_node_neighbor(&self, node_unique_index: usize, neighbor_unique_index: usize) -> Option<bool> {
        let node_index = self.unique_index_mapping.get(&node_unique_index)?;
        let node = self.nodes.get(*node_index)?;
        Some(node.neighbor_unique_indices.contains(&neighbor_unique_index))
    }

    /// Returns the node with the given unique index.
    pub fn get_node(&self, unique_index: usize) -> Option<&Node> {
        let node_index = self.unique_index_mapping.get(&unique_index)?;
        self.nodes.get(*node_index)
    }

    /// Returns the node with the given unique index.
    pub fn get_node_mut(&mut self, unique_index: usize) -> Option<&mut Node> {
        let node_index = self.unique_index_mapping.get(&unique_index)?;
        self.nodes.get_mut(*node_index)
    }

    /// When node a is a neighbor of node b, then node b is a neighbor of node a.
    pub fn create_bidirectional_connections(&mut self) {
        let mut new_connections = Vec::new();
        for node in &self.nodes {
            for neighbor_unique_index in &node.neighbor_unique_indices {
                let neighbor = self
                    .get_node(*neighbor_unique_index)
                    .expect("failed to get neighbor to create bidirectional connections.");
                if !neighbor.neighbor_unique_indices.contains(&node.unique_index) {
                    new_connections.push((neighbor.unique_index, node.unique_index));
                }
            }
        }
        for (node_unique_index, neighbor_unique_index) in new_connections {
            let node = self
                .get_node_mut(node_unique_index)
                .expect("failed to get node to create bidirectional connections.");
            node.neighbor_unique_indices.push(neighbor_unique_index);
        }
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

    pub fn to_dot(&self) -> String {
        let mut dot = String::new();
        dot.push_str("graph {\n");
        for node in &self.nodes {
            dot.push_str(&format!("  {};\n", node.unique_index));
        }
        for node in &self.nodes {
            for neighbor_unique_index in &node.neighbor_unique_indices {
                dot.push_str(&format!("  {} -- {};\n", node.unique_index, neighbor_unique_index));
            }
        }
        dot.push_str("}\n");
        dot
    }
}

pub struct Node {
    unique_index: usize,
    cluster_index: usize,
    neighbor_unique_indices: Vec<usize>,
    children_indices: Vec<usize>,
    depth: usize,
}

#[cfg(test)]
mod tests {
    use jeriya_shared::function_name;
    use jeriya_test::create_test_result_folder_for_function;

    use super::*;

    #[test]
    fn smoke() {
        let mut cluster_graph = ClusterGraph::new();
        cluster_graph.push_cluster(500, vec![0, 1, 2], vec![501 /*502*/]); // missing because the graph must be able to handle missing bidirectional connections
        cluster_graph.push_cluster(501, vec![3, 4], vec![500]);
        cluster_graph.push_cluster(502, vec![5], vec![500]);

        cluster_graph.create_bidirectional_connections();
        assert!(cluster_graph.validate());

        assert!(cluster_graph.has_node_neighbor(500, 501).unwrap());
        assert!(cluster_graph.has_node_neighbor(500, 502).unwrap());
        assert!(cluster_graph.has_node_neighbor(501, 500).unwrap());
        assert!(cluster_graph.has_node_neighbor(502, 500).unwrap());

        // Write dot file
        let dot = cluster_graph.to_dot();
        let directory = create_test_result_folder_for_function(function_name!());
        std::fs::write(directory.join("cluster_graph.dot"), dot).unwrap();
    }
}
