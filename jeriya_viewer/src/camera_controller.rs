use std::time::Duration;

use color_eyre as ey;
use ey::eyre::ContextCompat;
use jeriya_backend::{
    instances::{
        camera_instance::{CameraInstance, CameraTransform},
        instance_group::InstanceGroup,
    },
    transactions::PushEvent,
};
use jeriya_shared::{
    nalgebra::{Vector2, Vector3},
    Handle,
};

pub struct CameraController {
    is_dirty: bool,

    theta: f32,
    phi: f32,
    r: f32,

    rotate_theta_speed: f32,
    rotate_phi_speed: f32,
    zoom_speed: f32,

    is_rotating_right: bool,
    is_rotating_left: bool,
    is_rotating_up: bool,
    is_rotating_down: bool,
    is_zooming_in: bool,
    is_zooming_out: bool,

    cursor_position: Vector2<f32>,
}

impl CameraController {
    /// Create a new camera controller and a camera.
    pub fn new(rotate_theta_speed: f32, rotate_phi_speed: f32, zoom_speed: f32) -> Self {
        Self {
            is_dirty: true,
            theta: std::f32::consts::FRAC_PI_2,
            phi: 0.0,
            r: 1.0,
            rotate_theta_speed,
            rotate_phi_speed,
            zoom_speed,
            is_rotating_right: false,
            is_rotating_left: false,
            is_rotating_up: false,
            is_rotating_down: false,
            is_zooming_in: false,
            is_zooming_out: false,
            cursor_position: Vector2::zeros(),
        }
    }

    pub fn set_rotating_right(&mut self, is_rotating_right: bool) {
        self.is_rotating_right = is_rotating_right;
        self.is_dirty = true;
    }

    pub fn set_rotating_left(&mut self, is_rotating_left: bool) {
        self.is_rotating_left = is_rotating_left;
        self.is_dirty = true;
    }

    pub fn set_rotating_up(&mut self, is_rotating_up: bool) {
        self.is_rotating_up = is_rotating_up;
        self.is_dirty = true;
    }

    pub fn set_rotating_down(&mut self, is_rotating_down: bool) {
        self.is_rotating_down = is_rotating_down;
        self.is_dirty = true;
    }

    pub fn set_zooming_in(&mut self, is_zooming_in: bool) {
        self.is_zooming_in = is_zooming_in;
        self.is_dirty = true;
    }

    pub fn set_zooming_out(&mut self, is_zooming_out: bool) {
        self.is_zooming_out = is_zooming_out;
        self.is_dirty = true;
    }

    /// Rotate the camera around the Y axis.
    pub fn rotate_right(&mut self, delta: f32) {
        self.phi += delta;
        self.is_dirty = true;
    }

    /// Rotate the camera around the local X axis.
    pub fn rotate_up(&mut self, delta: f32) {
        self.theta = (self.theta - delta).max(0.0).min(std::f32::consts::PI);
        self.is_dirty = true;
    }

    /// Zoom the camera out.
    pub fn zoom_out(&mut self, delta: f32) {
        self.r = (self.r + self.zoom_speed * delta).max(0.1);
        self.is_dirty = true;
    }

    /// Update the camera's position and rotation.
    pub fn update(
        &mut self,
        dt: Duration,
        transaction: &mut impl PushEvent,
        instance_group: &mut InstanceGroup,
        camera_instance_handle: Handle<CameraInstance>,
    ) -> ey::Result<()> {
        if !self.is_dirty {
            return Ok(());
        }

        println!("r={} theta={} phi={}", self.r, self.theta, self.phi);

        if self.is_rotating_right {
            self.rotate_right(-self.rotate_theta_speed * dt.as_secs_f32());
        }
        if self.is_rotating_left {
            self.rotate_right(self.rotate_theta_speed * dt.as_secs_f32());
        }
        if self.is_rotating_up {
            self.rotate_up(self.rotate_phi_speed * dt.as_secs_f32());
        }
        if self.is_rotating_down {
            self.rotate_up(-self.rotate_phi_speed * dt.as_secs_f32());
        }
        if self.is_zooming_in {
            self.zoom_out(-dt.as_secs_f32());
        }
        if self.is_zooming_out {
            self.zoom_out(dt.as_secs_f32());
        }

        let camera = instance_group
            .camera_instances()
            .get_mut(&camera_instance_handle)
            .wrap_err("Failed to find camera instance")?;
        let position = Vector3::new(
            self.r * self.theta.sin() * self.phi.cos(),
            self.r * self.theta.cos(),
            self.r * self.theta.sin() * self.phi.sin(),
        );
        let forward = -position;
        let up = Vector3::y();
        let camera_transform = CameraTransform::new(position, forward, up);
        camera.mutate_via(transaction).set_transform(camera_transform);
        Ok(())
    }
}
