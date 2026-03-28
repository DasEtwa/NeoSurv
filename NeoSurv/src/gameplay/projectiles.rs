use glam::{IVec3, Mat4, Vec3};

use crate::renderer::{MeshInstance, StaticModelMesh};

use super::{
    enemies::EnemyRoster,
    hit_detection::find_first_sphere_overlap,
    viewmodel::build_box_mesh,
    weapons::{WeaponDefinition, WeaponFireMode},
};

#[derive(Debug, Clone, Copy)]
struct ProjectileInstance {
    weapon_id: &'static str,
    position: Vec3,
    velocity: Vec3,
    radius: f32,
    gravity: f32,
    remaining_lifetime: f32,
    damage: i32,
}

#[derive(Debug, Default)]
pub(crate) struct ProjectileSystem {
    active: Vec<ProjectileInstance>,
}

impl ProjectileSystem {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn reset(&mut self) {
        self.active.clear();
    }

    pub(crate) fn spawn(&mut self, origin: Vec3, direction: Vec3, weapon: WeaponDefinition) {
        let WeaponFireMode::Projectile {
            speed,
            gravity,
            max_lifetime,
            radius,
        } = weapon.fire_mode
        else {
            tracing::warn!(
                weapon = weapon.id,
                "attempted projectile spawn with non-projectile weapon"
            );
            return;
        };

        let dir = direction.normalize_or_zero();
        if dir.length_squared() <= f32::EPSILON {
            return;
        }

        self.active.push(ProjectileInstance {
            weapon_id: weapon.id,
            position: origin + dir * 0.7,
            velocity: dir * speed,
            radius,
            gravity,
            remaining_lifetime: max_lifetime,
            damage: weapon.shot_damage,
        });
    }

    pub(crate) fn tick<F, G>(
        &mut self,
        dt_seconds: f32,
        enemies: &mut EnemyRoster,
        mut is_solid: F,
        mut hits_static_prop: G,
    )
    where
        F: FnMut(IVec3) -> bool,
        G: FnMut(Vec3, f32) -> bool,
    {
        let mut next_active = Vec::with_capacity(self.active.len());

        for mut projectile in self.active.drain(..) {
            projectile.remaining_lifetime -= dt_seconds;
            if projectile.remaining_lifetime <= 0.0 {
                continue;
            }

            projectile.velocity.y -= projectile.gravity * dt_seconds;
            projectile.position += projectile.velocity * dt_seconds;

            if is_solid(projectile.position.floor().as_ivec3()) {
                tracing::debug!(
                    weapon = projectile.weapon_id,
                    "projectile collided with world"
                );
                continue;
            }

            if hits_static_prop(projectile.position, projectile.radius) {
                tracing::debug!(
                    weapon = projectile.weapon_id,
                    "projectile collided with static prop"
                );
                continue;
            }

            if let Some(target_index) = find_first_sphere_overlap(
                projectile.position,
                projectile.radius,
                enemies.target_hitboxes(),
            ) {
                if let Some(impact) = enemies.apply_damage(target_index, projectile.damage) {
                    impact.log();
                }
                continue;
            }

            next_active.push(projectile);
        }

        self.active = next_active;
    }

    pub(crate) fn build_templates() -> Vec<StaticModelMesh> {
        vec![
            build_box_mesh(
                "projectile-launcher-template",
                Vec3::splat(-0.18),
                Vec3::splat(0.18),
                [1.0, 1.0, 1.0, 1.0],
            ),
            build_box_mesh(
                "projectile-grenade-template",
                Vec3::splat(-0.24),
                Vec3::splat(0.24),
                [1.0, 1.0, 1.0, 1.0],
            ),
        ]
    }

    pub(crate) fn build_instances(&self) -> Vec<MeshInstance> {
        self.active
            .iter()
            .map(|projectile| {
                let template_label = match projectile.weapon_id {
                    "grenade" => "projectile-grenade-template",
                    _ => "projectile-launcher-template",
                };
                MeshInstance::new(
                    template_label,
                    Mat4::from_translation(projectile.position),
                    [1.0, 0.68, 0.24, 1.0],
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projectile_stops_when_hitting_static_prop_collision() {
        let mut system = ProjectileSystem::new();
        let mut enemies = EnemyRoster::new();
        system.spawn(Vec3::ZERO, Vec3::Z, WeaponDefinition::launcher());

        system.tick(
            0.05,
            &mut enemies,
            |_| false,
            |position, radius| position.z >= 1.0 - radius,
        );

        assert!(system.active.is_empty());
    }
}
