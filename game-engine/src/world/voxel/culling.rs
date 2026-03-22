use glam::{Mat4, Vec3, Vec4};

use crate::world::voxel::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, ChunkCoord};

const PLANE_NORMAL_EPSILON: f32 = 1.0e-6;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Aabb {
    pub(crate) min: Vec3,
    pub(crate) max: Vec3,
}

impl Aabb {
    pub(crate) fn from_chunk_coord(coord: ChunkCoord) -> Self {
        let origin = coord.origin_world().as_vec3();
        let size = Vec3::new(
            CHUNK_SIZE_X as f32,
            CHUNK_SIZE_Y as f32,
            CHUNK_SIZE_Z as f32,
        );

        Self {
            min: origin,
            max: origin + size,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Plane {
    normal: Vec3,
    d: f32,
}

impl Plane {
    fn from_raw(raw: Vec4) -> Option<Self> {
        if !raw.is_finite() {
            return None;
        }

        let normal = raw.truncate();
        let normal_len = normal.length();

        if !normal_len.is_finite() || normal_len <= PLANE_NORMAL_EPSILON {
            return None;
        }

        Some(Self {
            normal: normal / normal_len,
            d: raw.w / normal_len,
        })
    }

    fn signed_distance_to_point(self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.d
    }

    fn excludes_aabb(self, aabb: &Aabb) -> bool {
        let support = Vec3::new(
            if self.normal.x >= 0.0 {
                aabb.max.x
            } else {
                aabb.min.x
            },
            if self.normal.y >= 0.0 {
                aabb.max.y
            } else {
                aabb.min.y
            },
            if self.normal.z >= 0.0 {
                aabb.max.z
            } else {
                aabb.min.z
            },
        );

        self.signed_distance_to_point(support) < 0.0
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    pub(crate) fn from_view_projection(view: Mat4, projection: Mat4) -> Option<Self> {
        if !view.is_finite() || !projection.is_finite() {
            return None;
        }

        let clip = projection * view;
        if !clip.is_finite() {
            return None;
        }

        // glam stores matrices column-major. Transposing gives us easy row vectors.
        let clip_t = clip.transpose();
        let row0 = clip_t.x_axis;
        let row1 = clip_t.y_axis;
        let row2 = clip_t.z_axis;
        let row3 = clip_t.w_axis;

        // perspective_rh uses a [0, 1] depth range, so the near plane comes from row2.
        let left = Plane::from_raw(row3 + row0)?;
        let right = Plane::from_raw(row3 - row0)?;
        let bottom = Plane::from_raw(row3 + row1)?;
        let top = Plane::from_raw(row3 - row1)?;
        let near = Plane::from_raw(row2)?;
        let far = Plane::from_raw(row3 - row2)?;

        Some(Self {
            planes: [left, right, bottom, top, near, far],
        })
    }

    pub(crate) fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        self.planes.iter().all(|plane| !plane.excludes_aabb(aabb))
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::world::camera::Camera;

    #[test]
    fn frustum_intersects_aabb_in_front_of_camera() {
        let mut camera = Camera::default();
        camera.position = Vec3::ZERO;

        let frustum =
            Frustum::from_view_projection(camera.view_matrix(), camera.projection_matrix(1.0))
                .expect("camera matrices should produce a valid frustum");

        let aabb = Aabb {
            min: Vec3::new(-1.0, -1.0, -6.0),
            max: Vec3::new(1.0, 1.0, -4.0),
        };

        assert!(frustum.intersects_aabb(&aabb));
    }

    #[test]
    fn frustum_rejects_aabb_behind_camera() {
        let mut camera = Camera::default();
        camera.position = Vec3::ZERO;

        let frustum =
            Frustum::from_view_projection(camera.view_matrix(), camera.projection_matrix(1.0))
                .expect("camera matrices should produce a valid frustum");

        let aabb = Aabb {
            min: Vec3::new(-1.0, -1.0, 4.0),
            max: Vec3::new(1.0, 1.0, 6.0),
        };

        assert!(!frustum.intersects_aabb(&aabb));
    }

    #[test]
    fn frustum_rejects_aabb_far_outside_view_cone() {
        let mut camera = Camera::default();
        camera.position = Vec3::ZERO;

        let frustum =
            Frustum::from_view_projection(camera.view_matrix(), camera.projection_matrix(1.0))
                .expect("camera matrices should produce a valid frustum");

        let aabb = Aabb {
            min: Vec3::new(60.0, -1.0, -12.0),
            max: Vec3::new(64.0, 1.0, -8.0),
        };

        assert!(!frustum.intersects_aabb(&aabb));
    }
}
