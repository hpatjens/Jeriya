use std::sync::Arc;

use jeriya_shared::{
    aabb::AABB,
    nalgebra::Vector3,
    rayon::iter::{IntoParallelRefIterator, ParallelIterator},
};

pub struct BuildContext<'c> {
    pub cluster_point_count: usize,
    pub point_positions: &'c [Vector3<f32>],
}

pub struct PointClusteringOctree {
    root_node: Node,
    node_count: usize,
    proto_cluster_count: usize,
    max_cluster_depth: usize,
}

impl PointClusteringOctree {
    pub fn new(build_context: &BuildContext) -> Self {
        let bounds = Self::bound(&build_context.point_positions);
        let indices = (0..build_context.point_positions.len()).collect::<Vec<_>>(); // TODO: There should be a special function to build a node on all indices
        let root_node = Self::build_node(build_context, indices, &bounds);
        let node_count = Self::count_nodes(&root_node);
        let proto_cluster_count = Self::count_proto_clusters(&root_node);
        let max_cluster_depth = Self::find_max_proto_cluster_depth(&root_node);
        let result = Self {
            root_node,
            node_count,
            proto_cluster_count,
            max_cluster_depth,
        };

        // let mut nodes_per_depth = Vec::new();
        // result.visit_nodes_breadth_first(|depth, _node| {
        //     if depth >= nodes_per_depth.len() {
        //         nodes_per_depth.push(0);
        //     }
        //     nodes_per_depth[depth] += 1;
        // });
        // info!("nodes_per_depth: {:?}", nodes_per_depth);

        result
    }

    /// Returns the number of `Node`s in the `PointClusteringOctree`.
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Returns the number of `ProtoCluster`s in the `PointClusteringOctree`.
    pub fn proto_cluster_count(&self) -> usize {
        self.proto_cluster_count
    }

    /// Returns the maximum depth of the `ProtoCluster`s in the `PointClusteringOctree`.
    pub fn max_proto_cluster_depth(&self) -> usize {
        self.max_cluster_depth
    }

    /// Visits all the `Node`s in the `PointClusteringOctree` in a breadth first order.
    pub fn visit_nodes_breadth_first(&self, mut f: impl FnMut(usize, &Node)) {
        fn visit(node: &Node, depth: usize, f: &mut impl FnMut(usize, &Node)) {
            f(depth, node);
            match node {
                Node::Leaf(_) => {}
                Node::Inner(inner_node) => {
                    for child in &inner_node.children {
                        if let Some(child) = child.as_ref() {
                            visit(child, depth + 1, f);
                        }
                    }
                }
            }
        }
        visit(&self.root_node, 0, &mut f);
    }

    /// Visits all the `ProtoCluster`s in the `PointClusteringOctree`.
    pub fn visit_proto_clusters_breadth_first(&self, mut f: impl FnMut(usize, &ProtoCluster)) {
        fn visit(proto_cluster: &ProtoCluster, depth: usize, f: &mut impl FnMut(usize, &ProtoCluster)) {
            f(depth, proto_cluster);
            for child in &proto_cluster.children {
                visit(child, depth + 1, f);
            }
        }
        match &self.root_node {
            Node::Leaf(proto_cluster) => visit(proto_cluster, 0, &mut f),
            Node::Inner(inner_node) => visit(&inner_node.root_proto_cluster, 0, &mut f),
        }
    }

    fn find_max_proto_cluster_depth(node: &Node) -> usize {
        fn depth(proto_cluster: &ProtoCluster) -> usize {
            1 + proto_cluster.children.iter().map(|child| depth(child)).max().unwrap_or(0)
        }
        match node {
            Node::Leaf(proto_cluster) => depth(proto_cluster),
            Node::Inner(inner_node) => depth(&inner_node.root_proto_cluster),
        }
    }

