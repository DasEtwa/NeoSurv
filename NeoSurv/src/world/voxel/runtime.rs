use std::collections::{HashMap, HashSet, VecDeque};

use glam::{IVec3, Mat4, Vec3};

use crate::world::voxel::{
    block::BlockType,
    chunk::{ChunkCoord, ChunkData, split_world_position},
    culling::{Aabb, Frustum},
    meshing::{ChunkMesh, ChunkNeighborSolidity},
    pipeline::{ChunkBuildOutput, ChunkBuildResult, ChunkGenerationPipeline},
    raycast::raycast_voxels,
};

const FRUSTUM_CULL_TELEMETRY_INTERVAL_CALLS: u64 = 240;

#[derive(Debug, Clone)]
pub(crate) enum ChunkMeshUpdate {
    Upsert { coord: ChunkCoord, mesh: ChunkMesh },
    Remove { coord: ChunkCoord },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct VoxelTickReport {
    pub(crate) requested: usize,
    pub(crate) completed: usize,
    pub(crate) loaded_chunks: usize,
    pub(crate) pending_chunks: usize,
    pub(crate) mesh_updates_queued: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ChunkResidentState {
    #[default]
    Absent,
    Resident,
}

#[derive(Debug, Clone, Copy, Default)]
struct ChunkRuntimeState {
    requested_revision: u64,
    integrated_revision: u64,
    pending_revision: Option<u64>,
    needs_remesh_after_pending: bool,
    resident_state: ChunkResidentState,
}

#[derive(Debug)]
pub(crate) struct VoxelWorld {
    pipeline: ChunkGenerationPipeline,
    chunks: HashMap<ChunkCoord, ChunkData>,
    meshes: HashMap<ChunkCoord, ChunkMesh>,
    chunk_states: HashMap<ChunkCoord, ChunkRuntimeState>,
    integration_backlog: VecDeque<ChunkBuildResult>,
    mesh_updates: VecDeque<ChunkMeshUpdate>,
    dirty_remesh_queue: VecDeque<ChunkCoord>,
    dirty_remesh_set: HashSet<ChunkCoord>,
    horizontal_radius: i32,
    vertical_radius: i32,
    chunk_retention_horizontal_radius: u32,
    chunk_retention_vertical_radius: u32,
    max_chunk_integrations_per_tick: usize,
    max_completed_drains_per_tick: usize,
    max_dirty_remesh_requests_per_tick: usize,
    frustum_cull_call_count: u64,
}

impl VoxelWorld {
    pub(crate) fn new(seed: u32, worker_count: usize) -> Self {
        let horizontal_radius = 4;
        let vertical_radius = 2;

        Self {
            pipeline: ChunkGenerationPipeline::new(seed, worker_count),
            chunks: HashMap::new(),
            meshes: HashMap::new(),
            chunk_states: HashMap::new(),
            integration_backlog: VecDeque::new(),
            mesh_updates: VecDeque::new(),
            dirty_remesh_queue: VecDeque::new(),
            dirty_remesh_set: HashSet::new(),
            horizontal_radius,
            vertical_radius,
            chunk_retention_horizontal_radius: (horizontal_radius + 1) as u32,
            chunk_retention_vertical_radius: (vertical_radius + 1) as u32,
            max_chunk_integrations_per_tick: 16,
            max_completed_drains_per_tick: 128,
            max_dirty_remesh_requests_per_tick: 8,
            frustum_cull_call_count: 0,
        }
    }

    pub(crate) fn tick(&mut self, camera_position: Vec3) -> VoxelTickReport {
        let camera_world = camera_position.floor().as_ivec3();
        let center_chunk = ChunkCoord::from_world(camera_world);

        self.evict_outside_retention(center_chunk);

        let mut requested = 0;

        for y in -self.vertical_radius..=self.vertical_radius {
            for z in -self.horizontal_radius..=self.horizontal_radius {
                for x in -self.horizontal_radius..=self.horizontal_radius {
                    let coord =
                        ChunkCoord::new(center_chunk.x + x, center_chunk.y + y, center_chunk.z + z);

                    if self.chunks.contains_key(&coord) || self.has_pending_request(coord) {
                        continue;
                    }

                    if self.request_generate_chunk(coord) {
                        requested += 1;
                    }
                }
            }
        }

        for _ in 0..self.max_dirty_remesh_requests_per_tick.max(1) {
            let Some(coord) = self.dirty_remesh_queue.pop_front() else {
                break;
            };
            self.dirty_remesh_set.remove(&coord);

            if !self.chunks.contains_key(&coord) || !self.is_within_retention(center_chunk, coord) {
                continue;
            }

            if self.has_pending_request(coord) {
                if self.defer_remesh_until_pending_finishes(coord) {
                    requested += 1;
                }
                continue;
            }

            if self.request_remesh_chunk(coord) {
                requested += 1;
            }
        }

        for built in self
            .pipeline
            .drain_completed(self.max_completed_drains_per_tick.max(1))
        {
            let Some(state) = self.chunk_states.get_mut(&built.coord) else {
                continue;
            };

            if state.pending_revision != Some(built.revision) {
                continue;
            }

            state.pending_revision = None;

            if self.is_within_retention(center_chunk, built.coord) {
                self.integration_backlog.push_back(built);
            }
        }

        let mut completed = 0;
        for _ in 0..self.max_chunk_integrations_per_tick.max(1) {
            let Some(built) = self.integration_backlog.pop_front() else {
                break;
            };

            if !self.is_within_retention(center_chunk, built.coord) {
                continue;
            }

            if self.integrate_chunk_result(center_chunk, built) {
                completed += 1;
            }
        }

        VoxelTickReport {
            requested,
            completed,
            loaded_chunks: self.chunks.len(),
            pending_chunks: self.pending_chunk_count(),
            mesh_updates_queued: self.mesh_updates.len(),
        }
    }

    pub(crate) fn mark_chunk_dirty_for_remesh(&mut self, coord: ChunkCoord) -> bool {
        if !self.chunks.contains_key(&coord) {
            return false;
        }

        if self.has_pending_request(coord) {
            return self.defer_remesh_until_pending_finishes(coord);
        }

        if !self.dirty_remesh_set.insert(coord) {
            return false;
        }

        self.chunk_states
            .entry(coord)
            .or_insert_with(|| Self::resident_state_for_loaded_chunk());
        self.dirty_remesh_queue.push_back(coord);
        true
    }

    pub(crate) fn drain_mesh_updates(&mut self, max_updates: usize) -> Vec<ChunkMeshUpdate> {
        let mut updates = Vec::with_capacity(max_updates.max(1));

        for _ in 0..max_updates.max(1) {
            let Some(update) = self.mesh_updates.pop_front() else {
                break;
            };
            updates.push(update);
        }

        updates
    }

    pub(crate) fn pending_mesh_update_count(&self) -> usize {
        self.mesh_updates.len()
    }

    pub(crate) fn visible_chunk_coords(
        &mut self,
        camera_position: Vec3,
        max_distance_in_chunks: u32,
        view: Mat4,
        projection: Mat4,
    ) -> Vec<ChunkCoord> {
        let center_chunk = ChunkCoord::from_world(camera_position.floor().as_ivec3());
        let radius_sq = {
            let radius = i64::from(max_distance_in_chunks.max(1));
            radius * radius
        };
        let frustum = Frustum::from_view_projection(view, projection);
        let mut candidates_in_radius = 0usize;

        let mut visible: Vec<_> = self
            .meshes
            .keys()
            .copied()
            .filter(|coord| {
                let in_distance = Self::chunk_distance_sq(*coord, center_chunk) <= radius_sq;
                if !in_distance {
                    return false;
                }

                candidates_in_radius += 1;

                match frustum {
                    Some(frustum) => frustum.intersects_aabb(&Aabb::from_chunk_coord(*coord)),
                    None => true,
                }
            })
            .collect();

        visible.sort_by_key(|coord| Self::chunk_distance_sq(*coord, center_chunk));

        self.frustum_cull_call_count = self.frustum_cull_call_count.wrapping_add(1);
        if self
            .frustum_cull_call_count
            .is_multiple_of(FRUSTUM_CULL_TELEMETRY_INTERVAL_CALLS)
        {
            tracing::debug!(
                frustum_valid = frustum.is_some(),
                candidates = candidates_in_radius,
                visible = visible.len(),
                max_distance_in_chunks,
                "voxel frustum culling"
            );
        }

        visible
    }

    pub(crate) fn block_at_world(&self, world: IVec3) -> Option<BlockType> {
        let (chunk_coord, local) = split_world_position(world);
        self.chunks
            .get(&chunk_coord)
            .map(|chunk| chunk.block(local))
            .filter(|block| block.is_solid())
    }

    pub(crate) fn raycast_solid_distance(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Option<f32> {
        raycast_voxels(origin, direction, max_distance, |world| self.block_at_world(world))
            .map(|hit| hit.distance)
    }

    #[allow(dead_code)]
    pub(crate) fn set_block_world(&mut self, world: IVec3, block: BlockType) -> bool {
        let (chunk_coord, local) = split_world_position(world);

        {
            let Some(chunk) = self.chunks.get_mut(&chunk_coord) else {
                return false;
            };

            if chunk.block(local) == block {
                return true;
            }

            chunk.set_block(local, block);
        }

        let _ = self.mark_chunk_dirty_for_remesh(chunk_coord);
        self.mark_adjacent_chunks_dirty_for_remesh(chunk_coord);
        true
    }

    #[cfg(test)]
    pub(crate) fn loaded_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    fn pending_chunk_count(&self) -> usize {
        self.chunk_states
            .values()
            .filter(|state| state.pending_revision.is_some())
            .count()
    }

    fn has_pending_request(&self, coord: ChunkCoord) -> bool {
        self.chunk_states
            .get(&coord)
            .and_then(|state| state.pending_revision)
            .is_some()
    }

    fn resident_state_for_loaded_chunk() -> ChunkRuntimeState {
        ChunkRuntimeState {
            resident_state: ChunkResidentState::Resident,
            ..ChunkRuntimeState::default()
        }
    }

    fn request_generate_chunk(&mut self, coord: ChunkCoord) -> bool {
        let next_revision = self
            .chunk_states
            .get(&coord)
            .map(|state| state.requested_revision)
            .unwrap_or(0)
            .saturating_add(1);
        let neighbors = self.collect_neighbor_solidity(coord);

        if !self
            .pipeline
            .request_generate_chunk(coord, next_revision, neighbors)
        {
            return false;
        }

        let state = self.chunk_states.entry(coord).or_default();
        state.requested_revision = next_revision;
        state.pending_revision = Some(next_revision);
        state.needs_remesh_after_pending = false;
        true
    }

    fn request_remesh_chunk(&mut self, coord: ChunkCoord) -> bool {
        let Some(chunk_snapshot) = self.chunks.get(&coord).cloned() else {
            return false;
        };
        let next_revision = self
            .chunk_states
            .get(&coord)
            .map(|state| state.requested_revision)
            .unwrap_or(0)
            .saturating_add(1);
        let neighbors = self.collect_neighbor_solidity(coord);

        if !self
            .pipeline
            .request_remesh(coord, next_revision, chunk_snapshot, neighbors)
        {
            return false;
        }

        let state = self
            .chunk_states
            .entry(coord)
            .or_insert_with(|| Self::resident_state_for_loaded_chunk());
        state.requested_revision = next_revision;
        state.pending_revision = Some(next_revision);
        state.needs_remesh_after_pending = false;
        state.resident_state = ChunkResidentState::Resident;
        true
    }

    fn defer_remesh_until_pending_finishes(&mut self, coord: ChunkCoord) -> bool {
        let state = self
            .chunk_states
            .entry(coord)
            .or_insert_with(|| Self::resident_state_for_loaded_chunk());

        if state.pending_revision.is_none() || state.needs_remesh_after_pending {
            return false;
        }

        state.requested_revision = state
            .requested_revision
            .max(state.pending_revision.unwrap_or(0))
            .saturating_add(1);
        state.needs_remesh_after_pending = true;
        true
    }

    fn queue_deferred_remesh_if_needed(&mut self, coord: ChunkCoord) -> bool {
        let (revision, should_request) = {
            let Some(state) = self.chunk_states.get(&coord) else {
                return false;
            };
            (
                state.requested_revision,
                state.needs_remesh_after_pending
                    && state.pending_revision.is_none()
                    && state.requested_revision > state.integrated_revision
                    && self.chunks.contains_key(&coord),
            )
        };

        if !should_request {
            return false;
        }

        let Some(chunk_snapshot) = self.chunks.get(&coord).cloned() else {
            return false;
        };
        let neighbors = self.collect_neighbor_solidity(coord);

        if !self
            .pipeline
            .request_remesh(coord, revision, chunk_snapshot, neighbors)
        {
            return false;
        }

        if let Some(state) = self.chunk_states.get_mut(&coord) {
            state.pending_revision = Some(revision);
            state.needs_remesh_after_pending = false;
            return true;
        }

        false
    }

    fn integrate_chunk_result(&mut self, center_chunk: ChunkCoord, built: ChunkBuildResult) -> bool {
        let Some(state_snapshot) = self.chunk_states.get(&built.coord).copied() else {
            return false;
        };

        let waiting_for_followup =
            state_snapshot.needs_remesh_after_pending && built.revision < state_snapshot.requested_revision;

        if built.revision <= state_snapshot.integrated_revision
            || (built.revision < state_snapshot.requested_revision && !waiting_for_followup)
        {
            return false;
        }

        if let Some(state) = self.chunk_states.get_mut(&built.coord)
            && state.pending_revision == Some(built.revision)
        {
            state.pending_revision = None;
        }

        let mut chunk_changed = false;
        match built.output {
            ChunkBuildOutput::BuiltMesh(mesh) => {
                chunk_changed = self
                    .chunks
                    .get(&built.coord)
                    .map(|existing| existing != &built.chunk)
                    .unwrap_or(true);
                self.chunks.insert(built.coord, built.chunk);
                self.meshes.insert(built.coord, mesh.clone());
                self.mesh_updates.push_back(ChunkMeshUpdate::Upsert {
                    coord: built.coord,
                    mesh,
                });
                if let Some(state) = self.chunk_states.get_mut(&built.coord) {
                    state.resident_state = ChunkResidentState::Resident;
                }
            }
            ChunkBuildOutput::BuiltEmptyButValid => {
                chunk_changed = self
                    .chunks
                    .get(&built.coord)
                    .map(|existing| existing != &built.chunk)
                    .unwrap_or(true);
                self.chunks.insert(built.coord, built.chunk);
                self.meshes.remove(&built.coord);
                self.mesh_updates
                    .push_back(ChunkMeshUpdate::Remove { coord: built.coord });
                if let Some(state) = self.chunk_states.get_mut(&built.coord) {
                    state.resident_state = ChunkResidentState::Resident;
                }
            }
            ChunkBuildOutput::SkippedOrNotReady | ChunkBuildOutput::Failed => {}
        }

        if let Some(state) = self.chunk_states.get_mut(&built.coord) {
            state.integrated_revision = built.revision;
        }

        if chunk_changed {
            self.mark_adjacent_chunks_dirty_for_remesh(built.coord);
        }

        let _ = self.queue_deferred_remesh_if_needed(built.coord);

        if !self.is_within_retention(center_chunk, built.coord) {
            return false;
        }

        true
    }

    fn chunk_distance_sq(coord: ChunkCoord, center: ChunkCoord) -> i64 {
        let dx = i64::from(coord.x - center.x);
        let dy = i64::from(coord.y - center.y);
        let dz = i64::from(coord.z - center.z);
        dx * dx + dy * dy + dz * dz
    }

    fn is_within_retention(&self, center: ChunkCoord, coord: ChunkCoord) -> bool {
        Self::is_within_retention_bounds(
            center,
            coord,
            self.chunk_retention_horizontal_radius,
            self.chunk_retention_vertical_radius,
        )
    }

    fn is_within_retention_bounds(
        center: ChunkCoord,
        coord: ChunkCoord,
        horizontal_radius: u32,
        vertical_radius: u32,
    ) -> bool {
        center.x.abs_diff(coord.x) <= horizontal_radius
            && center.y.abs_diff(coord.y) <= vertical_radius
            && center.z.abs_diff(coord.z) <= horizontal_radius
    }

    fn collect_neighbor_solidity(&self, coord: ChunkCoord) -> ChunkNeighborSolidity {
        let mut neighbors = ChunkNeighborSolidity::default();

        if let Some(chunk) = self
            .chunks
            .get(&ChunkCoord::new(coord.x + 1, coord.y, coord.z))
        {
            neighbors.set_positive_x(chunk);
        }
        if let Some(chunk) = self
            .chunks
            .get(&ChunkCoord::new(coord.x - 1, coord.y, coord.z))
        {
            neighbors.set_negative_x(chunk);
        }
        if let Some(chunk) = self
            .chunks
            .get(&ChunkCoord::new(coord.x, coord.y + 1, coord.z))
        {
            neighbors.set_positive_y(chunk);
        }
        if let Some(chunk) = self
            .chunks
            .get(&ChunkCoord::new(coord.x, coord.y - 1, coord.z))
        {
            neighbors.set_negative_y(chunk);
        }
        if let Some(chunk) = self
            .chunks
            .get(&ChunkCoord::new(coord.x, coord.y, coord.z + 1))
        {
            neighbors.set_positive_z(chunk);
        }
        if let Some(chunk) = self
            .chunks
            .get(&ChunkCoord::new(coord.x, coord.y, coord.z - 1))
        {
            neighbors.set_negative_z(chunk);
        }

        neighbors
    }

    fn mark_adjacent_chunks_dirty_for_remesh(&mut self, coord: ChunkCoord) {
        let neighbors = [
            ChunkCoord::new(coord.x + 1, coord.y, coord.z),
            ChunkCoord::new(coord.x - 1, coord.y, coord.z),
            ChunkCoord::new(coord.x, coord.y + 1, coord.z),
            ChunkCoord::new(coord.x, coord.y - 1, coord.z),
            ChunkCoord::new(coord.x, coord.y, coord.z + 1),
            ChunkCoord::new(coord.x, coord.y, coord.z - 1),
        ];

        for neighbor in neighbors {
            let _ = self.mark_chunk_dirty_for_remesh(neighbor);
        }
    }

    fn evict_outside_retention(&mut self, center: ChunkCoord) {
        let horizontal_radius = self.chunk_retention_horizontal_radius;
        let vertical_radius = self.chunk_retention_vertical_radius;
        let is_within = |coord: ChunkCoord| {
            Self::is_within_retention_bounds(center, coord, horizontal_radius, vertical_radius)
        };

        self.chunks.retain(|coord, _| is_within(*coord));

        let mut removed_meshes = Vec::new();
        self.meshes.retain(|coord, _| {
            let keep = is_within(*coord);
            if !keep {
                removed_meshes.push(*coord);
            }
            keep
        });

        self.chunk_states.retain(|coord, _| is_within(*coord));
        self.integration_backlog
            .retain(|built| is_within(built.coord));

        self.mesh_updates.retain(|update| match update {
            ChunkMeshUpdate::Upsert { coord, .. } => is_within(*coord),
            ChunkMeshUpdate::Remove { .. } => true,
        });

        self.dirty_remesh_queue.retain(|coord| is_within(*coord));
        self.dirty_remesh_set.retain(|coord| is_within(*coord));

        for coord in removed_meshes {
            self.mesh_updates
                .push_back(ChunkMeshUpdate::Remove { coord });
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::Mat4;

    use super::*;
    use crate::{
        world::camera::Camera,
        world::voxel::{chunk::LocalCoord, meshing::build_chunk_mesh},
    };

    fn chunk_result(coord: ChunkCoord) -> ChunkBuildResult {
        ChunkBuildResult {
            coord,
            revision: 1,
            chunk: ChunkData::new(coord),
            output: ChunkBuildOutput::BuiltEmptyButValid,
        }
    }

    fn solid_chunk_result(coord: ChunkCoord) -> ChunkBuildResult {
        let mut chunk = ChunkData::new(coord);
        chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        let mesh = build_chunk_mesh(&chunk);

        ChunkBuildResult {
            coord,
            revision: 1,
            chunk,
            output: ChunkBuildOutput::BuiltMesh(mesh),
        }
    }

    fn resident_state(integrated_revision: u64) -> ChunkRuntimeState {
        ChunkRuntimeState {
            integrated_revision,
            resident_state: ChunkResidentState::Resident,
            ..ChunkRuntimeState::default()
        }
    }

    #[test]
    fn world_tick_submits_chunk_jobs() {
        let mut world = VoxelWorld::new(13, 1);
        let report = world.tick(Vec3::ZERO);

        assert!(report.requested > 0);
        assert!(report.pending_chunks > 0);
    }

    #[test]
    fn tick_evicts_chunks_outside_retention_radius() {
        let mut world = VoxelWorld::new(13, 1);
        world.chunk_retention_horizontal_radius = 1;
        world.chunk_retention_vertical_radius = 1;
        world.horizontal_radius = 0;
        world.vertical_radius = 0;

        let near = ChunkCoord::new(0, 0, 0);
        let far = ChunkCoord::new(6, 0, 6);

        world.chunks.insert(near, ChunkData::new(near));
        world.chunks.insert(far, ChunkData::new(far));
        world.meshes.insert(near, ChunkMesh::default());
        world.meshes.insert(far, ChunkMesh::default());
        world.chunk_states.insert(
            far,
            ChunkRuntimeState {
                requested_revision: 1,
                pending_revision: Some(1),
                resident_state: ChunkResidentState::Resident,
                ..ChunkRuntimeState::default()
            },
        );
        world.integration_backlog.push_back(chunk_result(far));

        world.tick(Vec3::ZERO);

        assert!(world.chunks.contains_key(&near));
        assert!(!world.chunks.contains_key(&far));
        assert!(!world.meshes.contains_key(&far));
        assert!(!world.chunk_states.contains_key(&far));
        assert!(
            world
                .integration_backlog
                .iter()
                .all(|built| built.coord != far)
        );

        let updates = world.drain_mesh_updates(8);
        assert!(
            updates
                .iter()
                .any(|update| matches!(update, ChunkMeshUpdate::Remove { coord } if *coord == far))
        );
    }

    #[test]
    fn tick_limits_chunk_integrations_per_frame() {
        let mut world = VoxelWorld::new(13, 1);
        world.horizontal_radius = -1;
        world.vertical_radius = -1;
        world.chunk_retention_horizontal_radius = 64;
        world.chunk_retention_vertical_radius = 64;
        world.max_chunk_integrations_per_tick = 2;

        world
            .integration_backlog
            .push_back(chunk_result(ChunkCoord::new(0, 0, 0)));
        world.chunk_states.insert(ChunkCoord::new(0, 0, 0), ChunkRuntimeState::default());
        world
            .integration_backlog
            .push_back(chunk_result(ChunkCoord::new(1, 0, 0)));
        world.chunk_states.insert(ChunkCoord::new(1, 0, 0), ChunkRuntimeState::default());
        world
            .integration_backlog
            .push_back(chunk_result(ChunkCoord::new(2, 0, 0)));
        world.chunk_states.insert(ChunkCoord::new(2, 0, 0), ChunkRuntimeState::default());

        let report = world.tick(Vec3::ZERO);

        assert_eq!(report.completed, 2);
        assert_eq!(world.loaded_chunk_count(), 2);
        assert_eq!(world.integration_backlog.len(), 1);
    }

    #[test]
    fn integrates_mesh_updates_for_renderer_upload_queue() {
        let mut world = VoxelWorld::new(13, 1);
        world.horizontal_radius = -1;
        world.vertical_radius = -1;
        world.chunk_retention_horizontal_radius = 8;
        world.chunk_retention_vertical_radius = 8;

        let coord = ChunkCoord::new(0, 0, 0);
        world.chunk_states.insert(coord, ChunkRuntimeState::default());
        world
            .integration_backlog
            .push_back(solid_chunk_result(coord));

        let report = world.tick(Vec3::ZERO);
        assert_eq!(report.completed, 1);

        let updates = world.drain_mesh_updates(4);
        assert!(updates.iter().any(
            |update| matches!(update, ChunkMeshUpdate::Upsert { coord: c, .. } if *c == coord)
        ));
    }

    #[test]
    fn dirty_remesh_marking_is_deduplicated() {
        let mut world = VoxelWorld::new(13, 1);
        let coord = ChunkCoord::new(0, 0, 0);

        world.chunks.insert(coord, ChunkData::new(coord));

        assert!(world.mark_chunk_dirty_for_remesh(coord));
        assert!(!world.mark_chunk_dirty_for_remesh(coord));
    }

    #[test]
    fn integrating_changed_chunk_marks_loaded_neighbors_dirty() {
        let mut world = VoxelWorld::new(13, 1);
        world.horizontal_radius = -1;
        world.vertical_radius = -1;
        world.chunk_retention_horizontal_radius = 8;
        world.chunk_retention_vertical_radius = 8;

        let center = ChunkCoord::new(0, 0, 0);
        let east = ChunkCoord::new(1, 0, 0);

        world.chunks.insert(center, ChunkData::new(center));
        world.chunk_states.insert(center, resident_state(0));
        world.chunk_states.insert(east, ChunkRuntimeState::default());

        let mut changed_chunk = ChunkData::new(east);
        changed_chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        let changed_mesh = build_chunk_mesh(&changed_chunk);

        world.integration_backlog.push_back(ChunkBuildResult {
            coord: east,
            revision: 1,
            chunk: changed_chunk,
            output: ChunkBuildOutput::BuiltMesh(changed_mesh),
        });

        world.tick(Vec3::ZERO);

        assert!(world.dirty_remesh_set.contains(&center));
        assert!(
            world
                .dirty_remesh_queue
                .iter()
                .any(|coord| *coord == center)
        );
    }

    #[test]
    fn integrating_unchanged_chunk_does_not_mark_neighbors_dirty() {
        let mut world = VoxelWorld::new(13, 1);
        world.horizontal_radius = -1;
        world.vertical_radius = -1;
        world.chunk_retention_horizontal_radius = 8;
        world.chunk_retention_vertical_radius = 8;

        let center = ChunkCoord::new(0, 0, 0);
        let east = ChunkCoord::new(1, 0, 0);

        world.chunks.insert(center, ChunkData::new(center));
        world.chunk_states.insert(center, resident_state(0));

        let mut existing_chunk = ChunkData::new(east);
        existing_chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        world.chunks.insert(east, existing_chunk.clone());
        world.chunk_states.insert(east, resident_state(0));

        let existing_mesh = build_chunk_mesh(&existing_chunk);
        world.integration_backlog.push_back(ChunkBuildResult {
            coord: east,
            revision: 1,
            chunk: existing_chunk,
            output: ChunkBuildOutput::BuiltMesh(existing_mesh),
        });

        world.tick(Vec3::ZERO);

        assert!(!world.dirty_remesh_set.contains(&center));
        assert!(
            !world
                .dirty_remesh_queue
                .iter()
                .any(|coord| *coord == center)
        );
    }

    #[test]
    fn stale_chunk_completion_does_not_override_newer_state() {
        let mut world = VoxelWorld::new(13, 1);
        world.horizontal_radius = -1;
        world.vertical_radius = -1;
        world.chunk_retention_horizontal_radius = 8;
        world.chunk_retention_vertical_radius = 8;

        let coord = ChunkCoord::new(0, 0, 0);
        let mut current_chunk = ChunkData::new(coord);
        current_chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        let current_mesh = build_chunk_mesh(&current_chunk);

        world.chunks.insert(coord, current_chunk.clone());
        world.meshes.insert(coord, current_mesh.clone());
        world.chunk_states.insert(
            coord,
            ChunkRuntimeState {
                requested_revision: 2,
                integrated_revision: 2,
                resident_state: ChunkResidentState::Resident,
                ..ChunkRuntimeState::default()
            },
        );

        world.integration_backlog.push_back(ChunkBuildResult {
            coord,
            revision: 1,
            chunk: ChunkData::new(coord),
            output: ChunkBuildOutput::BuiltEmptyButValid,
        });

        let report = world.tick(Vec3::ZERO);

        assert_eq!(report.completed, 0);
        assert_eq!(world.chunks.get(&coord), Some(&current_chunk));
        assert!(world.meshes.contains_key(&coord));
        assert!(world.drain_mesh_updates(4).is_empty());
    }

    #[test]
    fn dirty_chunk_marked_while_pending_requests_followup_remesh() {
        let mut world = VoxelWorld::new(13, 1);
        world.horizontal_radius = -1;
        world.vertical_radius = -1;
        world.chunk_retention_horizontal_radius = 8;
        world.chunk_retention_vertical_radius = 8;

        let coord = ChunkCoord::new(0, 0, 0);
        let mut chunk = ChunkData::new(coord);
        chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        let mesh = build_chunk_mesh(&chunk);

        world.chunks.insert(coord, chunk.clone());
        world.chunk_states.insert(
            coord,
            ChunkRuntimeState {
                requested_revision: 1,
                integrated_revision: 0,
                pending_revision: Some(1),
                resident_state: ChunkResidentState::Resident,
                ..ChunkRuntimeState::default()
            },
        );

        assert!(world.mark_chunk_dirty_for_remesh(coord));

        world.integration_backlog.push_back(ChunkBuildResult {
            coord,
            revision: 1,
            chunk,
            output: ChunkBuildOutput::BuiltMesh(mesh),
        });

        let report = world.tick(Vec3::ZERO);

        assert_eq!(report.completed, 1);
        let state = world.chunk_states.get(&coord).copied().unwrap();
        assert_eq!(state.requested_revision, 2);
        assert_eq!(state.pending_revision, Some(2));
        assert!(!state.needs_remesh_after_pending);
    }

    #[test]
    fn resident_chunk_does_not_flicker_absent_on_failed_build_output() {
        let mut world = VoxelWorld::new(13, 1);
        world.horizontal_radius = -1;
        world.vertical_radius = -1;
        world.chunk_retention_horizontal_radius = 8;
        world.chunk_retention_vertical_radius = 8;

        let coord = ChunkCoord::new(0, 0, 0);
        let mut chunk = ChunkData::new(coord);
        chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        let mesh = build_chunk_mesh(&chunk);

        world.chunks.insert(coord, chunk.clone());
        world.meshes.insert(coord, mesh);
        world.chunk_states.insert(coord, resident_state(1));

        world.integration_backlog.push_back(ChunkBuildResult {
            coord,
            revision: 2,
            chunk: ChunkData::new(coord),
            output: ChunkBuildOutput::Failed,
        });

        let report = world.tick(Vec3::ZERO);

        assert_eq!(report.completed, 1);
        assert_eq!(world.chunks.get(&coord), Some(&chunk));
        assert!(world.meshes.contains_key(&coord));
        assert!(world.drain_mesh_updates(2).is_empty());
    }

    #[test]
    fn chunk_border_change_marks_loaded_neighbor_dirty() {
        let mut world = VoxelWorld::new(13, 1);
        let center = ChunkCoord::new(0, 0, 0);
        let east = ChunkCoord::new(1, 0, 0);

        world.chunks.insert(center, ChunkData::new(center));
        world.chunks.insert(east, ChunkData::new(east));

        let changed = world.set_block_world(IVec3::new(15, 0, 0), BlockType::Stone);

        assert!(changed);
        assert!(world.dirty_remesh_set.contains(&center));
        assert!(world.dirty_remesh_set.contains(&east));
    }

    #[test]
    fn visible_chunk_filter_uses_distance_radius() {
        let mut world = VoxelWorld::new(13, 1);
        let near = ChunkCoord::new(0, 0, -1);
        let far = ChunkCoord::new(6, 0, 6);

        world.meshes.insert(near, ChunkMesh::default());
        world.meshes.insert(far, ChunkMesh::default());

        let camera = Camera::default();
        let visible = world.visible_chunk_coords(
            Vec3::ZERO,
            2,
            camera.view_matrix(),
            camera.projection_matrix(1.0),
        );

        assert!(visible.contains(&near));
        assert!(!visible.contains(&far));
    }

    #[test]
    fn visible_chunk_filter_applies_frustum_after_distance() {
        let mut world = VoxelWorld::new(13, 1);
        let front = ChunkCoord::new(0, 0, -1);
        let behind = ChunkCoord::new(0, 0, 1);

        world.meshes.insert(front, ChunkMesh::default());
        world.meshes.insert(behind, ChunkMesh::default());

        let mut camera = Camera::default();
        camera.position = Vec3::ZERO;

        let visible = world.visible_chunk_coords(
            camera.position,
            4,
            camera.view_matrix(),
            camera.projection_matrix(1.0),
        );

        assert!(visible.contains(&front));
        assert!(!visible.contains(&behind));
    }

    #[test]
    fn visible_chunk_filter_falls_back_to_distance_when_frustum_is_invalid() {
        let mut world = VoxelWorld::new(13, 1);
        let front = ChunkCoord::new(0, 0, -1);
        let behind = ChunkCoord::new(0, 0, 1);

        world.meshes.insert(front, ChunkMesh::default());
        world.meshes.insert(behind, ChunkMesh::default());

        let camera = Camera::default();
        let invalid_view = Mat4::from_cols_array(&[f32::NAN; 16]);

        let visible =
            world.visible_chunk_coords(Vec3::ZERO, 4, invalid_view, camera.projection_matrix(1.0));

        assert!(visible.contains(&front));
        assert!(visible.contains(&behind));
    }
}
