use glam::Vec3;

use crate::{
    chat::ChatState,
    renderer::{MeshInstance, StaticModelMesh},
    ui::{build_box_mesh, build_text_mesh, overlay_instance, sanitize_text, text_width},
    world::{camera::Camera, state::WorldRuntimeState},
};

const HUD_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 1.60);

pub(crate) fn build_hud_templates(world: &WorldRuntimeState, _chat: &ChatState) -> Vec<StaticModelMesh> {
    let mut meshes = Vec::new();

    if let Some(player) = world.local_player() {
        let health_ratio = (player.health as f32 / player.max_health.max(1) as f32).clamp(0.0, 1.0);
        let fill_top = -0.84 + 0.32 * health_ratio;

        meshes.push(build_box_mesh(
            "hud-health-shell",
            Vec3::new(-0.96, -0.90, -0.02),
            Vec3::new(-0.82, -0.48, 0.02),
            [0.08, 0.02, 0.03, 0.16],
        ));
        meshes.push(build_box_mesh(
            "hud-health-shell-highlight",
            Vec3::new(-0.95, -0.49, -0.01),
            Vec3::new(-0.83, -0.47, 0.01),
            [1.0, 0.42, 0.42, 0.10],
        ));
        if health_ratio > 0.0 {
            meshes.push(build_box_mesh(
                "hud-health-fill",
                Vec3::new(-0.94, -0.86, -0.005),
                Vec3::new(-0.84, fill_top, 0.005),
                [0.90, 0.10, 0.14, 0.72],
            ));
        }

        meshes.push(build_box_mesh(
            "hud-health-waterline",
            Vec3::new(-0.94, -0.60, -0.004),
            Vec3::new(-0.84, -0.58, 0.004),
            [1.0, 0.90, 0.90, 0.12],
        ));

        let time_chip = format!(
            "{} D{}",
            world.time_of_day.label(),
            world.time_of_day.elapsed_days
        );
        meshes.push(build_box_mesh(
            "hud-time-chip",
            Vec3::new(-0.14, 0.82, -0.02),
            Vec3::new(0.14, 0.90, 0.02),
            [0.07, 0.09, 0.12, 0.18],
        ));
        meshes.push(build_text_mesh(
            "hud-time-chip-text",
            &sanitize_text(&time_chip),
            Vec3::new(-0.095, 0.874, 0.02),
            0.0085,
            [0.92, 0.96, 1.0, 0.92],
        ));

        for index in 0..4usize {
            let selected = index == player.inventory.selected_weapon_slot && index < 2;
            let x = -0.26 + index as f32 * 0.14;
            let accent_color = match index {
                0 => [0.95, 0.72, 0.42, 0.48],
                1 => [0.55, 0.76, 0.96, 0.48],
                2 => [0.40, 0.86, 0.54, 0.48],
                _ => [0.98, 0.58, 0.28, 0.48],
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
                Vec3::new(x, -0.91, -0.02),
                Vec3::new(x + 0.11, -0.80, 0.02),
                if selected {
                    [0.92, 0.92, 0.96, 0.22]
                } else {
                    [0.10, 0.11, 0.14, 0.16]
                },
            ));
            meshes.push(build_box_mesh(
                format!("hud-slot-accent-{index}"),
                Vec3::new(x + 0.01, -0.90, -0.01),
                Vec3::new(x + 0.10, -0.88, 0.01),
                if selected {
                    [0.98, 0.94, 0.82, 0.56]
                } else {
                    accent_color
                },
            ));
            meshes.push(build_text_mesh(
                format!("hud-slot-number-{index}"),
                short_label,
                Vec3::new(x + 0.012, -0.835, 0.02),
                0.0065,
                [1.0, 1.0, 1.0, 0.72],
            ));

            if count > 1 {
                let count_text = sanitize_text(&format!("X{count}"));
                let width = text_width(&count_text, 0.0055);
                meshes.push(build_box_mesh(
                    format!("hud-slot-count-bg-{index}"),
                    Vec3::new(x + 0.11 - width - 0.010, -0.90, -0.005),
                    Vec3::new(x + 0.10, -0.85, 0.005),
                    [0.03, 0.03, 0.04, 0.24],
                ));
                meshes.push(build_text_mesh(
                    format!("hud-slot-count-{index}"),
                    &count_text,
                    Vec3::new(x + 0.11 - width - 0.006, -0.864, 0.02),
                    0.0055,
                    [1.0, 0.98, 0.92, 0.92],
                ));
            }
        }
    }

    meshes
}

pub(crate) fn build_hud_instances(camera: &Camera, world: &WorldRuntimeState, chat: &ChatState) -> Vec<MeshInstance> {
    let mut instances = Vec::new();

    if let Some(player) = world.local_player() {
        instances.push(overlay_instance("hud-health-shell", camera, HUD_OFFSET));
        instances.push(overlay_instance(
            "hud-health-shell-highlight",
            camera,
            HUD_OFFSET + Vec3::new(0.0, 0.0, -0.01),
        ));
        if player.health > 0 {
            instances.push(overlay_instance(
                "hud-health-fill",
                camera,
                HUD_OFFSET + Vec3::new(0.0, 0.0, 0.01),
            ));
        }
        instances.push(overlay_instance(
            "hud-health-waterline",
            camera,
            HUD_OFFSET + Vec3::new(0.0, 0.0, 0.015),
        ));
        instances.push(overlay_instance("hud-time-chip", camera, HUD_OFFSET));
        instances.push(overlay_instance(
            "hud-time-chip-text",
            camera,
            HUD_OFFSET + Vec3::new(0.0, 0.0, 0.01),
        ));

        for index in 0..4usize {
            instances.push(overlay_instance(
                format!("hud-slot-shell-{index}"),
                camera,
                HUD_OFFSET,
            ));
            instances.push(overlay_instance(
                format!("hud-slot-accent-{index}"),
                camera,
                HUD_OFFSET + Vec3::new(0.0, 0.0, 0.01),
            ));
            instances.push(overlay_instance(
                format!("hud-slot-number-{index}"),
                camera,
                HUD_OFFSET + Vec3::new(0.0, 0.0, 0.02),
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
                    HUD_OFFSET + Vec3::new(0.0, 0.0, 0.015),
                ));
                instances.push(overlay_instance(
                    format!("hud-slot-count-{index}"),
                    camera,
                    HUD_OFFSET + Vec3::new(0.0, 0.0, 0.02),
                ));
            }
        }
    }

    instances.extend(chat.build_overlay_instances(camera));
    instances
}
