use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use crate::Camera;

pub struct ObjectContainer {
    cameras: Arc<Mutex<ObjectGroup<Camera>>>,
}

pub struct ObjectGroup<T> {
    phantom_data: PhantomData<T>,
}
