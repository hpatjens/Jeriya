pub mod mesh_attributes;
pub mod mesh_attributes_group;
pub mod resource_group;
mod texture2d;

use std::sync::{
    mpsc::{Receiver, Sender},
    Arc, Weak,
};

pub use texture2d::*;

use jeriya_shared::{parking_lot::Mutex, AsDebugInfo};

use crate::gpu_index_allocator::{AllocateGpuIndex, GpuIndexAllocation, ProvideAllocateGpuIndex};

use self::{mesh_attributes::MeshAttributes, mesh_attributes_group::MeshAttributesEvent};

/// Trait that provides access to the `Sender` that is used to send [`ResourceEvent`]s to the resource thread
pub trait ResourceReceiver {
    fn sender(&self) -> &Sender<ResourceEvent>;
}

/// Trait that is implemented by the renderer to provide a [`ResourceReceiver`] implementation.
pub trait ProvideResourceReceiver {
    type ResourceReceiver: ResourceReceiver;
    fn provide_resource_receiver(&self) -> &Self::ResourceReceiver;
}

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource: AsDebugInfo {}

/// Event that is sent to the resource thread to update the resources
#[derive(Debug)]
pub enum ResourceEvent {
    FrameStart,
    MeshAttributes(Vec<MeshAttributesEvent>),
}

/// A [`ResourceReceiver`] that can be used for testing
pub struct MockBackend {
    pub sender: Sender<ResourceEvent>,
    pub receiver: Mutex<Receiver<ResourceEvent>>,
}

impl ResourceReceiver for MockBackend {
    fn sender(&self) -> &Sender<ResourceEvent> {
        &self.sender
    }
}

impl AllocateGpuIndex<MeshAttributes> for MockBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<MeshAttributes>> {
        Some(GpuIndexAllocation::new_unchecked(0))
    }
    fn free_gpu_index(&self, _gpu_index_allocation: GpuIndexAllocation<MeshAttributes>) {}
}

/// A mock that acts as the renderer in the context of resources.
pub struct MockRenderer(Arc<MockBackend>);

impl MockRenderer {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Arc<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        Arc::new(Self(Arc::new(MockBackend {
            sender,
            receiver: Mutex::new(receiver),
        })))
    }

    pub fn sender(&self) -> &Sender<ResourceEvent> {
        self.0.sender()
    }

    pub fn receiver(&self) -> &Mutex<Receiver<ResourceEvent>> {
        &self.0.receiver
    }
}

impl ProvideResourceReceiver for MockRenderer {
    type ResourceReceiver = MockBackend;
    fn provide_resource_receiver(&self) -> &Self::ResourceReceiver {
        &self.0
    }
}

impl ProvideAllocateGpuIndex<MeshAttributes> for MockRenderer {
    type AllocateGpuIndex = MockBackend;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(&self.0)
    }
}

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use jeriya_shared::{debug_info, nalgebra::Vector3};

    use crate::resources::{mesh_attributes::MeshAttributes, mesh_attributes_group::MeshAttributesGroup, MockRenderer};

    /// Creates a new [`MeshAttributes`] with a single vertex
    pub fn new_dummy_mesh_attributes() -> Arc<MeshAttributes> {
        let renderer = MockRenderer::new();
        let mut mesh_attributes_group = MeshAttributesGroup::new(&renderer, debug_info!("my_mesh_attributes_group"));
        let mesh_attributes_builder = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_indices(vec![0])
            .with_debug_info(debug_info!("my_attributes"));
        mesh_attributes_group.insert_with(mesh_attributes_builder).unwrap()
    }

    #[macro_export]
    macro_rules! match_one_mesh_attributes_event {
        ($backend:expr, $p:pat, $($b:tt)*) => {{
            use crate::resources::ResourceEvent;
            const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
            let ResourceEvent::MeshAttributes(mesh_attributes_events) = $backend.receiver().lock().recv_timeout(TIMEOUT).unwrap() else {
                panic!("failed to receive event")
            };
            assert_eq!(mesh_attributes_events.len(), 1);
            // At the time of writing, the MeshAttributesEvent has only the Insert variant
            #[allow(irrefutable_let_patterns)]
            let $p = &mesh_attributes_events[0] else {
                panic!("unexpected event")
            };
            $($b)*
        }};
    }

    pub fn assert_events_empty(backend: &MockRenderer) {
        assert!(&backend.receiver().lock().try_iter().next().is_none());
        assert!(backend.receiver().lock().try_iter().next().is_none());
    }
}
