use jeriya_shared::{
    debug_info,
    nalgebra::{Matrix4, Vector3},
    thiserror, DebugInfo, Handle,
};

use crate::{elements::camera::Camera, gpu_index_allocator::GpuIndexAllocation};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The Camera of the CameraInstance is not set")]
    CameraNotSet,
    #[error("The allocation of the CameraInstance failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Event {
    Noop,
    Insert(CameraInstance),
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
pub struct CameraInstance {
    camera_handle: Handle<Camera>,
    camera_gpu_index_allocation: GpuIndexAllocation<Camera>,
    handle: Handle<CameraInstance>,
    gpu_index_allocation: GpuIndexAllocation<CameraInstance>,
    transform: CameraTransform,
    debug_info: DebugInfo,
}

impl CameraInstance {
    pub fn builder() -> CameraInstanceBuilder {
        CameraInstanceBuilder::default()
    }

    /// Returns the [`Handle`] of the [`Camera`] that this [`CameraInstance`] is an instance of.
    pub fn camera_handle(&self) -> &Handle<Camera> {
        &self.camera_handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`Camera`] that this [`CameraInstance`] is an instance of.
    pub fn camera_gpu_index_allocation(&self) -> &GpuIndexAllocation<Camera> {
        &self.camera_gpu_index_allocation
    }

    /// Returns the [`Handle`] of the [`CameraInstance`]
    pub fn handle(&self) -> &Handle<CameraInstance> {
        &self.handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`CameraInstance`]
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<CameraInstance> {
        &self.gpu_index_allocation
    }

    /// Returns the transform of the [`CameraInstance`]
    pub fn transform(&self) -> &CameraTransform {
        &self.transform
    }

    /// Returns the [`DebugInfo`] of the [`CameraInstance`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[derive(Default)]
pub struct CameraInstanceBuilder {
    camera_handle: Option<Handle<Camera>>,
    camera_gpu_index_allocation: Option<GpuIndexAllocation<Camera>>,
    transform: Option<CameraTransform>,
    debug_info: Option<DebugInfo>,
}

impl CameraInstanceBuilder {
    /// Creates a new [`CameraInstanceBuilder`].
    pub fn with_camera(mut self, camera: &Camera) -> Self {
        self.camera_handle = Some(camera.handle().clone());
        self.camera_gpu_index_allocation = Some(camera.gpu_index_allocation().clone());
        self
    }

    /// Sets the [`CameraTransform`] of the [`CameraInstance`].
    pub fn with_transform(mut self, transform: CameraTransform) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Sets the [`DebugInfo`] of the [`CameraInstance`].
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`CameraInstance`].
    pub(crate) fn build(
        self,
        handle: Handle<CameraInstance>,
        gpu_index_allocation: GpuIndexAllocation<CameraInstance>,
    ) -> Result<CameraInstance> {
        let camera_handle = self.camera_handle.ok_or(Error::CameraNotSet)?;
        let camera_gpu_index_allocation = self.camera_gpu_index_allocation.ok_or(Error::AllocationFailed)?;
        Ok(CameraInstance {
            camera_handle,
            camera_gpu_index_allocation,
            handle,
            gpu_index_allocation,
            transform: self.transform.unwrap_or_default(),
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous CameraInstance")),
        })
    }
}
