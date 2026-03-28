use glam::Vec3;

use crate::{
    chat::ChatState,
    renderer::{MeshInstance, StaticModelMesh},
    ui::{build_box_mesh, build_text_mesh, overlay_instance, sanitize_text, text_width},
    world::{camera::Camera, state::WorldRuntimeState},
};

const HUD_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 1.68);

pub(crate) fn build_hud_templates(world: &WorldRuntimeState, _chat: &ChatState) -> Vec<StaticModelMesh> {
    let mut meshes = Vec::new();

    if let Some(player) = world.local_player() {
        let health_ratio = (player.health as f32 / player.max_health.max(1) as f32).clamp(0.0, 1.0);
        let health_fill_right = -0.86 + 0.34 * health_ratio;
        let health_value = sanitize_text(&format!("{}", player.health.max(0)));
        let time_chip = format!(
            "{} D{}",
            world.time_of_day.label(),
            world.time_of_day.elapsed_days
        );

        meshes.push(build_box_mesh(
            "hud-dock-shadow",
            Vec3::new(-0.48, -0.98, -0.04),
            Vec3::new(0.34, -0.72, 0.04),
            [0.01, 0.01, 0.02, 0.12],
        ));
        meshes.push(build_box_mesh(
            "hud-dock-frame",
            Vec3::new(-0.44, -0.94, -0.03),
            Vec3::new(0.30, -0.75, 0.03),
            [0.04, 0.05, 0.07, 0.28],
        ));
        meshes.push(build_box_mesh(
            "hud-dock-rail",
            Vec3::new(-0.44, -0.78, -0.01),
            Vec3::new(0.30, -0.75, 0.01),
            [0.92, 0.74, 0.42, 0.24],
        ));

        meshes.push(build_box_mesh(
            "hud-health-shell",
            Vec3::new(-0.95, -0.94, -0.03),
            Vec3::new(-0.50, -0.82, 0.03),
            [0.05, 0.06, 0.08, 0.24],
        ));
        meshes.push(build_box_mesh(
            "hud-health-track",
            Vec3::new(-0.85, -0.90, -0.01),
            Vec3::new(-0.52, -0.84, 0.01),
            [0.14, 0.05, 0.06, 0.32],
        ));
        if health_ratio > 0.0 {
            meshes.push(build_box_mesh(
                "hud-health-fill",
                Vec3::new(-0.84, -0.89, -0.006),
                Vec3::new(health_fill_right, -0.85, 0.006),
                [0.90, 0.18, 0.16, 0.80],
            ));
        }
        meshes.push(build_text_mesh(
            "hud-health-label",
            "HP",
            Vec3::new(-0.92, -0.865, 0.02),
            0.0075,
            [0.98, 0.92, 0.84, 0.82],
        ));
        meshes.push(build_text_mesh(
            "hud-health-value",
            &health_value,
            Vec3::new(-0.60 - text_width(&health_value, 0.0085), -0.862, 0.02),
            0.0085,
            [1.0, 0.98, 0.94, 0.96],
        ));

        meshes.push(build_box_mesh(
            "hud-time-chip",
            Vec3::new(-0.16, 0.80, -0.03),
            Vec3::new(0.16, 0.90, 0.03),
            [0.04, 0.06, 0.08, 0.18],
        ));
        meshes.push(build_box_mesh(
            "hud-time-chip-rail",
            Vec3::new(-0.16, 0.87, -0.01),
            Vec3::new(0.16, 0.90, 0.01),
            [0.92, 0.74, 0.42, 0.20],
        ));
        meshes.push(build_text_mesh(
            "hud-time-chip-text",
            &sanitize_text(&time_chip),
            Vec3::new(-text_width(&sanitize_text(&time_chip), 0.0085) * 0.5, 0.862, 0.02),
            0.0085,
            [0.94, 0.97, 1.0, 0.94],
        ));

        for index in 0..4usize {
            let selected = index == player.inventory.selected_weapon_slot && index < 2;
            let x = -0.39 + index as f32 * 0.18;
            let accent_color = match index {
                0 => [0.95, 0.72, 0.42, 0.48],
                1 => [0.55, 0.76, 0.96, 0.48],
                2 => [0.48, 0.86, 0.60, 0.42],
                _ => [0.98, 0.58, 0.28, 0.44],
            };
            let count = player
                .inventory
                .slots
                .get(index)
                .and_then(|slot| slot.stack)
                .map(|stack| stack.count)
                .unwrap_or(0);
            let short_label = match index {
                0 => "1",
                1 => "2",
                2 => "3",
                _ => "4",
            };

            meshes.push(build_box_mesh(
                format!("hud-slot-shell-{index}"),
                Vec3::new(x, -0.90, -0.02),
                Vec3::new(x + 0.13, -0.79, 0.02),
                if selected {
                    [0.16, 0.15, 0.14, 0.36]
                } else {
                    [0.07, 0.08, 0.10, 0.22]
                },
            ));
            meshes.push(build_box_mesh(
                format!("hud-slot-accent-{index}"),
                Vec3::new(x + 0.01, -0.885, -0.01),
                Vec3::new(x + 0.12, -0.865, 0.01),
                if selected {
                    [0.98, 0.90, 0.70, 0.74]
                } else {
                    accent_color
                },
            ));
            meshes.push(build_text_mesh(
                format!("hud-slot-number-{index}"),
                short_label,
                Vec3::new(x + 0.012, -0.825, 0.02),
                0.0068,
                [1.0, 0.98, 0.92, 0.80],
            ));

            if count > 1 {
                let count_text = sanitize_text(&format!("X{count}"));
                let width = text_width(&count_text, 0.0055);
                meshes.push(build_box_mesh(
                    format!("hud-slot-count-bg-{index}"),
                    Vec3::new(x + 0.13 - width - 0.014, -0.885, -0.005),
                    Vec3::new(x + 0.12, -0.84, 0.005),
                    [0.03, 0.03, 0.04, 0.28],
                ));
                meshes.push(build_text_mesh(
                    format!("hud-slot-count-{index}"),
                    &count_text,
                    Vec3::new(x + 0.13 - width - 0.009, -0.854, 0.02),
                    0.0055,
                    [1.0, 0.98, 0.92, 0.94],
                ));
            }
        }
    }

    meshes
}

