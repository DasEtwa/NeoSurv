# Tech Stack (Fixed Decisions)

Diese Datei ist die **verbindliche Basis** für das Projekt.

| Was | Warum / Vorteil | Einsatz |
|---|---|---|
| Rust (Edition 2024 / stable) | Memory-safe, performant, starkes Typing, kein GC | Gesamte Engine (Core, Renderer, ECS, Systems) |
| wgpu 28 | Einheitliche API über Vulkan/OpenGL/Metal/DX/WebGPU | Rendering-Backend-Abstraktion |
| Vulkan (via wgpu) | Moderne Features + höchste Performance | Primary Renderer |
| OpenGL (via wgpu) | Kompatibilität/Fallback | Secondary/Fallback Renderer |
| winit 0.30 | Plattformübergreifendes Window + Input | Event-Loop, Fenster, Keyboard/Mouse |
| pollster | Blocking Async für Setup | Adapter/Device Init in Sync-Startup |
| glam | Schnelle Math-Typen (Vec/Mat) | Kamera, Transform, Movement |
| legion | ECS für Entities/Components/Systems | ECS-Basis |
| serde + ron | Flexible Serialisierung + lesbares Scene-Format | Save/Load |
| egui + eframe (optional Step) | Schneller Editor-Start | später In-Engine Editor |

## Hinweis zu `legion`

`legion ~0.7` ist aktuell nicht als stable release verfügbar; crates.io bietet `0.4.x`.
Aktuell ist deshalb `0.4.x` gepinnt. Wenn gewünscht, migrieren wir auf eine Alternative (`bevy_ecs`, `hecs`) oder experimentellen Fork.

## Meilenstein-Definition (M1)

- Window + Event-Loop
- Clear-Screen Render mit wgpu
- WASD + Mouse-Look Input Capture
- Backend-Switch per `Config.toml`
- Resize/Close stabil
