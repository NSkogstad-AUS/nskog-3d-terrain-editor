use glam::{Vec2, Vec3};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::terrain::WORLD_RADIUS;

pub struct InputState {
    pub position: Vec3,
    yaw: f32,
    pitch: f32,
    base_speed: f32,
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
            base_speed: speed,
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

        let mut dir = Vec3::ZERO;
        if self.w {
            dir += self.forward();
        }
        if self.s {
            dir -= self.forward();
        }
        if self.a {
            dir -= self.right();
        }
        if self.d {
            dir += self.right();
        }

        if dir != Vec3::ZERO {
            let speed = if self.shift {
                self.base_speed * 3.0
            } else {
                self.base_speed
            };
            let delta = dir.normalize_or_zero() * speed * dt;
            self.position += delta;
        }
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

    fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize_or_zero()
    }

    pub fn take_randomize(&mut self) -> bool {
        let r = self.randomize;
        self.randomize = false;
        r
    }
}
