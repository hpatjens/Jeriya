use std::f32::consts::PI;

use crate::{
    nalgebra::{Matrix4, Vector3},
    nalgebra_glm,
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
