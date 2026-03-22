use legion::World;

use crate::ecs::{
    components::{SceneEntity, Transform},
    resources::Time,
};

pub(crate) fn bootstrap_world() -> World {
    let mut world = World::default();
    world.push((
        SceneEntity {
            id: 1,
            name: "player".to_owned(),
        },
        Transform {
            position: glam::Vec3::new(0.0, 1.8, 0.0),
            rotation: glam::Vec3::ZERO,
        },
    ));
    world
}

pub(crate) fn tick(_world: &mut World, _time: Time) {
    // Placeholder für künftige ECS-Systeme.
}
