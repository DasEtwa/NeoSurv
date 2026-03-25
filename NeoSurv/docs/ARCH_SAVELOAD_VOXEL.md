# Save/Load + Voxel Chunk Architecture Blueprint

## Status / Intent

This document defines a concrete target architecture that combines:

- **scene persistence** (already started in Step 6)
- **voxel chunk streaming + generation + meshing**

It is designed to fit the current engine layout (`engine.rs`, `world/*`, `renderer/*`) with minimal disruption and clear ownership boundaries.

---

## 1) Module Boundaries

> Proposed modules are additive and can be introduced incrementally.

### Existing (baseline)

- `src/world/scene.rs` — serializable high-level scene model
- `src/world/save_load.rs` — RON serialization envelope (versioned)
- `src/world/scene_manager.rs` — active scene persistence operations

### Proposed additions

```text
src/world/
  persistence/
    mod.rs
    scene_persistence.rs      # trait + file-backed implementation
    scene_snapshot.rs         # DTO for persisted scene/chunk metadata
  voxel/
    mod.rs
    types.rs                  # ChunkCoord, Voxel, ChunkData, ChunkMeta
    chunk_store.rs            # main-thread owned loaded chunk map + states
    chunk_provider.rs         # trait for load/generate provider
    meshing.rs                # mesh requests/results + queue trait
    streaming.rs              # view-distance scheduler + request orchestration
  upload/
    mod.rs
    gpu_upload.rs             # main-thread boundary for GPU resource creation
```

### Responsibilities

- **Persistence layer**: durable scene/chunk state to disk (versioned, atomic writes).
- **Voxel data layer**: canonical CPU-side chunk voxel data + state transitions.
- **Meshing layer**: chunk voxel data -> mesh payload jobs.
- **Upload boundary**: CPU mesh payload -> GPU buffers/textures on main thread.
- **Engine loop**: drives orchestration, applies results, and renders.

---

## 2) Core Data Flow

## A. Startup / Load flow

1. `EngineApp::new` asks `ScenePersistenceService::load_scene(...)`.
2. Scene metadata + player/world seed are loaded.
3. `ChunkStore` starts empty; `StreamingController` requests chunks around spawn.
4. For each required chunk:
   - `ChunkProvider` tries disk cache (if present), else procedural generation.
   - result enters `ChunkStore` as CPU voxel data.
   - `MeshingQueue::enqueue` creates mesh jobs.
5. Meshing results are consumed on main thread and passed to `GpuUploadBoundary`.
6. Renderer receives chunk mesh handles and draws visible chunks.

## B. Runtime stream/update flow

1. Camera/player position changes.
2. `StreamingController` computes target chunk set (view distance + hysteresis).
3. Newly needed chunks are requested; distant chunks are marked unload candidates.
4. Dirty chunks (edited voxels) are re-meshed; old mesh handles retired.
5. GPU uploads happen only on main thread in a per-frame budget.

## C. Save flow

1. Save trigger (manual/auto/checkpoint) creates immutable `SceneSnapshot`.
2. Snapshot includes scene metadata + dirty chunk references or serialized payload.
3. Persistence worker writes to `*.tmp`, fsync, then atomic rename.
4. Commit report is returned to main thread (success/failure + diagnostics).

---

## 3) Lifecycle Model

Each chunk in `ChunkStore` follows a strict state machine:

```text
Unloaded
  -> Requested
  -> ReadyVoxelData
  -> MeshingQueued
  -> MeshReadyCPU
  -> UploadedGPU
  -> Visible
  -> (Dirty) -> MeshingQueued ...
  -> Evicting
  -> Unloaded
```

Rules:

- State transitions happen on **main thread only**.
- Worker threads produce immutable results; they do not mutate `ChunkStore` directly.
- Generation/meshing results carry `epoch/generation_id` so stale results can be dropped safely.

---

## 4) Threading Model

### Thread roles

- **Main thread**
  - input, ECS/world updates, chunk state transitions, renderer calls, GPU uploads
- **Persistence worker (1 thread initially)**
  - disk IO for scene/chunk save/load
- **Generation workers (N threads)**
  - procedural chunk generation and optional decode from disk cache
