use std::{collections::VecDeque, f32::consts::PI};

use parking_lot::MutexGuard;

use crate::{
    nalgebra::{Matrix4, Vector3},
    nalgebra_glm, Handle, IndexingContainer,
};

/// Type of projection for a camera.
#[derive(Debug, Clone)]
pub enum CameraProjection {
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
    Perspective {
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    },
}

impl CameraProjection {
    /// Returns the projection matrix for `CameraProjection`.
    pub fn projection_matrix(&self) -> Matrix4<f32> {
        match *self {
            CameraProjection::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => nalgebra_glm::ortho_rh_zo(left, right, bottom, top, near, far),
            CameraProjection::Perspective { fov, aspect, near, far } => nalgebra_glm::perspective_rh_zo(aspect, fov, near, far),
        }
    }
}

impl Default for CameraProjection {
    fn default() -> Self {
        Self::Perspective {
            fov: PI / 2.0,
            aspect: 1.0,
            near: 0.1,
            far: 10000.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraTransform {
    position: Vector3<f32>,
    forward: Vector3<f32>,
    up: Vector3<f32>,
}

impl Default for CameraTransform {
    fn default() -> Self {
        Self {
            position: Vector3::new(0.0, 0.0, 0.0),
            forward: Vector3::new(0.0, 0.0, 1.0),
            up: Vector3::new(0.0, -1.0, 0.0),
        }
    }
}

impl CameraTransform {
    /// Returns the view matrix for the camera.
    pub fn view_matrix(&self) -> Matrix4<f32> {
        Matrix4::look_at_rh(&self.position.into(), &(self.position + self.forward).into(), &self.up)
    }
}

/// A camera.
#[derive(Debug, Clone)]
pub struct Camera {
    projection: CameraProjection,
    transform: CameraTransform,
    cached_projection_matrix: Matrix4<f32>,
    cached_view_matrix: Matrix4<f32>,
    cached_matrix: Matrix4<f32>,
}

impl Default for Camera {
    fn default() -> Self {
        let projection = CameraProjection::default();
        let transform = CameraTransform::default();
        let cached_projection_matrix = projection.projection_matrix();
        let cached_view_matrix = transform.view_matrix();
        let cached_matrix = cached_projection_matrix * cached_view_matrix;
        Self {
            projection,
            transform,
            cached_projection_matrix,
            cached_view_matrix,
            cached_matrix,
        }
    }
}

impl Camera {
    /// Creates a new camera.
    pub fn new(projection: CameraProjection, transform: CameraTransform) -> Self {
        let cached_projection_matrix = projection.projection_matrix();
        let cached_view_matrix = transform.view_matrix();
        let cached_matrix = cached_projection_matrix * cached_view_matrix;
        Self {
            projection,
            transform,
            cached_projection_matrix,
            cached_view_matrix,
            cached_matrix,
        }
    }

    /// Returns the [`CameraProjection`] of the camera.
    pub fn projection(&self) -> &CameraProjection {
        &self.projection
    }

    /// Returns the [`CameraTransform`] of the camera.
    pub fn transform(&self) -> &CameraTransform {
        &self.transform
    }

    /// Performes the necessary updates to the cached matrices when the view changes.
    fn update_cached_matrices_on_view_change(&mut self) {
        self.cached_view_matrix = self.transform.view_matrix();
        self.cached_matrix = self.cached_projection_matrix * self.cached_view_matrix;
    }

    /// Performes the necessary updates to the cached matrices when the projection changes.
    fn update_cached_matrices_on_projection_change(&mut self) {
        self.cached_projection_matrix = self.projection.projection_matrix();
        self.cached_matrix = self.cached_projection_matrix * self.cached_view_matrix;
    }

    /// Sets the [`CameraProjection`] of the camera.
    pub fn set_projection(&mut self, projection: CameraProjection) {
        self.projection = projection;
        self.update_cached_matrices_on_projection_change();
    }

    /// Sets the [`CameraTransform`] of the camera.
    pub fn set_transform(&mut self, transform: CameraTransform) {
        self.transform = transform;
        self.update_cached_matrices_on_view_change();
    }

    /// Sets the position of the camera.
    ///
    /// Prefer the method [`Camera::set_transform`] if you want to set the position, forward and up vectors at the same time.
    pub fn set_position(&mut self, position: Vector3<f32>) {
        self.transform.position = position;
        self.update_cached_matrices_on_view_change();
    }

    /// Sets the forward vector of the camera.
    ///
    /// Prefer the method [`Camera::set_transform`] if you want to set the position, forward and up vectors at the same time.
    pub fn set_forward(&mut self, forward: Vector3<f32>) {
        self.transform.forward = forward;
        self.update_cached_matrices_on_view_change();
    }

    /// Sets the up vector of the camera.
    ///
    /// Prefer the method [`Camera::set_transform`] if you want to set the position, forward and up vectors at the same time.
    pub fn set_up(&mut self, up: Vector3<f32>) {
        self.transform.up = up;
        self.update_cached_matrices_on_view_change();
    }
}

pub struct EventQueue<T> {
    events: VecDeque<T>,
}

impl<T> EventQueue<T> {
    pub fn new() -> Self {
        Self { events: VecDeque::new() }
    }

    pub fn push(&mut self, event: T) {
        self.events.push_back(event);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.events.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

pub enum CameraEvent {
    Insert {
        handle: Handle<Camera>,
        camera: Camera,
    },
    Remove {
        handle: Handle<Camera>,
    },
    SetProjection {
        handle: Handle<Camera>,
        projection: CameraProjection,
    },
    SetTransform {
        handle: Handle<Camera>,
        transform: CameraTransform,
    },
    SetPosition {
        handle: Handle<Camera>,
        position: Vector3<f32>,
    },
    SetForward {
        handle: Handle<Camera>,
        forward: Vector3<f32>,
    },
    SetUp {
        handle: Handle<Camera>,
        up: Vector3<f32>,
    },
}

pub struct CameraAccessMut<'event, 'cont, 'mutex> {
    handle: Handle<Camera>,
    camera: &'cont mut Camera,
    camera_event_queue: &'mutex mut MutexGuard<'event, EventQueue<CameraEvent>>,
}

impl<'event, 'cont, 'mutex> CameraAccessMut<'event, 'cont, 'mutex> {
    pub fn new(
        handle: Handle<Camera>,
        camera: &'cont mut Camera,
        camera_event_queue: &'mutex mut MutexGuard<'event, EventQueue<CameraEvent>>,
    ) -> Self {
        Self {
            handle,
            camera,
            camera_event_queue,
        }
    }
}

impl<'event, 'cont, 'mutex> CameraAccessMut<'event, 'cont, 'mutex> {
    pub fn projection(&self) -> &CameraProjection {
        &self.camera.projection
    }

    pub fn transform(&self) -> &CameraTransform {
        &self.camera.transform
    }

    pub fn set_projection(&mut self, projection: CameraProjection) {
        self.camera.set_projection(projection);
        self.camera_event_queue.push(CameraEvent::SetProjection {
            handle: self.handle.clone(),
            projection: self.camera.projection.clone(),
        });
    }

    pub fn set_transform(&mut self, transform: CameraTransform) {
        self.camera.set_transform(transform);
        self.camera_event_queue.push(CameraEvent::SetTransform {
            handle: self.handle.clone(),
            transform: self.camera.transform.clone(),
        });
    }

    pub fn set_position(&mut self, position: Vector3<f32>) {
        self.camera.set_position(position);
        self.camera_event_queue.push(CameraEvent::SetPosition {
            handle: self.handle.clone(),
            position: self.camera.transform.position,
        });
    }

    pub fn set_forward(&mut self, forward: Vector3<f32>) {
        self.camera.set_forward(forward);
        self.camera_event_queue.push(CameraEvent::SetForward {
            handle: self.handle.clone(),
            forward: self.camera.transform.forward,
        });
    }

    pub fn set_up(&mut self, up: Vector3<f32>) {
        self.camera.set_up(up);
        self.camera_event_queue.push(CameraEvent::SetUp {
            handle: self.handle.clone(),
            up: self.camera.transform.up,
        });
    }
}

pub struct CameraContainerGuard<'event, 'cont> {
    camera_event_queue: MutexGuard<'event, EventQueue<CameraEvent>>,
    cameras: MutexGuard<'cont, IndexingContainer<Camera>>,
}

impl<'event, 'cont> CameraContainerGuard<'event, 'cont> {
    pub fn new(
        camera_event_queue: MutexGuard<'event, EventQueue<CameraEvent>>,
        cameras: MutexGuard<'cont, IndexingContainer<Camera>>,
    ) -> Self {
        Self {
            camera_event_queue,
            cameras,
        }
    }

    /// Inserts the given [`Camera`] into the container and returns a [`Handle`] to it.
    pub fn insert(&mut self, camera: Camera) -> Handle<Camera> {
        let camera2 = camera.clone();
        let handle = self.cameras.insert(camera);
        self.camera_event_queue.push(CameraEvent::Insert {
            handle: handle.clone(),
            camera: camera2,
        });
        handle
    }

    /// Removes the [`Camera`] with the given [`Handle`] from the container and returns it.
    pub fn remove(&mut self, handle: &Handle<Camera>) -> Option<Camera> {
        let camera = self.cameras.remove(handle);
        self.camera_event_queue.push(CameraEvent::Remove { handle: handle.clone() });
        camera
    }

    /// Returns a reference to the [`Camera`] with the given [`Handle`].
    pub fn get(&self, handle: &Handle<Camera>) -> Option<&Camera> {
        self.cameras.get(handle)
    }

    /// Returns a [`CameraAccessMut`] to the [`Camera`] with the given [`Handle`].
    pub fn get_mut<'s>(&'s mut self, handle: &Handle<Camera>) -> Option<CameraAccessMut<'event, '_, 's>> {
        self.cameras
            .get_mut(handle)
            .map(|camera| CameraAccessMut::new(handle.clone(), camera, &mut self.camera_event_queue))
    }
}
