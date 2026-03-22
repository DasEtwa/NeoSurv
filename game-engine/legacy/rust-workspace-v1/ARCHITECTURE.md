# Engine Architecture (Initial)

## Crates

- `engine` (Library)
  - `config` – globale Engine-Konfiguration
  - `render` – wgpu-basierte Renderer-Abstraktion (Vulkan/OpenGL target)
  - `ecs` – Legion Components + World bootstrap
  - `scene` – serde + RON scene serialization
  - `worldgen` – noise-basierte Terrain-Generation
  - `voxel` – Chunk-Model + Greedy-Meshing/Raycast placeholders
  - `editor` – egui Inspector foundation
- `sandbox` (Binary)
  - nutzt `engine` und dient als schneller Integrationstest

## Geplante nächste Schritte

1. `winit` Event-Loop + Fenster erstellen
2. Echten `wgpu` Device/Surface Setup ergänzen
3. ECS Systems + Scheduler-Pipeline definieren
4. Chunk-Streaming + Thread-Pool Jobs aufbauen
5. Greedy-Meshing in `voxel` implementieren