- **Meshing workers (N threads)**
  - greedy/naive meshing from voxel snapshots

### Communication

Use message passing (channels) between orchestrator and workers:

- `ChunkRequest` -> provider workers
- `ChunkDataResult` <- provider workers
- `MeshRequest` -> meshing workers
- `MeshBuildResult` <- meshing workers
- `PersistenceCommand` -> persistence worker
- `PersistenceResult` <- persistence worker

No cross-thread shared mutable `ChunkStore`. If shared data is needed, use immutable snapshots (`Arc<ChunkData>` or compact copy DTO).

---

## 5) Ownership Model

- `EngineApp` owns high-level systems (`SceneManager`/future `WorldOrchestrator`).
- `ChunkStore` (main-thread) owns authoritative runtime chunk state and lifecycle.
- Workers own temporary job-local data and return results by value.
- Persistence service owns filesystem paths and serialization format details.
- Renderer owns GPU buffers/handles; world layer only stores opaque IDs/handles.

**Key invariant:**
CPU voxel truth is separate from GPU representation. GPU resources are derived caches and may be rebuilt anytime.

---

## 6) Interface / Contract Proposals

These are proposed API contracts (trait-first, implementation-agnostic).

```rust
use anyhow::Result;
use std::path::Path;

// ---------- Shared domain types ----------
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord { pub x: i32, pub y: i32, pub z: i32 }

#[derive(Debug, Clone)]
pub struct SceneSnapshot {
    pub scene_id: String,
    pub world_seed: u64,
    pub player_transform: [f32; 7], // pos xyz + rot xyzw (or replace with math type)
    pub version: u32,
}

#[derive(Debug, Clone)]
pub struct ChunkData {
    pub coord: ChunkCoord,
    pub voxels: Vec<u16>,   // palette/material id; keep layout explicit in impl docs
    pub revision: u64,
}

#[derive(Debug, Clone)]
pub struct MeshPayload {
    pub coord: ChunkCoord,
    pub revision: u64,
    pub vertices: Vec<u8>,   // packed vertex bytes (or typed vertex Vec)
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct SaveOptions {
    pub include_chunk_cache: bool,
    pub compress_chunks: bool,
}

// ---------- 1) Scene persistence service ----------
pub trait ScenePersistenceService: Send + Sync {
    fn load_scene(&self, scene_id: &str) -> Result<SceneSnapshot>;
    fn save_scene(&self, snapshot: &SceneSnapshot, options: SaveOptions) -> Result<()>;

    fn load_chunk(&self, scene_id: &str, coord: ChunkCoord) -> Result<Option<ChunkData>>;
    fn save_chunk(&self, scene_id: &str, chunk: &ChunkData) -> Result<()>;

    fn list_scenes(&self) -> Result<Vec<String>>;
    fn delete_scene(&self, scene_id: &str) -> Result<()>;
}

// ---------- 2) Chunk provider / generator ----------
#[derive(Debug, Clone)]
pub struct ChunkRequest {
    pub scene_id: String,
    pub coord: ChunkCoord,
    pub world_seed: u64,
    pub priority: u8,
    pub epoch: u64,
}

#[derive(Debug, Clone)]
pub enum ChunkSource {
    Disk,
    Generated,
}

#[derive(Debug, Clone)]
pub struct ChunkDataResult {
    pub request: ChunkRequest,
    pub source: ChunkSource,
    pub data: ChunkData,
}

pub trait ChunkProvider: Send + Sync {
    fn request_chunk(&self, req: ChunkRequest) -> Result<()>;
    fn try_recv_ready_chunk(&self) -> Option<ChunkDataResult>;
    fn cancel_epoch(&self, epoch: u64);
}

// ---------- 3) Meshing queue ----------
#[derive(Debug, Clone)]
pub struct MeshRequest {
    pub coord: ChunkCoord,
    pub center: ChunkData,
    pub neighbors: [Option<ChunkData>; 6],
    pub epoch: u64,
}

#[derive(Debug, Clone)]
pub struct MeshBuildResult {
    pub coord: ChunkCoord,
    pub revision: u64,
    pub epoch: u64,
    pub payload: MeshPayload,
}

pub trait MeshingQueue: Send + Sync {
    fn enqueue(&self, request: MeshRequest) -> Result<()>;
    fn try_recv(&self) -> Option<MeshBuildResult>;
    fn pending_jobs(&self) -> usize;
    fn cancel_epoch(&self, epoch: u64);
}

// ---------- 4) Main-thread upload boundary ----------
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkGpuHandle(pub u64);

pub trait GpuUploadBoundary {
    fn upload_chunk_mesh(&mut self, payload: MeshPayload) -> Result<ChunkGpuHandle>;
    fn retire_chunk_mesh(&mut self, handle: ChunkGpuHandle) -> Result<()>;
    fn upload_budget_bytes_per_frame(&self) -> usize;
}
```

