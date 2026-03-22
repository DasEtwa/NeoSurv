# Step 9 — Performance Architecture (Greedy Meshing, Cross-Chunk Culling, Frustum Pass)

## Goal

Ship a **safe, incremental performance package** on top of Step 8:
1. Replace face-by-face meshing with **greedy meshing** (major vertex/index reduction).
2. Make face culling **neighbor-aware across chunk boundaries**.
3. Add first **CPU-side frustum culling** pass before draw submission.

This plan is docs-first and optimized for parallel coding with low merge risk.

---

## Scope and non-goals

### In scope (Step 9)
- World-side meshing architecture upgrade (greedy + boundary-aware sampling).
- Runtime dirty propagation when neighboring chunk state changes.
- CPU frustum test for chunk AABBs integrated into visible chunk selection.
- Telemetry counters for mesh/build/visibility impact.

### Out of scope (future steps)
- GPU occlusion culling / Hi-Z.
- LOD meshes.
- Mesh compression.
- Full edit-aware persistence redesign.

---

## 1) Module boundaries and responsibilities

## A. Meshing core (world CPU, no renderer dependency)

**Primary files:**
- `src/world/voxel/meshing.rs` (can later split into folder if desired)
- `src/world/voxel/chunk.rs` (chunk dimensions + coordinate helpers)

**Responsibilities:**
- Build `ChunkMesh` from chunk voxel data.
- Sample neighbor occupancy across chunk boundaries (6 directions).
- Provide two algorithms:
  - **Baseline** (existing face culling; kept as fallback/debug)
  - **Greedy** (Step 9 default)

**Key boundary contract:**
- Input: center chunk + neighbor sampling view.
- Output: `ChunkMesh` only (no runtime state, no wgpu state).

---

## B. Runtime dependency + remesh scheduler

**Primary file:**
- `src/world/voxel/runtime.rs`

**Responsibilities:**
- Own chunk lifecycle and canonical CPU chunk data.
- Track dirty remesh requests with dedupe and frame budget.
- Propagate remesh dirtiness when adjacent chunks are loaded/updated/evicted.
- Drop stale remesh results (revision/epoch guard).
- Emit `ChunkMeshUpdate::{Upsert, Remove}` for renderer bridge.

**Must not do:**
- GPU uploads, render pipeline decisions.

---

## C. Generation/meshing worker pipeline

**Primary file:**
- `src/world/voxel/pipeline.rs`

**Responsibilities:**
- Keep generation and (re)meshing off main thread.
- Support two job intents (conceptually):
  - generate chunk data + initial mesh
  - remesh existing chunk with neighbor snapshot
- Return results with `coord` + mesh revision token.

**Why this boundary matters:**
- Greedy meshing is CPU-heavier than naive; worker offload preserves frame pacing.

---

## D. Visibility module (CPU culling)

**New file (recommended):**
- `src/world/voxel/visibility.rs`

**Responsibilities:**
- Frustum plane extraction from `projection * view`.
- Chunk AABB construction from `ChunkCoord` and `CHUNK_SIZE_*`.
- AABB-vs-frustum testing.
- Combined filter: distance gate + frustum gate.

**Must not do:**
- Any renderer buffer ownership.

---

## E. Engine + renderer bridge

**Primary files:**
- `src/engine.rs`
- `src/renderer/mod.rs`

**Responsibilities:**
- Engine computes camera matrices and requests visibility list from world.
- Renderer remains consumer of `ChunkMeshUpdate` + visible coord set.
- No frustum math inside renderer in Step 9 phase 1.

---

## 2) Data flow: chunk updates -> mesh generation -> visibility -> draw

1. **Camera move / frame tick**
   - `engine.rs` updates camera and calls `voxel_world.tick(camera.position)`.

2. **Chunk lifecycle changes**
   - Runtime requests new chunks and integrates completed jobs.
   - On integrate/remove, runtime marks affected chunk + loaded neighbors dirty (deduped queue).

3. **Meshing jobs (worker)**
   - For each dirty coord, worker receives center chunk + neighbor boundary snapshot (or equivalent lookup payload).
   - Worker builds greedy mesh and returns `(coord, mesh, revision)`.

4. **Runtime integration of mesh result**
   - If result revision is stale, discard.
   - Else update `meshes[coord]` and enqueue `ChunkMeshUpdate::Upsert` (or `Remove` when empty).

5. **Visibility selection**
   - Engine builds `CameraMatrices`.
   - Runtime visibility pass applies:
     - distance radius gate (existing)
     - frustum gate (new)
   - Output: ordered visible chunk coords.

