use std::sync::Weak;

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::{
    gpu_index_allocator::AllocateGpuIndex,
    transactions::{self, PushEvent},
};

use super::point_cloud::{self, Error, PointCloud, PointCloudBuilder};

pub struct PointCloudGroup {
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<PointCloud>>,
    indexing_container: IndexingContainer<PointCloud>,
    debug_info: DebugInfo,
}

impl PointCloudGroup {
    /// Creates a new [`PointCloudGroup`]
    pub(crate) fn new(gpu_index_allocator: &Weak<dyn AllocateGpuIndex<PointCloud>>, debug_info: DebugInfo) -> Self {
        Self {
            gpu_index_allocator: gpu_index_allocator.clone(),
            debug_info,
            indexing_container: IndexingContainer::new(),
        }
    }

    /// Returns the [`PointCloud`] with the given [`Handle`]
    pub fn get(&self, handle: &Handle<PointCloud>) -> Option<&PointCloud> {
        self.indexing_container.get(handle)
    }

    /// Returns the [`DebugInfo`] of the [`PointCloudGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    /// Returns a [`PointCloudGroupAccessMut`] that can be used to mutate the [`PointCloudGroup`] via the given [`Transaction`] or [`TransactionRecorder`].
    pub fn mutate_via<'g, 't, P: PushEvent>(&'g mut self, transaction: &'t mut P) -> PointCloudGroupAccessMut<'g, 't, P> {
        PointCloudGroupAccessMut {
            point_cloud_group: self,
            transaction,
        }
    }
}

pub struct PointCloudGroupAccessMut<'g, 't, P: PushEvent> {
    point_cloud_group: &'g mut PointCloudGroup,
    transaction: &'t mut P,
}

impl<'g, 't, P: PushEvent> PointCloudGroupAccessMut<'g, 't, P> {
    /// Inserts a [`PointCloud`] into the [`PointCloudGroup`].
    pub fn insert_with(&mut self, point_cloud_builder: PointCloudBuilder) -> point_cloud::Result<Handle<PointCloud>> {
        self.point_cloud_group
            .indexing_container
            .insert_with(|handle| {
                let gpu_index_allocator = self
                    .point_cloud_group
                    .gpu_index_allocator
                    .upgrade()
                    .expect("the gpu_index_allocator was dropped");
                let gpu_index_allocation = gpu_index_allocator.allocate_gpu_index().ok_or(Error::AllocationFailed)?;
                let result = point_cloud_builder.build(*handle, gpu_index_allocation);
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
                    .push_event(transactions::Event::PointCloud(point_cloud::Event::Insert(point_cloud.clone())));
                handle
            })
    }
}
