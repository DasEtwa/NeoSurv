# Tokenburner

`tokenburner` is a Rust-based voxel FPS prototype built on `winit` + `wgpu`.
The project already has a working chunk streaming, meshing, and rendering path.
The next milestone is to turn that foundation into a small playable shooter MVP.

## Current Direction

The project is no longer aiming at a generic engine sandbox first.
The near-term target is a voxel shooter with:

- a small to medium-sized map
- tight first-person movement
- 1-2 weapons
- simple enemies
- destructible voxel terrain
- quick restart and dev hot reload instead of full scene persistence

## What Already Works

- `winit` application loop and window lifecycle
- `wgpu` renderer with Vulkan/OpenGL backend selection
- async chunk generation and meshing workers
- greedy voxel meshing with neighbor-aware boundary culling
- budgeted mesh upload to the GPU
- chunk visibility filtering and draw submission
- first-person camera, mouse look, sprint, crouch, jump, collision
- block-level world queries and runtime block edits
- basic hitscan interaction path and simple target logic
- OBJ model loading for static/viewmodel meshes

## Current Architecture

The runtime is still centered around [`src/engine.rs`](src/engine.rs), but the important layers are already visible:

- `src/renderer/`
  - owns `wgpu`, GPU buffers, chunk upload budgeting, and draw calls
- `src/world/voxel/`
  - owns chunk data, meshes, streaming, retention, remesh queues, and visibility
- `src/world/voxel/pipeline.rs`
  - runs generation and remeshing asynchronously on worker threads
- `src/input/handler.rs`
  - collects keyboard and mouse state
- `src/game/model.rs`
  - loads static OBJ meshes used by the current prototype

## Planned MVP Refactor

The next 2-3 iterations are about shrinking the project around the shooter loop.

### Priority Changes

1. Reduce chunk streaming/view distance to a tighter combat-friendly radius.
2. Remove heavy scene save/load flow and replace it with static map boot or simple proc-gen.
3. Split gameplay logic out of `src/engine.rs`.
4. Add explicit gameplay modules for weapons, projectiles, hit detection, and damage.

### Target MVP Feature Set

- fixed combat arena or compact procedural map
- hitscan weapon
- projectile weapon
- enemies with health and simple behavior
- damage application and kill feedback
- destructible voxel blocks
- quick restart / hot reload for development

## Suggested Near-Term Module Shape

This is the direction we should refactor toward:

```text
src/
  main.rs
  engine.rs              # app bootstrap + top-level frame orchestration
  player.rs              # player state, movement, camera coupling
  input/
    handler.rs           # raw input capture
    actions.rs           # input mapping for gameplay actions
  gameplay/
    mod.rs
    weapons.rs
    projectiles.rs
    hit_detection.rs
    damage.rs
    enemy.rs
  renderer/
    mod.rs
    ...
  world/
    mod.rs
    camera.rs
    map.rs               # static map or compact proc-gen bootstrap
    voxel/
      runtime.rs
      pipeline.rs
      generation.rs
      meshing.rs
      culling.rs
      raycast.rs
      ...
```

## Build

```bash
cargo check
cargo test
cargo run
```

## Config

Runtime options are currently loaded from `Config.toml`.
Graphics backend switching is still supported while the shooter gameplay is built out.

## Controls

Current prototype controls:

- `WASD` move
- mouse look
- `Shift` sprint
- `V` crouch
- `Space` jump
- left mouse / `E` shoot
- `Esc`, `Tab`, `F1` toggle cursor capture/menu state

Some legacy scene hotkeys still exist in code, but they are expected to be removed during the FPS refactor.

## Notes

- The renderer and voxel meshing path are the main assets of the codebase. Preserve them.
- ECS and scene persistence are currently lightweight and can be simplified aggressively.
- The project has working tests around chunk math, meshing, visibility, runtime behavior, and scene serialization.
