pub mod inanimate_mesh;
pub mod inanimate_mesh_group;
pub mod model;
mod texture2d;

pub use inanimate_mesh::InanimateMesh;
pub use model::Model;
pub use texture2d::*;

use jeriya_shared::AsDebugInfo;

use crate::ResourceReceiver;

use self::{inanimate_mesh::InanimateMeshEvent, inanimate_mesh_group::InanimateMeshGroup};

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource: AsDebugInfo {}

/// Event that is sent to the resource thread to update the resources
pub enum ResourceEvent {
    FrameStart,
    InanimateMesh(Vec<InanimateMeshEvent>),
}

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
    use std::sync::mpsc::{self, Receiver, Sender};

    use jeriya_shared::{debug_info, nalgebra::Vector3};
    use jeriya_test::spectral::asserting;

    use crate::inanimate_mesh::MeshType;

    use super::*;

    struct DummyBackend {
        sender: Sender<ResourceEvent>,
        receiver: Receiver<ResourceEvent>,
    }

    impl DummyBackend {
        fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            Self { sender, receiver }
        }
    }

    impl ResourceReceiver for DummyBackend {
        fn sender(&self) -> &Sender<ResourceEvent> {
            &self.sender
        }
    }

    macro_rules! match_one_inanimate_mesh_event {
        ($backend:expr, $p:pat, $($b:tt)*) => {{
            const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
            let ResourceEvent::InanimateMesh(inanimate_mesh_events) = $backend.receiver.recv_timeout(TIMEOUT).unwrap() else {
                panic!("failed to receive event")
            };
            asserting("event count").that(&inanimate_mesh_events.len()).is_equal_to(1);
            let $p = &inanimate_mesh_events[0] else {
                panic!("unexpected event")
            };
            $($b)*
        }};
    }

    #[test]
    fn insert() {
        let backend = DummyBackend::new();
        let resource_group = ResourceGroup::new(&backend);
        resource_group
            .inanimate_meshes()
            .create(
                MeshType::Points,
                vec![Vector3::new(0.0, 0.0, 0.0)],
                vec![Vector3::new(0.0, 1.0, 0.0)],
            )
            .with_debug_info(debug_info!("my_inanimate_mesh"))
            .with_indices(vec![0])
            .build()
            .unwrap();
        match_one_inanimate_mesh_event!(
            backend,
            InanimateMeshEvent::Insert {
                inanimate_mesh,
                vertex_positions,
                vertex_normals,
                indices,
            },
            asserting("type").that(&inanimate_mesh.mesh_type()).is_equal_to(&MeshType::Points);
            asserting("debug info")
                .that(&inanimate_mesh.debug_info().name())
                .is_equal_to(&"my_inanimate_mesh");
            asserting("vertex positions")
                .that(&vertex_positions.as_slice())
                .is_equal_to([Vector3::new(0.0, 0.0, 0.0)].as_slice());
            asserting("vertex normals")
                .that(&vertex_normals.as_slice())
                .is_equal_to([Vector3::new(0.0, 1.0, 0.0)].as_slice());
            asserting("indices")
                .that(&indices.as_ref().unwrap().as_slice())
                .is_equal_to([0].as_slice());
        );
    }

    #[test]
    fn set_vertex_positions() {
        let backend = DummyBackend::new();
        let resource_group = ResourceGroup::new(&backend);
        let inanimate_mesh = resource_group
            .inanimate_meshes()
            .create(
                MeshType::Points,
                vec![Vector3::new(0.0, 0.0, 0.0)],
                vec![Vector3::new(0.0, 1.0, 0.0)],
            )
            .with_debug_info(debug_info!("my_inanimate_mesh"))
            .with_indices(vec![0])
            .build()
            .unwrap();
        inanimate_mesh.set_vertex_positions(vec![Vector3::new(1.0, 1.0, 1.0)]).unwrap();
        match_one_inanimate_mesh_event!(backend, InanimateMeshEvent::Insert { .. }, {});
        match_one_inanimate_mesh_event!(
            backend,
            InanimateMeshEvent::SetVertexPositions {
                inanimate_mesh,
                vertex_positions,
            },
            asserting("debug info")
                .that(&inanimate_mesh.debug_info().name())
                .is_equal_to(&"my_inanimate_mesh");
            asserting("vertex positions")
                .that(&vertex_positions.as_slice())
                .is_equal_to([Vector3::new(1.0, 1.0, 1.0)].as_slice());
        );
    }
}
