use crate::{inanimate_mesh_group::InanimateMeshGroup, model::ModelGroup, ResourceReceiver};

pub struct ResourceGroup {
    inanimate_mesh_group: InanimateMeshGroup,
    model_group: ModelGroup,
}

impl ResourceGroup {
    /// Creates a new [`ResourceGroup`]
    ///
    /// Pass the [`Renderer`] as the `resource_receiver` parameter.
    pub fn new(resource_receiver: &impl ResourceReceiver) -> Self {
        let inanimate_mesh_group = InanimateMeshGroup::new(resource_receiver.sender().clone());
        let model_group = ModelGroup::new(&inanimate_mesh_group);
        Self {
            inanimate_mesh_group,
            model_group,
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
}

#[cfg(test)]
mod tests {
    use jeriya_shared::nalgebra::Vector3;
    use jeriya_test::spectral::asserting;

    use crate::{
        inanimate_mesh::{InanimateMeshEvent, MeshType},
        match_one_inanimate_mesh_event,
        model::ModelSource,
        resources::tests::{assert_events_empty, DummyBackend},
    };

    use super::*;

    #[test]
    fn smoke_test_inanimate_meshes() {
        let backend = DummyBackend::new();
        let resource_group = ResourceGroup::new(&backend);
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
            .that(&backend.receiver.try_iter().count())
            .is_equal_to(1);
    }

    #[test]
    fn smoke_test_models() {
        let backend = DummyBackend::new();
        let resource_group = ResourceGroup::new(&backend);
        let suzanne = jeriya_content::model::Model::import("../sample_assets/rotated_cube.glb").unwrap();
        let model = resource_group.models().create(ModelSource::Model(suzanne)).build().unwrap();
        // Currently, the GPU doesn't support models directly but only inanimate meshes. So, the model
        // just inserts the inanimate meshes into the inanimate mesh group.
        match_one_inanimate_mesh_event!(backend, InanimateMeshEvent::Insert { .. }, {});
        drop(model);
        assert_events_empty(&backend);
    }
}
