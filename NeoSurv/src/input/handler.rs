use std::collections::HashSet;

use glam::{Vec2, Vec3};
use winit::{
    event::{DeviceEvent, ElementState, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Debug, Default)]
pub(crate) struct InputHandler {
    pressed_keys: HashSet<KeyCode>,
    just_pressed_keys: HashSet<KeyCode>,
    mouse_delta: Vec2,
    typed_text: String,
}

impl InputHandler {
    pub(crate) fn clear(&mut self) {
        self.pressed_keys.clear();
        self.just_pressed_keys.clear();
        self.mouse_delta = Vec2::ZERO;
        self.typed_text.clear();
    }

    pub(crate) fn handle_window_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if event.state == ElementState::Pressed
                && let Some(text) = &event.text
            {
                for ch in text.chars() {
                    if !ch.is_control() {
                        self.typed_text.push(ch);
                    }
                }
            }

            if let PhysicalKey::Code(code) = event.physical_key {
                match event.state {
                    ElementState::Pressed => {
                        if self.pressed_keys.insert(code) {
                            self.just_pressed_keys.insert(code);
                        }
                    }
                    ElementState::Released => {
                        self.pressed_keys.remove(&code);
                        self.just_pressed_keys.remove(&code);
                    }
                }
            }
        }
    }

    pub(crate) fn handle_device_event(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.mouse_delta += Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    pub(crate) fn consume_key_press(&mut self, key: KeyCode) -> bool {
        self.just_pressed_keys.remove(&key)
    }

    pub(crate) fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    pub(crate) fn frame_movement_axis(&self) -> Vec3 {
        let mut dir = Vec3::ZERO;

        if self.pressed_keys.contains(&KeyCode::KeyW) {
            dir.z -= 1.0;
        }
        if self.pressed_keys.contains(&KeyCode::KeyS) {
            dir.z += 1.0;
        }
        if self.pressed_keys.contains(&KeyCode::KeyA) {
            dir.x -= 1.0;
        }
        if self.pressed_keys.contains(&KeyCode::KeyD) {
            dir.x += 1.0;
        }

        if dir.length_squared() > 0.0 {
            dir.normalize()
        } else {
            dir
        }
    }

    pub(crate) fn take_mouse_delta(&mut self) -> Vec2 {
        let delta = self.mouse_delta;
        self.mouse_delta = Vec2::ZERO;
        delta
    }

    pub(crate) fn take_typed_text(&mut self) -> String {
        std::mem::take(&mut self.typed_text)
    }
}
