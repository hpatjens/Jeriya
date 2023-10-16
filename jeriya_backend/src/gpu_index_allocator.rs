use std::{collections::VecDeque, marker::PhantomData, sync::Weak};

use jeriya_shared::derive_where::derive_where;

/// Trait that enables allocating a new and unique index for a given type
pub trait AllocateGpuIndex<T>: Send + Sync {
    fn allocate_gpu_index(&self) -> Option<GpuIndexAllocation<T>>;
    fn free_gpu_index(&self, gpu_index_allocation: GpuIndexAllocation<T>);
}

/// Trait that is implemented by the renderer to provide a [`AllocateGpuIndex`] implementation.
pub trait ProvideAllocateGpuIndex<T> {
    type AllocateGpuIndex: AllocateGpuIndex<T> + 'static;
    fn provide_gpu_index_allocator(&self) -> Weak<Self::AllocateGpuIndex>;
}

/// Allocator for managing unique indices of values in GPU memory
pub struct GpuIndexAllocator<T> {
    capacity: usize,
    free_list: VecDeque<usize>,
    next_index: usize,
    phantom_data: PhantomData<T>,
}

impl<T> GpuIndexAllocator<T> {
    /// Creates a new [`GpuIndexAllocator`] with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            free_list: VecDeque::new(),
            next_index: 0,
            phantom_data: PhantomData,
        }
    }

    /// Allocates a new [`GpuIndexAllocation`] if possible.
    pub fn allocate_gpu_index(&mut self) -> Option<GpuIndexAllocation<T>> {
        if let Some(index) = self.free_list.pop_front() {
            Some(GpuIndexAllocation::new_unchecked(index))
        } else if self.next_index >= self.capacity {
            None
        } else {
            let index = self.next_index;
            self.next_index += 1;
            Some(GpuIndexAllocation::new_unchecked(index))
        }
    }

    /// Frees the given index
    pub fn free_gpu_index(&mut self, gpu_index_allocation: GpuIndexAllocation<T>) {
        self.free_list.push_back(gpu_index_allocation.index());
    }

    /// Returns the number of allocated indices
    pub fn len(&self) -> usize {
        self.next_index - self.free_list.len()
    }

    /// Returns true if no indices are currently allocated
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Allocation of a unique index for a given type
#[derive_where(Debug, PartialEq, Eq, Clone, Copy)]
#[derive_where(crate = jeriya_shared::derive_where)]
pub struct GpuIndexAllocation<T> {
    index: usize,
    phantom_data: PhantomData<T>,
}

impl<T> GpuIndexAllocation<T> {
    pub fn new_unchecked(index: usize) -> Self {
        Self {
            index,
            phantom_data: PhantomData,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut allocator = GpuIndexAllocator::<u32>::new(2);
        assert_eq!(allocator.len(), 0);
        assert!(allocator.is_empty());

        let a1 = allocator.allocate_gpu_index().unwrap();
        assert_eq!(allocator.len(), 1);
        assert!(!allocator.is_empty());
        assert_eq!(a1.index(), 0);

        let a2 = allocator.allocate_gpu_index().unwrap();
        assert_eq!(allocator.len(), 2);
        assert!(!allocator.is_empty());
        assert_eq!(a2.index(), 1);

        assert_eq!(allocator.allocate_gpu_index(), None);

        allocator.free_gpu_index(a1);
        assert_eq!(allocator.len(), 1);
        assert!(!allocator.is_empty());

        allocator.free_gpu_index(a2);
        assert_eq!(allocator.len(), 0);
        assert!(allocator.is_empty());
    }
}
