use std::collections::VecDeque;

use glam::Vec3;

use crate::{
    renderer::{MeshInstance, StaticModelMesh},
    ui::{build_box_mesh, build_text_mesh, overlay_instance, sanitize_text, text_width},
    world::camera::Camera,
};

const MAX_CHAT_LINES: usize = 6;
const CHAT_LINE_SCALE: f32 = 0.0085;
const CHAT_PANEL_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 1.56);

#[derive(Debug, Clone)]
struct ChatLine {
    text: String,
    is_system: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ChatState {
    open: bool,
    input: String,
    lines: VecDeque<ChatLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChatVisualState {
    pub(crate) open: bool,
    pub(crate) input: String,
    pub(crate) lines: Vec<(String, bool)>,
}

impl ChatState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn open(&mut self) {
        self.open = true;
        self.input.clear();
    }

    pub(crate) fn open_with_slash(&mut self) {
        self.open = true;
        self.input.clear();
        self.input.push('/');
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.input.clear();
    }

    pub(crate) fn append_text(&mut self, text: &str) {
        if !self.open {
            return;
        }

        for ch in text.chars() {
            if !ch.is_control() && self.input.len() < 72 {
                self.input.push(ch);
            }
        }
    }

    pub(crate) fn backspace(&mut self) {
        if self.open {
            self.input.pop();
        }
    }

    pub(crate) fn submit(&mut self) -> Option<String> {
        if !self.open {
            return None;
        }

        let submitted = self.input.trim().to_string();
        self.open = false;
        self.input.clear();
        if submitted.is_empty() {
            return None;
        }

        self.push_line(format!("> {submitted}"), !submitted.starts_with('/'));
        Some(submitted)
    }

    pub(crate) fn push_system_line(&mut self, text: impl Into<String>) {
        self.push_line(text.into(), true);
    }

    pub(crate) fn visual_state(&self) -> ChatVisualState {
        ChatVisualState {
            open: self.open,
            input: self.input.clone(),
            lines: self
                .lines
                .iter()
                .map(|line| (line.text.clone(), line.is_system))
                .collect(),
        }
    }

    pub(crate) fn build_overlay_templates(&self) -> Vec<StaticModelMesh> {
        if self.lines.is_empty() && !self.open {
            return Vec::new();
        }

        let mut meshes = Vec::new();
        let visible_lines = if self.open { 5 } else { 3 };
        let panel_height = if self.open { 0.26 } else { 0.18 };
        let line_step = if self.open { 0.048 } else { 0.042 };

        meshes.push(build_box_mesh(
            "chat-panel-shell",
            Vec3::new(-0.98, -0.28, -0.03),
            Vec3::new(-0.40, -0.28 + panel_height, 0.03),
            [0.05, 0.06, 0.08, 0.16],
        ));
        meshes.push(build_box_mesh(
            "chat-panel-accent",
            Vec3::new(-0.98, -0.28 + panel_height - 0.02, -0.02),
            Vec3::new(-0.40, -0.28 + panel_height, 0.02),
            [0.72, 0.74, 0.80, 0.06],
        ));

        for (index, line) in self.lines.iter().rev().take(visible_lines).enumerate() {
            let y = -0.06 - index as f32 * line_step;
            let color = if line.is_system {
                [0.98, 0.94, 0.78, 0.90]
            } else {
                [0.84, 0.92, 1.0, 0.90]
            };

            meshes.push(build_text_mesh(
                format!("chat-line-{index}"),
                &sanitize_text(&line.text),
                Vec3::new(-0.95, y, 0.02),
                CHAT_LINE_SCALE,
                color,
            ));
        }

        if self.open {
            let prompt = format!("> {}", self.input);
            let prompt_width = text_width(&sanitize_text(&prompt), CHAT_LINE_SCALE);
            let input_left = -0.95;
            let input_right = (input_left + prompt_width + 0.04).min(-0.44);

            meshes.push(build_box_mesh(
                "chat-input-shell",
                Vec3::new(input_left, -0.26, -0.02),
                Vec3::new(input_right, -0.18, 0.02),
                [0.12, 0.10, 0.08, 0.22],
            ));
            meshes.push(build_text_mesh(
                "chat-input-text",
                &sanitize_text(&prompt),
                Vec3::new(-0.94, -0.21, 0.02),
                CHAT_LINE_SCALE,
                [1.0, 0.98, 0.92, 0.96],
            ));
        }

        meshes
    }

    pub(crate) fn build_overlay_instances(&self, camera: &Camera) -> Vec<MeshInstance> {
        if self.lines.is_empty() && !self.open {
            return Vec::new();
        }

        let mut instances = Vec::new();
        let visible_lines = if self.open { 5 } else { 3 };

        instances.push(overlay_instance("chat-panel-shell", camera, CHAT_PANEL_OFFSET));
        instances.push(overlay_instance(
            "chat-panel-accent",
            camera,
            CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, -0.01),
        ));

        for index in 0..self.lines.len().min(visible_lines) {
            instances.push(overlay_instance(
                format!("chat-line-{index}"),
                camera,
                CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, -0.02),
            ));
        }

        if self.open {
            instances.push(overlay_instance(
                "chat-input-shell",
                camera,
                CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.01),
            ));
            instances.push(overlay_instance(
                "chat-input-text",
                camera,
                CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.015),
            ));
        }

        instances
    }

    fn push_line(&mut self, text: String, is_system: bool) {
        self.lines.push_back(ChatLine { text, is_system });
        while self.lines.len() > MAX_CHAT_LINES {
            let _ = self.lines.pop_front();
        }
    }
}