6. **Renderer stage**
   - Engine drains mesh updates to renderer upload queue.
   - Renderer budgeted uploads + draw over visible coords.

---

## 3) Migration plan: baseline face culling -> greedy meshing

Use a phased rollout to avoid regressions.

### Phase 0 — API prep (no behavior change)
- Keep existing `build_chunk_mesh` behavior.
- Introduce explicit meshing input contract:
  - center chunk
  - neighbor sampling adapter
- Add tests for current baseline counts so refactor preserves correctness.

### Phase 1 — Boundary-aware naive culling
- Upgrade naive culler to query neighbor chunks on boundary voxels.
- Missing neighbor => treat as air (conservative) **but** queue neighbor remesh on adjacency change.
- Validate seam correctness first before introducing greedy complexity.

### Phase 2 — Greedy meshing implementation (feature-selectable)
- Implement axis-sweep greedy merge by face orientation and material.
- Merge only faces with same:
  - orientation/normal
  - material_id
  - visibility sign
- Preserve UV tiling by scaling UVs with merged quad extents (avoid stretched checker artifacts).
- Keep baseline mesher callable for debug/compare.

### Phase 3 — Switch default + telemetry
- Default mesher = greedy.
- Keep runtime fallback flag/const to baseline for emergency rollback.
- Log mesh reduction metrics per integrated chunk:
  - vertices
  - indices
  - meshing job time (if available)

### Phase 4 — Cleanup
- Remove dead code paths only after acceptance/perf checks pass.

---

## 4) Neighbor chunk dependency strategy (dirty propagation)

## Why required
Without propagation, chunk boundaries can remain incorrect because a chunk meshed while neighbor was missing will keep exposed faces after that neighbor loads.

## Dependency rules
For any chunk `C`, define orthogonal neighbors: `±X, ±Y, ±Z`.

### On chunk integrate/upsert (`C`)
- Mark `C` dirty for remesh (if meshed without full neighbor context).
- For each loaded neighbor `N` of `C`: mark `N` dirty.
- Dedupe via existing `dirty_remesh_set`.

### On chunk remove/evict (`C`)
- Emit renderer remove for `C`.
- For each loaded neighbor `N`: mark `N` dirty (newly exposed faces).

### On block edit (future-proof hook)
- Mark edited chunk dirty.
- If edit touches chunk border voxel, mark corresponding neighbor dirty.

## Scheduling / backpressure
- Keep per-frame remesh request cap (`max_dirty_remesh_requests_per_tick`).
- Prefer nearest dirty chunks first (camera distance priority).
- Maintain revision token per chunk:
  - increment on each remesh enqueue intent
  - attach to job
  - discard late results with older revision

This prevents stale neighbor meshes from overwriting newer results.

---

## 5) Frustum culling plan (CPU first pass) + fallbacks

## First-pass design
- Build frustum planes each frame from `VP = projection * view`.
- For each candidate chunk (already distance-filtered), compute AABB:
  - `min = coord.origin_world()`
  - `max = min + (CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z)`
- Run conservative AABB-plane intersection test.
- Keep chunk if intersecting or inside all planes.

## Recommended ordering for low CPU cost
1. Distance gate (cheap scalar math).
2. Frustum AABB test for remaining candidates.
3. Sort by distance (optional existing behavior).

## Fallbacks / safety
- If frustum extraction yields invalid values (NaN/inf), fallback to distance-only list.
- Add quick kill-switch (`distance_only_visibility`) for troubleshooting.
- Keep near-ring safety: always include chunks within 1 chunk radius around camera to avoid pop on borderline plane jitter.
- On any visibility subsystem failure, renderer should still draw using distance-only output.

---

## 6) Merge-conflict map for coding agents + merge order

## Suggested agent split

### Agent A — Meshing algorithm & tests
**Own:**
- `src/world/voxel/meshing.rs`
- meshing unit tests in same file/module

**Delivers:**
- neighbor-aware naive path
- greedy path
- deterministic mesh-count tests

---

### Agent B — Runtime dirty propagation + revision safety
**Own:**
- `src/world/voxel/runtime.rs`
- `src/world/voxel/mod.rs` (exports only if needed)

**Delivers:**
- adjacency dirty propagation
- remesh scheduling priorities
- stale-result drop by revision

---

### Agent C — Pipeline job model for remesh offload
**Own:**
- `src/world/voxel/pipeline.rs`

**Delivers:**
- generation vs remesh job intent separation
- remesh result transport with revision token

---

