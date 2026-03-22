# Step 8 — Voxel Rendering Integration Architecture

## Goal

Integrate the existing Step 7 voxel pipeline into the renderer with minimal risk:
- keep chunk generation/meshing async and non-blocking
- upload chunk meshes to GPU under explicit per-frame budget
- draw only currently relevant chunks
- preserve clear ownership boundaries between world/runtime and renderer/GPU state

This is a docs-first implementation plan intended for parallel execution by 3 coding agents.

---

## 1) Module boundaries

## A. Voxel runtime (CPU world state, no GPU ownership)

**Primary module:** `src/world/voxel/runtime.rs`

**Responsibilities:**
- Own authoritative CPU chunk data and CPU meshes (`chunks`, `meshes`, `pending`, backlog)
- Drive chunk request/integration lifecycle (`tick`)
- Decide chunk retention/unload based on camera-centric retention bounds
- Emit **render deltas** (what changed for rendering this frame)

**Must not do:**
- Create/own GPU buffers
- Depend on `wgpu` types

**Step 8 addition (contract):**
- Add a small runtime-to-renderer payload API (example shape):
  - `VoxelRenderDelta::Upsert { coord, mesh, revision }`
  - `VoxelRenderDelta::Remove { coord, revision }`
- Runtime remains source-of-truth for CPU mesh data and revisions.

---

## B. Renderer upload cache (CPU mesh payload -> GPU buffer cache)

**New module family:**
- `src/renderer/voxel/mod.rs`
- `src/renderer/voxel/upload_cache.rs`

**Responsibilities:**
- Own per-chunk GPU resources (vertex/index buffers + counts + revision)
- Accept render deltas and enqueue upload work
- Coalesce redundant updates (keep latest revision per chunk)
- Apply bounded uploads each frame
- Evict GPU entries on runtime unload/remove

**Must not do:**
- Generate voxel chunks or meshes
- Mutate world/runtime state

**Core type boundary:**
- CPU in: `ChunkCoord + ChunkMesh + revision`
- GPU out: `GpuChunkMeshEntry { vertex_buffer, index_buffer, index_count, revision, last_visible_frame }`

---

## C. Draw stage (visibility + issuing draw calls)

**New module family:**
- `src/renderer/voxel/draw_stage.rs`
- shader: `assets/shaders/voxel.wgsl` (or integrate into current shader strategy)

**Responsibilities:**
- Build draw list from upload cache keys + visibility filter
- Bind voxel render pipeline + camera uniforms
- Draw each visible chunk (`draw_indexed`)
- Keep draw stage stateless relative to world logic (it reads cache + camera only)

**Must not do:**
- Own lifecycle decisions for chunk load/unload
- Block frame on large uploads

---

## 2) Data flow (chunk generation -> mesh -> GPU -> draw)

1. **Chunk generation + meshing (existing Step 7)**
   - `ChunkGenerationPipeline` worker produces `ChunkBuildResult { coord, chunk, mesh }`.

2. **Runtime integration (main thread)**
   - `VoxelWorld::tick(camera_pos)` drains completed results.
   - Integrates `chunk` + `mesh` into runtime maps.
   - Emits `VoxelRenderDelta::Upsert` for new/changed chunk meshes.
   - On retention eviction: emits `VoxelRenderDelta::Remove`.

3. **Renderer ingest (main thread)**
   - Engine passes deltas to renderer (single call per frame).
   - Upload cache stores/coalesces pending uploads keyed by `ChunkCoord`.

4. **Budgeted GPU upload (main thread, before draw)**
   - Renderer consumes pending uploads according to frame budget.
   - Creates/replaces `wgpu::Buffer` objects for that chunk key.

5. **Visibility + draw**
   - Draw stage filters cache entries by visibility strategy.
   - Issues indexed draws for visible chunk entries.

6. **Cleanup path**
   - Remove delta deletes cache entry; dropped `wgpu::Buffer`s are released by RAII.

---

## 3) Ownership and lifetime model (per-chunk GPU buffers)

## Authoritative ownership

- **`VoxelWorld` owns CPU truth:**
  - `ChunkData` and `ChunkMesh`
  - revision generation / staleness decisions
- **`RendererVoxelUploadCache` owns GPU truth:**
  - `wgpu::Buffer` objects and draw metadata

No shared mutable ownership across these boundaries.

## Lifetime rules

1. **Create:** on first upsert for `coord`, upload cache creates GPU buffers.
2. **Replace:** on upsert with newer revision, upload cache creates new buffers and swaps entry.
3. **Drop stale:** if upsert revision <= cached revision, ignore payload.
4. **Remove:** on runtime unload/remove, cache entry is deleted.
5. **Frame safety:** apply updates before draw list build in same frame to avoid referencing removed entries.

## Revision policy

- Monotonic per-chunk revision in runtime.
- Revision travels with each upsert/remove event.
- Renderer performs last-write-wins by revision, never by arrival order.

---

## 4) Frame budget strategy for uploads

Use two hard budgets to prevent upload spikes:

- `max_upload_bytes_per_frame` (start: **4 MiB**)  
- `max_upload_chunks_per_frame` (start: **8 chunks**)

Apply whichever limit is hit first.

## Queue prioritization

Priority key for pending uploads:
1. Distance to camera chunk (near first)
2. Event kind (`Upsert` before low-priority maintenance)
3. Revision recency

## Coalescing/backpressure

- Maintain one pending upsert per chunk (replace older payload with newest revision).
- If queue grows too large, prefer dropping stale revisions rather than extending frame time.
- Keep metrics:
  - pending upload count
  - bytes uploaded this frame
  - chunks uploaded this frame
  - dropped stale updates

