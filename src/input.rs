use glam::Vec2;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct InputState {
    pub offset: Vec2,
    speed: f32,
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    randomize: bool,
}

impl InputState {
    pub fn new(speed: f32) -> Self {
        Self {
            offset: Vec2::ZERO,
            speed,
            w: false,
            a: false,
            s: false,
            d: false,
            randomize: false,
        }
    }

    pub fn handle_key(&mut self, event: &KeyEvent) -> bool {
        let pressed = matches!(event.state, ElementState::Pressed);
        match event.physical_key {
            PhysicalKey::Code(KeyCode::KeyW) => self.w = pressed,
            PhysicalKey::Code(KeyCode::KeyA) => self.a = pressed,
            PhysicalKey::Code(KeyCode::KeyS) => self.s = pressed,
            PhysicalKey::Code(KeyCode::KeyD) => self.d = pressed,
            PhysicalKey::Code(KeyCode::KeyR) if pressed => {
                self.randomize = true;
            }
            _ => return false,
        }
        true
    }

    pub fn update(&mut self, dt: f32) {
        let mut dir = Vec2::ZERO;
        if self.w {
            dir.y += 1.0;
        }
        if self.s {
            dir.y -= 1.0;
        }
        if self.a {
            dir.x -= 1.0;
        }
        if self.d {
            dir.x += 1.0;
        }

        if dir != Vec2::ZERO {
            let delta = dir.normalize_or_zero() * self.speed * dt;
            self.offset += delta;
        }
    }

    pub fn take_randomize(&mut self) -> bool {
        let r = self.randomize;
        self.randomize = false;
        r
    }
}