    /// Returns the number of nodes in the octree.
    fn count_nodes(node: &Node) -> usize {
        match node {
            Node::Leaf(_) => 1,
            Node::Inner(inner_node) => {
                let children_count = inner_node
                    .children
                    .iter()
                    .filter_map(|child| child.as_ref())
                    .map(|node| Self::count_nodes(&*node))
                    .sum::<usize>();
                1 + children_count
            }
        }
    }

    /// Returns the number of proto clusters in the octree.
    fn count_proto_clusters(node: &Node) -> usize {
        // The `Node` is only used as the entry point into the `ProtoCluster`s. The counting
        // is done on the tree of `ProtoCluster`s. The number of references from the `Node`s
        // to the `ProtoCluster`s are ignored.
        fn count(proto_cluster: &ProtoCluster) -> usize {
            let children_count = proto_cluster
                .children
                .iter()
                .map(|proto_cluster| count(proto_cluster))
                .sum::<usize>();
            1 + children_count
        }
        match node {
            Node::Leaf(proto_cluster) => count(proto_cluster),
            Node::Inner(inner_node) => count(&inner_node.root_proto_cluster),
        }
    }

    /// Returns the `AABB` that bounds all the points in the given `point_positions`.
    fn bound(point_positions: &[Vector3<f32>]) -> AABB {
        point_positions
            .par_iter()
            .fold(|| AABB::empty(), |aabb, point_position: &Vector3<f32>| aabb.union(point_position))
            .reduce(|| AABB::empty(), |aabb1, aabb2| aabb1.union(&aabb2))
    }

    /// Returns the index of the quadrant that the given `point_position` is in. This index can be used to access the children of a `Node`.
    fn quadrant_index(point_position: &Vector3<f32>, aabb: &AABB) -> usize {
        let center = aabb.center();
        let mut quadrant_index = 0;
        if point_position.x > center.x {
            quadrant_index |= 1;
        }
        if point_position.y > center.y {
            quadrant_index |= 2;
        }
        if point_position.z > center.z {
            quadrant_index |= 4;
        }
        quadrant_index
    }

    /// Returns the `AABB` of the quadrant with the given `quadrant_index`.
    fn quadrant_aabb(aabb: &AABB, quadrant_index: usize) -> AABB {
        let center = aabb.center();
        let qx = quadrant_index & 1 != 0;
        let qy = quadrant_index & 2 != 0;
        let qz = quadrant_index & 4 != 0;
        let min = Vector3::new(
            if qx { center.x } else { aabb.min.x },
            if qy { center.y } else { aabb.min.y },
            if qz { center.z } else { aabb.min.z },
        );
        let max = Vector3::new(
            if qx { aabb.max.x } else { center.x },
            if qy { aabb.max.y } else { center.y },
            if qz { aabb.max.z } else { center.z },
        );
        AABB::new(min, max)
    }

    /// Returns an array of 8 empty quadrants.
    fn empty_quadrants() -> [Vec<usize>; 8] {
        #[rustfmt::skip]
        let result = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        result
    }

    fn build_leaf_cluster(indices: Vec<usize>, aabb: &AABB) -> Node {
        Node::Leaf(Arc::new(ProtoCluster {
            aabb: *aabb,
            indices,
            children: Vec::new(),
        }))
    }

    /// Combines the given `nodes` into clusters.
    fn combine_into_clusters(build_context: &BuildContext, outer_aabb: &AABB, nodes: &[Option<Box<Node>>]) -> Arc<ProtoCluster> {
        let mut children = Vec::new();
        let mut indices = Vec::new();
        for node in nodes.iter().filter_map(|node| node.as_ref()).map(|node| &**node) {
            // match node {
            //     Node::Leaf(_) => info!("--> leaf"), // -> 770
            //     Node::Inner(_) => {},
            // }

            children.push(match node {
                Node::Leaf(proto_cluster) => Arc::clone(proto_cluster),
                Node::Inner(InnerNode { root_proto_cluster, .. }) => Arc::clone(root_proto_cluster),
            });
            indices.extend(match node {
                Node::Leaf(proto_cluster) => &proto_cluster.indices,
                Node::Inner(InnerNode { root_proto_cluster, .. }) => &root_proto_cluster.indices,
            });
        }

        // Remove every second point until the number of points is less than the cluster point count.
        let mut i = 0;
        while indices.len() > build_context.cluster_point_count {
            indices.remove(i);
            i += 2;
            if i >= indices.len() {
                i = 0;
            }
        }

        Arc::new(ProtoCluster {
            aabb: outer_aabb.clone(),
            indices,
            children,
        })
    }

