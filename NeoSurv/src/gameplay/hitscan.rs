use glam::Vec3;

use super::{
    enemies::EnemyRoster,
    hit_detection::find_first_hitscan_hit,
    weapons::{WeaponDefinition, WeaponFireMode},
};

pub(super) fn fire_hitscan_shot(
    enemies: &mut EnemyRoster,
    origin: Vec3,
    direction: Vec3,
    weapon: WeaponDefinition,
    world_blocker_distance: Option<f32>,
) {
    let WeaponFireMode::Hitscan { range } = weapon.fire_mode else {
        tracing::warn!(
            weapon = weapon.id,
            "attempted hitscan fire with non-hitscan weapon"
        );
        return;
    };

    let Some(hit) = find_first_hitscan_hit(origin, direction, range, enemies.target_hitboxes())
    else {
        tracing::debug!("shot miss");
        return;
    };

    if world_blocker_distance
        .is_some_and(|blocker_distance| blocker_distance + 0.001 < hit.distance)
    {
        tracing::debug!(
            hit_distance = hit.distance,
            blocker_distance = world_blocker_distance,
            "shot blocked by world geometry"
        );
        return;
    }

    let Some(impact) = enemies.apply_damage(hit.target_index, weapon.shot_damage) else {
        tracing::warn!(
            target_index = hit.target_index,
            "enemy hit resolved to missing target"
        );
        return;
    };

    impact.log();
}
