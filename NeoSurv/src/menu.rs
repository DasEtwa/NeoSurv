use glam::Vec3;

use crate::{
    renderer::{MeshInstance, StaticModelMesh},
    ui::{build_box_mesh, build_text_mesh, overlay_instance, text_width},
    world::camera::Camera,
};

pub(crate) enum MenuCommand {
    PlaySelectedWorld,
    SelectPreviousWorld,
    SelectNextWorld,
    CreateWorld,
    SaveWorld,
    Quit,
}

#[derive(Debug)]
pub(crate) struct StartMenuState {
    selected_button: usize,
}

impl StartMenuState {
    pub(crate) fn new() -> Self {
        Self { selected_button: 0 }
    }

    pub(crate) fn move_selection_up(&mut self) {
        self.selected_button = self.selected_button.saturating_sub(1);
    }

    pub(crate) fn move_selection_down(&mut self) {
        self.selected_button = (self.selected_button + 1).min(MENU_BUTTON_COUNT - 1);
    }

    pub(crate) fn activate_selected(&self) -> MenuCommand {
        match self.selected_button {
            0 => MenuCommand::PlaySelectedWorld,
            1 => MenuCommand::CreateWorld,
            2 => MenuCommand::SaveWorld,
            _ => MenuCommand::Quit,
        }
    }

    pub(crate) fn selected_button(&self) -> usize {
        self.selected_button
    }

    pub(crate) fn build_templates(&self, world_name: &str) -> Vec<StaticModelMesh> {
        let mut meshes = Vec::new();
        let world_name = sanitize_menu_text(world_name);
        let title = "TOKENBURNER";
        let subtitle = "SURVIVAL PROTOTYPE";

        meshes.push(build_box_mesh(
            "menu-shell-shadow",
            Vec3::new(-0.54, -0.42, -0.04),
            Vec3::new(0.56, 0.42, 0.04),
            [0.01, 0.01, 0.02, 0.12],
        ));
        meshes.push(build_box_mesh(
            "menu-shell",
            Vec3::new(-0.50, -0.38, -0.03),
            Vec3::new(0.52, 0.38, 0.03),
            [0.05, 0.06, 0.08, 0.22],
        ));
        meshes.push(build_box_mesh(
            "menu-shell-rail",
            Vec3::new(-0.50, 0.32, -0.01),
            Vec3::new(0.52, 0.38, 0.01),
            [0.94, 0.82, 0.50, 0.16],
        ));
        meshes.push(build_box_mesh(
            "menu-side-bar",
            Vec3::new(-0.50, -0.38, -0.01),
            Vec3::new(-0.42, 0.38, 0.01),
            [0.12, 0.11, 0.10, 0.22],
        ));

        meshes.push(build_text_mesh(
            "menu-title",
            title,
            Vec3::new(-text_width(title, 0.014) * 0.5, 0.255, 0.02),
            0.014,
            [0.98, 0.96, 0.86, 0.98],
        ));
        meshes.push(build_text_mesh(
            "menu-subtitle",
            subtitle,
            Vec3::new(-text_width(subtitle, 0.0065) * 0.5, 0.205, 0.02),
            0.0065,
            [0.84, 0.88, 0.94, 0.70],
        ));

        meshes.push(build_box_mesh(
            "menu-world-row",
            Vec3::new(-0.32, 0.08, -0.02),
            Vec3::new(0.36, 0.18, 0.02),
            [0.08, 0.09, 0.11, 0.20],
        ));
        meshes.push(build_box_mesh(
            "menu-world-row-rail",
            Vec3::new(-0.32, 0.15, -0.01),
            Vec3::new(0.36, 0.18, 0.01),
            [0.56, 0.74, 0.96, 0.18],
        ));
        meshes.push(build_text_mesh(
            "menu-world-label",
            "WORLD",
            Vec3::new(-0.28, 0.145, 0.02),
            0.0085,
            [0.82, 0.86, 0.92, 0.84],
        ));
        meshes.push(build_text_mesh(
            "menu-world-name",
            &world_name,
            Vec3::new(-text_width(&world_name, 0.0105) * 0.5, 0.112, 0.02),
            0.0105,
            [0.98, 0.98, 0.92, 0.94],
        ));
        meshes.push(build_text_mesh(
            "menu-world-nav-left",
            "<",
            Vec3::new(-0.29, 0.112, 0.02),
            0.010,
            [0.96, 0.82, 0.48, 0.84],
        ));
        meshes.push(build_text_mesh(
            "menu-world-nav-right",
            ">",
            Vec3::new(0.32, 0.112, 0.02),
            0.010,
            [0.96, 0.82, 0.48, 0.84],
        ));

        for (index, (label, local_y)) in [
            ("PLAY", -0.01),
            ("NEW WORLD", -0.11),
            ("SAVE WORLD", -0.21),
            ("QUIT", -0.31),
        ]
        .into_iter()
        .enumerate()
        {
            let is_selected = self.selected_button == index;
            let panel_color = if is_selected {
                [0.16, 0.14, 0.12, 0.38]
            } else {
                [0.07, 0.08, 0.10, 0.20]
            };
            let rail_color = if is_selected {
                [0.98, 0.86, 0.62, 0.72]
            } else {
                [0.92, 0.92, 0.96, 0.12]
            };
            let text_color = if is_selected {
                [1.0, 0.98, 0.88, 0.98]
            } else {
                [0.88, 0.90, 0.94, 0.84]
            };

            meshes.push(build_box_mesh(
                format!("menu-button-{index}"),
                Vec3::new(-0.28, local_y - 0.045, -0.02),
                Vec3::new(0.34, local_y + 0.028, 0.02),
                panel_color,
            ));
            meshes.push(build_box_mesh(
                format!("menu-button-{index}-rail"),
                Vec3::new(-0.28, local_y - 0.045, -0.01),
                Vec3::new(-0.22, local_y + 0.028, 0.01),
                rail_color,
            ));
            meshes.push(build_box_mesh(
                format!("menu-button-{index}-marker"),
                Vec3::new(-0.20, local_y - 0.010, -0.008),
                Vec3::new(-0.17, local_y + 0.005, 0.008),
                if is_selected {
                    [0.98, 0.92, 0.74, 0.90]
                } else {
                    [0.30, 0.32, 0.36, 0.20]
                },
            ));
            meshes.push(build_text_mesh(
                format!("menu-button-{index}-text"),
                label,
                Vec3::new(-text_width(label, 0.0115) * 0.5 + 0.04, local_y, 0.02),
                0.0115,
                text_color,
            ));
        }

        let footer = "W/S SELECT   A/D WORLD   ENTER CONFIRM";
        meshes.push(build_text_mesh(
            "menu-footer",
            footer,
            Vec3::new(-text_width(footer, 0.0075) * 0.5, -0.40, 0.02),
            0.0075,
            [0.84, 0.88, 0.94, 0.66],
        ));

        meshes
    }

