# NeoSurv Known Bugs

This file tracks the current known issues gathered from bug sweeps and live debugging.

## Critical / High

### 1. Viewmodel weapon render/pose is currently broken

Status: fixed

Symptoms:

- weapon can appear too high / too centered
- weapon can disappear entirely depending on the active path
- hand/arm presentation is unstable across the recent viewmodel changes
- current viewmodel path no longer matches the last known-good in-game pose

Likely areas:

- `src/engine.rs`
- `src/gameplay/viewmodel.rs`
- `src/renderer/mod.rs`
- `src/gameplay/mod.rs`

Notes:

- the old world-space weapon path and the new template/instance path have both been touched recently
- this should be re-fixed in isolation instead of continuing to patch around it inside larger renderer/input changes

What was fixed:

- gameplay now renders the weapon, hand, and muzzle flash through one consistent viewmodel template/instance path instead of mixing overlay instances with the legacy world-space mesh path
- gameplay overlay template sync now includes the active weapon viewmodel templates together with HUD/chat templates so the weapon path no longer disappears when the renderer switches view layers
- the template cache key now includes the selected weapon item, so changing weapons refreshes the correct viewmodel templates instead of reusing stale ones

### 2. Gameplay/menu/capture state is currently unstable

Status: fixed

Symptoms:

- player can end up unable to use `WASD`
- gameplay/menu state can desync while trying to enter the game
- attempted mouse-capture fallback changes have made this area unreliable
- current behavior can leave the game feeling paused or half in menu state

Likely areas:

- `src/engine.rs`
- `src/input/handler.rs`
- `src/menu.rs`
- `src/player.rs`

Notes:

- treat this as a regression introduced during the recent viewmodel / state-path work
- this should be stabilized before more gameplay-facing bugfixes are stacked on top

What was fixed:

- entering menu mode now closes chat and clears pending fire/capture side effects so menu and gameplay input no longer remain half-active together
- `Escape` now closes chat first instead of globally forcing menu mode, preventing the previous chat-plus-menu desync that could steal `WASD` or `Enter` for the wrong layer
- capture toggles are ignored while chat is open, and focus-loss/menu transitions now go through one shared menu-open path with consistent state resets

### 3. Dynamic/UI/viewmodel meshes were too expensive per frame

Status: fixed

Symptoms:

- FPS dropped heavily during movement, menu usage, chat, and combat
- chunk streaming could visibly fall behind
- UI/viewmodel overlay geometry was rebuilt in CPU world space every frame even when content was unchanged
- dynamic/viewmodel/UI buffers were re-uploaded every frame despite stable mesh labels

What was fixed:

- dynamic/viewmodel rendering now uses cached templates plus per-frame instances instead of rebuilding transformed mesh geometry every frame
- HUD/menu/chat templates only resync when their actual content changes
- stable template meshes no longer trigger repeated GPU buffer rewrites when geometry is unchanged

### 4. Chunk upload path still needed smarter prioritization

Status: fixed

Symptoms:

- visible holes / delayed chunk appearance under movement or camera turns
- upload backlog competed with gameplay mesh updates
- stale queued chunk uploads could consume frame budget after newer data already existed

Likely areas:

- `src/renderer/mod.rs`
- `src/world/voxel/runtime.rs`

What was fixed:

- queued chunk uploads are now deduplicated per chunk coordinate so stale work stops consuming upload budget
- removes are prioritized first, then visible chunks, then nearby chunks before far backlog work

### 5. Command/chat submissions can invalidate static world meshes unnecessarily

Status: fixed

Symptoms:

- local chat, `/help`, usage errors, and unknown commands could trigger static-world mesh cache rebuilds
- unnecessary static mesh rebuilds could stack on top of UI/chat usage spikes

What was fixed:

- command outcomes now explicitly report whether the world actually changed
- local chat, `/help`, usage errors, and unknown commands no longer invalidate the static world mesh cache

### 6. Terrain holes appear after runtime/world streaming updates

Status: fixed

