use std::collections::{HashMap, HashSet, VecDeque};

use glam::{IVec3, Mat4, Vec3};

use crate::world::voxel::{
    block::BlockType,
    chunk::{ChunkCoord, ChunkData, split_world_position},
    culling::{Aabb, Frustum},
    meshing::{ChunkMesh, ChunkNeighborSolidity},
    pipeline::{ChunkBuildResult, ChunkGenerationPipeline},
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

#[derive(Debug)]
pub(crate) struct VoxelWorld {
    pipeline: ChunkGenerationPipeline,
    chunks: HashMap<ChunkCoord, ChunkData>,
    meshes: HashMap<ChunkCoord, ChunkMesh>,
    pending: HashSet<ChunkCoord>,
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
            pending: HashSet::new(),
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

                    if self.chunks.contains_key(&coord) || self.pending.contains(&coord) {
                        continue;
                    }

                    let neighbors = self.collect_neighbor_solidity(coord);
                    if self.pipeline.request_generate_chunk(coord, neighbors) {
                        self.pending.insert(coord);
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

            if !self.chunks.contains_key(&coord)
                || self.pending.contains(&coord)
                || !self.is_within_retention(center_chunk, coord)
            {
                continue;
            }

            let Some(chunk_snapshot) = self.chunks.get(&coord).cloned() else {
                continue;
            };
            let neighbors = self.collect_neighbor_solidity(coord);

            if self
                .pipeline
                .request_remesh(coord, chunk_snapshot, neighbors)
            {
                self.pending.insert(coord);
                requested += 1;
            }
        }

        for built in self
            .pipeline
            .drain_completed(self.max_completed_drains_per_tick.max(1))
        {
            self.pending.remove(&built.coord);

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

            let chunk_changed = self
                .chunks
                .get(&built.coord)
                .map(|existing| existing != &built.chunk)
                .unwrap_or(true);

            self.chunks.insert(built.coord, built.chunk);

            let mesh = built.mesh;
            if mesh.is_empty() {
                self.meshes.remove(&built.coord);
                self.mesh_updates
                    .push_back(ChunkMeshUpdate::Remove { coord: built.coord });
            } else {
                self.meshes.insert(built.coord, mesh.clone());
                self.mesh_updates.push_back(ChunkMeshUpdate::Upsert {
                    coord: built.coord,
                    mesh,
                });
            }

            if chunk_changed {
                self.mark_adjacent_chunks_dirty_for_remesh(built.coord);
            }

            completed += 1;
        }

        VoxelTickReport {
            requested,
            completed,
            loaded_chunks: self.chunks.len(),
            pending_chunks: self.pending.len(),
            mesh_updates_queued: self.mesh_updates.len(),
        }
    }

    pub(crate) fn mark_chunk_dirty_for_remesh(&mut self, coord: ChunkCoord) -> bool {
        if !self.chunks.contains_key(&coord) {
            return false;
        }

        if !self.dirty_remesh_set.insert(coord) {
            return false;
        }

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

        self.pending.retain(|coord| is_within(*coord));
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
            chunk: ChunkData::new(coord),
            mesh: ChunkMesh::default(),
        }
    }

    fn solid_chunk_result(coord: ChunkCoord) -> ChunkBuildResult {
        let mut chunk = ChunkData::new(coord);
        chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        let mesh = build_chunk_mesh(&chunk);

        ChunkBuildResult { coord, chunk, mesh }
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
        world.pending.insert(far);
        world.integration_backlog.push_back(chunk_result(far));

        world.tick(Vec3::ZERO);

        assert!(world.chunks.contains_key(&near));
        assert!(!world.chunks.contains_key(&far));
        assert!(!world.meshes.contains_key(&far));
        assert!(!world.pending.contains(&far));
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
        world
            .integration_backlog
            .push_back(chunk_result(ChunkCoord::new(1, 0, 0)));
        world
            .integration_backlog
            .push_back(chunk_result(ChunkCoord::new(2, 0, 0)));

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

        let mut changed_chunk = ChunkData::new(east);
        changed_chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        let changed_mesh = build_chunk_mesh(&changed_chunk);

        world.integration_backlog.push_back(ChunkBuildResult {
            coord: east,
            chunk: changed_chunk,
            mesh: changed_mesh,
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

        let mut existing_chunk = ChunkData::new(east);
        existing_chunk.set_block(LocalCoord::new(0, 0, 0), BlockType::Stone);
        world.chunks.insert(east, existing_chunk.clone());

        let existing_mesh = build_chunk_mesh(&existing_chunk);
        world.integration_backlog.push_back(ChunkBuildResult {
            coord: east,
            chunk: existing_chunk,
            mesh: existing_mesh,
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
