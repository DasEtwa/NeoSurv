# Step 6 — Save/Load Core (serde + RON + ECS sync)

## What changed

### 1) Scene serialization stayed versioned + got better diagnostics

`src/world/save_load.rs` still uses the versioned envelope:

```rust
SceneFile {
    version: u32,
    scene: Scene,
}
```

- Current version: `SCENE_FILE_VERSION = 1`
- RON remains the on-disk format
- Legacy compatibility is preserved (old files containing only `Scene` still load)
- Error messages now include more context:
  - scene name when serializing/saving
  - target file path on write/read
  - envelope parse failure + legacy parse fallback failure details

### 2) ECS ↔ Scene sync is implemented

Added `src/world/scene_sync.rs`:

- `scene_from_world(world, scene_name)`
  - snapshots ECS entities into serializable `Scene`
  - captures `Transform.position`
  - uses deterministic order (`id` sort) for stable output
- `replace_world_from_scene(world, scene)`
  - clears world and rebuilds it from `Scene`
  - validates duplicate scene IDs and fails with explicit errors

Also added ECS metadata component in `src/ecs/components.rs`:

- `SceneEntity { id, name }`

This component keeps save/load identity (id + name) tied to live ECS entities.

### 3) SceneManager now supports named slots

`src/world/scene_manager.rs` now supports:

- Active slot (existing behavior): `default`
- Named slots via:
  - `save_scene_slot(scene_id)`
  - `load_scene_slot(scene_id)`
  - `save_world_to_slot(world, scene_id)`
  - `load_scene_slot_into_world(world, scene_id)`
- World-aware helpers:
  - `save_world_to_active_scene(world)`
  - `load_active_scene_into_world(world)`
  - `apply_current_scene_to_world(world)`

Slot IDs are validated (`[A-Za-z0-9_-]`) to prevent path traversal / invalid filenames.

### 4) Runtime hooks now sync real ECS state

`src/engine.rs` behavior:

- Startup:
  - load `assets/scenes/default.ron` into `SceneManager` if present
  - apply current scene (loaded or built-in demo) into ECS world
- Hotkeys:
  - `F5` = save **active slot** from live ECS world
  - `F9` = load **active slot** into live ECS world
  - `F6` = save quick slot (`assets/scenes/quick.ron`)
  - `F10` = load quick slot

Existing `F5/F9` behavior remains intact, now with ECS synchronization.

### 5) Bootstrap world includes scene identity component

`src/ecs/systems.rs` bootstrap entity now includes:

- `SceneEntity { id: 1, name: "player" }`
- `Transform`

This keeps bootstrap data aligned with save/load mapping.

---

## Tests added

### `src/world/scene_sync.rs`

- `ecs_scene_ron_roundtrip_preserves_entities`
  - ECS → Scene → RON → Scene → ECS → Scene
  - verifies entity identity + position survive roundtrip

### `src/world/save_load.rs`

- `from_ron_accepts_legacy_scene_format`
  - validates backward compatibility with pre-envelope files
- `from_ron_rejects_unsupported_version`
  - validates version mismatch handling

---

## Usage

- Save default slot: `F5` → `assets/scenes/default.ron`
- Load default slot: `F9`
- Save quick slot: `F6` → `assets/scenes/quick.ron`
- Load quick slot: `F10`

You can add more slots by calling `SceneManager` named-slot APIs from gameplay/editor code.

---

## Notes / limitations

- Current scene mapping serializes entity identity + position (rotation is reset to zero on scene->ECS rebuild).
- Rebuild strategy is intentional and minimal: loading a scene currently replaces world entities.
- Renderer/camera/input paths are unchanged except for the new optional quick-slot hotkeys.
