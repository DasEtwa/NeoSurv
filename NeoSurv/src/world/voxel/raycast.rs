use glam::{IVec3, Vec3};

use crate::world::voxel::block::BlockType;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RaycastHit {
    pub(crate) block: BlockType,
    pub(crate) block_pos: IVec3,
    pub(crate) previous_pos: IVec3,
    pub(crate) distance: f32,
}

pub(crate) fn raycast_voxels<F>(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    mut sample: F,
) -> Option<RaycastHit>
where
    F: FnMut(IVec3) -> Option<BlockType>,
{
    let dir = direction.normalize_or_zero();
    if dir.length_squared() == 0.0 || max_distance <= 0.0 {
        return None;
    }

    let mut voxel = origin.floor().as_ivec3();
    let mut previous = voxel;

    let step = IVec3::new(signum_i32(dir.x), signum_i32(dir.y), signum_i32(dir.z));

    let mut t_max = Vec3::new(
        int_bound(origin.x, dir.x),
        int_bound(origin.y, dir.y),
        int_bound(origin.z, dir.z),
    );

    let t_delta = Vec3::new(
        if dir.x != 0.0 {
            (1.0 / dir.x).abs()
        } else {
            f32::INFINITY
        },
        if dir.y != 0.0 {
            (1.0 / dir.y).abs()
        } else {
            f32::INFINITY
        },
        if dir.z != 0.0 {
            (1.0 / dir.z).abs()
        } else {
            f32::INFINITY
        },
    );

    let mut traveled = 0.0;

    loop {
        if traveled > max_distance {
            break;
        }

        if let Some(block) = sample(voxel).filter(|block| block.is_solid()) {
            return Some(RaycastHit {
                block,
                block_pos: voxel,
                previous_pos: previous,
                distance: traveled,
            });
        }

        previous = voxel;

        if t_max.x < t_max.y {
            if t_max.x < t_max.z {
                voxel.x += step.x;
                traveled = t_max.x;
                t_max.x += t_delta.x;
            } else {
                voxel.z += step.z;
                traveled = t_max.z;
                t_max.z += t_delta.z;
            }
        } else if t_max.y < t_max.z {
            voxel.y += step.y;
            traveled = t_max.y;
            t_max.y += t_delta.y;
        } else {
            voxel.z += step.z;
            traveled = t_max.z;
            t_max.z += t_delta.z;
        }
    }

    None
}

fn signum_i32(value: f32) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn int_bound(s: f32, ds: f32) -> f32 {
    if ds == 0.0 {
        return f32::INFINITY;
    }

    if ds > 0.0 {
        ((s.floor() + 1.0) - s) / ds
    } else {
        (s - s.floor()) / -ds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raycast_hits_expected_voxel() {
        let hit = raycast_voxels(
            Vec3::new(0.1, 0.1, 0.1),
            Vec3::new(1.0, 0.0, 0.0),
            10.0,
            |pos| {
                if pos == IVec3::new(3, 0, 0) {
                    Some(BlockType::Stone)
                } else {
                    Some(BlockType::Air)
                }
            },
        )
        .expect("ray should hit");

        assert_eq!(hit.block, BlockType::Stone);
        assert_eq!(hit.block_pos, IVec3::new(3, 0, 0));
        assert_eq!(hit.previous_pos, IVec3::new(2, 0, 0));
        assert!(hit.distance <= 3.0);
    }

    #[test]
    fn raycast_misses_when_no_block() {
        let hit = raycast_voxels(Vec3::ZERO, Vec3::X, 5.0, |_| Some(BlockType::Air));
        assert!(hit.is_none());
    }
}
