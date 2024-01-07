use jeriya_shared::{
    aabb::AABB,
    log::{info, trace},
    nalgebra::Vector3,
    rayon::iter::{IntoParallelRefIterator, ParallelIterator},
};

/// Information for creating the `PointClusteringOctree`.
pub struct BuildContext<'c> {
    pub cluster_point_count: usize,
    pub point_positions: &'c [Vector3<f32>],
}

/// A `ProtoCluster` is a cluster that is not yet part of a `Page`. It contains its indices and
/// the `AABB` of the quadrant of the octree node in which it was created.
#[derive(Debug)]
pub struct ProtoCluster {
    /// The `AABB` of the quadrant of the octree node in which this cluster was created.
    pub quadrant_aabb: AABB,
    /// Indices of the points in the cluster. These indices point into the `point_positions` slice of the `BuildContext`.
    pub indices: Vec<usize>,
    /// Level is 0 for the leaf clusters and the max of all children + 1 for the other clusters. This
    /// means that it is not guaranteed that all clusters with the level 0 have the same depth.
    pub level: usize,
    /// There is either one or two children per cluster
    pub children: Vec<ProtoCluster>,
}

/// Octree that is used for creating clusters for a point cloud.
pub struct PointClusteringOctree {
    root_proto_cluster: ProtoCluster,
    proto_cluster_count: usize,
    max_cluster_depth: usize,
}

impl PointClusteringOctree {
    /// Creates a new `PointClusteringOctree` from the given `BuildContext`.
    pub fn new(build_context: &BuildContext) -> Self {
        info!(
            "Creating a PointClusteringOctree for {} points",
            build_context.point_positions.len()
        );
        let aabb = Self::bound(&build_context.point_positions);
        trace!("Outer AABB of the octree: {:?}", aabb);
        let indices = (0..build_context.point_positions.len()).collect::<Vec<_>>();
        let root_proto_cluster = Self::build_clusters_in_aabb(build_context, indices, &aabb);
        let proto_cluster_count = Self::count_proto_clusters(&root_proto_cluster);
        info!("Created a PointClusteringOctree with {} clusters", proto_cluster_count);
        let max_cluster_depth = Self::find_max_proto_cluster_depth(&root_proto_cluster);
        assert_eq!(
            max_cluster_depth, root_proto_cluster.level,
            "mismatch between cluster depth and level"
        );
        Self {
            root_proto_cluster,
            proto_cluster_count,
            max_cluster_depth,
        }
    }

    /// Returns the root `ProtoCluster` of the `PointClusteringOctree`.
    pub fn root(&self) -> &ProtoCluster {
        &self.root_proto_cluster
    }

    /// Returns the number of `ProtoCluster`s in the `PointClusteringOctree`.
    pub fn proto_cluster_count(&self) -> usize {
        self.proto_cluster_count
    }

    /// Returns the maximum depth of the `ProtoCluster`s in the `PointClusteringOctree`.
    pub fn max_proto_cluster_depth(&self) -> usize {
        self.max_cluster_depth
    }

    /// Visits all the `ProtoCluster`s in the `PointClusteringOctree`.
    pub fn visit_proto_clusters_breadth_first(&self, mut f: impl FnMut(usize, &ProtoCluster)) {
        // TODO: Make this breadth first
        fn visit(proto_cluster: &ProtoCluster, depth: usize, f: &mut impl FnMut(usize, &ProtoCluster)) {
            for child in &proto_cluster.children {
                visit(child, depth + 1, f);
            }
            f(depth, proto_cluster);
        }
        visit(&self.root_proto_cluster, 0, &mut f);
    }

    fn find_max_proto_cluster_depth(proto_cluster: &ProtoCluster) -> usize {
        proto_cluster
            .children
            .iter()
            .map(|child| 1 + Self::find_max_proto_cluster_depth(child))
            .max()
            .unwrap_or(0)
    }

