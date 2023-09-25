use std::sync::Arc;

use jeriya_shared::{derive_new::new, nalgebra::Affine3, parking_lot::MutexGuard, EventQueue, Handle, IndexingContainer};

use crate::Model;

#[derive(new, Debug, Clone)]
pub struct ModelInstance {
    pub model: Arc<Model>,
    pub transform: Affine3<f32>,
}

pub enum ModelInstanceEvent {
    Insert {
        handle: Handle<ModelInstance>,
        model_instance: ModelInstance,
    },
    SetTransform {
        handle: Handle<ModelInstance>,
        transform: Affine3<f32>,
    },
    SetModel {
        handle: Handle<ModelInstance>,
        model: Arc<Model>,
    },
}

pub struct ModelInstanceAccessMut<'event, 'cont, 'mutex> {
    handle: Handle<ModelInstance>,
    model_instance: &'cont mut ModelInstance,
    event_queue: &'mutex mut MutexGuard<'event, EventQueue<ModelInstanceEvent>>,
}

impl<'event, 'cont, 'mutex> ModelInstanceAccessMut<'event, 'cont, 'mutex> {
    /// Returns the [`Model`]
    pub fn model(&self) -> &Arc<Model> {
        &self.model_instance.model
    }

    /// Sets the [`Model`] of the [`ModelInstance`]
    pub fn set_model(&mut self, model: &Arc<Model>) {
        self.model_instance.model = model.clone();
        self.event_queue.push(ModelInstanceEvent::SetModel {
            handle: self.handle.clone(),
            model: model.clone(),
        });
    }

    /// Returns the transform of the [`ModelInstance`]
    pub fn transform(&self) -> &Affine3<f32> {
        &self.model_instance.transform
    }

    /// Sets the transform of the [`ModelInstance`]
    pub fn set_transform(&mut self, transform: Affine3<f32>) {
        self.model_instance.transform = transform;
        self.event_queue.push(ModelInstanceEvent::SetTransform {
            handle: self.handle.clone(),
            transform,
        });
    }
}

#[derive(new)]
pub struct ModelInstanceContainerGuard<'event, 'cont> {
    event_queue: MutexGuard<'event, EventQueue<ModelInstanceEvent>>,
    model_instances: MutexGuard<'cont, IndexingContainer<ModelInstance>>,
}

impl<'event, 'cont> ModelInstanceContainerGuard<'event, 'cont> {
    pub fn insert(&mut self, model_instance: ModelInstance) -> crate::Result<Handle<ModelInstance>> {
        let model_instance2 = model_instance.clone();
        let handle = self.model_instances.insert(model_instance);
        self.event_queue.push(ModelInstanceEvent::Insert {
            handle: handle.clone(),
            model_instance: model_instance2,
        });
        Ok(handle)
    }
}
