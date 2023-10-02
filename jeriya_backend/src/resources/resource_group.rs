use std::sync::Arc;

use jeriya_shared::{debug_info, DebugInfo};

use crate::{
    gpu_index_allocator::IntoAllocateGpuIndex, inanimate_mesh_group::InanimateMeshGroup, mesh_attributes::MeshAttributes,
    mesh_attributes_group::MeshAttributesGroup, model::ModelGroup, IntoResourceReceiver, ResourceReceiver,
};

pub struct ResourceGroup {
    inanimate_mesh_group: InanimateMeshGroup,
    model_group: ModelGroup,
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
        let inanimate_mesh_group = InanimateMeshGroup::new(
            resource_receiver.sender().clone(),
            debug_info!(format!("{}-inanimate-mesh-group", debug_info.name())),
        );
        let model_group = ModelGroup::new(&inanimate_mesh_group, debug_info!(format!("{}-model-group", debug_info.name())));
        let mesh_attributes_group = MeshAttributesGroup::new(backend, debug_info!(format!("{}-mesh-attributes-group", debug_info.name())));
        Self {
            inanimate_mesh_group,
            model_group,
            mesh_attributes_group,
            debug_info,
        }
    }

    /// Returns the [`InanimateMeshGroup`] that manages the inanimate meshes.
    pub fn inanimate_meshes(&self) -> &InanimateMeshGroup {
        &self.inanimate_mesh_group
    }

    /// Returns the [`ModelGroup`] that manages the models.
    pub fn models(&self) -> &ModelGroup {
        &self.model_group
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

    use crate::{
        inanimate_mesh::{InanimateMeshEvent, MeshType},
        match_one_inanimate_mesh_event,
        mesh_attributes::MeshAttributes,
        model::ModelSource,
        resources::tests::assert_events_empty,
        MockRenderer,
    };

    use super::*;

    #[test]
    fn smoke_test_inanimate_meshes() {
        let renderer = MockRenderer::new();
        let resource_group = ResourceGroup::new(&renderer, debug_info!("my_resource_group"));
        let inanimate_mesh = resource_group
            .inanimate_meshes()
            .create(
                MeshType::Points,
                vec![Vector3::new(0.0, 0.0, 0.0)],
                vec![Vector3::new(0.0, 1.0, 0.0)],
            )
            .build()
            .unwrap();
        drop(inanimate_mesh);
        asserting("events are received")
            .that(&renderer.receiver().try_iter().count())
            .is_equal_to(1);
    }

    #[test]
    fn smoke_test_models() {
        let renderer = MockRenderer::new();
        let resource_group = ResourceGroup::new(&renderer, debug_info!("my_resource_group"));
        let suzanne = jeriya_content::model::Model::import("../sample_assets/rotated_cube.glb").unwrap();
        let model = resource_group.models().create(ModelSource::Model(suzanne)).build().unwrap();
        // Currently, the GPU doesn't support models directly but only inanimate meshes. So, the model
        // just inserts the inanimate meshes into the inanimate mesh group.
        match_one_inanimate_mesh_event!(renderer, InanimateMeshEvent::Insert { .. }, {});
        drop(model);
        assert_events_empty(&renderer);
    }

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
