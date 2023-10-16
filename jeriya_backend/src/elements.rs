use std::sync::{Arc, Weak};

use jeriya_shared::parking_lot::Mutex;

use crate::gpu_index_allocator::{AllocateGpuIndex, GpuIndexAllocation, GpuIndexAllocator, ProvideAllocateGpuIndex};

use self::rigid_mesh::RigidMesh;

pub mod camera;
pub mod camera_group;
pub mod element_group;
pub mod helper;
pub mod rigid_mesh;
pub mod rigid_mesh_group;

pub struct MockRenderer {
    backend: Arc<MockBackend>,
}

impl MockRenderer {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            backend: Arc::new(MockBackend {
                rigid_mesh_gpu_index_allocator: Mutex::new(GpuIndexAllocator::new(100)),
            }),
        })
    }
}

impl ProvideAllocateGpuIndex<RigidMesh> for MockRenderer {
    type AllocateGpuIndex = MockBackend;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(&self.backend)
    }
}

pub struct MockBackend {
    rigid_mesh_gpu_index_allocator: Mutex<GpuIndexAllocator<RigidMesh>>,
}

impl AllocateGpuIndex<RigidMesh> for MockBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<RigidMesh>> {
        self.rigid_mesh_gpu_index_allocator.lock().allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<RigidMesh>) {
        self.rigid_mesh_gpu_index_allocator.lock().free_gpu_index(gpu_index_allocation)
    }
}
