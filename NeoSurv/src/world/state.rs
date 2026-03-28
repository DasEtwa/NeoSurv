use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::{
    gameplay::{EnemyKind, EnemyRoster, build_box_mesh},
    inventory::{InventoryState, ItemId, ItemStack, clamp_health},
    player::SavedPlayerPose,
    renderer::StaticModelMesh,
    world::voxel::generation::TerrainGenerator,
};

const WORLD_RUNTIME_VERSION: u32 = 1;
const LOCAL_PLAYER_ID: u64 = 1;
const PLAYER_MAX_HEALTH: i32 = 100;
const DAY_NIGHT_CYCLE_SECONDS: f32 = 30.0 * 60.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct TimeOfDayState {
    pub(crate) normalized_time: f32,
    pub(crate) elapsed_days: u32,
}

impl Default for TimeOfDayState {
    fn default() -> Self {
        Self {
            normalized_time: 0.20,
            elapsed_days: 0,
        }
    }
}

impl TimeOfDayState {
    pub(crate) fn advance(&mut self, dt_seconds: f32) {
        let cycle_delta = dt_seconds / DAY_NIGHT_CYCLE_SECONDS;
        let total = self.normalized_time + cycle_delta.max(0.0);
        self.elapsed_days = self.elapsed_days.saturating_add(total.floor() as u32);
        self.normalized_time = total.fract();
    }

    pub(crate) fn set_normalized_time(&mut self, normalized_time: f32) {
        self.normalized_time = normalized_time.rem_euclid(1.0);
    }

    pub(crate) fn is_night(&self) -> bool {
        self.normalized_time >= 0.5
    }

