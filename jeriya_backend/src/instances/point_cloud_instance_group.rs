use std::sync::{Arc, Weak};

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::{
    gpu_index_allocator::{AllocateGpuIndex, ProvideAllocateGpuIndex},
    transactions::{self, PushEvent},
};

use super::point_cloud_instance::{self, Error, PointCloudInstance, PointCloudInstanceBuilder};

pub struct PointCloudInstanceGroup {
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<PointCloudInstance>>,
    indexing_container: IndexingContainer<PointCloudInstance>,
    debug_info: DebugInfo,
}

impl PointCloudInstanceGroup {
    /// Creates a new [`PointCloudInstanceGroup`]
    pub fn new(gpu_index_allocator: &Arc<impl ProvideAllocateGpuIndex<PointCloudInstance>>, debug_info: DebugInfo) -> Self {
        Self {
            gpu_index_allocator: gpu_index_allocator.provide_gpu_index_allocator(),
            indexing_container: IndexingContainer::new(),
            debug_info,
        }
    }

    /// Returns the [`PointCloudInstance`] with the given [`Handle`]
    pub fn get(&self, handle: &Handle<PointCloudInstance>) -> Option<&PointCloudInstance> {
        self.indexing_container.get(handle)
    }

    /// Returns the [`DebugInfo`] of the [`PointCloudInstanceGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    /// Returns a [`PointCloudInstanceGroupAccessMut`] that can be used to mutate the [`PointCloudInstanceGroup`] via the given [`Transaction`] or [`TransactionRecorder`].
    pub fn mutate_via<'g, 't, P: PushEvent>(&'g mut self, transaction: &'t mut P) -> PointCloudInstanceGroupAccessMut<'g, 't, P> {
        PointCloudInstanceGroupAccessMut {
            point_cloud_group: self,
            transaction,
        }
    }
}

pub struct PointCloudInstanceGroupAccessMut<'g, 't, P: PushEvent> {
    point_cloud_group: &'g mut PointCloudInstanceGroup,
    transaction: &'t mut P,
}

impl<'g, 't, P: PushEvent> PointCloudInstanceGroupAccessMut<'g, 't, P> {
    /// Inserts a [`PointCloudInstance`] into the [`PointCloudInstanceGroup`].
    pub fn insert_with(
        &mut self,
        point_cloud_instance_builder: PointCloudInstanceBuilder,
    ) -> point_cloud_instance::Result<Handle<PointCloudInstance>> {
        self.point_cloud_group
            .indexing_container
            .insert_with(|handle| {
                let gpu_index_allocator = self
                    .point_cloud_group
                    .gpu_index_allocator
                    .upgrade()
                    .expect("the gpu_index_allocator was dropped");
                let gpu_index_allocation = gpu_index_allocator.allocate_gpu_index().ok_or(Error::AllocationFailed)?;
                let result = point_cloud_instance_builder.build(*handle, gpu_index_allocation);
                if result.is_err() {
                    gpu_index_allocator.free_gpu_index(gpu_index_allocation);
                }
                result
            })
            .map(|handle| {
                let point_cloud = self
                    .point_cloud_group
                    .indexing_container
                    .get(&handle)
                    .expect("just inserted value not found")
                    .clone();
                self.transaction
                    .push_event(transactions::Event::PointCloudInstance(point_cloud_instance::Event::Insert(
                        point_cloud.clone(),
                    )));
                handle
            })
    }
}
