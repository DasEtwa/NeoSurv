use glam::Vec3;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TargetHitbox {
    pub(super) target_index: usize,
    pub(super) min: Vec3,
    pub(super) max: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HitscanHit {
    pub(super) target_index: usize,
    pub(super) distance: f32,
}

pub(super) fn find_first_hitscan_hit(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    targets: impl IntoIterator<Item = TargetHitbox>,
) -> Option<HitscanHit> {
    let mut best_hit: Option<HitscanHit> = None;

    for target in targets {
        let Some(distance) = ray_aabb_distance(origin, direction, target.min, target.max) else {
            continue;
        };

        if distance > max_distance {
            continue;
        }

        match best_hit {
            Some(current) if current.distance <= distance => {}
            _ => {
                best_hit = Some(HitscanHit {
                    target_index: target.target_index,
                    distance,
                });
            }
        }
    }

    best_hit
}

pub(super) fn find_first_sphere_overlap(
    center: Vec3,
    radius: f32,
    targets: impl IntoIterator<Item = TargetHitbox>,
) -> Option<usize> {
    targets.into_iter().find_map(|target| {
        let expanded_min = target.min - Vec3::splat(radius);
        let expanded_max = target.max + Vec3::splat(radius);

        let overlaps = center.cmpge(expanded_min).all() && center.cmple(expanded_max).all();
        overlaps.then_some(target.target_index)
    })
}

fn ray_aabb_distance(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv_dir = Vec3::new(
        if direction.x.abs() > f32::EPSILON {
            1.0 / direction.x
        } else {
            f32::INFINITY
        },
        if direction.y.abs() > f32::EPSILON {
            1.0 / direction.y
        } else {
            f32::INFINITY
        },
        if direction.z.abs() > f32::EPSILON {
            1.0 / direction.z
        } else {
            f32::INFINITY
        },
    );

    let t1 = (min - origin) * inv_dir;
    let t2 = (max - origin) * inv_dir;
    let t_min = t1.min(t2);
    let t_max = t1.max(t2);

    let near = t_min.max_element();
    let far = t_max.min_element();

    if far < 0.0 || near > far {
        None
    } else {
        Some(near.max(0.0))
    }
}
