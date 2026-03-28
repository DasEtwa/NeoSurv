use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::{
    renderer::{MeshInstance, StaticModelMesh},
    world::state::PlayerRuntimeState,
};

use super::{
    damage::{DamageEvent, DamageResolution, resolve_damage},
    hit_detection::TargetHitbox,
    viewmodel::build_box_mesh,
};

const DEFAULT_ENEMY_MAX_HP: i32 = 100;
const ENEMY_MOVE_SPEED: f32 = 2.4;
const ENEMY_ATTACK_RANGE: f32 = 1.4;
const ENEMY_ATTACK_DAMAGE: i32 = 12;
const ENEMY_ATTACK_COOLDOWN: f32 = 1.1;
const ENEMY_AGGRO_RANGE: f32 = 18.0;
const ENEMY_WANDER_RADIUS: f32 = 3.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum EnemyKind {
    MeleeHunter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum EnemyBrainState {
    Idle,
    Wander,
    Chase,
    Attack,
    ReturnToSpawn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EnemyActor {
    pub(crate) id: u64,
    pub(crate) kind: EnemyKind,
    pub(crate) brain_state: EnemyBrainState,
    pub(crate) position: [f32; 3],
    pub(crate) spawn_origin: [f32; 3],
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) attack_cooldown: f32,
    pub(crate) wander_phase: f32,
    pub(crate) leash_radius: f32,
    pub(crate) spawner_id: Option<u64>,
}

impl EnemyActor {
    fn new(id: u64, kind: EnemyKind, position: Vec3, spawner_id: Option<u64>) -> Self {
        Self {
            id,
            kind,
            brain_state: EnemyBrainState::Idle,
            position: position.to_array(),
            spawn_origin: position.to_array(),
            hp: DEFAULT_ENEMY_MAX_HP,
            max_hp: DEFAULT_ENEMY_MAX_HP,
            attack_cooldown: 0.0,
            wander_phase: 0.0,
            leash_radius: 20.0,
            spawner_id,
        }
    }

    fn position_vec3(&self) -> Vec3 {
        Vec3::from_array(self.position)
    }

    fn spawn_origin_vec3(&self) -> Vec3 {
        Vec3::from_array(self.spawn_origin)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EnemyImpact {
    pub(crate) enemy_id: u64,
    pub(crate) kind: EnemyKind,
    pub(crate) position: Vec3,
    pub(crate) remaining_hp: Option<i32>,
    pub(crate) remaining_enemies: usize,
    pub(crate) total_eliminations: u32,
}

impl EnemyImpact {
    pub(crate) fn log(self) {
        match self.remaining_hp {
            Some(remaining_hp) => {
                tracing::info!(
                    enemy_id = self.enemy_id,
                    x = self.position.x,
                    y = self.position.y,
                    z = self.position.z,
                    enemy_kind = ?self.kind,
                    hp = remaining_hp,
                    "enemy hit"
                );
            }
            None => {
                tracing::info!(
                    enemy_id = self.enemy_id,
                    x = self.position.x,
                    y = self.position.y,
                    z = self.position.z,
                    enemy_kind = ?self.kind,
                    eliminations = self.total_eliminations,
                    remaining_enemies = self.remaining_enemies,
                    "enemy eliminated"
                );
            }
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct EnemyRoster {
    total_eliminations: u32,
    enemies: Vec<EnemyActor>,
}

impl EnemyRoster {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn enemies(&self) -> &[EnemyActor] {
        &self.enemies
    }

    pub(crate) fn spawn_enemy(
        &mut self,
        id: u64,
        kind: EnemyKind,
        position: Vec3,
        spawner_id: Option<u64>,
    ) {
        self.enemies
            .push(EnemyActor::new(id, kind, position, spawner_id));
    }

    pub(crate) fn target_hitboxes(&self) -> Vec<TargetHitbox> {
        self.enemies
            .iter()
            .enumerate()
            .map(|(index, enemy)| {
                let base = enemy.position_vec3();
                TargetHitbox {
                    target_index: index,
                    min: base + Vec3::new(-0.45, 0.0, -0.45),
                    max: base + Vec3::new(0.45, 1.9, 0.45),
                }
            })
            .collect()
    }

    pub(crate) fn apply_damage(&mut self, target_index: usize, amount: i32) -> Option<EnemyImpact> {
        let current_enemy = self.enemies.get(target_index)?.clone();

        match resolve_damage(current_enemy.hp, DamageEvent { amount }) {
            DamageResolution::Survived { remaining_hp } => {
                let remaining_enemies = self.enemies.len();
                let total_eliminations = self.total_eliminations;
                let enemy = self.enemies.get_mut(target_index)?;
                enemy.hp = remaining_hp;
                Some(EnemyImpact {
                    enemy_id: enemy.id,
                    kind: enemy.kind,
                    position: enemy.position_vec3(),
                    remaining_hp: Some(remaining_hp),
                    remaining_enemies,
                    total_eliminations,
                })
            }
            DamageResolution::Eliminated => {
                let enemy = self.enemies.swap_remove(target_index);
                self.total_eliminations = self.total_eliminations.saturating_add(1);
                Some(EnemyImpact {
                    enemy_id: enemy.id,
                    kind: enemy.kind,
                    position: enemy.position_vec3(),
                    remaining_hp: None,
                    remaining_enemies: self.enemies.len(),
                    total_eliminations: self.total_eliminations,
                })
            }
        }
    }

    pub(crate) fn tick_ai<F>(
        &mut self,
        dt_seconds: f32,
        player_position: Vec3,
        player_runtime: &mut PlayerRuntimeState,
        mut find_surface_height: F,
    ) -> bool
    where
        F: FnMut(i32, i32) -> Option<i32>,
    {
        let mut player_damaged = false;

        for enemy in &mut self.enemies {
            enemy.attack_cooldown = (enemy.attack_cooldown - dt_seconds).max(0.0);
            enemy.wander_phase += dt_seconds;

            let position = enemy.position_vec3();
            let spawn_origin = enemy.spawn_origin_vec3();
            let to_player = player_position - position;
            let to_spawn = spawn_origin - position;
            let horizontal_to_player = Vec3::new(to_player.x, 0.0, to_player.z);
            let horizontal_to_spawn = Vec3::new(to_spawn.x, 0.0, to_spawn.z);
            let distance_to_player = horizontal_to_player.length();
            let distance_to_spawn = horizontal_to_spawn.length();

            let mut desired_move = Vec3::ZERO;

            if distance_to_player <= ENEMY_ATTACK_RANGE {
                enemy.brain_state = EnemyBrainState::Attack;
                if enemy.attack_cooldown <= 0.0 {
                    player_runtime.apply_damage(ENEMY_ATTACK_DAMAGE);
                    enemy.attack_cooldown = ENEMY_ATTACK_COOLDOWN;
                    player_damaged = true;
                    tracing::info!(
                        enemy_id = enemy.id,
                        hp = player_runtime.health,
                        "player hit by enemy"
                    );
                }
            } else if distance_to_player <= ENEMY_AGGRO_RANGE {
                enemy.brain_state = EnemyBrainState::Chase;
                desired_move = horizontal_to_player.normalize_or_zero() * ENEMY_MOVE_SPEED;
            } else if distance_to_spawn > enemy.leash_radius {
                enemy.brain_state = EnemyBrainState::ReturnToSpawn;
                desired_move = horizontal_to_spawn.normalize_or_zero() * ENEMY_MOVE_SPEED;
            } else {
                enemy.brain_state = EnemyBrainState::Wander;
                let wander_target = spawn_origin
                    + Vec3::new(
                        enemy.wander_phase.cos() * ENEMY_WANDER_RADIUS,
                        0.0,
                        enemy.wander_phase.sin() * ENEMY_WANDER_RADIUS,
                    );
                desired_move =
                    (wander_target - position).normalize_or_zero() * (ENEMY_MOVE_SPEED * 0.45);
            }

            let mut new_position = position + desired_move * dt_seconds;
            if let Some(surface) =
                find_surface_height(new_position.x.round() as i32, new_position.z.round() as i32)
            {
                new_position.y = surface as f32 + 1.0;
            }
            enemy.position = new_position.to_array();
        }

        player_damaged
    }

    pub(crate) fn snap_to_terrain<F>(&mut self, mut find_surface_height: F)
    where
        F: FnMut(i32, i32) -> Option<i32>,
    {
        for enemy in &mut self.enemies {
            let position = enemy.position_vec3();
            if let Some(surface) =
                find_surface_height(position.x.round() as i32, position.z.round() as i32)
            {
                let snapped = Vec3::new(position.x, surface as f32 + 1.0, position.z);
                enemy.position = snapped.to_array();
                enemy.spawn_origin = snapped.to_array();
            }
        }
    }

    pub(crate) fn build_templates() -> Vec<StaticModelMesh> {
        vec![
            build_box_mesh(
                "enemy-body-template",
                Vec3::new(-0.40, 0.0, -0.28),
                Vec3::new(0.40, 1.25, 0.28),
                [1.0, 1.0, 1.0, 1.0],
            ),
            build_box_mesh(
                "enemy-head-template",
                Vec3::new(-0.28, 1.25, -0.28),
                Vec3::new(0.28, 1.85, 0.28),
                [1.0, 1.0, 1.0, 1.0],
            ),
        ]
    }

    pub(crate) fn build_instances(&self) -> Vec<MeshInstance> {
        let mut instances = Vec::with_capacity(self.enemies.len() * 2);

        for enemy in &self.enemies {
            let base = enemy.position_vec3();
            let hp_ratio = (enemy.hp as f32 / enemy.max_hp.max(1) as f32).clamp(0.0, 1.0);
            let body_color = match enemy.brain_state {
                EnemyBrainState::Chase | EnemyBrainState::Attack => {
                    [0.96, 0.18 + 0.40 * hp_ratio, 0.22, 1.0]
                }
                EnemyBrainState::ReturnToSpawn => [0.78, 0.44, 0.28, 1.0],
                EnemyBrainState::Wander | EnemyBrainState::Idle => {
                    [0.64, 0.64 * hp_ratio, 0.70, 1.0]
                }
            };

            instances.push(MeshInstance::new(
                "enemy-body-template",
                Mat4::from_translation(base),
                body_color,
            ));
            instances.push(MeshInstance::new(
                "enemy-head-template",
                Mat4::from_translation(base),
                [0.94, 0.90, 0.78, 1.0],
            ));
        }

        instances
    }
}
