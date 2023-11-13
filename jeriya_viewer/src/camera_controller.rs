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

#[derive(Debug, Clone)]
pub struct Config {
    /// Speed of the camera's vertical rotation around using the keyboard.
    pub rotate_theta_speed_keyboard: f32,
    /// Speed of the camera's vertical rotation around using the mouse.
    pub rotate_theta_speed_mouse_cursor: f32,
    /// Speed of the camera's horizontal rotation around using the keyboard.
    pub rotate_phi_speed_keyboard: f32,
    /// Speed of the camera's horizontal rotation around using the mouse.
    pub rotate_phi_speed_mouse_cursor: f32,
    /// Speed of the camera's zoom using the mouse.
    pub zoom_speed_mouse_wheel: f32,
    /// Speed of the camera's zoom using the keyboard.
    pub zoom_speed_keyboard: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rotate_theta_speed_keyboard: 1.0,
            rotate_theta_speed_mouse_cursor: 1.0,
            rotate_phi_speed_keyboard: 1.0,
            rotate_phi_speed_mouse_cursor: 1.0,
            zoom_speed_mouse_wheel: 1.0,
            zoom_speed_keyboard: 10.0,
        }
    }
}

pub struct CameraController {
    is_dirty: bool,

    config: Config,

    theta: f32,
    phi: f32,
    r: f32,

    is_rotating_right: bool,
    is_rotating_left: bool,
    is_rotating_up: bool,
    is_rotating_down: bool,
    is_zooming_in: bool,
    is_zooming_out: bool,

    cursor_position: Vector2<f32>,
    cursor_position_on_last_update: Vector2<f32>,
    is_cursor_rotation_active: bool,
}

impl CameraController {
    /// Create a new camera controller and a camera.
    pub fn new(config: Config) -> Self {
        Self {
            is_dirty: true,
            config,
            theta: std::f32::consts::FRAC_PI_2 / 2.0,
            phi: 3.0 * std::f32::consts::FRAC_PI_2 / 2.0,
            r: 3.0,
            is_rotating_right: false,
            is_rotating_left: false,
            is_rotating_up: false,
            is_rotating_down: false,
            is_zooming_in: false,
            is_zooming_out: false,
            cursor_position: Vector2::zeros(),
            cursor_position_on_last_update: Vector2::zeros(),
            is_cursor_rotation_active: false,
        }
    }

    /// Determine whether the current change in the cursor position should be considered as a rotation.
    pub fn set_cursor_rotation_active(&mut self, is_cursor_rotation_active: bool) {
        self.is_cursor_rotation_active = is_cursor_rotation_active;
        self.is_dirty = true;
    }

    /// Set the cursor position.
    pub fn set_cursor_position(&mut self, cursor_position: Vector2<f32>) {
        self.cursor_position = cursor_position;
        self.is_dirty = true;
    }

    /// Determine whether the camera is rotating around the Y axis.
    pub fn set_rotating_right(&mut self, is_rotating_right: bool) {
        self.is_rotating_right = is_rotating_right;
        self.is_dirty = true;
    }

    /// Determine whether the camera is rotating around the Y axis.
    pub fn set_rotating_left(&mut self, is_rotating_left: bool) {
        self.is_rotating_left = is_rotating_left;
        self.is_dirty = true;
    }

    /// Determine whether the camera is rotating around the X axis.
    pub fn set_rotating_up(&mut self, is_rotating_up: bool) {
        self.is_rotating_up = is_rotating_up;
        self.is_dirty = true;
    }

    /// Determine whether the camera is rotating around the X axis.
    pub fn set_rotating_down(&mut self, is_rotating_down: bool) {
        self.is_rotating_down = is_rotating_down;
        self.is_dirty = true;
    }

    /// Determine whether the camera is zooming in.
    pub fn set_zooming_in(&mut self, is_zooming_in: bool) {
        self.is_zooming_in = is_zooming_in;
        self.is_dirty = true;
    }

    /// Determine whether the camera is zooming out.    
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
        let epsilon = 0.001;
        self.theta = (self.theta - delta).max(epsilon).min(std::f32::consts::PI - epsilon);
        self.is_dirty = true;
    }

    /// Zoom the camera out.
    pub fn zoom_out(&mut self, delta: f32) {
        self.r = (self.r + self.config.zoom_speed_mouse_wheel * delta).max(0.1);
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

        // Rotate the camera based on the cursor's movement.
        let cursor_delta = self.cursor_position - self.cursor_position_on_last_update;
        self.cursor_position_on_last_update = self.cursor_position;
        if self.is_cursor_rotation_active {
            self.rotate_right(cursor_delta.x * self.config.rotate_theta_speed_mouse_cursor * dt.as_secs_f32());
            self.rotate_up(cursor_delta.y * self.config.rotate_phi_speed_mouse_cursor * dt.as_secs_f32());
        }

        // Rotate the camera based on the keyboard input.
        if self.is_rotating_right {
            self.rotate_right(-self.config.rotate_theta_speed_keyboard * dt.as_secs_f32());
        }
        if self.is_rotating_left {
            self.rotate_right(self.config.rotate_theta_speed_keyboard * dt.as_secs_f32());
        }
        if self.is_rotating_up {
            self.rotate_up(self.config.rotate_phi_speed_keyboard * dt.as_secs_f32());
        }
        if self.is_rotating_down {
            self.rotate_up(-self.config.rotate_phi_speed_keyboard * dt.as_secs_f32());
        }
        if self.is_zooming_in {
            self.zoom_out(-self.config.zoom_speed_keyboard * dt.as_secs_f32());
        }
        if self.is_zooming_out {
            self.zoom_out(self.config.zoom_speed_keyboard * dt.as_secs_f32());
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
