use glam::Vec3;
use legion::World;

#[derive(Debug, Clone, Copy)]
pub struct Position(pub Vec3);

#[derive(Debug, Clone, Copy)]
pub struct Velocity(pub Vec3);

pub fn demo_world() -> World {
    let mut world = World::default();

    world.push((
        Position(Vec3::new(0.0, 10.0, 0.0)),
        Velocity(Vec3::new(0.0, -9.81, 0.0)),
    ));

    world
}
