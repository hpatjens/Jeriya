use std::sync::Arc;

use jeriya_shared::{derive_new::new, parking_lot::MutexGuard, EventQueue, Handle, IndexingContainer};

use crate::Model;

#[derive(new, Debug, Clone)]
pub struct ModelInstance {
    pub model: Arc<Model>,
}

pub enum ModelInstanceEvent {
    Insert {
        handle: Handle<ModelInstance>,
        model_instance: ModelInstance,
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