### Agent D — Visibility/frustum integration
**Own:**
- `src/world/voxel/visibility.rs` (new)
- `src/engine.rs` (localized visibility wiring)
- `src/world/voxel/runtime.rs` (visibility API surface only, coordinated with B)

**Delivers:**
- frustum extraction + AABB tests
- combined visible list path used by engine

---

## Conflict hotspots

| File | Risk | Primary owner | Mitigation |
|---|---|---|---|
| `src/world/voxel/runtime.rs` | High | Agent B | B owns structural edits; D adds only agreed API hook section |
| `src/engine.rs` | Medium/High | Agent D | Limit edits to visibility block near current `visible_chunk_coords` call |
| `src/world/voxel/meshing.rs` | Medium | Agent A | Keep API stable to avoid B/C rebases |
| `src/world/voxel/pipeline.rs` | Medium | Agent C | Avoid touching runtime internals beyond message types |
| `src/world/voxel/mod.rs` | Low | Agent B | Single export commit, late in queue |

## Recommended merge order
1. **Agent A**: meshing API prep + tests + neighbor-aware naive mode.
2. **Agent C**: pipeline support for remesh jobs/revisions.
3. **Agent B**: runtime dirty propagation + revision-safe integration wiring.
4. **Agent A (follow-up)**: greedy mode default switch + metrics.
5. **Agent D**: frustum visibility module + engine wiring.
6. **Final integration commit**: telemetry naming cleanup + docs sync.

Rationale: runtime/pipeline changes depend on stable meshing contract; frustum is mostly orthogonal and safest to land after mesh pipeline is stable.

---

## 7) Acceptance checklist

## Build and quality gates
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --all-targets`
- [ ] `cargo test --all-targets`

## Meshing correctness gates
- [ ] Full solid chunk (no neighbors) produces only outer shell quads (greedy collapse, not per-voxel faces).
- [ ] Full solid chunk with solid neighbor on `+X` culls `+X` boundary faces.
- [ ] When that neighbor unloads, boundary faces reappear after remesh.
- [ ] No cracks/holes at chunk borders while moving across chunk seams.
- [ ] Material boundaries do not merge incorrectly across different `material_id`.

## Visibility correctness gates
- [ ] Frustum culling removes off-screen chunks while preserving on-screen chunks.
- [ ] Rotating camera 360° causes expected chunk set changes without severe popping.
- [ ] Distance-only fallback path can be toggled and renders correctly.

## Streaming/render stability gates
- [ ] Upload queue remains bounded under movement (no runaway backlog).
- [ ] No panic on resize/minimize/restore during chunk streaming.
- [ ] Raycast (`E`) remains consistent with visible terrain positions.

---

## 8) Smoke tests + perf checks

## Smoke 1 — Border seam correctness
1. `cargo run`
2. Move until multiple chunk boundaries are visible.
3. Expected: no visible double-faces/flicker seams at chunk edges.

## Smoke 2 — Neighbor load/unload propagation
1. Move forward to stream new chunks, then retreat to force eviction radius behavior.
2. Expected: exposed/hidden border faces update correctly as neighbors appear/disappear.

## Smoke 3 — Frustum behavior
1. Stand still and rotate camera slowly 360°.
2. Expected: chunk draw count changes with view direction; no missing near terrain.

## Perf check 1 — Mesh complexity reduction
- Compare against baseline mesher on same seed/camera path:
  - total uploaded vertices/indices over 60s.
- Expected: substantial reduction (target: **>=30% fewer indices**, usually much higher).

## Perf check 2 — Frame pacing under streaming
- Observe telemetry every `STREAM_TELEMETRY_INTERVAL_FRAMES`.
- Expected: lower upload pressure and stable frame cadence versus baseline.

## Perf check 3 — Visibility efficiency
- Track:
  - candidate chunks (distance)
  - frustum-passed chunks
  - drawn chunks
- Expected: frustum-passed < distance candidates in directional views; no correctness regressions.

---

## 9) Key Step 9 decisions (summary)

1. **Greedy meshing is the default**, baseline mesher remains a rollback/debug path during rollout.
2. **Neighbor-aware boundary sampling is mandatory** for seam correctness.
3. **Dirty propagation to 6-neighbor set** is required on chunk integrate/remove.
4. **Revision-guarded remesh integration** prevents stale async results from overriding current meshes.
5. **Frustum culling is CPU-side and conservative first**, with explicit distance-only fallback.
6. **Renderer remains mostly unchanged** in Step 9; optimizations happen world-side first.

---

## Implementation note for coding kickoff

Start with **A -> C -> B** before frustum work. This yields early measurable gains (mesh reduction + seam correctness) while keeping runtime stable for final visibility integration.
