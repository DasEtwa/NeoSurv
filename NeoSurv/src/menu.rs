use glam::Vec3;

use crate::{
    renderer::StaticModelMesh,
    ui::{build_box_mesh, build_text_mesh, text_width, transform_overlay_mesh},
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

    pub(crate) fn build_meshes(&self, camera: &Camera, world_name: &str) -> Vec<StaticModelMesh> {
        let mut meshes = Vec::new();
        let world_name = sanitize_menu_text(world_name);
        let title = "TOKENBURNER";

        meshes.push(transform_overlay_mesh(
            &build_box_mesh(
                "menu-shell",
                Vec3::new(-0.46, -0.30, -0.03),
                Vec3::new(0.46, 0.34, 0.03),
                [0.05, 0.06, 0.08, 0.18],
            ),
            "menu-shell-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.32),
        ));
        meshes.push(transform_overlay_mesh(
            &build_box_mesh(
                "menu-shell-accent",
                Vec3::new(-0.46, 0.30, -0.02),
                Vec3::new(0.46, 0.34, 0.02),
                [0.94, 0.82, 0.50, 0.10],
            ),
            "menu-shell-accent-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.31),
        ));

        meshes.push(transform_overlay_mesh(
            &build_text_mesh(
                "menu-title",
                title,
                Vec3::new(-text_width(title, 0.013) * 0.5, 0.255, 0.02),
                0.013,
                [0.98, 0.96, 0.86, 0.96],
            ),
            "menu-title-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.30),
        ));

        meshes.push(transform_overlay_mesh(
            &build_box_mesh(
                "menu-world-row",
                Vec3::new(-0.34, 0.10, -0.02),
                Vec3::new(0.34, 0.18, 0.02),
                [0.10, 0.10, 0.12, 0.16],
            ),
            "menu-world-row-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.29),
        ));
        meshes.push(transform_overlay_mesh(
            &build_text_mesh(
                "menu-world-label",
                "WORLD",
                Vec3::new(-0.30, 0.155, 0.02),
                0.0085,
                [0.82, 0.86, 0.92, 0.84],
            ),
            "menu-world-label-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.28),
        ));
        meshes.push(transform_overlay_mesh(
            &build_text_mesh(
                "menu-world-name",
                &world_name,
                Vec3::new(-text_width(&world_name, 0.010) * 0.5, 0.118, 0.02),
                0.010,
                [0.98, 0.98, 0.92, 0.92],
            ),
            "menu-world-name-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.28),
        ));
        meshes.push(transform_overlay_mesh(
            &build_text_mesh(
                "menu-world-nav-left",
                "<",
                Vec3::new(-0.34, 0.118, 0.02),
                0.010,
                [0.96, 0.82, 0.48, 0.80],
            ),
            "menu-world-nav-left-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.28),
        ));
        meshes.push(transform_overlay_mesh(
            &build_text_mesh(
                "menu-world-nav-right",
                ">",
                Vec3::new(0.31, 0.118, 0.02),
                0.010,
                [0.96, 0.82, 0.48, 0.80],
            ),
            "menu-world-nav-right-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.28),
        ));

        for (index, (label, local_y)) in [
            ("PLAY", -0.02),
            ("NEW WORLD", -0.12),
            ("SAVE WORLD", -0.22),
            ("QUIT", -0.32),
        ]
        .into_iter()
        .enumerate()
        {
            let is_selected = self.selected_button == index;
            let panel_color = if is_selected {
                [0.22, 0.18, 0.14, 0.34]
            } else {
                [0.08, 0.09, 0.10, 0.18]
            };
            let accent_color = if is_selected {
                [0.94, 0.82, 0.50, 0.60]
            } else {
                [0.92, 0.92, 0.96, 0.18]
            };
            let text_color = if is_selected {
                [1.0, 0.98, 0.88, 0.98]
            } else {
                [0.88, 0.90, 0.94, 0.82]
            };

            meshes.push(transform_overlay_mesh(
                &build_box_mesh(
                    format!("menu-button-{index}"),
                    Vec3::new(-0.34, local_y - 0.042, -0.02),
                    Vec3::new(0.34, local_y + 0.028, 0.02),
                    panel_color,
                ),
                format!("menu-button-{index}-overlay"),
                camera,
                Vec3::new(0.0, 0.02, 2.27),
            ));
            meshes.push(transform_overlay_mesh(
                &build_box_mesh(
                    format!("menu-button-{index}-accent"),
                    Vec3::new(-0.34, local_y - 0.042, -0.01),
                    Vec3::new(-0.29, local_y + 0.028, 0.01),
                    accent_color,
                ),
                format!("menu-button-{index}-accent-overlay"),
                camera,
                Vec3::new(0.0, 0.02, 2.26),
            ));
            meshes.push(transform_overlay_mesh(
                &build_text_mesh(
                    format!("menu-button-{index}-text"),
                    label,
                    Vec3::new(-text_width(label, 0.011) * 0.5, local_y, 0.02),
                    0.011,
                    text_color,
                ),
                format!("menu-button-{index}-label-overlay"),
                camera,
                Vec3::new(0.0, 0.02, 2.25),
            ));
        }

        let footer = "W/S SELECT  A/D WORLD  ENTER USE";
        meshes.push(transform_overlay_mesh(
            &build_text_mesh(
                "menu-footer",
                footer,
                Vec3::new(-text_width(footer, 0.0075) * 0.5, -0.38, 0.02),
                0.0075,
                [0.84, 0.88, 0.94, 0.62],
            ),
            "menu-footer-overlay",
            camera,
            Vec3::new(0.0, 0.02, 2.24),
        ));

        meshes
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
