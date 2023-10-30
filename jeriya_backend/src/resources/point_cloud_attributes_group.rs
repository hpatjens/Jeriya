use std::sync::{mpsc::Sender, Arc, Weak};

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::gpu_index_allocator::{AllocateGpuIndex, ProvideAllocateGpuIndex};

use super::{
    point_cloud_attributes::{self, PointCloudAttributes, PointCloudAttributesBuilder},
    ProvideResourceReceiver, ResourceEvent, ResourceReceiver,
};

/// Event that is sent to the resource thread to update the resources
#[derive(Debug)]
pub enum PointCloudAttributesEvent {
    Insert {
        handle: Handle<Arc<PointCloudAttributes>>,
        point_cloud_attributes: Arc<PointCloudAttributes>,
    },
}

pub struct PointCloudAttributesGroup {
    point_cloud_attributes: IndexingContainer<Arc<PointCloudAttributes>>,
    resource_event_sender: Sender<ResourceEvent>,
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<PointCloudAttributes>>,
    debug_info: DebugInfo,
}

impl PointCloudAttributesGroup {
    /// Creates a new [`PointCloudAttributesGroup`]
    pub(crate) fn new<B>(backend: &Arc<B>, debug_info: DebugInfo) -> Self
    where
        B: ProvideResourceReceiver + ProvideAllocateGpuIndex<PointCloudAttributes>,
    {
        let resource_event_sender = backend.provide_resource_receiver().sender().clone();
        let gpu_index_allocator = backend.provide_gpu_index_allocator();
        Self {
            point_cloud_attributes: IndexingContainer::new(),
            resource_event_sender,
            gpu_index_allocator,
            debug_info,
        }
    }

    /// Inserts a [`PointCloudAttributes`] into the [`PointCloudAttributesGroup`]
    pub fn insert_with(
        &mut self,
        point_cloud_attributes_builder: PointCloudAttributesBuilder,
    ) -> point_cloud_attributes::Result<Arc<PointCloudAttributes>> {
        let handle = self.point_cloud_attributes.insert_with(|handle| {
            let gpu_index_allocator = &self.gpu_index_allocator.upgrade().expect("gpu index allocator cannot be dropped");
            let gpu_index_allocation = gpu_index_allocator
                .allocate_gpu_index()
                .ok_or(point_cloud_attributes::Error::AllocationFailed)?;
            let result = point_cloud_attributes_builder.build(*handle, gpu_index_allocation).map(Arc::new);
            if result.is_err() {
                gpu_index_allocator.free_gpu_index(gpu_index_allocation);
            }
            result
        })?;
        let value = self
            .point_cloud_attributes
            .get(&handle)
            .expect("just inserted value not found")
            .clone();
        self.resource_event_sender
            .send(ResourceEvent::PointCloudAttributes(vec![PointCloudAttributesEvent::Insert {
                handle,
                point_cloud_attributes: value.clone(),
            }]))
            .expect("resource event cannot be sent");
        Ok(value)
    }

    /// Returns the [`DebugInfo`] of the [`PointCloudAttributesGroup`].
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::{debug_info, nalgebra::Vector3};

    use crate::{match_one_point_cloud_attributes_event, resources::tests::assert_events_empty, resources::MockRenderer};

    use super::*;

    #[test]
    fn smoke() {
        let renderer = MockRenderer::new();
        let mut point_cloud_attributes_group = PointCloudAttributesGroup::new(&renderer, debug_info!("my_mesh_attributes_group"));
        let point_cloud_attributes_builder = PointCloudAttributes::builder()
            .with_point_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_debug_info(debug_info!("my_attributes"));
        point_cloud_attributes_group.insert_with(point_cloud_attributes_builder).unwrap();
        match_one_point_cloud_attributes_event!(
            renderer,
            PointCloudAttributesEvent::Insert { handle, point_cloud_attributes },
            assert_eq!(handle.index(), 0);
            assert_eq!(point_cloud_attributes.debug_info().name(), "my_attributes");
        );
        assert_events_empty(&renderer);
    }
}
