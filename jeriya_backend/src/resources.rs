pub mod inanimate_mesh;
pub mod inanimate_mesh_group;
pub mod mesh_attributes;
pub mod mesh_attributes_group;
pub mod model;
pub mod resource_group;
mod texture2d;

pub use inanimate_mesh::InanimateMesh;
pub use model::Model;
pub use texture2d::*;

use jeriya_shared::AsDebugInfo;

use self::{inanimate_mesh::InanimateMeshEvent, mesh_attributes_group::MeshAttributesEvent};

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource: AsDebugInfo {}

/// Event that is sent to the resource thread to update the resources
#[derive(Debug)]
pub enum ResourceEvent {
    FrameStart,
    InanimateMesh(Vec<InanimateMeshEvent>),
    MeshAttributes(Vec<MeshAttributesEvent>),
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, Receiver, Sender};

    use jeriya_test::spectral::{assert_that, asserting, prelude::OptionAssertions};

    use crate::{ResourceEvent, ResourceReceiver};

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
            use crate::resources::ResourceEvent;
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

    #[macro_export]
    macro_rules! match_one_mesh_attributes_event {
        ($backend:expr, $p:pat, $($b:tt)*) => {{
            use jeriya_test::spectral::prelude::*;
            use crate::resources::ResourceEvent;
            const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
            let ResourceEvent::MeshAttributes(mesh_attributes_events) = $backend.receiver.recv_timeout(TIMEOUT).unwrap() else {
                panic!("failed to receive event")
            };
            asserting("event count").that(&mesh_attributes_events).has_length(1);
            // At the time of writing, the MeshAttributesEvent has only the Insert variant
            #[allow(irrefutable_let_patterns)]
            let $p = &mesh_attributes_events[0] else {
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
}
