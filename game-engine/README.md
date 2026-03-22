# Tokenburner Engine Foundation

WSL2-first Rust Game-Engine Fundament mit Windows-Mirror nach `Z:\Workspace`.

## Ziel (Milestone 1)

- Fenster + Event-Loop (`winit`)
- wgpu Surface/Device Init
- Clear-Screen Render (dynamische Farbe pro Frame)
- WASD + Mouse-Look Input erfasst
- `Config.toml` Backend-Switch (Vulkan/OpenGL)
- Stabil bei Resize + Close

## Struktur

```
.
├── Cargo.toml
├── Config.toml
├── src/
│   ├── main.rs
│   ├── engine.rs
│   ├── renderer/
│   │   ├── mod.rs
│   │   ├── backend_trait.rs
│   │   ├── vulkan.rs
│   │   └── opengl.rs
│   ├── ecs/
│   │   ├── components.rs
│   │   ├── systems.rs
│   │   └── resources.rs
│   ├── input/
│   │   └── handler.rs
│   ├── editor/
│   │   └── gui.rs
│   ├── world/
│   │   ├── scene.rs
│   │   └── save_load.rs
│   └── config.rs
├── assets/
│   └── shaders/
└── legacy/
    ├── cpp-bootstrap/
    └── rust-workspace-v1/
```

## Build & Run (WSL)

```bash
cd /home/max/.openclaw/workspace/game-engine
cargo run
```

## Config wechseln

`Config.toml`:

```toml
[graphics]
backend = "vulkan" # oder "opengl"
vsync = true
```

## Windows Mirror

```bash
./scripts/sync_to_windows.sh
```

Standardziel: `/mnt/z/Workspace/game-engine` (entspricht `Z:\Workspace\game-engine`)

## Hinweis zu `legion`

Die aktuell verfügbare stabile Version auf crates.io ist `0.4.x`.
Wenn du zwingend `~0.7` willst, brauchen wir ein alternatives ECS oder einen Fork/anderen Branch.
