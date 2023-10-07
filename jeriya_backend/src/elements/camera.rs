use jeriya_shared::{
    debug_info,
    nalgebra::{Matrix4, Vector3},
    nalgebra_glm, thiserror, DebugInfo, Handle,
};

use crate::{
    gpu_index_allocator::GpuIndexAllocation,
    transactions::{self, PushEvent},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The allocation of the Camera failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Event {
    Noop,
    Insert(Camera),
    UpdateProjection(GpuIndexAllocation<Camera>, Matrix4<f32>),
    UpdateView(GpuIndexAllocation<Camera>, Matrix4<f32>),
}

/// Type of projection for a camera.
#[derive(Debug, Clone, PartialEq)]
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
        Self::Orthographic {
            left: -1.0,
            right: 1.0,
            bottom: 1.0,
            top: -1.0,
            near: 0.0,
            far: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone)]
pub struct Camera {
    projection: CameraProjection,
    transform: CameraTransform,
    cached_projection_matrix: Matrix4<f32>,
    cached_view_matrix: Matrix4<f32>,
    cached_matrix: Matrix4<f32>,
    debug_info: DebugInfo,
    handle: Handle<Camera>,
    gpu_index_allocation: GpuIndexAllocation<Camera>,
}

impl Camera {
    /// Creates a new [`CameraBuilder`] for a [`Camera`].
    pub fn builder() -> CameraBuilder {
        CameraBuilder::default()
    }

    /// Returns the [`CameraProjection`] of the camera.
    pub fn projection(&self) -> &CameraProjection {
        &self.projection
    }

    /// Returns the [`CameraTransform`] of the camera.
    pub fn transform(&self) -> &CameraTransform {
        &self.transform
    }

    /// Returns the projection matrix of the camera.
    pub fn projection_matrix(&self) -> Matrix4<f32> {
        self.cached_projection_matrix
    }

    /// Returns the view matrix of the camera.
    pub fn view_matrix(&self) -> Matrix4<f32> {
        self.cached_view_matrix
    }

    /// Returns the view-projection matrix of the camera.
    pub fn matrix(&self) -> Matrix4<f32> {
        self.cached_matrix
    }

    /// Returns the [`DebugInfo`] of the [`Camera`].
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    /// Returns the [`Handle`] of the [`Camera`].
    pub fn handle(&self) -> &Handle<Camera> {
        &self.handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`Camera`].
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<Camera> {
        &self.gpu_index_allocation
    }
}

pub struct CameraAccessMut<'g, 't, P: PushEvent> {
    camera: &'g mut Camera,
    transaction: &'t mut P,
}

impl<'g, 't, P: PushEvent> CameraAccessMut<'g, 't, P> {
    /// Creates a new [`CameraAccessMut`] for a [`Camera`].
    pub fn new(camera: &'g mut Camera, transaction: &'t mut P) -> Self {
        Self { camera, transaction }
    }

    /// Sets the [`CameraProjection`] of the [`Camera`].
    pub fn set_projection(&mut self, projection: CameraProjection) {
        self.camera.projection = projection;
        self.camera.cached_projection_matrix = self.camera.projection.projection_matrix();
        self.camera.cached_matrix = self.camera.cached_projection_matrix * self.camera.cached_view_matrix;
        self.transaction.push_event(transactions::Event::Camera(Event::UpdateProjection(
            self.camera.gpu_index_allocation.clone(),
            self.camera.cached_projection_matrix,
        )))
    }

    /// Sets the [`CameraTransform`] of the [`Camera`].
    pub fn set_transform(&mut self, transform: CameraTransform) {
        self.camera.transform = transform;
        self.camera.cached_view_matrix = self.camera.transform.view_matrix();
        self.camera.cached_matrix = self.camera.cached_projection_matrix * self.camera.cached_view_matrix;
        self.transaction.push_event(transactions::Event::Camera(Event::UpdateView(
            self.camera.gpu_index_allocation.clone(),
            self.camera.cached_view_matrix,
        )))
    }
}

#[derive(Default)]
pub struct CameraBuilder {
    projection: Option<CameraProjection>,
    transform: Option<CameraTransform>,
    debug_info: Option<DebugInfo>,
}

