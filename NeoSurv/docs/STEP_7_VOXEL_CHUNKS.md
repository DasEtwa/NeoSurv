# Step 7 — Voxel Chunk Foundation

## Goal

Build a first robust voxel stack that can evolve into full world streaming + voxel rendering,
without destabilizing the existing engine loop.

---

## Implemented

## 1) Voxel domain model (`src/world/voxel/`)

### Block model

- `BlockType` enum in `block.rs`:
  - `Air`, `Grass`, `Dirt`, `Stone`, `Sand`
- Utility helpers:
  - `is_solid()`
  - `material_id()` (mesh material key)

### Chunk model

- `ChunkCoord` (`x, y, z` chunk-space coordinates)
- `LocalCoord` (`x, y, z` voxel-space inside one chunk)
- `ChunkData` container with dense block storage (`Vec<BlockType>`)
- Fixed dimensions (currently `16 x 16 x 16`)

### Coordinate conversion helpers

- `ChunkCoord::from_world(IVec3)`
- `ChunkCoord::origin_world()`
- `ChunkCoord::world_from_local(LocalCoord)`
- `ChunkData::world_to_local(IVec3)`
- `split_world_position(IVec3) -> (ChunkCoord, LocalCoord)`

Negative world coordinates are handled correctly via `div_euclid` / `rem_euclid`.

---

## 2) Deterministic terrain generation (`generation.rs`)

- Added dependency: `noise = "0.9"`
- `TerrainGenerator` uses two `OpenSimplex` fields:
  - macro terrain shape
  - detail perturbation
- Deterministic generation from a fixed seed.
- Terrain layering currently:
  - top: `Grass` (or `Sand` below sea-level)
  - subsurface: `Dirt`
  - deep: `Stone`
  - above surface: `Air`

This gives repeatable and non-flat chunk content suitable for meshing and picking.

---

## 3) Threaded chunk generation pipeline (`pipeline.rs`)

- `ChunkGenerationPipeline`:
  - background worker threads
  - async job queue for chunk requests
  - non-blocking completion drain (`try_recv` loop)
- `ChunkBuildResult` includes:
  - generated `ChunkData`
  - prebuilt `ChunkMesh`

Drop behavior cleanly sends worker shutdown jobs and joins worker threads.

---

## 4) Meshing foundation (`meshing.rs`)

Implemented **face-culling baseline mesher** (correctness-first, not greedy yet):

- For each solid voxel:
  - inspect 6 neighbors
  - emit quad only when neighbor is non-solid (or out of chunk)
- Output type `ChunkMesh`:
  - `vertices: Vec<MeshVertex>`
  - `indices: Vec<u32>`
- `MeshVertex` currently carries:
  - position
  - normal
  - uv
  - material id

This is directly usable as mesh-ready geometry data for future renderer upload.

---

## 5) Raycast/picking foundation (`raycast.rs`)

- Implemented voxel DDA / grid stepping raycast (`Amanatides & Woo` style)
- `raycast_voxels(...)` returns `RaycastHit` with:
  - hit block type
  - block position
  - previous position (placement target support)
  - traveled distance

`VoxelWorld::raycast(...)` wires this directly against loaded voxel chunk data.

---

## 6) Runtime integration (`runtime.rs` + `engine.rs`)

### `VoxelWorld` runtime

- Tracks:
  - loaded chunks
  - generated meshes
  - pending chunk requests
- `tick(camera_position)`:
  - requests chunk jobs around camera chunk (radius-based)
  - drains completed jobs non-blockingly
  - integrates chunk + mesh caches
  - returns `VoxelTickReport`

### Engine loop integration

- `EngineApp` now owns `voxel_world: VoxelWorld`
- Each frame (`render`):
  - call `voxel_world.tick(camera.position)`
  - emit periodic debug telemetry
- Added simple picking hook:
  - `E` key runs forward raycast up to 8 units
  - logs hit/miss in tracing output

This keeps the render loop responsive while chunk generation happens on background workers.

---

## File map

- `Cargo.toml` (added `noise`)
- `src/world/mod.rs` (exports `voxel`)
- `src/world/voxel/mod.rs`
- `src/world/voxel/block.rs`
- `src/world/voxel/chunk.rs`
- `src/world/voxel/generation.rs`
- `src/world/voxel/meshing.rs`
- `src/world/voxel/pipeline.rs`
- `src/world/voxel/raycast.rs`
- `src/world/voxel/runtime.rs`
- `src/engine.rs` (runtime tick + pick integration)

---

## Known caveats (intentional for foundation)

1. Meshing is face-culling baseline, not greedy meshing yet.
2. Boundary face culling is chunk-local (doesn’t inspect neighbor chunk solidity yet).
3. No mesh upload/render bridge yet (data is generated + cached only).
4. No chunk unload policy yet (loaded cache grows as camera explores).
5. No LOD/frustum prioritization yet.

---

## Suggested next tasks

1. Replace baseline mesher with greedy meshing (major vertex/index reduction).
2. Add neighbor-aware boundary culling.
3. Add chunk upload path into renderer (GPU buffers per chunk mesh).
4. Add priority scheduling (distance + view-cone aware generation).
5. Add chunk eviction / streaming budget.
6. Add edit operations (place/remove blocks) + dirty remesh queue.