Contract notes:

- `epoch` allows fast invalidation when player teleports / scene reloads.
- `revision` prevents older mesh results from replacing newer chunk edits.
- `GpuUploadBoundary` is **not Send/Sync** by default: expected main-thread usage only.

---

## 7) Phase Roadmap

## Phase A (already done) — Save/Load foundation

Delivered (current repo):

- versioned scene file envelope (`SCENE_FILE_VERSION`)
- RON serialization helpers + legacy compatibility
- `SceneManager` for active scene save/load
- runtime keybinds (`F5` save, `F9` load)

Acceptance criteria (met):

- engine boots even if scene file is missing/corrupt (fallback scene)
- scene save writes a readable RON file
- scene load restores `Scene` payload via manager API

## Phase B — Unified runtime orchestration (next)

Scope:

- Introduce `ChunkStore` and chunk lifecycle state machine
- Add `ChunkProvider` + `MeshingQueue` interfaces and basic worker-backed impls
- Stream chunks around player/camera with fixed view distance
- Connect mesh results to renderer via `GpuUploadBoundary`

Acceptance criteria:

- moving camera causes deterministic chunk load/unload around origin
- no panics under rapid movement / repeated save-load
- stale chunk/mesh results are dropped via `epoch` check
- frametime p95 remains stable (no long blocking IO on main thread)

## Phase C — Durability + performance hardening

Scope:

- atomic persistence for scene + chunk cache
- dirty-chunk tracking + incremental saves
- upload/meshing budgets + backpressure
- basic recovery paths for partial/corrupt save artifacts

Acceptance criteria:

- kill/restart during save does not destroy last valid save
- chunk memory is bounded by configured budgets
- edit-heavy sessions remain playable without sustained stutter spikes
- compatibility policy documented for future file format versions

---

## 8) Risk Register + Mitigations

## 8.1 Race conditions

Risks:

- stale worker results applied after scene reload/teleport
- concurrent writes to same chunk from edit + generation result

Mitigations:

- `epoch` token on every async request/result
- per-chunk `revision` monotonic counter
- apply transitions only on main thread

## 8.2 Memory growth

Risks:

- unbounded loaded chunks and queued jobs
- duplicate voxel copies between provider/mesher/render prep

Mitigations:

- hard cap loaded chunks by distance + max count
- bounded queues with backpressure/drop strategy
- compact chunk storage format and snapshot reuse where possible

## 8.3 IO corruption / partial writes

Risks:

- crash or power loss during save
- truncated or half-written scene/chunk files

Mitigations:

- write temp file + fsync + atomic rename
- keep previous generation until rename success
- embed version + checksum/hash per file (phase C)

## 8.4 Stutter points

Risks:

- synchronous disk IO on main thread
- too many GPU uploads in one frame
- meshing bursts after fast movement

Mitigations:

- all blocking IO off main thread
- per-frame upload byte budget
- request prioritization (near chunks first) + hysteresis + throttling

---

## 9) Integration Notes (minimal-change path)

1. Keep current `SceneManager` API as façade.
2. Introduce new traits behind `world::persistence` and `world::voxel` without immediate full wiring.
3. Add orchestrator in `engine.rs` progressively:
   - collect ready chunk data
   - enqueue meshing
   - consume mesh results
   - upload within budget
4. Preserve current F5/F9 behavior while chunk cache saving becomes incremental.

This sequence keeps risk low and avoids broad refactors while enabling a clear path from current Step 6 to chunked voxel streaming.