impl CameraBuilder {
    /// Sets the [`CameraProjection`] of the [`Camera`].
    pub fn with_projection(mut self, projection: CameraProjection) -> Self {
        self.projection = Some(projection);
        self
    }

    /// Sets the [`CameraTransform`] of the [`Camera`].
    pub fn with_transform(mut self, transform: CameraTransform) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Sets the [`DebugInfo`] of the [`Camera`].
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    pub(crate) fn build(self, handle: Handle<Camera>, gpu_index_allocation: GpuIndexAllocation<Camera>) -> Result<Camera> {
        let projection = self.projection.unwrap_or_default();
        let transform = self.transform.unwrap_or_default();
        let cached_projection_matrix = projection.projection_matrix();
        let cached_view_matrix = transform.view_matrix();
        Ok(Camera {
            projection,
            transform,
            cached_projection_matrix,
            cached_view_matrix,
            cached_matrix: cached_projection_matrix * cached_view_matrix,
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous Camera")),
            handle,
            gpu_index_allocation,
        })
    }
}

#[cfg(test)]
mod tests {
    use jeriya_test::spectral::assert_that;

    use super::*;

    #[test]
    fn default() {
        let camera = Camera::builder()
            .build(Handle::zero(), GpuIndexAllocation::new_unchecked(0))
            .unwrap();
        assert_that!(camera.projection()).is_equal_to(&CameraProjection::Orthographic {
            left: -1.0,
            right: 1.0,
            bottom: 1.0,
            top: -1.0,
            near: 0.0,
            far: 1.0,
        });
        assert_that!(camera.transform()).is_equal_to(&CameraTransform {
            position: Vector3::new(0.0, 0.0, 0.0),
            forward: Vector3::new(0.0, 0.0, 1.0),
            up: Vector3::new(0.0, -1.0, 0.0),
        });
        let view = nalgebra_glm::ortho_rh_zo(-1.0, 1.0, 1.0, -1.0, 0.0, 1.0);
        let projection = Matrix4::look_at_rh(
            &Vector3::new(0.0, 0.0, 0.0).into(),
            &Vector3::new(0.0, 0.0, 1.0).into(),
            &Vector3::new(0.0, -1.0, 0.0),
        );
        assert_that!(camera.projection_matrix()).is_equal_to(&projection);
        assert_that!(camera.view_matrix()).is_equal_to(&view);
        assert_that!(camera.matrix()).is_equal_to(&view * projection);
    }

    #[test]
    fn smoke() {
        let camera = Camera::builder()
            .with_debug_info(debug_info!("my_camera"))
            .with_projection(CameraProjection::Perspective {
                fov: 90.0,
                aspect: 1.2,
                near: 0.1,
                far: 100.0,
            })
            .with_transform(CameraTransform {
                position: Vector3::new(1.0, 2.0, 3.0),
                forward: Vector3::new(4.0, 5.0, 6.0),
                up: Vector3::new(0.0, 1.0, 0.0),
            })
            .build(Handle::zero(), GpuIndexAllocation::new_unchecked(0))
            .unwrap();
        assert_that!(camera.projection()).is_equal_to(&CameraProjection::Perspective {
            fov: 90.0,
            aspect: 1.2,
            near: 0.1,
            far: 100.0,
        });
        assert_that!(camera.transform()).is_equal_to(&CameraTransform {
            position: Vector3::new(1.0, 2.0, 3.0),
            forward: Vector3::new(4.0, 5.0, 6.0),
            up: Vector3::new(0.0, 1.0, 0.0),
        });
        let view = Matrix4::look_at_rh(
            &Vector3::new(1.0, 2.0, 3.0).into(),
            &Vector3::new(5.0, 7.0, 9.0).into(),
            &Vector3::new(0.0, 1.0, 0.0),
        );
        let projection = nalgebra_glm::perspective_rh_zo(1.2, 90.0, 0.1, 100.0);
        assert_that!(camera.projection_matrix()).is_equal_to(&projection);
        assert_that!(camera.view_matrix()).is_equal_to(&view);
        assert_that!(camera.matrix()).is_equal_to(&projection * view);
    }
}
