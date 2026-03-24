# Game

A voxel-based 3D game written in Rust with `wgpu` and `winit`.

This project started as more of an engine-style experiment and is now being turned into an actual game.

## Current Status

Work in progress.

Current features and active areas of development include:
- voxel world rendering
- movement and shooting
- imported OBJ weapon models
- early enemy and spawn systems
- cleanup and refactoring of older systems
- experimental save/load functionality
- debug and chat command foundations
- ongoing gameplay and world improvements

## Direction

The focus of this repository is no longer to build a generic engine.
The focus is to build a real playable game on top of the current voxel foundation.

Planned systems include:
- seeded/random world generation
- better terrain and world variety
- more blocks and environmental content
- improved AI
- structures and zone-based gameplay
- loot and inventory systems
- menus and game flow
- persistent saves

## Build

    cargo run

Release build:

    cargo run --release

## Notes

This is an actively developed personal project.
Expect refactors, unfinished systems, and larger structural changes as development continues.

## License

Unless otherwise noted, the source code in this repository is licensed under the GNU General Public License v3.0.

This repository may also contain third-party assets and materials that are licensed separately.
See `THIRD_PARTY_NOTICES.md` for more information.
