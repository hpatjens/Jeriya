use std::marker::PhantomData;

use crate::{AsDebugInfo, Backend, Camera, DebugInfo, Handle, ObjectContainerHandler, ObjectGroupGuardHandler};

#[derive(Default)]
pub struct ObjectContainer<B: Backend> {
    object_container_handler: B::ObjectContainerHandler,
}

impl<B: Backend> ObjectContainer<B> {
    pub fn new(object_container_handler: B::ObjectContainerHandler) -> Self {
        Self { object_container_handler }
    }

    pub fn cameras(&self) -> ObjectGroupGuard<Camera, B> {
        self.object_container_handler.cameras()
    }
}

impl<B: Backend> AsDebugInfo for ObjectContainer<B> {
    fn as_debug_info(&self) -> &DebugInfo {
        self.object_container_handler.as_debug_info()
    }
}

#[derive(Default)]
pub struct ObjectGroupGuard<'a, T, B: Backend>
where
    T: 'a,
{
    object_group_guard_handler: B::ObjectGroupGuardHandler<'a, T>,
    phantom_data: PhantomData<T>,
}

impl<'a, T, B: Backend> ObjectGroupGuard<'a, T, B> {
    pub fn new(object_group_guard_handler: B::ObjectGroupGuardHandler<'a, T>) -> Self {
        Self {
            object_group_guard_handler,
            phantom_data: PhantomData,
        }
    }

    /// Inserts a new object and returns a [`Handle`] to it.
    pub fn insert(&mut self, object: T) -> Handle<T> {
        self.object_group_guard_handler.insert(object)
    }

    /// Returns a reference to the object for the given [`Handle`].
    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        self.object_group_guard_handler.get(handle)
    }

    /// Returns a mutable reference to the object for the given [`Handle`].
    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        self.object_group_guard_handler.get_mut(handle)
    }
}

impl<'a, T: Default, B: Backend> ObjectGroupGuard<'a, T, B> {
    /// Removes the object with the given handle and returns it.
    pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
        self.object_group_guard_handler.remove(handle)
    }
}
