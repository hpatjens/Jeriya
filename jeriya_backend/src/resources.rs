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
#[derive(Debug)]
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

    use jeriya_shared::nalgebra::Vector3;
    use jeriya_test::spectral::{assert_that, asserting, prelude::OptionAssertions};

    use crate::inanimate_mesh::MeshType;

    use super::*;

    pub struct DummyBackend {
        pub(crate) sender: Sender<ResourceEvent>,
        pub(crate) receiver: Receiver<ResourceEvent>,
    }

    impl DummyBackend {
        pub fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            Self { sender, receiver }
        }
    }

    impl ResourceReceiver for DummyBackend {
        fn sender(&self) -> &Sender<ResourceEvent> {
            &self.sender
        }
    }

    #[macro_export]
    macro_rules! match_one_inanimate_mesh_event {
        ($backend:expr, $p:pat, $($b:tt)*) => {{
            use jeriya_test::spectral::prelude::*;
            const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
            let ResourceEvent::InanimateMesh(inanimate_mesh_events) = $backend.receiver.recv_timeout(TIMEOUT).unwrap() else {
                panic!("failed to receive event")
            };
            asserting("event count").that(&inanimate_mesh_events).has_length(1);
            let $p = &inanimate_mesh_events[0] else {
                panic!("unexpected event")
            };
            $($b)*
        }};
    }

    pub fn assert_events_empty(backend: &DummyBackend) {
        asserting("events empty")
            .that(&backend.receiver.try_iter().next().is_none())
            .is_equal_to(true);
        assert_that!(backend.receiver.try_iter().next()).is_none();
    }

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