    /// Returns the number of proto clusters in the octree.
    fn count_proto_clusters(proto_cluster: &ProtoCluster) -> usize {
        1 + proto_cluster
            .children
            .iter()
            .map(|proto_cluster| Self::count_proto_clusters(proto_cluster))
            .sum::<usize>()
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

    /// Creates a leaf cluster from the given `indices` and `aabb`.
    fn build_leaf_cluster(indices: Vec<usize>, aabb: &AABB) -> ProtoCluster {
        ProtoCluster {
            quadrant_aabb: *aabb,
            indices,
            level: 0,
            children: Vec::new(),
        }
    }

    /// Merges the points of the two given clusters into a single cluster.
    fn merge_two_clusters(
        build_context: &BuildContext,
        proto_cluster_a: ProtoCluster,
        proto_cluster_b: ProtoCluster,
        octree_quadrant_aabb: &AABB,
    ) -> ProtoCluster {
        let mut indices = proto_cluster_a
            .indices
            .iter()
            .chain(proto_cluster_b.indices.iter())
            .copied()
            .collect::<Vec<_>>();

        let mut i = 0;
        while indices.len() > build_context.cluster_point_count {
            indices.remove(i);
            i += 2;
            if i >= indices.len() {
                i = 0;
            }
        }

        let level = 1 + proto_cluster_a.level.max(proto_cluster_b.level);
        ProtoCluster {
            quadrant_aabb: *octree_quadrant_aabb,
            indices,
            level,
            children: vec![proto_cluster_a, proto_cluster_b],
        }
    }

    /// Merges pairs of clusters from the given `children` into single clusters.
    /// The resulting Vec contains half as many elements as the given Vec.
    fn merge_cluster_pairs(
        build_context: &BuildContext,
        mut children: Vec<Option<ProtoCluster>>,
        outer_aabb: &AABB,
    ) -> Vec<Option<ProtoCluster>> {
        let mut new_children = Vec::new();
        for i in (0..children.len()).step_by(2) {
            if i + 1 < children.len() {
                let child_a = children[i].take().expect("failed to get some");
                let child_b = children[i + 1].take().expect("failed to get some");
                let merged_cluster = Self::merge_two_clusters(build_context, child_a, child_b, &outer_aabb);
                new_children.push(Some(merged_cluster));
            } else {
                new_children.push(children[i].take());
            }
        }
        jeriya_shared::assert!(
            (children.len() + 1) / 2 == new_children.len(),
            "the number of children should be halved"
        );
        new_children
    }

    /// Combines the given `nodes` into clusters.
    fn combine_into_clusters(
        build_context: &BuildContext,
        outer_aabb: &AABB,
        child_quadrant_clusters: [Option<ProtoCluster>; 8],
    ) -> ProtoCluster {
        let mut children = child_quadrant_clusters.into_iter().filter(Option::is_some).collect::<Vec<_>>();

        while children.len() > 1 {
            children = Self::merge_cluster_pairs(build_context, children, outer_aabb);
        }

        children
            .into_iter()
            .next()
            .expect("failed to get first")
            .expect("failed to get some")
    }

    /// Creates a node from the given points.
    fn build_clusters_in_aabb(build_context: &BuildContext, indices: Vec<usize>, aabb: &AABB) -> ProtoCluster {
        // When the number of points is less than the cluster point count, create a leaf cluster directly.
        if indices.len() <= build_context.cluster_point_count {
            return Self::build_leaf_cluster(indices, aabb);
        }

        // Sort the points into 8 groups based on the quadrant they are in
        let quadrants_indices = indices
            .par_iter()
            .fold(Self::empty_quadrants, |mut children, index| {
                let point_position = &build_context.point_positions[*index];
                let quadrant_index = Self::quadrant_index(point_position, &aabb);
                let child = &mut children[quadrant_index];
                child.push(*index);
                children
            })
            .reduce(Self::empty_quadrants, |mut children1, children2| {
                for (child1, child2) in children1.iter_mut().zip(children2.iter()) {
                    child1.extend(child2.iter());
                }
                children1
            });
        jeriya_shared::assert_eq!(quadrants_indices.len(), 8, "there should be 8 quadrants");
        jeriya_shared::assert_eq!(
            quadrants_indices.iter().map(|quadrant| quadrant.len()).sum::<usize>(),
            indices.len(),
            "the number of points in the quadrants should be equal to the number of points in the node"
        );

        // Create a node for each quadrant and continue recursively
        let child_quadrant_clusters: [Option<ProtoCluster>; 8] = quadrants_indices
            .into_iter()
            .enumerate()
            .map(|(quadrant_index, quadrant_indices)| {
                let quadrant_aabb = Self::quadrant_aabb(&aabb, quadrant_index);
                if quadrant_indices.is_empty() {
                    None
                } else {
                    Some(Self::build_clusters_in_aabb(build_context, quadrant_indices, &quadrant_aabb))
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("failed to convert quadrants into array");

        // Combine the nodes into a single cluster
        Self::combine_into_clusters(build_context, aabb, child_quadrant_clusters)
    }
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