    pub(crate) fn build_instances(&self, camera: &Camera) -> Vec<MeshInstance> {
        let base = Vec3::new(0.0, 0.02, 2.30);
        let mut instances = vec![
            overlay_instance("menu-shell-shadow", camera, base + Vec3::new(0.0, 0.0, -0.03)),
            overlay_instance("menu-shell", camera, base),
            overlay_instance("menu-shell-rail", camera, base + Vec3::new(0.0, 0.0, 0.01)),
            overlay_instance("menu-side-bar", camera, base + Vec3::new(0.0, 0.0, 0.01)),
            overlay_instance("menu-title", camera, base + Vec3::new(0.0, 0.0, 0.02)),
            overlay_instance("menu-subtitle", camera, base + Vec3::new(0.0, 0.0, 0.02)),
            overlay_instance("menu-world-row", camera, base),
            overlay_instance("menu-world-row-rail", camera, base + Vec3::new(0.0, 0.0, 0.01)),
            overlay_instance("menu-world-label", camera, base + Vec3::new(0.0, 0.0, 0.02)),
            overlay_instance("menu-world-name", camera, base + Vec3::new(0.0, 0.0, 0.02)),
            overlay_instance("menu-world-nav-left", camera, base + Vec3::new(0.0, 0.0, 0.02)),
            overlay_instance("menu-world-nav-right", camera, base + Vec3::new(0.0, 0.0, 0.02)),
        ];

        for index in 0..MENU_BUTTON_COUNT {
            instances.push(overlay_instance(
                format!("menu-button-{index}"),
                camera,
                base,
            ));
            instances.push(overlay_instance(
                format!("menu-button-{index}-rail"),
                camera,
                base + Vec3::new(0.0, 0.0, 0.01),
            ));
            instances.push(overlay_instance(
                format!("menu-button-{index}-marker"),
                camera,
                base + Vec3::new(0.0, 0.0, 0.015),
            ));
            instances.push(overlay_instance(
                format!("menu-button-{index}-text"),
                camera,
                base + Vec3::new(0.0, 0.0, 0.02),
            ));
        }

        instances.push(overlay_instance("menu-footer", camera, base + Vec3::new(0.0, 0.0, 0.02)));
        instances
    }
}

const MENU_BUTTON_COUNT: usize = 4;

fn sanitize_menu_text(text: &str) -> String {
    let upper = text.to_ascii_uppercase();
    if upper.is_empty() {
        return "WORLD".to_string();
    }

    upper
        .chars()
        .map(|ch| if glyph_rows(ch).is_some() { ch } else { ' ' })
        .collect()
}

fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    match ch {
        'A' => Some([
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'B' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ]),
        'C' => Some([
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ]),
        'D' => Some([
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ]),
        'E' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ]),
        'F' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'G' => Some([
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ]),
        'H' => Some([
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'I' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ]),
        'J' => Some([
            0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
        ]),
        'K' => Some([
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ]),
        'L' => Some([
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ]),
        'M' => Some([
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ]),
        'N' => Some([
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ]),
        'O' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'P' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'Q' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ]),
        'R' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ]),
        'S' => Some([
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        'T' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'U' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'V' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ]),
        'W' => Some([
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ]),
        'X' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ]),
        'Y' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'Z' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ]),
        '0' => Some([
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ]),
        '1' => Some([
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        '2' => Some([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ]),
        '3' => Some([
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        '4' => Some([
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ]),
        '5' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ]),
        '6' => Some([
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ]),
        '7' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ]),
        '8' => Some([
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ]),
        '9' => Some([
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ]),
        '<' => Some([
            0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001,
        ]),
        '>' => Some([
            0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000,
        ]),
        ' ' => Some([0, 0, 0, 0, 0, 0, 0]),
        _ => None,
    }
}
