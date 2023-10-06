use std::sync::Arc;

use jeriya_shared::{debug_info, DebugInfo};

use crate::{
    gpu_index_allocator::IntoAllocateGpuIndex, mesh_attributes::MeshAttributes, mesh_attributes_group::MeshAttributesGroup,
    IntoResourceReceiver,
};

pub struct ResourceGroup {
    mesh_attributes_group: MeshAttributesGroup,
    debug_info: DebugInfo,
}

impl ResourceGroup {
    /// Creates a new [`ResourceGroup`]
    ///
    /// Pass the [`Renderer`] as the `resource_receiver` parameter.
    pub fn new<B>(backend: &Arc<B>, debug_info: DebugInfo) -> Self
    where
        B: IntoResourceReceiver + IntoAllocateGpuIndex<MeshAttributes>,
    {
        let resource_receiver = backend.into_resource_receiver();
        let mesh_attributes_group = MeshAttributesGroup::new(backend, debug_info!(format!("{}-mesh-attributes-group", debug_info.name())));
        Self {
            mesh_attributes_group,
            debug_info,
        }
    }

    /// Returns the [`MeshAttributesGroup`] that manages the mesh attributes.
    pub fn mesh_attributes(&mut self) -> &mut MeshAttributesGroup {
        &mut self.mesh_attributes_group
    }

    /// Returns the [`DebugInfo`] of the [`ResourceGroup`].
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::{debug_info, nalgebra::Vector3};
    use jeriya_test::spectral::asserting;

    use crate::{mesh_attributes::MeshAttributes, MockRenderer};

    use super::*;

    #[test]
    fn smoke_test_mesh_attributes() {
        let renderer = MockRenderer::new();
        let mut resource_group = ResourceGroup::new(&renderer, debug_info!("my_resource_group"));
        let mesh_attributes_builder = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)]);
        let mesh_attributes = resource_group.mesh_attributes().insert_with(mesh_attributes_builder);
        drop(mesh_attributes);
        asserting("events are received")
            .that(&renderer.receiver().try_iter().count())
            .is_equal_to(1);
    }
}