This keeps frame pacing stable while world streaming continues.

---

## 5) Visibility strategy

## Near-term (Step 8 target)

Use simple and deterministic visibility:
- Draw only chunks currently present in upload cache.
- Apply distance/retention gate aligned with runtime retention bounds.
- Skip empty meshes (`index_count == 0`).

This is enough to render streamed terrain correctly with low implementation risk.

## Future (Step 9+)

1. **Frustum culling (CPU):**
   - Compute chunk AABB from `ChunkCoord` + chunk dimensions.
   - Test against camera frustum planes each frame.

2. **Occlusion strategy:**
   - Phase 1: coarse software occlusion / depth pyramid heuristic
   - Phase 2: optional GPU occlusion queries with temporal coherence

3. **Optional LOD:**
   - Distance-based mesh simplification / reduced update priority for far chunks.

---

## 6) Merge conflict map for 3 coding agents

## Agent responsibilities (single-writer rule)

### Agent A — Runtime bridge (world-side)
**Own files:**
- `src/world/voxel/runtime.rs`
- `src/world/voxel/mod.rs` (exports if needed)
- `src/engine.rs` (**only voxel bridge callsite block**)

**Deliverables:**
- render delta API from `VoxelWorld`
- removal/upsert events on integrate + eviction
- engine forwards runtime deltas to renderer each frame

---

### Agent B — Upload cache (renderer-side state)
**Own files:**
- `src/renderer/voxel/mod.rs`
- `src/renderer/voxel/upload_cache.rs`
- `src/renderer/mod.rs` (**only upload-cache integration sections**)

**Deliverables:**
- per-chunk GPU cache
- upload queue + budget/coalescing
- renderer API: ingest deltas + process uploads

---

### Agent C — Draw stage (renderer-side draw)
**Own files:**
- `src/renderer/voxel/draw_stage.rs`
- `assets/shaders/voxel.wgsl`
- `src/renderer/mod.rs` (**only draw pipeline and draw-pass wiring sections if unavoidable**)

**Deliverables:**
- voxel render pipeline
- visible chunk draw list + draw submission
- per-frame draw stats

---

## Conflict hotspots + mitigation

| Hotspot file | Risk | Primary owner | Mitigation |
|---|---|---|---|
| `src/renderer/mod.rs` | High | Agent B | B owns structural edits; C contributes through new files and minimal agreed anchor edits only |
| `src/engine.rs` | Medium | Agent A | Restrict to one localized block in `render()` after `voxel_world.tick(...)` |
| `src/world/voxel/runtime.rs` | Medium | Agent A | No renderer imports; keep API payload-only |
| `assets/shaders/*` | Low | Agent C | Isolated file (`voxel.wgsl`) to avoid touching existing `clear.wgsl` |

## Merge order (recommended)

1. **Agent B** (upload cache + renderer API surface, compile-safe no-op draw path allowed)
2. **Agent C** (draw stage + shader using B API)
3. **Agent A** (runtime delta emission + engine bridge wiring)
4. Final integration cleanup commit (small conflict resolution + telemetry polish)

Rationale: runtime bridge depends on renderer ingest API being present; draw stage depends on cache primitives.

---

## 7) Step 8 acceptance checklist

## Build/quality gates
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --all-targets`
- [ ] `cargo test --all-targets`

## Functional gates
- [ ] Moving camera causes generated chunks to become visible as voxel geometry (not only clear quad)
- [ ] Chunk unload/retention removal also removes rendered chunk (no ghost geometry)
- [ ] Remeshed chunk updates visible geometry (replacement works, no duplicate draws)
- [ ] Frame remains responsive during streaming (uploads budgeted, no long stalls)
- [ ] Existing raycast (`E`) still works against runtime chunk data

## Telemetry/debug gates
- [ ] log or counter for uploaded chunks/bytes per frame
- [ ] log or counter for visible drawn chunk count
- [ ] log or counter for pending upload queue length

---

## 8) Smoke tests (manual, fast)

## Smoke 1 — cold start and initial terrain
1. `cargo run`
2. Wait 5-10s near spawn
3. Expected:
   - chunk generation logs appear
   - voxel geometry appears onscreen
   - no panic

## Smoke 2 — movement streaming
1. Move continuously for ~30s (WASD + mouse-look)
2. Expected:
   - nearby chunks stream in visually
   - far chunks disappear after retention distance
   - no sustained hitching spikes

## Smoke 3 — update/replace correctness
1. Trigger a remesh path (if available) or force chunk refresh in dev path
2. Expected:
   - same chunk coord updates geometry in place
   - no duplicate overlapping chunk draw for same coord

## Smoke 4 — window lifecycle robustness
1. Resize window repeatedly, minimize/restore
2. Expected:
   - renderer recovers cleanly
   - chunk rendering resumes correctly

## Smoke 5 — runtime/render sync sanity
1. Press `E` for voxel raycast hit/miss while looking at rendered voxels
2. Expected:
   - raycast logs correspond to visible chunk geometry region

---

## 9) Key Step 8 decisions (summary)

1. **Strict boundary:** world/runtime owns CPU chunk+mesh truth; renderer owns GPU cache truth.
2. **Delta bridge:** runtime emits upsert/remove events; renderer is fully event-driven.
3. **Revision-first correctness:** renderer ignores stale updates by per-chunk revision.
4. **Budgeted uploads:** fixed per-frame byte/chunk caps with coalescing/backpressure.
5. **Visibility now simple, future-ready:** distance/retention now; frustum/occlusion next.
6. **Merge-safe plan:** single-writer ownership for conflict hotspots, explicit merge order.
