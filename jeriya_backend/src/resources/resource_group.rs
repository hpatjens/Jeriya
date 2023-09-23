use crate::{inanimate_mesh_group::InanimateMeshGroup, ResourceReceiver};

pub struct ResourceGroup {
    inanimate_mesh_group: InanimateMeshGroup,
}

impl ResourceGroup {
    /// Creates a new [`ResourceGroup`]
    pub fn new(resource_receiver: &impl ResourceReceiver) -> Self {
        Self {
            inanimate_mesh_group: InanimateMeshGroup::new(resource_receiver.sender().clone()),
        }
    }

    pub fn inanimate_meshes(&self) -> &InanimateMeshGroup {
        &self.inanimate_mesh_group
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::nalgebra::Vector3;
    use jeriya_test::spectral::asserting;

    use crate::{inanimate_mesh::MeshType, resources::tests::DummyBackend};

    use super::*;

    #[test]
    fn smoke() {
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
}
