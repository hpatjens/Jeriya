use std::sync::{mpsc::Sender, Arc, Weak};

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::{
    gpu_index_allocator::{AllocateGpuIndex, ProvideAllocateGpuIndex},
    resources::{
        mesh_attributes::{self, MeshAttributeBuilder, MeshAttributes},
        ProvideResourceReceiver, ResourceEvent, ResourceReceiver,
    },
};

pub struct MeshAttributesGroup {
    mesh_attributes: IndexingContainer<Arc<MeshAttributes>>,
    resource_event_sender: Sender<ResourceEvent>,
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<MeshAttributes>>,
    debug_info: DebugInfo,
}

impl MeshAttributesGroup {
    /// Creates a new [`MeshAttributesGroup`]
    pub(crate) fn new<B>(backend: &Arc<B>, debug_info: DebugInfo) -> Self
    where
        B: ProvideResourceReceiver + ProvideAllocateGpuIndex<MeshAttributes>,
    {
        let resource_event_sender = backend.provide_resource_receiver().sender().clone();
        let gpu_index_allocator = backend.provide_gpu_index_allocator();
        Self {
            mesh_attributes: IndexingContainer::new(),
            resource_event_sender,
            gpu_index_allocator,
            debug_info,
        }
    }

    /// Inserts a [`MeshAttributes`] into the [`MeshAttributesGroup`]
    pub fn insert_with(&mut self, mesh_attributes_builder: MeshAttributeBuilder) -> mesh_attributes::Result<Arc<MeshAttributes>> {
        let handle = self.mesh_attributes.insert_with(|handle| {
            let gpu_index_allocator = &self.gpu_index_allocator.upgrade().expect("gpu index allocator cannot be dropped");
            let gpu_index_allocation = gpu_index_allocator
                .allocate_gpu_index()
                .ok_or(mesh_attributes::Error::AllocationFailed)?;
            let result = mesh_attributes_builder.build(*handle, gpu_index_allocation).map(Arc::new);
            if result.is_err() {
                gpu_index_allocator.free_gpu_index(gpu_index_allocation);
            }
            result
        })?;
        let value = self.mesh_attributes.get(&handle).expect("just inserted value not found").clone();
        self.resource_event_sender
            .send(ResourceEvent::MeshAttributes(vec![MeshAttributesEvent::Insert {
                handle,
                mesh_attributes: value.clone(),
            }]))
            .expect("resource event cannot be sent");
        Ok(value)
    }

    /// Returns the [`DebugInfo`] of the [`MeshAttributesGroup`].
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

/// Event that is sent to the resource thread to update the resources
#[derive(Debug)]
pub enum MeshAttributesEvent {
    Insert {
        handle: Handle<Arc<MeshAttributes>>,
        mesh_attributes: Arc<MeshAttributes>,
    },
}

#[cfg(test)]
mod tests {
    use jeriya_shared::{debug_info, nalgebra::Vector3};

    use crate::{match_one_mesh_attributes_event, resources::tests::assert_events_empty, resources::MockRenderer};

    use super::*;

    #[test]
    fn smoke() {
        let renderer = MockRenderer::new();
        let mut mesh_attributes_group = MeshAttributesGroup::new(&renderer, debug_info!("my_mesh_attributes_group"));
        let mesh_attributes_builder = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_indices(vec![0])
            .with_debug_info(debug_info!("my_attributes"));
        mesh_attributes_group.insert_with(mesh_attributes_builder).unwrap();
        match_one_mesh_attributes_event!(
            renderer,
            MeshAttributesEvent::Insert { handle, mesh_attributes },
            assert_eq!(handle.index(), 0);
            assert_eq!(mesh_attributes.debug_info().name(), "my_attributes");
        );
        assert_events_empty(&renderer);
    }
}
