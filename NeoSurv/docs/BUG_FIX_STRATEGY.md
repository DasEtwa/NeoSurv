# Tokenburner Bug Fix Strategy (Parallel Branch Integration)

## Purpose

This document is the integration playbook for parallel bugfix branches. It maps current issues, merge risk, and a concrete verification + rollback plan.

---

## 0) Current audit snapshot (code + build state)

Audit basis:
- static code review of `src/` + existing architecture docs
- local gate runs on current working tree

Observed state now:
- `cargo fmt --all -- --check` ✅ passes
- `cargo test` / `cargo check` ❌ currently blocked by a **compile regression** in `src/world/voxel/runtime.rs`:
  - borrow checker error `E0502` in `evict_outside_retention` (closure captures `&self` while `retain` mutably borrows maps/sets)
  - affected lines: around `runtime.rs:154-160`
- `cargo clippy --all-targets --all-features` was previously warning-heavy (dead_code etc.), but is currently blocked by the compile failure above.

Implication:
- Voxel lifecycle branch currently contains a **hard blocker**; no other branch should merge on top until this is resolved.

---

## 1) Issue clusters

## A. Init / Persistence

### Findings
1. **Config parse failures are silent** (`config::AppConfig::load_or_default`): invalid `Config.toml` falls back to defaults without diagnostics.
2. **Scene load/apply pipeline is structurally solid** (`SceneManager` + `save_load` + `scene_sync`) and has helpful context errors.
3. **Atomic scene write exists** in `save_load::atomic_write` (good durability baseline).
4. **Scene→ECS replace is destructive by design** (`world.clear()`); acceptable, but means partial data not merged.
5. **Rotation/components are not fully persisted** (known Step-6 limitation).

### Architectural intent
- Keep save/load path deterministic and explicit.
- Improve operator visibility (warn on config fallback).

## B. Input / Camera

### Findings
1. Focus-loss clear exists (`input.clear()`), good.
2. Occlusion/minimize path skips rendering (`can_render`), good.
3. Potential edge: mouse deltas can still accumulate via `DeviceEvent` while not actively rendering/focused in some platform paths, causing camera jump on return.
4. Hotkeys are consumed in `window_event` path globally; behavior is okay but easy to conflict when refactoring engine event flow.

### Architectural intent
- Normalize input state on focus/occlusion transitions.
- Keep movement deterministic with clamped `dt`.

## C. Voxel lifecycle

### Findings
1. **Hard compile break** in retention eviction (`E0502`) — top priority.
2. Chunk retention/backlog integration is being added (good direction for memory control/frame pacing).
3. No epoch/cancel concept yet; stale completed jobs are dropped only by retention checks.
4. Runtime correctness is concentrated in `runtime.rs` (single hot file, high merge-conflict risk).

### Architectural intent
- Keep `tick()` non-blocking, bounded work per frame.
- Ensure retention + integration backlog logic is borrow-safe and test-covered.

## D. Render loop / Lint

### Findings
1. Render loop has basic resilience: handles `SurfaceError::{Lost,Outdated,Timeout}` and occlusion flow.
2. Panic paths still exist in non-test code (`expect` in worker setup/mutex path, etc.) and should be reviewed.
3. Dead-code warnings are widespread; without strict lint gate this can hide regressions.

### Architectural intent
- No panics on routine runtime paths.
- Promote warnings to actionable gate status once compile is stable.

---

## 2) Risk matrix (Impact × Confidence)

| ID | Cluster | Issue | Impact | Confidence | Notes |
|---|---|---|---|---|---|
| VXL-01 | Voxel lifecycle | `E0502` compile failure in eviction retain logic | High | High | Blocks all downstream validation |
| VXL-02 | Voxel lifecycle | stale/in-flight chunk results without epoch cancel | Medium | Medium | Can cause wasted work/churn after movement |
| INP-01 | Input/camera | potential mouse jump after occlusion/focus transitions | Medium | Medium | Platform-event dependent but common risk |
| PERS-01 | Init/persistence | silent config fallback hides misconfiguration | Medium | High | Operational debugging pain |
| PERS-02 | Init/persistence | scene load replaces world wholesale (non-merge) | Low-Med | High | Acceptable now, but should stay explicit |
| RND-01 | Render/lint | panic-capable `expect` in runtime paths | Medium | Medium | Avoid panic in engine runtime |
| LINT-01 | Render/lint | warning backlog (dead code etc.) | Low | High | Can mask real issues over time |

---

## 3) Recommended fix order (strict)

1. **Voxel lifecycle unblock (VXL-01)**
   - fix borrow conflict in `runtime.rs` eviction logic
   - re-run compile/tests immediately
2. **Voxel lifecycle stabilization (VXL-02)**
   - retention/backlog behavior tests + limits validated
