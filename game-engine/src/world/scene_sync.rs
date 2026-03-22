use std::collections::HashSet;

use anyhow::{Result, bail};
use glam::Vec3;
use legion::{Entity, IntoQuery, Read, TryRead, World};

use crate::{
    ecs::components::{SceneEntity, Transform},
    world::scene::{EntityRecord, Scene},
};

/// Creates a serializable scene snapshot from the live ECS world state.
pub(crate) fn scene_from_world(world: &World, scene_name: impl Into<String>) -> Scene {
    fn allocate_generated_id(
        next_id: &mut u64,
        assigned_ids: &mut HashSet<u64>,
        reserved_scene_ids: &HashSet<u64>,
    ) -> u64 {
        let start = *next_id;

        loop {
            let candidate = *next_id;
            *next_id = next_id.wrapping_add(1);

            if assigned_ids.contains(&candidate) || reserved_scene_ids.contains(&candidate) {
                assert_ne!(
                    *next_id, start,
                    "failed to allocate unique scene entity id: id space exhausted"
                );
                continue;
            }

            assigned_ids.insert(candidate);
            return candidate;
        }
    }

    let mut query = <(Entity, Read<Transform>, TryRead<SceneEntity>)>::query();

    let reserved_scene_ids: HashSet<u64> = query
        .iter(world)
        .filter_map(|(_, _, maybe_scene_entity)| {
            maybe_scene_entity.map(|scene_entity| scene_entity.id)
        })
        .collect();

    let max_existing_id = reserved_scene_ids.iter().copied().max().unwrap_or(0);

    // Wrap around explicitly so worlds containing u64::MAX still allocate valid fresh ids.
    let mut next_generated_id = max_existing_id.wrapping_add(1);
    let mut assigned_ids = HashSet::new();
    let mut entities = Vec::new();

    for (_entity, transform, maybe_scene_entity) in query.iter(world) {
        let (id, name) = if let Some(scene_entity) = maybe_scene_entity {
            let id = if assigned_ids.insert(scene_entity.id) {
                scene_entity.id
            } else {
                allocate_generated_id(
                    &mut next_generated_id,
                    &mut assigned_ids,
                    &reserved_scene_ids,
                )
            };

            (id, scene_entity.name.clone())
        } else {
            let generated_id = allocate_generated_id(
                &mut next_generated_id,
                &mut assigned_ids,
                &reserved_scene_ids,
            );
            (generated_id, format!("entity_{generated_id}"))
        };

        entities.push(EntityRecord {
            id,
            name,
            position: transform.position,
        });
    }

    entities.sort_by_key(|entity| entity.id);

    Scene {
        name: scene_name.into(),
        entities,
    }
}

/// Replaces the ECS world with entities from a scene snapshot.
pub(crate) fn replace_world_from_scene(world: &mut World, scene: &Scene) -> Result<()> {
    let mut seen_ids = HashSet::with_capacity(scene.entities.len());

    for entity in &scene.entities {
        if !seen_ids.insert(entity.id) {
            bail!(
                "scene '{}' contains duplicate entity id {}",
                scene.name,
                entity.id
            );
        }
    }

    world.clear();

    for entity in &scene.entities {
        world.push((
            SceneEntity {
                id: entity.id,
                name: entity.name.clone(),
            },
            Transform {
                position: entity.position,
                rotation: Vec3::ZERO,
            },
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{replace_world_from_scene, scene_from_world};
    use crate::{
        ecs::components::{SceneEntity, Transform},
        world::{save_load, scene::Scene},
    };

    #[test]
    fn ecs_scene_ron_roundtrip_preserves_entities() {
        let scene = Scene {
            name: "roundtrip".to_owned(),
            entities: vec![
                crate::world::scene::EntityRecord {
                    id: 7,
                    name: "player".to_owned(),
                    position: glam::Vec3::new(1.0, 2.0, 3.0),
                },
                crate::world::scene::EntityRecord {
                    id: 99,
                    name: "npc".to_owned(),
                    position: glam::Vec3::new(-2.5, 0.0, 8.0),
                },
            ],
        };

        let mut world = legion::World::default();
        replace_world_from_scene(&mut world, &scene).expect("scene should apply to world");

        let world_scene = scene_from_world(&world, "roundtrip");
        let ron = save_load::to_ron(&world_scene).expect("scene should serialize");
        let parsed_scene = save_load::from_ron(&ron).expect("scene should deserialize");

        let mut world_after_parse = legion::World::default();
        replace_world_from_scene(&mut world_after_parse, &parsed_scene)
            .expect("parsed scene should apply to world");

        let reparsed_world_scene = scene_from_world(&world_after_parse, "roundtrip");

        assert_eq!(world_scene, reparsed_world_scene);
    }

    #[test]
    fn generated_ids_skip_reserved_and_duplicates() {
        let mut world = legion::World::default();

        world.push((
            SceneEntity {
                id: 1,
                name: "first".to_owned(),
            },
            Transform {
                position: glam::Vec3::ZERO,
                rotation: glam::Vec3::ZERO,
            },
        ));

        world.push((
            SceneEntity {
                id: 1,
                name: "duplicate".to_owned(),
            },
            Transform {
                position: glam::Vec3::X,
                rotation: glam::Vec3::ZERO,
            },
        ));

        world.push((
            SceneEntity {
                id: 2,
                name: "other".to_owned(),
            },
            Transform {
                position: glam::Vec3::Y,
                rotation: glam::Vec3::ZERO,
            },
        ));

        world.push((Transform {
            position: glam::Vec3::Z,
            rotation: glam::Vec3::ZERO,
        },));

        let scene = scene_from_world(&world, "ids");
        let ids: Vec<u64> = scene.entities.iter().map(|entity| entity.id).collect();
        let unique: HashSet<u64> = ids.iter().copied().collect();

        assert_eq!(ids.len(), 4);
        assert_eq!(unique.len(), 4);
        assert_eq!(ids, vec![1, 2, 3, 4]);
    }

    #[test]
    fn generated_ids_wrap_after_u64_max_without_collision() {
        let mut world = legion::World::default();

        world.push((
            SceneEntity {
                id: u64::MAX,
                name: "max".to_owned(),
            },
            Transform {
                position: glam::Vec3::ZERO,
                rotation: glam::Vec3::ZERO,
            },
        ));

        world.push((Transform {
            position: glam::Vec3::X,
            rotation: glam::Vec3::ZERO,
        },));

        let scene = scene_from_world(&world, "wrap");
        let ids: HashSet<u64> = scene.entities.iter().map(|entity| entity.id).collect();

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&u64::MAX));
        assert!(ids.contains(&0));
    }
}
