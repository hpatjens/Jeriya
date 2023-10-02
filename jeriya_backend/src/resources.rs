pub mod inanimate_mesh;
pub mod inanimate_mesh_group;
pub mod mesh_attributes;
pub mod mesh_attributes_group;
pub mod model;
pub mod resource_group;
mod texture2d;

use std::sync::{
    mpsc::{Receiver, Sender},
    Arc,
};

pub use inanimate_mesh::InanimateMesh;
pub use model::Model;
pub use texture2d::*;

use jeriya_shared::AsDebugInfo;

use self::{inanimate_mesh::InanimateMeshEvent, mesh_attributes_group::MeshAttributesEvent};

/// Trait that provides access to the `Sender` that is used to send [`ResourceEvent`]s to the resource thread
pub trait ResourceReceiver {
    fn sender(&self) -> &Sender<ResourceEvent>;
}

/// Trait that is implemented by the renderer to provide a [`ResourceReceiver`] implementation.
pub trait IntoResourceReceiver {
    type ResourceReceiver: ResourceReceiver;
    fn into_resource_receiver(&self) -> &Self::ResourceReceiver;
}

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource: AsDebugInfo {}

/// Event that is sent to the resource thread to update the resources
#[derive(Debug)]
pub enum ResourceEvent {
    FrameStart,
    InanimateMesh(Vec<InanimateMeshEvent>),
    MeshAttributes(Vec<MeshAttributesEvent>),
}

/// A [`ResourceReceiver`] that can be used for testing
pub struct MockResourceReceiver {
    pub sender: Sender<ResourceEvent>,
    pub receiver: Receiver<ResourceEvent>,
}

impl ResourceReceiver for MockResourceReceiver {
    fn sender(&self) -> &Sender<ResourceEvent> {
        &self.sender
    }
}

/// A mock that acts as the renderer in the context of resources.
pub struct MockRenderer(Arc<MockResourceReceiver>);

impl MockRenderer {
    pub fn new() -> Arc<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        Arc::new(Self(Arc::new(MockResourceReceiver { sender, receiver })))
    }

    pub fn sender(&self) -> &Sender<ResourceEvent> {
        self.0.sender()
    }

    pub fn receiver(&self) -> &Receiver<ResourceEvent> {
        &self.0.receiver
    }
}

impl IntoResourceReceiver for MockRenderer {
    type ResourceReceiver = MockResourceReceiver;
    fn into_resource_receiver(&self) -> &Self::ResourceReceiver {
        &self.0
    }
}

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use jeriya_shared::{debug_info, nalgebra::Vector3};
    use jeriya_test::spectral::{assert_that, asserting, prelude::OptionAssertions};

    use crate::{mesh_attributes::MeshAttributes, mesh_attributes_group::MeshAttributesGroup, MockRenderer};

    /// Creates a new [`MeshAttributes`] with a single vertex
    pub fn new_dummy_mesh_attributes() -> Arc<MeshAttributes> {
        let backend = MockRenderer::new();
        let mut mesh_attributes_group = MeshAttributesGroup::new(backend.sender().clone(), debug_info!("my_mesh_attributes_group"));
        let mesh_attributes_builder = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_indices(vec![0])
            .with_debug_info(debug_info!("my_attributes"));
        mesh_attributes_group.insert_with(mesh_attributes_builder).unwrap()
    }

    #[macro_export]
    macro_rules! match_one_inanimate_mesh_event {
        ($backend:expr, $p:pat, $($b:tt)*) => {{
            use jeriya_test::spectral::prelude::*;
            use crate::resources::ResourceEvent;
            const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
            let ResourceEvent::InanimateMesh(inanimate_mesh_events) = $backend.receiver().recv_timeout(TIMEOUT).unwrap() else {
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
            let ResourceEvent::MeshAttributes(mesh_attributes_events) = $backend.receiver().recv_timeout(TIMEOUT).unwrap() else {
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

    pub fn assert_events_empty(backend: &MockRenderer) {
        asserting("events empty")
            .that(&backend.receiver().try_iter().next().is_none())
            .is_equal_to(true);
        assert_that!(backend.receiver().try_iter().next()).is_none();
    }
}