    pub(crate) fn label(&self) -> &'static str {
        if self.is_night() { "NIGHT" } else { "DAY" }
    }

    pub(crate) fn daylight_factor(&self) -> f32 {
        let angle = self.normalized_time * std::f32::consts::TAU;
        (0.45 + 0.55 * angle.cos().mul_add(-0.5, 0.5)).clamp(0.08, 1.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlayerRuntimeState {
    pub(crate) id: u64,
    pub(crate) pose: Option<SavedPlayerPose>,
    pub(crate) health: i32,
    pub(crate) max_health: i32,
    pub(crate) inventory: InventoryState,
}

impl PlayerRuntimeState {
    fn new_local() -> Self {
        Self {
            id: LOCAL_PLAYER_ID,
            pose: None,
            health: PLAYER_MAX_HEALTH,
            max_health: PLAYER_MAX_HEALTH,
            inventory: InventoryState::new_default_loadout(),
        }
    }

    pub(crate) fn apply_damage(&mut self, amount: i32) {
        self.health = clamp_health(self.health - amount.max(0), self.max_health);
    }

    pub(crate) fn heal(&mut self, amount: i32) {
        self.health = clamp_health(self.health + amount.max(0), self.max_health);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum StructureArchetype {
    Outpost,
    Ruin,
    Stronghold,
}

impl StructureArchetype {
    fn chest_tier(self) -> ChestTier {
        match self {
            Self::Outpost => ChestTier::Common,
            Self::Ruin => ChestTier::Rare,
            Self::Stronghold => ChestTier::Epic,
        }
    }

    fn spawner_count(self) -> usize {
        match self {
            Self::Outpost => 1,
            Self::Ruin => 1,
            Self::Stronghold => 2,
        }
    }

    fn chest_count(self) -> usize {
        match self {
            Self::Outpost => 1,
            Self::Ruin => 2,
            Self::Stronghold => 3,
        }
    }

    fn structure_color(self) -> [f32; 4] {
        match self {
            Self::Outpost => [0.44, 0.28, 0.16, 1.0],
            Self::Ruin => [0.48, 0.48, 0.52, 1.0],
            Self::Stronghold => [0.24, 0.24, 0.30, 1.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StructureInstance {
    pub(crate) id: u64,
    pub(crate) archetype: StructureArchetype,
    pub(crate) position: [i32; 3],
    pub(crate) seed: u64,
    pub(crate) chest_ids: Vec<u64>,
    pub(crate) spawner_ids: Vec<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum ChestTier {
    Common,
    Rare,
    Epic,
}

impl ChestTier {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Common => "COMMON",
            Self::Rare => "RARE",
            Self::Epic => "EPIC",
        }
    }

    fn color(self) -> [f32; 4] {
        match self {
            Self::Common => [0.70, 0.56, 0.28, 1.0],
            Self::Rare => [0.48, 0.62, 0.88, 1.0],
            Self::Epic => [0.88, 0.70, 0.28, 1.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChestState {
    pub(crate) id: u64,
    pub(crate) structure_id: Option<u64>,
    pub(crate) tier: ChestTier,
    pub(crate) position: [i32; 3],
    pub(crate) contents: Vec<ItemStack>,
    pub(crate) opened: bool,
    pub(crate) respawn_after_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpawnerState {
    pub(crate) id: u64,
    pub(crate) structure_id: Option<u64>,
    pub(crate) position: [i32; 3],
    pub(crate) cooldown_remaining: f32,
    pub(crate) base_cooldown: f32,
    pub(crate) base_cap: u32,
    pub(crate) night_multiplier: f32,
    pub(crate) active_enemy_ids: Vec<u64>,
}

impl SpawnerState {
    pub(crate) fn effective_cap(&self, time_of_day: TimeOfDayState) -> u32 {
        if time_of_day.is_night() {
            ((self.base_cap as f32) * self.night_multiplier)
                .round()
                .max(self.base_cap as f32) as u32
        } else {
            self.base_cap
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorldRuntimeState {
    pub(crate) version: u32,
    pub(crate) next_runtime_id: u64,
    pub(crate) time_of_day: TimeOfDayState,
    pub(crate) players: Vec<PlayerRuntimeState>,
    pub(crate) structures: Vec<StructureInstance>,
    pub(crate) chests: Vec<ChestState>,
    pub(crate) spawners: Vec<SpawnerState>,
    pub(crate) enemies: EnemyRoster,
}

impl WorldRuntimeState {
    pub(crate) fn new_singleplayer(seed: u32) -> Self {
        let mut state = Self {
            version: WORLD_RUNTIME_VERSION,
            next_runtime_id: 10,
            time_of_day: TimeOfDayState::default(),
            players: vec![PlayerRuntimeState::new_local()],
            structures: Vec::new(),
            chests: Vec::new(),
            spawners: Vec::new(),
            enemies: EnemyRoster::new(),
        };
        state.populate_generated_content(seed);
        state
    }

    pub(crate) fn local_player(&self) -> Option<&PlayerRuntimeState> {
        self.players
            .iter()
            .find(|player| player.id == LOCAL_PLAYER_ID)
    }

    pub(crate) fn local_player_mut(&mut self) -> Option<&mut PlayerRuntimeState> {
        self.players
            .iter_mut()
            .find(|player| player.id == LOCAL_PLAYER_ID)
    }

    pub(crate) fn sync_local_player_pose(&mut self, pose: SavedPlayerPose) {
        if let Some(player) = self.local_player_mut() {
            player.pose = Some(pose);
        }
    }

    pub(crate) fn local_player_pose(&self) -> Option<SavedPlayerPose> {
        self.local_player().and_then(|player| player.pose)
    }

    pub(crate) fn select_weapon_slot(&mut self, slot: usize) {
        if let Some(player) = self.local_player_mut() {
            player.inventory.select_weapon_slot(slot);
        }
    }

    pub(crate) fn selected_weapon_item(&self) -> Option<ItemId> {
        self.local_player()
            .and_then(|player| player.inventory.selected_weapon())
    }

    pub(crate) fn use_heal_item(&mut self) -> Option<String> {
        let player = self.local_player_mut()?;
        if player.inventory.consume_heal() {
            player.heal(35);
            Some(format!("USED MEDKIT HP {}", player.health))
        } else {
            Some("NO MEDKITS".to_string())
        }
    }

    pub(crate) fn consume_throwable(&mut self) -> bool {
        self.local_player_mut()
            .map(|player| player.inventory.consume_throwable())
            .unwrap_or(false)
    }

    pub(crate) fn terrain_snap(&mut self, seed: u32) {
        let generator = TerrainGenerator::new(seed);

        for structure in &mut self.structures {
            let surface = generator.surface_height(structure.position[0], structure.position[2]);
            structure.position[1] = surface + 1;
        }

        for chest in &mut self.chests {
            let surface = generator.surface_height(chest.position[0], chest.position[2]);
            chest.position[1] = surface + 1;
        }

        for spawner in &mut self.spawners {
            let surface = generator.surface_height(spawner.position[0], spawner.position[2]);
            spawner.position[1] = surface + 1;
        }

        self.enemies
            .snap_to_terrain(|x, z| Some(generator.surface_height(x, z)));
    }

    pub(crate) fn build_world_meshes(&self) -> Vec<StaticModelMesh> {
        let mut meshes = Vec::new();

        for structure in &self.structures {
            let base = Vec3::new(
                structure.position[0] as f32 + 0.5,
                structure.position[1] as f32,
                structure.position[2] as f32 + 0.5,
            );
            let color = structure.archetype.structure_color();

            match structure.archetype {
                StructureArchetype::Outpost => {
                    meshes.push(build_box_mesh(
                        format!("structure-{}-base", structure.id),
                        base + Vec3::new(-2.2, 0.0, -2.2),
                        base + Vec3::new(2.2, 2.6, 2.2),
                        color,
                    ));
                    meshes.push(build_box_mesh(
                        format!("structure-{}-roof", structure.id),
                        base + Vec3::new(-2.6, 2.5, -2.6),
                        base + Vec3::new(2.6, 3.1, 2.6),
                        [0.18, 0.12, 0.08, 1.0],
                    ));
                }
                StructureArchetype::Ruin => {
                    for (index, offset) in [
                        Vec3::new(-2.0, 0.0, -2.0),
                        Vec3::new(2.0, 0.0, -2.0),
                        Vec3::new(-2.0, 0.0, 2.0),
                        Vec3::new(2.0, 0.0, 2.0),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        meshes.push(build_box_mesh(
                            format!("structure-{}-pillar-{index}", structure.id),
                            base + offset + Vec3::new(-0.4, 0.0, -0.4),
                            base + offset + Vec3::new(0.4, 3.2, 0.4),
                            color,
                        ));
                    }
                    meshes.push(build_box_mesh(
                        format!("structure-{}-floor", structure.id),
                        base + Vec3::new(-2.6, -0.2, -2.6),
                        base + Vec3::new(2.6, 0.2, 2.6),
                        [0.30, 0.30, 0.34, 1.0],
                    ));
                }
                StructureArchetype::Stronghold => {
                    meshes.push(build_box_mesh(
                        format!("structure-{}-core", structure.id),
                        base + Vec3::new(-3.2, 0.0, -3.2),
                        base + Vec3::new(3.2, 3.8, 3.2),
                        color,
                    ));
                    meshes.push(build_box_mesh(
                        format!("structure-{}-gate", structure.id),
                        base + Vec3::new(-1.0, 0.0, -3.4),
                        base + Vec3::new(1.0, 2.0, -2.6),
                        [0.08, 0.08, 0.10, 1.0],
                    ));
                }
            }
        }

        for chest in &self.chests {
            let base = Vec3::new(
                chest.position[0] as f32 + 0.5,
                chest.position[1] as f32,
                chest.position[2] as f32 + 0.5,
            );
            let mut color = chest.tier.color();
            if chest.opened {
                color[3] = 0.55;
            }

            meshes.push(build_box_mesh(
                format!("chest-{}-base", chest.id),
                base + Vec3::new(-0.55, 0.0, -0.40),
                base + Vec3::new(0.55, 0.55, 0.40),
                color,
            ));
            meshes.push(build_box_mesh(
                format!("chest-{}-lid", chest.id),
                base + Vec3::new(-0.58, 0.52, -0.43),
                base + Vec3::new(0.58, 0.88, 0.43),
                [0.22, 0.14, 0.08, 1.0],
            ));
        }

        meshes
    }

    pub(crate) fn try_open_chest(
        &mut self,
        player_position: Vec3,
        player_forward: Vec3,
        max_occluder_distance: Option<f32>,
    ) -> Option<String> {
        let (target_index, target_distance) = self.find_target_chest(player_position, player_forward)?;
        if max_occluder_distance.is_some_and(|distance| distance + 0.001 < target_distance) {
            return Some("CHEST BLOCKED".to_string());
        }

        let looted_items = self.chests.get(target_index)?.contents.clone();
        let tier = self.chests.get(target_index)?.tier;
        if self.chests.get(target_index)?.opened {
            return Some(format!("{} CHEST IS EMPTY", tier.label()));
        }

        let Some(player) = self.local_player_mut() else {
            return None;
        };

        let mut looted = Vec::new();
        let mut remaining_contents = Vec::new();
        for stack in looted_items {
            let remaining = player.inventory.grant_item(stack.item_id, stack.count);
            let gained = stack.count.saturating_sub(remaining);
            if gained > 0 {
                looted.push(format!("{} X{}", stack.item_id.label(), gained));
            }
            if remaining > 0 {
                remaining_contents.push(ItemStack::new(stack.item_id, remaining));
            }
        }

        if let Some(chest) = self.chests.get_mut(target_index) {
            chest.contents = remaining_contents;
            chest.opened = chest.contents.is_empty();
        }

        Some(if looted.is_empty() {
            format!("{} CHEST HAD NO SPACEABLE LOOT", tier.label())
        } else if self
            .chests
            .get(target_index)
            .is_some_and(|chest| !chest.contents.is_empty())
        {
            format!(
                "LOOTED {} CHEST: {} (SOME ITEMS LEFT)",
                tier.label(),
                looted.join(", ")
            )
        } else {
            format!("LOOTED {} CHEST: {}", tier.label(), looted.join(", "))
        })
    }

    pub(crate) fn ray_static_prop_distance(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Option<f32> {
        let mut best_distance: Option<f32> = None;

        for structure in &self.structures {
            for (min, max) in structure_occluder_bounds(structure) {
                let Some(distance) = ray_aabb_distance(origin, direction, min, max) else {
                    continue;
                };
                if distance > max_distance {
                    continue;
                }
                match best_distance {
                    Some(current) if current <= distance => {}
                    _ => best_distance = Some(distance),
                }
            }
        }

        for chest in &self.chests {
            let (min, max) = chest_bounds(chest);
            let Some(distance) = ray_aabb_distance(origin, direction, min, max) else {
                continue;
            };
            if distance > max_distance {
                continue;
            }
            match best_distance {
                Some(current) if current <= distance => {}
                _ => best_distance = Some(distance),
            }
        }

        best_distance
    }

    pub(crate) fn static_prop_bounds(&self) -> Vec<(Vec3, Vec3)> {
        let mut bounds = Vec::with_capacity(self.structures.len() * 5 + self.chests.len());
        for structure in &self.structures {
            bounds.extend(structure_occluder_bounds(structure));
        }
        for chest in &self.chests {
            bounds.push(chest_bounds(chest));
        }
        bounds
    }

    pub(crate) fn is_static_prop_cell_solid(&self, world: IVec3) -> bool {
        let cell_min = world.as_vec3();
        let cell_max = cell_min + Vec3::ONE;
        self.static_prop_bounds()
            .into_iter()
            .any(|(min, max)| aabb_intersects_aabb(cell_min, cell_max, min, max))
    }

    pub(crate) fn static_prop_intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.static_prop_bounds()
            .into_iter()
            .any(|(min, max)| sphere_intersects_aabb(center, radius, min, max))
    }

    pub(crate) fn spawn_debug_chest(&mut self, position: IVec3, tier: ChestTier) -> u64 {
        let id = self.alloc_id();
        self.chests.push(ChestState {
            id,
            structure_id: None,
            tier,
            position: position.to_array(),
            contents: generate_loot_for_tier(tier, splitmix64(id)),
            opened: false,
            respawn_after_days: None,
        });
        id
    }

    pub(crate) fn tick_spawners<F>(
        &mut self,
        dt_seconds: f32,
        player_position: Vec3,
        mut find_surface_height: F,
    ) where
        F: FnMut(i32, i32) -> Option<i32>,
    {
        let enemy_ids: std::collections::HashSet<u64> = self
            .enemies
            .enemies()
            .iter()
            .map(|enemy| enemy.id)
            .collect();
        let mut pending_spawns = Vec::new();
        let mut next_runtime_id = self.next_runtime_id;

        for spawner in &mut self.spawners {
            spawner
                .active_enemy_ids
                .retain(|enemy_id| enemy_ids.contains(enemy_id));
            spawner.cooldown_remaining = (spawner.cooldown_remaining - dt_seconds).max(0.0);

            let spawner_position = Vec3::new(
                spawner.position[0] as f32 + 0.5,
                spawner.position[1] as f32,
                spawner.position[2] as f32 + 0.5,
            );

            if player_position.distance(spawner_position) > 64.0 {
                continue;
            }

            let cap = spawner.effective_cap(self.time_of_day) as usize;
            if spawner.active_enemy_ids.len() >= cap || spawner.cooldown_remaining > 0.0 {
                continue;
            }

            let spawn_id = next_runtime_id;
            next_runtime_id = next_runtime_id.saturating_add(1);
            let spawn_position = {
                let mut position = spawner_position;
                position.x += (spawn_id % 3) as f32 - 1.0;
                position.z += ((spawn_id / 3) % 3) as f32 - 1.0;
                if let Some(surface) =
                    find_surface_height(position.x.round() as i32, position.z.round() as i32)
                {
                    position.y = surface as f32 + 1.0;
                }
                position
            };

            pending_spawns.push((spawner.id, spawn_id, spawn_position));
            spawner.active_enemy_ids.push(spawn_id);
            spawner.cooldown_remaining = spawner.base_cooldown;
        }

        self.next_runtime_id = next_runtime_id;
        for (spawner_id, enemy_id, position) in pending_spawns {
            self.enemies
                .spawn_enemy(enemy_id, EnemyKind::MeleeHunter, position, Some(spawner_id));
        }
    }

    pub(crate) fn tick_enemy_ai<F, G>(
        &mut self,
        dt_seconds: f32,
        player_position: Vec3,
        find_surface_height: F,
        is_walk_blocked: G,
    ) where
        F: FnMut(i32, i32) -> Option<i32>,
        G: FnMut(IVec3) -> bool,
    {
        let Some(local_player_index) = self
            .players
            .iter()
            .position(|player| player.id == LOCAL_PLAYER_ID)
        else {
            return;
        };
        let player_runtime = &mut self.players[local_player_index];
        self.enemies.tick_ai(
            dt_seconds,
            player_position,
            player_runtime,
            find_surface_height,
            is_walk_blocked,
        );
    }

    fn find_target_chest(&self, origin: Vec3, direction: Vec3) -> Option<(usize, f32)> {
        let direction = direction.normalize_or_zero();
        let mut best: Option<(usize, f32)> = None;

        for (index, chest) in self.chests.iter().enumerate() {
            let (min, max) = chest_bounds(chest);

            let Some(distance) = ray_aabb_distance(origin, direction, min, max) else {
                continue;
            };
            if distance > 4.5 {
                continue;
            }

            match best {
                Some((_, best_distance)) if best_distance <= distance => {}
                _ => best = Some((index, distance)),
            }
        }

        best
    }

    pub(crate) fn alloc_id(&mut self) -> u64 {
        let id = self.next_runtime_id;
        self.next_runtime_id = self.next_runtime_id.saturating_add(1);
        id
    }

    fn populate_generated_content(&mut self, seed: u32) {
        self.structures.clear();
        self.chests.clear();
        self.spawners.clear();

        let archetypes = [
            StructureArchetype::Outpost,
            StructureArchetype::Outpost,
            StructureArchetype::Ruin,
            StructureArchetype::Ruin,
            StructureArchetype::Stronghold,
        ];

        for (index, archetype) in archetypes.into_iter().enumerate() {
            let salt = splitmix64(seed as u64 ^ (index as u64).wrapping_mul(0x9E37_79B9));
            let angle = (index as f32 / archetypes.len() as f32) * std::f32::consts::TAU
                + (salt as f32 / u64::MAX as f32) * 0.65;
            let radius = 18.0 + index as f32 * 9.0 + (salt % 7) as f32;
            let x = (angle.cos() * radius).round() as i32;
            let z = (angle.sin() * radius).round() as i32;
            let structure_id = self.alloc_id();
            let structure_seed = splitmix64(salt ^ 0xA0A0_1F1Fu64);

            let mut chest_ids = Vec::new();
            let mut spawner_ids = Vec::new();

            for chest_index in 0..archetype.chest_count() {
                let offset = structure_socket_offset(chest_index, true);
                let chest_id = self.alloc_id();
                chest_ids.push(chest_id);
                self.chests.push(ChestState {
                    id: chest_id,
                    structure_id: Some(structure_id),
                    tier: archetype.chest_tier(),
                    position: [x + offset.x, 0, z + offset.z],
                    contents: generate_loot_for_tier(
                        archetype.chest_tier(),
                        structure_seed ^ chest_id,
                    ),
                    opened: false,
                    respawn_after_days: None,
                });
            }

            for spawner_index in 0..archetype.spawner_count() {
                let offset = structure_socket_offset(spawner_index, false);
                let spawner_id = self.alloc_id();
                spawner_ids.push(spawner_id);
                self.spawners.push(SpawnerState {
                    id: spawner_id,
                    structure_id: Some(structure_id),
                    position: [x + offset.x, 0, z + offset.z],
                    cooldown_remaining: 1.5 + spawner_index as f32,
                    base_cooldown: match archetype {
                        StructureArchetype::Outpost => 8.0,
                        StructureArchetype::Ruin => 6.0,
                        StructureArchetype::Stronghold => 4.0,
                    },
                    base_cap: match archetype {
                        StructureArchetype::Outpost => 1,
                        StructureArchetype::Ruin => 2,
                        StructureArchetype::Stronghold => 3,
                    },
                    night_multiplier: match archetype {
                        StructureArchetype::Outpost => 2.0,
                        StructureArchetype::Ruin => 2.0,
                        StructureArchetype::Stronghold => 2.5,
                    },
                    active_enemy_ids: Vec::new(),
                });
            }

            self.structures.push(StructureInstance {
                id: structure_id,
                archetype,
                position: [x, 0, z],
                seed: structure_seed,
                chest_ids,
                spawner_ids,
            });
        }

        self.terrain_snap(seed);
    }
}

fn structure_socket_offset(index: usize, chest: bool) -> IVec3 {
    let offsets = if chest {
        [
            IVec3::new(2, 0, 2),
            IVec3::new(-2, 0, 2),
            IVec3::new(0, 0, -2),
        ]
    } else {
        [
            IVec3::new(5, 0, 0),
            IVec3::new(-5, 0, 0),
            IVec3::new(0, 0, 5),
        ]
    };

    offsets[index % offsets.len()]
}

fn structure_occluder_bounds(structure: &StructureInstance) -> Vec<(Vec3, Vec3)> {
    let base = Vec3::new(
        structure.position[0] as f32 + 0.5,
        structure.position[1] as f32,
        structure.position[2] as f32 + 0.5,
    );

    match structure.archetype {
        StructureArchetype::Outpost => vec![
            (
                base + Vec3::new(-2.2, 0.0, -2.2),
                base + Vec3::new(2.2, 2.6, 2.2),
            ),
            (
                base + Vec3::new(-2.6, 2.5, -2.6),
                base + Vec3::new(2.6, 3.1, 2.6),
            ),
        ],
        StructureArchetype::Ruin => vec![
            (
                base + Vec3::new(-2.6, -0.2, -2.6),
                base + Vec3::new(2.6, 0.2, 2.6),
            ),
            (
                base + Vec3::new(-2.4, 0.0, -2.4),
                base + Vec3::new(-1.6, 3.2, -1.6),
            ),
            (
                base + Vec3::new(1.6, 0.0, -2.4),
                base + Vec3::new(2.4, 3.2, -1.6),
            ),
            (
                base + Vec3::new(-2.4, 0.0, 1.6),
                base + Vec3::new(-1.6, 3.2, 2.4),
            ),
            (
                base + Vec3::new(1.6, 0.0, 1.6),
                base + Vec3::new(2.4, 3.2, 2.4),
            ),
        ],
        StructureArchetype::Stronghold => vec![
            (
                base + Vec3::new(-3.2, 0.0, -3.2),
                base + Vec3::new(3.2, 3.8, 3.2),
            ),
            (
                base + Vec3::new(-1.0, 0.0, -3.4),
                base + Vec3::new(1.0, 2.0, -2.6),
            ),
        ],
    }
}

fn chest_bounds(chest: &ChestState) -> (Vec3, Vec3) {
    let base = Vec3::new(
        chest.position[0] as f32,
        chest.position[1] as f32,
        chest.position[2] as f32,
    );
    (
        base + Vec3::new(-0.1, 0.0, -0.1),
        base + Vec3::new(1.1, 1.0, 1.1),
    )
}

fn generate_loot_for_tier(tier: ChestTier, seed: u64) -> Vec<ItemStack> {
    let mut loot = Vec::new();
    let medkits = ((seed & 0x3) as u32) + 1;
    let grenades = (((seed >> 7) & 0x3) as u32) + 1;

    match tier {
        ChestTier::Common => {
            loot.push(ItemStack::new(ItemId::Medkit, medkits.min(2)));
            if seed & 1 == 0 {
                loot.push(ItemStack::new(ItemId::Grenade, 1));
            }
        }
        ChestTier::Rare => {
            loot.push(ItemStack::new(ItemId::Medkit, medkits.min(3) + 1));
            loot.push(ItemStack::new(ItemId::Grenade, grenades.min(3) + 1));
        }
        ChestTier::Epic => {
            loot.push(ItemStack::new(ItemId::Medkit, medkits.min(4) + 2));
            loot.push(ItemStack::new(ItemId::Grenade, grenades.min(4) + 2));
        }
    }

    loot
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn ray_aabb_distance(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv_dir = Vec3::new(
        if direction.x.abs() > f32::EPSILON {
            1.0 / direction.x
        } else {
            f32::INFINITY
        },
        if direction.y.abs() > f32::EPSILON {
            1.0 / direction.y
        } else {
            f32::INFINITY
        },
        if direction.z.abs() > f32::EPSILON {
            1.0 / direction.z
        } else {
            f32::INFINITY
        },
    );

    let t1 = (min - origin) * inv_dir;
    let t2 = (max - origin) * inv_dir;
    let t_min = t1.min(t2);
    let t_max = t1.max(t2);
    let near = t_min.max_element();
    let far = t_max.min_element();

    if far < 0.0 || near > far {
        None
    } else {
        Some(near.max(0.0))
    }
}

fn aabb_intersects_aabb(min_a: Vec3, max_a: Vec3, min_b: Vec3, max_b: Vec3) -> bool {
    min_a.x < max_b.x
        && max_a.x > min_b.x
        && min_a.y < max_b.y
        && max_a.y > min_b.y
        && min_a.z < max_b.z
        && max_a.z > min_b.z
}

fn sphere_intersects_aabb(center: Vec3, radius: f32, min: Vec3, max: Vec3) -> bool {
    let closest = center.clamp(min, max);
    center.distance_squared(closest) <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_world_contains_structures_chests_and_spawners() {
        let state = WorldRuntimeState::new_singleplayer(42);
        assert!(!state.structures.is_empty());
        assert!(!state.chests.is_empty());
        assert!(!state.spawners.is_empty());
    }

    #[test]
    fn time_wraps_across_full_day() {
        let mut time = TimeOfDayState::default();
        time.advance(DAY_NIGHT_CYCLE_SECONDS + 10.0);
        assert_eq!(time.elapsed_days, 1);
        assert!(time.normalized_time > 0.0);
    }

    #[test]
    fn structures_block_cell_collision_queries() {
        let state = WorldRuntimeState::new_singleplayer(42);
        let structure = state.structures.first().expect("expected generated structure");
        let world = IVec3::new(
            structure.position[0],
            structure.position[1] + 1,
            structure.position[2],
        );

        assert!(state.is_static_prop_cell_solid(world));
    }

    #[test]
    fn static_props_block_sphere_collision_queries() {
        let state = WorldRuntimeState::new_singleplayer(42);
        let chest = state.chests.first().expect("expected generated chest");
        let center = Vec3::new(
            chest.position[0] as f32 + 0.5,
            chest.position[1] as f32 + 0.5,
            chest.position[2] as f32 + 0.5,
        );

        assert!(state.static_prop_intersects_sphere(center, 0.2));
    }
}