3. **Input/camera transition correctness (INP-01)**
   - clear/suppress stale mouse delta on focus/occlusion transitions
4. **Init/persistence observability (PERS-01/PERS-02)**
   - add explicit warning/logging when config fallback occurs
   - preserve current scene-replace behavior but document invariant
5. **Render/lint hardening (RND-01/LINT-01)**
   - reduce panic paths
   - move toward strict clippy gate

Reason: Step 1 restores buildability. Steps 2-3 protect runtime correctness. Steps 4-5 improve operational safety and long-term hygiene.

---

## 4) Conflict map (parallel agents)

## High-conflict hot files
- `src/engine.rs` (event loop, input, render cadence, hotkeys, voxel tick)
- `src/world/voxel/runtime.rs` (streaming, retention, integration)

## Medium-conflict files
- `src/input/handler.rs`
- `src/world/camera.rs`
- `src/world/scene_manager.rs`
- `src/world/save_load.rs`
- `src/world/scene_sync.rs`
- `src/world/voxel/pipeline.rs`

## Lower-conflict files
- `src/renderer/mod.rs`
- `src/ecs/*`
- docs only (`docs/*`)

## Suggested branch ownership
- **agent/init-persistence**: `config.rs`, `world/save_load.rs`, `world/scene_manager.rs`, `world/scene_sync.rs`
- **agent/input-camera**: `input/handler.rs`, `world/camera.rs`, minimal `engine.rs` touch
- **agent/voxel-lifecycle**: `world/voxel/runtime.rs`, `world/voxel/pipeline.rs`, minimal `engine.rs` touch
- **agent/render-lint**: `renderer/*`, panic/lint cleanup, minimal `engine.rs` touch

Rule: avoid broad edits to `engine.rs`; isolate small, reviewed hunks.

---

## 5) Merge and verification sequence

## Merge order
1. `bugfix/voxel-lifecycle` (unblock compile first)
2. `bugfix/input-camera`
3. `bugfix/init-persistence`
4. `bugfix/render-lint`

(If init-persistence is tiny/docs-only, swap 2 and 3 is acceptable.)

## Per-merge gate sequence (must pass before next merge)
1. `cargo fmt --all -- --check`
2. `cargo check --all-targets`
3. `cargo test --all-targets`
4. `cargo clippy --all-targets --all-features -- -D warnings` *(or temporary allowlist with explicit expiry)*
5. Smoke checklist run (below)

If any gate fails: stop queue, fix in same branch or revert merge commit.

---

## 6) Acceptance gates for bugfix integration

## A) Compile/Test/Lint gates

Mandatory:
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --all-targets`
- [ ] `cargo test --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` *(preferred final gate)*

Panic-path gate (manual):
- [ ] no new `unwrap/expect/panic!` in non-test runtime paths unless justified with comment + issue reference

## B) Runtime smoke checklist

Run this after each merged branch and once again after full stack merge:

1. **Startup**
   - [ ] app starts with normal config
   - [ ] app starts with missing `assets/scenes/default.ron` (fallback path works, no panic)

2. **Window lifecycle**
   - [ ] resize repeatedly (large/small)
   - [ ] minimize + restore
   - [ ] occlude/unocclude (if supported by WM) without render-loop failure

3. **Scene hotkeys**
   - [ ] `F5` saves default slot
   - [ ] `F9` loads default slot
   - [ ] `F6` saves quick slot
   - [ ] `F10` loads quick slot
   - [ ] files written under `assets/scenes/*.ron` and reload is successful

4. **Input/camera**
   - [ ] WASD movement remains smooth
   - [ ] mouse-look no large jump after alt-tab/minimize/restore

5. **Voxel runtime**
   - [ ] chunks request/integrate while moving camera
   - [ ] `E` raycast pick logs sane hit/miss behavior
   - [ ] no growth path causing immediate runaway pending queue under normal movement

6. **No panic paths**
   - [ ] no panic/backtrace in logs during the above sequence

---

## 7) Rollback plan

1. Tag baseline before integration queue (e.g., `pre-bugfix-integration`).
2. After each successful merge + gate run, create checkpoint tag (`gate-1-pass`, `gate-2-pass`, ...).
3. If a merge fails gates and quick fix is non-trivial:
   - revert that merge commit (preferred in shared branch), or
   - reset integration branch to previous checkpoint tag (if private integration branch).
4. Resume queue only after restored green state.

Operational rule: **Never continue stacking merges on a red build.**

---

## 8) Key decisions summary

- Treat voxel compile regression as immediate blocker.
- Keep branch integration serialized with hard gates between merges.
- Minimize `engine.rs` conflict surface by strict ownership and small patches.
- Enforce smoke coverage around startup, window lifecycle, save/load hotkeys, input recovery, and voxel runtime.
- Maintain explicit rollback checkpoints to de-risk parallel integration.
