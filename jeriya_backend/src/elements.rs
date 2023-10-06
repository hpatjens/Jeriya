use std::{
    cell::RefCell,
    sync::{Arc, Weak},
};

use crate::gpu_index_allocator::{AllocateGpuIndex, GpuIndexAllocation, GpuIndexAllocator, IntoAllocateGpuIndex};

use self::rigid_mesh::RigidMesh;

pub mod element_group;
pub mod helper;
pub mod rigid_mesh;
pub mod rigid_mesh_group;

pub struct MockRenderer {
    backend: Arc<MockBackend>,
}

impl MockRenderer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            backend: Arc::new(MockBackend {
                rigid_mesh_gpu_index_allocator: RefCell::new(GpuIndexAllocator::new(100)),
            }),
        })
    }
}

impl IntoAllocateGpuIndex<RigidMesh> for MockRenderer {
    type AllocateGpuIndex = MockBackend;
    fn into_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex> {
        Arc::downgrade(&self.backend)
    }
}

pub struct MockBackend {
    rigid_mesh_gpu_index_allocator: RefCell<GpuIndexAllocator<RigidMesh>>,
}

impl AllocateGpuIndex<RigidMesh> for MockBackend {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<RigidMesh>> {
        self.rigid_mesh_gpu_index_allocator.borrow_mut().allocate_gpu_index()
    }

    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<RigidMesh>) {
        self.rigid_mesh_gpu_index_allocator
            .borrow_mut()
            .free_gpu_index(gpu_index_allocation)
    }
}
