use std::sync::Arc;

use derive_more::Constructor;
use nalgebra::Affine3;
use parking_lot::MutexGuard;

use crate::{EventQueue, Handle, InanimateMesh, IndexingContainer, RendererConfig};

#[derive(Constructor, Debug, Clone)]
pub struct InanimateMeshInstance {
    pub inanimate_mesh: Arc<InanimateMesh>,
    pub transform: Affine3<f32>,
}

pub enum InanimateMeshInstanceEvent {
    Insert {
        handle: Handle<InanimateMeshInstance>,
        inanimate_mesh_instance: InanimateMeshInstance,
    },
    SetTransform {
        handle: Handle<InanimateMeshInstance>,
        transform: Affine3<f32>,
    },
    SetInanimateMesh {
        handle: Handle<InanimateMeshInstance>,
        inanimate_mesh: Arc<InanimateMesh>,
    },
}

#[derive(Constructor)]
pub struct InanimateMeshInstanceAccessMut<'event, 'cont, 'mutex> {
    handle: Handle<InanimateMeshInstance>,
    inanimate_mesh_instance: &'cont mut InanimateMeshInstance,
    event_queue: &'mutex mut MutexGuard<'event, EventQueue<InanimateMeshInstanceEvent>>,
}

impl<'event, 'cont, 'mutex> InanimateMeshInstanceAccessMut<'event, 'cont, 'mutex> {
    /// Returns the [`InanimateMesh`]
    pub fn inanimate_mesh(&self) -> &Arc<InanimateMesh> {
        &self.inanimate_mesh_instance.inanimate_mesh
    }

    /// Sets the [`InanimateMesh`] of the [`InanimateMeshInstance`]
    pub fn set_inanimate_mesh(&mut self, inanimate_mesh: &Arc<InanimateMesh>) {
        self.inanimate_mesh_instance.inanimate_mesh = inanimate_mesh.clone();
        self.event_queue.push(InanimateMeshInstanceEvent::SetInanimateMesh {
            handle: self.handle.clone(),
            inanimate_mesh: inanimate_mesh.clone(),
        });
    }

    /// Returns the transform of the [`InanimateMeshInstance`]
    pub fn transform(&self) -> &Affine3<f32> {
        &self.inanimate_mesh_instance.transform
    }

    /// Sets the transform of the [`InanimateMeshInstance`]
    pub fn set_transform(&mut self, transform: Affine3<f32>) {
        self.inanimate_mesh_instance.transform = transform;
        self.event_queue.push(InanimateMeshInstanceEvent::SetTransform {
            handle: self.handle.clone(),
            transform,
        });
    }
}

#[derive(Constructor)]
pub struct InanimateMeshInstanceContainerGuard<'event, 'cont> {
    event_queue: MutexGuard<'event, EventQueue<InanimateMeshInstanceEvent>>,
    inanimate_mesh_instances: MutexGuard<'cont, IndexingContainer<InanimateMeshInstance>>,
    rendering_config: Arc<RendererConfig>,
}

impl<'event, 'cont> InanimateMeshInstanceContainerGuard<'event, 'cont> {
    /// Inserts the given [`InanimateMeshInstance`] into the container and returns a [`Handle`] to it.
    ///
    /// # Errors
    ///
    /// Returns an error if the maximum number of [`InanimateMeshInstance`]s has been reached.
    pub fn insert(&mut self, inanimate_mesh_instance: InanimateMeshInstance) -> crate::Result<Handle<InanimateMeshInstance>> {
        if self.inanimate_mesh_instances.len() >= self.rendering_config.maximum_number_of_inanimate_mesh_instances {
            return Err(crate::Error::MaximumCapacityReached(
                self.rendering_config.maximum_number_of_inanimate_mesh_instances,
            ));
        }
        let inanimate_mesh_instance2 = inanimate_mesh_instance.clone();
        let handle = self.inanimate_mesh_instances.insert(inanimate_mesh_instance);
        self.event_queue.push(InanimateMeshInstanceEvent::Insert {
            handle: handle.clone(),
            inanimate_mesh_instance: inanimate_mesh_instance2,
        });
        Ok(handle)
    }

    /// Returns a reference to the [`InanimateMeshInstance`] with the given [`Handle`].
    pub fn get(&mut self, handle: &Handle<InanimateMeshInstance>) -> Option<&InanimateMeshInstance> {
        self.inanimate_mesh_instances.get(handle)
    }

    /// Returns a [`InanimateMeshInstanceAccessMut`] to the [`InanimateMeshInstance`] with the given [`Handle`].
    pub fn get_mut<'s>(&'s mut self, handle: &Handle<InanimateMeshInstance>) -> Option<InanimateMeshInstanceAccessMut<'event, '_, 's>> {
        self.inanimate_mesh_instances
            .get_mut(handle)
            .map(|i| InanimateMeshInstanceAccessMut::new(handle.clone(), i, &mut self.event_queue))
    }
}