pub(crate) fn build_hud_instances(camera: &Camera, world: &WorldRuntimeState, chat: &ChatState) -> Vec<MeshInstance> {
    let mut instances = Vec::new();

    if let Some(player) = world.local_player() {
        let base = HUD_OFFSET;
        instances.push(overlay_instance("hud-dock-shadow", camera, base + Vec3::new(0.0, 0.0, -0.03)));
        instances.push(overlay_instance("hud-dock-frame", camera, base));
        instances.push(overlay_instance("hud-dock-rail", camera, base + Vec3::new(0.0, 0.0, 0.01)));

        instances.push(overlay_instance("hud-health-shell", camera, base));
        instances.push(overlay_instance("hud-health-track", camera, base + Vec3::new(0.0, 0.0, 0.01)));
        if player.health > 0 {
            instances.push(overlay_instance("hud-health-fill", camera, base + Vec3::new(0.0, 0.0, 0.02)));
        }
        instances.push(overlay_instance("hud-health-label", camera, base + Vec3::new(0.0, 0.0, 0.02)));
        instances.push(overlay_instance("hud-health-value", camera, base + Vec3::new(0.0, 0.0, 0.02)));

        instances.push(overlay_instance("hud-time-chip", camera, base));
        instances.push(overlay_instance("hud-time-chip-rail", camera, base + Vec3::new(0.0, 0.0, 0.01)));
        instances.push(overlay_instance("hud-time-chip-text", camera, base + Vec3::new(0.0, 0.0, 0.02)));

        for index in 0..4usize {
            instances.push(overlay_instance(
                format!("hud-slot-shell-{index}"),
                camera,
                base,
            ));
            instances.push(overlay_instance(
                format!("hud-slot-accent-{index}"),
                camera,
                base + Vec3::new(0.0, 0.0, 0.01),
            ));
            instances.push(overlay_instance(
                format!("hud-slot-number-{index}"),
                camera,
                base + Vec3::new(0.0, 0.0, 0.02),
            ));

            let count = player
                .inventory
                .slots
                .get(index)
                .and_then(|slot| slot.stack)
                .map(|stack| stack.count)
                .unwrap_or(0);
            if count > 1 {
                instances.push(overlay_instance(
                    format!("hud-slot-count-bg-{index}"),
                    camera,
                    base + Vec3::new(0.0, 0.0, 0.015),
                ));
                instances.push(overlay_instance(
                    format!("hud-slot-count-{index}"),
                    camera,
                    base + Vec3::new(0.0, 0.0, 0.02),
                ));
            }
        }
    }

    instances.extend(chat.build_overlay_instances(camera));
    instances
}
