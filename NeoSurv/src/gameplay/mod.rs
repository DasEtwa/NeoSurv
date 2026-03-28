mod damage;
mod enemies;
mod hit_detection;
mod hitscan;
mod projectiles;
mod viewmodel;
mod weapons;

use glam::{IVec3, Vec3};

use crate::{
    inventory::ItemId,
    renderer::{MeshInstance, StaticModelMesh},
    world::camera::Camera,
};

use self::{
    hitscan::fire_hitscan_shot,
    projectiles::ProjectileSystem,
    viewmodel::ViewmodelAssets,
    weapons::{WeaponDefinition, WeaponEffects},
};

pub(crate) use self::{
    enemies::{EnemyKind, EnemyRoster},
    viewmodel::build_box_mesh,
};

#[derive(Debug)]
pub(crate) struct CombatState {
    primary_weapon: WeaponDefinition,
    secondary_weapon: WeaponDefinition,
    projectiles: ProjectileSystem,
    weapon_effects: WeaponEffects,
    viewmodel: ViewmodelAssets,
}

impl CombatState {
    pub(crate) fn new() -> Self {
        let primary_weapon = WeaponDefinition::sidearm();
        let secondary_weapon = WeaponDefinition::launcher();
        Self {
            primary_weapon,
            secondary_weapon,
            projectiles: ProjectileSystem::new(),
            weapon_effects: WeaponEffects::new(),
            viewmodel: ViewmodelAssets::new(primary_weapon),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.projectiles.reset();
        self.weapon_effects.reset();
    }

    pub(crate) fn tick_effects(&mut self, dt_seconds: f32) {
        self.weapon_effects.tick(dt_seconds);
    }

    pub(crate) fn weapon_for_item(&self, item_id: ItemId) -> Option<WeaponDefinition> {
        match item_id {
            ItemId::Sidearm => Some(self.primary_weapon),
            ItemId::Launcher => Some(self.secondary_weapon),
            ItemId::Grenade => Some(weapons::WeaponDefinition::grenade()),
            ItemId::Medkit => None,
        }
    }

    pub(crate) fn hitscan_range_for_item(&self, item_id: ItemId) -> Option<f32> {
        let weapon = self.weapon_for_item(item_id)?;
        match weapon.fire_mode {
            weapons::WeaponFireMode::Hitscan { range } => Some(range),
            weapons::WeaponFireMode::Projectile { .. } => None,
        }
    }

    pub(crate) fn fire_weapon(
        &mut self,
        item_id: ItemId,
        enemies: &mut EnemyRoster,
        origin: Vec3,
        direction: Vec3,
        world_blocker_distance: Option<f32>,
    ) {
        let Some(weapon) = self.weapon_for_item(item_id) else {
            return;
        };
        self.weapon_effects.register_shot(weapon);
        match weapon.fire_mode {
            weapons::WeaponFireMode::Hitscan { .. } => {
                fire_hitscan_shot(enemies, origin, direction, weapon, world_blocker_distance);
            }
            weapons::WeaponFireMode::Projectile { .. } => {
                self.projectiles.spawn(origin, direction, weapon);
            }
        }
    }

    pub(crate) fn tick_projectiles<F>(
        &mut self,
        dt_seconds: f32,
        enemies: &mut EnemyRoster,
        is_solid: F,
    ) where
        F: FnMut(IVec3) -> bool,
    {
        self.projectiles.tick(dt_seconds, enemies, is_solid);
    }

    pub(crate) fn dynamic_templates(&self) -> Vec<StaticModelMesh> {
        let mut templates = EnemyRoster::build_templates();
        templates.extend(ProjectileSystem::build_templates());
        templates
    }

    pub(crate) fn dynamic_instances(&self, enemies: &EnemyRoster) -> Vec<MeshInstance> {
        let mut instances = enemies.build_instances();
        instances.extend(self.projectiles.build_instances());
        instances
    }

    #[allow(dead_code)]
    pub(crate) fn viewmodel_templates(&self, selected_item: Option<ItemId>) -> Vec<StaticModelMesh> {
        let weapon = selected_item
            .and_then(|item_id| self.weapon_for_item(item_id))
            .unwrap_or(self.primary_weapon);
        self.viewmodel.build_template_meshes(weapon)
    }

    pub(crate) fn build_viewmodel_meshes(
        &self,
        camera: &Camera,
        selected_item: Option<ItemId>,
    ) -> Vec<StaticModelMesh> {
        let weapon = selected_item
            .and_then(|item_id| self.weapon_for_item(item_id))
            .unwrap_or(self.primary_weapon);
        self.viewmodel
            .build_meshes(camera, &self.weapon_effects, weapon)
    }

    #[allow(dead_code)]
    pub(crate) fn viewmodel_instances(
        &self,
        camera: &Camera,
        selected_item: Option<ItemId>,
    ) -> Vec<MeshInstance> {
        let weapon = selected_item
            .and_then(|item_id| self.weapon_for_item(item_id))
            .unwrap_or(self.primary_weapon);
        self.viewmodel
            .build_instances(camera, &self.weapon_effects, weapon)
    }
}