Symptoms:

- terrain looks correct when the world first loads
- holes/open gaps appear only after runtime updates or after moving around for a bit
- issue looks like chunks or remesh/upload state can regress after initial stable load

Likely areas:

- `src/world/voxel/runtime.rs`
- `src/renderer/mod.rs`
- `src/engine.rs`

What was fixed:

- voxel runtime now tracks per-chunk requested/integrated/pending revisions and drops stale completions without side effects
- dirty remesh requests are no longer lost when a chunk is already pending; they defer into a guaranteed follow-up remesh
- build outputs now distinguish valid empty results from other runtime states so chunks only disappear on accepted latest data or true eviction
- neighbor/border invalidation is covered by regression tests, and renderer queue tests now explicitly guard latest-op wins for same-chunk remove/upsert races

### 7. Structures are not yet fully real gameplay collision

Status: fixed

What is fixed:

- hitscan and chest interaction now use basic world occlusion checks
- player collision probes now treat structure/chest volumes as solid gameplay blockers instead of only voxel terrain
- projectile collision now stops on structure/chest volumes instead of passing through render-only props

Likely areas:

- `src/world/state.rs`
- `src/engine.rs`
- `src/gameplay/projectiles.rs`

What was fixed:

- static structure/chest bounds are now reused as gameplay collision volumes for player movement and gravity checks
- projectile updates now test against static prop collision as well as voxel terrain, so rockets/grenades no longer ignore structures
- the new collision path is covered with regression tests for cell blocking and projectile/static-prop impact

### 8. Enemy AI ignores proper navigation

Status: open

Symptoms:

- enemies can behave as if walls/props do not matter enough
- chase and return behavior still feels naive

Likely areas:

- `src/gameplay/enemies.rs`

## Medium

### 9. Spawner position validation is weak

Status: open

Symptoms:

- enemies may spawn in awkward or low-quality positions

Likely areas:

- `src/world/state.rs`

### 10. Debug chest spawning is not terrain-snapped enough

Status: open

Symptoms:

- debug chests can float or clip on slopes

Likely areas:

- `src/commands.rs`
- `src/world/state.rs`

### 11. UI quality is not production-ready

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

### Fixed: dynamic/UI/viewmodel hot path rebuilt and re-uploaded too much per frame

- renderer now caches reusable template geometry for dynamic, viewmodel, HUD, chat, and menu paths
- camera movement now updates per-frame instances instead of forcing full geometry rebuilds

### Fixed: gameplay viewmodel pose/render path mixed legacy meshes with overlay instances

- gameplay now uses a single template/instance render path for the active weapon, hand, and muzzle flash
- HUD/chat overlay template sync also refreshes the selected weapon templates, preventing stale or missing weapon overlays after weapon/view changes

### Fixed: gameplay/menu/capture state could desync and leave input half-paused

- menu transitions now close chat and clear pending fire requests so gameplay and menu input layers stay mutually exclusive
- `Escape` closes chat before opening the menu, preventing menu navigation from stealing gameplay/chat keys on the next frame
- capture toggles are ignored while chat is open, and focus-loss transitions reuse the same menu-open path

### Fixed: structures now participate in gameplay collision for player and projectiles

- player movement and gravity checks now treat structure/chest volumes as solid blockers alongside voxel terrain
- projectile updates now collide with static prop volumes instead of only voxel blocks

### Fixed: chunk upload queue could not deduplicate or prioritize visible chunks

- newest chunk op now wins per coordinate
- visible and nearby chunks upload ahead of stale far-backlog work

### Fixed: runtime chunk completions could regress visible terrain after streaming updates

- per-chunk runtime revision ownership now rejects stale chunk build completions
- dirty-during-pending remesh requests are preserved and replayed immediately after the active build finishes
- accepted chunk results replace resident state directly instead of transiently blanking active terrain

### Fixed: command/chat submissions could invalidate static world meshes unnecessarily

- static world mesh cache now only invalidates on real world-changing commands

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