    /// Creates a node from the given points.
    fn build_node(build_context: &BuildContext, indices: Vec<usize>, aabb: &AABB) -> Node {
        // When the number of points is less than the cluster point count, create a leaf cluster directly.
        if indices.len() <= build_context.cluster_point_count {
            // trace!("--> leaf"); -> 761
            return Self::build_leaf_cluster(indices.to_vec(), aabb);
        }

        // // Sort the points into 8 groups based on the quadrant they are in
        // let quadrants_indices = indices
        //     .par_iter()
        //     .fold(Self::empty_quadrants, |mut children, index| {
        //         let point_position = &build_context.point_positions[*index];
        //         let quadrant_index = Self::quadrant_index(point_position, &aabb);
        //         let child = &mut children[quadrant_index];
        //         child.push(*index);
        //         children
        //     })
        //     .reduce(Self::empty_quadrants, |mut children1, children2| {
        //         for (child1, child2) in children1.iter_mut().zip(children2.iter()) {
        //             child1.extend(child2.iter());
        //         }
        //         children1
        //     });
        // jeriya_shared::assert_eq!(quadrants_indices.len(), 8, "there should be 8 quadrants");
        // jeriya_shared::assert_eq!(
        //     quadrants_indices.iter().map(|quadrant| quadrant.len()).sum::<usize>(),
        //     indices.len(),
        //     "the number of points in the quadrants should be equal to the number of points in the node"
        // );

        let quadrants_indices = indices.iter().fold(Self::empty_quadrants(), |mut children, index| {
            let point_position = &build_context.point_positions[*index];
            let quadrant_index = Self::quadrant_index(point_position, &aabb);
            let child = &mut children[quadrant_index];
            child.push(*index);
            children
        });

        // Create a node for each quadrant and continue recursively
        let children_nodes = quadrants_indices
            .into_iter()
            .enumerate()
            .map(|(quadrant_index, quadrant)| {
                let quadrant_aabb = Self::quadrant_aabb(&aabb, quadrant_index);
                if quadrant.is_empty() {
                    None
                } else {
                    Some(Box::new(Self::build_node(build_context, quadrant, &quadrant_aabb)))
                }
            })
            .collect::<Vec<_>>();

        // Combine the nodes into a single cluster
        let root_proto_cluster = Self::combine_into_clusters(build_context, aabb, &children_nodes);

        Node::Inner(InnerNode {
            children: children_nodes.try_into().expect("failed to convert children into array"),
            root_proto_cluster,
        })
    }
}

#[derive(Debug)]
pub struct ProtoCluster {
    pub aabb: AABB,
    pub indices: Vec<usize>,
    children: Vec<Arc<ProtoCluster>>,
}

#[derive(Debug)]
pub enum Node {
    Leaf(Arc<ProtoCluster>),
    Inner(InnerNode),
}

#[derive(Debug)]
pub struct InnerNode {
    children: [Option<Box<Node>>; 8],
    root_proto_cluster: Arc<ProtoCluster>,
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn smoke() {
        // let point_positions = vec![
        //     Vector3::new(0.0, 0.0, 0.0),
        //     Vector3::new(1.0, 0.0, 0.0),
        //     Vector3::new(0.0, 1.0, 0.0),
        //     Vector3::new(0.0, 0.0, 1.0),
        // ];
        // let point_colors = point_positions.iter().map(|_| Vector3::new(1.0, 1.0, 1.0)).collect::<Vec<_>>();
        // let point_clustering_octree = PointClusteringOctree::new(&point_positions, &point_colors);
    }
}
