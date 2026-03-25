# NeoSurv Known Bugs

This file tracks the current known issues gathered from bug sweeps and live debugging.

## Critical / High

### 1. Dynamic/UI/viewmodel meshes are still too expensive per frame

Status: open

Symptoms:

- FPS drops heavily during movement, menu usage, chat, and combat
- chunk streaming can visibly fall behind

Likely areas:

- `src/engine.rs`
- `src/renderer/mod.rs`
- HUD / chat / menu mesh generation paths

Next fix:

- cache or diff dynamic meshes instead of rebuilding everything every frame

### 2. Chunk upload path still needs smarter prioritization

Status: open

Symptoms:

- visible holes / delayed chunk appearance under movement or camera turns
- upload backlog competes with gameplay mesh updates

Likely areas:

- `src/renderer/mod.rs`
- `src/world/voxel/runtime.rs`

Next fix:

- deduplicate queued chunk operations
- prioritize nearby visible chunks

### 3. Structures are not yet fully real gameplay collision

Status: partially fixed

What is fixed:

- hitscan and chest interaction now use basic world occlusion checks

What remains:

- structures are still mostly render-world objects instead of full gameplay collision volumes
- player/projectiles still need stronger structure collision integration

Likely areas:

- `src/world/state.rs`
- `src/engine.rs`
- `src/gameplay/projectiles.rs`

### 4. Enemy AI ignores proper navigation

Status: open

Symptoms:

- enemies can behave as if walls/props do not matter enough
- chase and return behavior still feels naive

Likely areas:

- `src/gameplay/enemies.rs`

## Medium

### 5. Spawner position validation is weak

Status: open

Symptoms:

- enemies may spawn in awkward or low-quality positions

Likely areas:

- `src/world/state.rs`

### 6. Debug chest spawning is not terrain-snapped enough

Status: open

Symptoms:

- debug chests can float or clip on slopes

Likely areas:

- `src/commands.rs`
- `src/world/state.rs`

### 7. UI quality is not production-ready

Status: open

Symptoms:

- HUD/menu/chat still look placeholder or overly debug-like
- user explicitly disliked the current UI pass

Likely areas:

- `src/hud.rs`
- `src/menu.rs`
- `src/chat.rs`
- `src/ui.rs`

## Recently Fixed

### Fixed: hitscan ignored world occlusion

- now blocked against basic voxel/static world occlusion

### Fixed: chest interaction could ignore geometry

- now blocked against basic occlusion before interaction

### Fixed: chest loot could be lost when inventory was full

- leftover items now remain in the chest

### Fixed: closing the window could skip saving

- close request now triggers save

### Fixed: chat/menu state did not fully block gameplay hotkeys

- gating for several item/fire inputs was tightened

### Fixed: voxel streaming radius mismatch was too aggressive

- runtime chunk load/retention radius was raised to better match rendered visibility

## Workflow Note

Before adding big new systems, prefer:

1. renderer/streaming perf cleanup
2. structure collision pass
3. AI/navigation cleanup
4. UI cleanup
