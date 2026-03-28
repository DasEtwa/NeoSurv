use std::collections::VecDeque;

use glam::Vec3;

use crate::{
    renderer::{MeshInstance, StaticModelMesh},
    ui::{build_box_mesh, build_text_mesh, overlay_instance, sanitize_text, text_width},
    world::camera::Camera,
};

const MAX_CHAT_LINES: usize = 6;
const CHAT_LINE_SCALE: f32 = 0.0085;
const CHAT_PANEL_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 1.60);

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
        let panel_height = if self.open { 0.30 } else { 0.21 };
        let line_step = if self.open { 0.050 } else { 0.044 };

        meshes.push(build_box_mesh(
            "chat-panel-shadow",
            Vec3::new(-0.99, -0.31, -0.04),
            Vec3::new(-0.33, -0.31 + panel_height, 0.04),
            [0.01, 0.01, 0.02, 0.12],
        ));
        meshes.push(build_box_mesh(
            "chat-panel-shell",
            Vec3::new(-0.96, -0.28, -0.03),
            Vec3::new(-0.36, -0.28 + panel_height, 0.03),
            [0.05, 0.06, 0.08, 0.20],
        ));
        meshes.push(build_box_mesh(
            "chat-panel-rail",
            Vec3::new(-0.96, -0.28 + panel_height - 0.025, -0.01),
            Vec3::new(-0.36, -0.28 + panel_height, 0.01),
            [0.72, 0.74, 0.80, 0.10],
        ));
        meshes.push(build_text_mesh(
            "chat-panel-tag",
            "COMMS",
            Vec3::new(-0.92, -0.28 + panel_height - 0.055, 0.02),
            0.0075,
            [0.96, 0.90, 0.78, 0.78],
        ));

        for (index, line) in self.lines.iter().rev().take(visible_lines).enumerate() {
            let y = -0.06 - index as f32 * line_step;
            let color = if line.is_system {
                [0.98, 0.94, 0.78, 0.92]
            } else {
                [0.84, 0.92, 1.0, 0.92]
            };

            meshes.push(build_text_mesh(
                format!("chat-line-{index}"),
                &sanitize_text(&line.text),
                Vec3::new(-0.92, y, 0.02),
                CHAT_LINE_SCALE,
                color,
            ));
        }

        if self.open {
            let prompt = format!("> {}", self.input);
            let prompt_text = sanitize_text(&prompt);
            let prompt_width = text_width(&prompt_text, CHAT_LINE_SCALE);
            let input_left = -0.92;
            let input_right = (input_left + prompt_width + 0.06).min(-0.40);

            meshes.push(build_box_mesh(
                "chat-input-shell",
                Vec3::new(input_left, -0.26, -0.02),
                Vec3::new(input_right, -0.17, 0.02),
                [0.10, 0.09, 0.08, 0.24],
            ));
            meshes.push(build_box_mesh(
                "chat-input-rail",
                Vec3::new(input_left, -0.19, -0.01),
                Vec3::new(input_right, -0.17, 0.01),
                [0.92, 0.74, 0.42, 0.16],
            ));
            meshes.push(build_text_mesh(
                "chat-input-text",
                &prompt_text,
                Vec3::new(-0.90, -0.205, 0.02),
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

        instances.push(overlay_instance("chat-panel-shadow", camera, CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, -0.03)));
        instances.push(overlay_instance("chat-panel-shell", camera, CHAT_PANEL_OFFSET));
        instances.push(overlay_instance("chat-panel-rail", camera, CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.01)));
        instances.push(overlay_instance("chat-panel-tag", camera, CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.02)));

        for index in 0..self.lines.len().min(visible_lines) {
            instances.push(overlay_instance(
                format!("chat-line-{index}"),
                camera,
                CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.02),
            ));
        }

        if self.open {
            instances.push(overlay_instance(
                "chat-input-shell",
                camera,
                CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.01),
            ));
            instances.push(overlay_instance(
                "chat-input-rail",
                camera,
                CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.015),
            ));
            instances.push(overlay_instance(
                "chat-input-text",
                camera,
                CHAT_PANEL_OFFSET + Vec3::new(0.0, 0.0, 0.02),
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
