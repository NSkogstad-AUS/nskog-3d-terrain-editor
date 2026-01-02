use glam::{Vec2, Vec3};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::terrain::WORLD_RADIUS;

pub struct InputState {
    pub position: Vec3,
    yaw: f32,
    pitch: f32,
    orbit_radius: f32,
    orbit_speed: f32,
    shift: bool,
    pub active: bool,
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    randomize: bool,
    last_cursor: Option<Vec2>,
    sensitivity: f32,
}

impl InputState {
    pub fn new(speed: f32) -> Self {
        let position = Vec3::new(0.0, WORLD_RADIUS * 0.8, WORLD_RADIUS * 1.8);
        let forward_to_origin = (Vec3::ZERO - position).normalize();
        let yaw = forward_to_origin.x.atan2(forward_to_origin.z);
        let pitch = forward_to_origin.y.asin();

        Self {
            position,
            yaw,
            pitch,
            orbit_radius: position.length(),
            orbit_speed: speed,
            shift: false,
            active: true,
            w: false,
            a: false,
            s: false,
            d: false,
            randomize: false,
            last_cursor: None,
            sensitivity: 0.0025,
        }
    }

    pub fn handle_key(&mut self, event: &KeyEvent) -> bool {
        let pressed = matches!(event.state, ElementState::Pressed);
        match event.physical_key {
            PhysicalKey::Code(KeyCode::KeyW) => {
                self.w = pressed;
                self.active = true;
            }
            PhysicalKey::Code(KeyCode::KeyA) => {
                self.a = pressed;
                self.active = true;
            }
            PhysicalKey::Code(KeyCode::KeyS) => {
                self.s = pressed;
                self.active = true;
            }
            PhysicalKey::Code(KeyCode::KeyD) => {
                self.d = pressed;
                self.active = true;
            }
            PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => self.shift = pressed,
            PhysicalKey::Code(KeyCode::KeyR) if pressed => {
                self.randomize = true;
            }
            _ => return false,
        }
        true
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active {
            return;
        }

        let mut yaw_delta = 0.0;
        let mut pitch_delta = 0.0;
        if self.w {
            pitch_delta += 1.0;
        }
        if self.s {
            pitch_delta -= 1.0;
        }
        if self.a {
            yaw_delta += 1.0;
        }
        if self.d {
            yaw_delta -= 1.0;
        }

        if yaw_delta != 0.0 || pitch_delta != 0.0 {
            let speed = if self.shift {
                self.orbit_speed * 2.5
            } else {
                self.orbit_speed
            };
            self.yaw += yaw_delta * speed * dt;
            self.pitch = (self.pitch + pitch_delta * speed * dt).clamp(-1.4, 1.4);
        }

        self.position = -self.forward() * self.orbit_radius;
    }

    pub fn handle_cursor_move(&mut self, pos: Vec2) {
        if !self.active {
            self.last_cursor = Some(pos);
            return;
        }
        if let Some(last) = self.last_cursor {
            let delta = pos - last;
            self.yaw -= delta.x * self.sensitivity;
            self.pitch = (self.pitch - delta.y * self.sensitivity).clamp(-1.4, 1.4);
        }
        self.last_cursor = Some(pos);
    }

    pub fn handle_mouse_delta(&mut self, delta: (f64, f64)) {
        if !self.active {
            return;
        }
        self.yaw -= delta.0 as f32 * self.sensitivity;
        self.pitch = (self.pitch - delta.1 as f32 * self.sensitivity).clamp(-1.4, 1.4);
    }

    pub fn forward(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), self.yaw.cos() * cp).normalize()
    }

    pub fn take_randomize(&mut self) -> bool {
        let r = self.randomize;
        self.randomize = false;
        r
    }
}
