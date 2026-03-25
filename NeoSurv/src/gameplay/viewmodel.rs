use std::path::PathBuf;

use glam::{Quat, Vec3};

use crate::{
    game::model::{self, StaticModelSpawn},
    renderer::{StaticModelMesh, StaticModelVertex},
    world::camera::Camera,
};

use super::weapons::{WeaponDefinition, WeaponEffects};

#[derive(Debug)]
pub(crate) struct ViewmodelAssets {
    weapon_source_meshes: Vec<StaticModelMesh>,
}

impl ViewmodelAssets {
    pub(crate) fn new(weapon: WeaponDefinition) -> Self {
        Self {
            weapon_source_meshes: load_weapon_source_meshes(weapon),
        }
    }

    pub(crate) fn build_meshes(
        &self,
        camera: &Camera,
        weapon_effects: &WeaponEffects,
        weapon: WeaponDefinition,
    ) -> Vec<StaticModelMesh> {
        let mut meshes = Vec::new();
        let forward = camera.forward().normalize_or_zero();
        let right = camera.right().normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let recoil_ratio = weapon_effects.recoil_ratio(weapon);
        let weapon_offset = Vec3::new(
            weapon.viewmodel_right_offset,
            -weapon.viewmodel_down_offset,
            weapon.viewmodel_distance - recoil_ratio * weapon.viewmodel_recoil_distance,
        );
        let model_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_z(-0.10)
            * Quat::from_rotation_x(0.05);

        if !self.weapon_source_meshes.is_empty() {
            meshes.extend(
                self.weapon_source_meshes
                    .iter()
                    .enumerate()
                    .map(|(index, mesh)| {
                        transform_viewmodel_mesh(
                            mesh,
                            format!("viewmodel-{}-{index}", weapon.id),
                            camera,
                            model_rotation,
                            weapon_offset,
                            weapon.viewmodel_scale,
                        )
                    }),
            );
        } else {
            let weapon_origin = camera.position
                + forward * weapon_offset.z
                + right * weapon_offset.x
                + up * weapon_offset.y;

            meshes.push(build_box_mesh(
                "fallback-sidearm-slide",
                weapon_origin + right * -0.11 + up * -0.05 + forward * -0.20,
                weapon_origin + right * 0.11 + up * 0.05 + forward * 0.16,
                [0.08, 0.08, 0.10, 1.0],
            ));
            meshes.push(build_box_mesh(
                "fallback-sidearm-grip",
                weapon_origin + right * -0.05 + up * -0.22 + forward * -0.02,
                weapon_origin + right * 0.05 + up * -0.02 + forward * 0.09,
                [0.22, 0.12, 0.08, 1.0],
            ));
            meshes.push(build_box_mesh(
                "fallback-sidearm-barrel",
                weapon_origin + right * -0.03 + up * -0.01 + forward * 0.16,
                weapon_origin + right * 0.03 + up * 0.03 + forward * 0.28,
                [0.20, 0.20, 0.22, 1.0],
            ));
        }

        if weapon_effects.muzzle_flash_active() {
            let flash_origin = camera.position
                + forward * (weapon_offset.z + 0.28)
                + right * (weapon_offset.x + 0.02)
                + up * (weapon_offset.y + 0.02);
            meshes.push(build_box_mesh(
                "muzzle-flash",
                flash_origin - Vec3::splat(0.08),
                flash_origin + Vec3::splat(0.08),
                [1.0, 0.92, 0.55, 1.0],
            ));
        }

        meshes
    }
}

pub(crate) fn build_box_mesh(
    label: impl Into<String>,
    min: Vec3,
    max: Vec3,
    color: [f32; 4],
) -> StaticModelMesh {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];

    let faces = [
        ([0usize, 1, 2, 3], [0.0, 0.0, -1.0]),
        ([5usize, 4, 7, 6], [0.0, 0.0, 1.0]),
        ([4usize, 0, 3, 7], [-1.0, 0.0, 0.0]),
        ([1usize, 5, 6, 2], [1.0, 0.0, 0.0]),
        ([3usize, 2, 6, 7], [0.0, 1.0, 0.0]),
        ([4usize, 5, 1, 0], [0.0, -1.0, 0.0]),
    ];

    for (face_index, (quad, normal)) in faces.into_iter().enumerate() {
        let base = (face_index * 4) as u32;
        let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

        for (corner_index, uv) in quad.into_iter().zip(uv) {
            vertices.push(StaticModelVertex {
                position: corners[corner_index].to_array(),
                normal,
                uv,
                color,
            });
        }

        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    StaticModelMesh {
        label: label.into(),
        vertices,
        indices,
    }
}

fn project_asset_path(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path)
}

fn load_weapon_source_meshes(weapon: WeaponDefinition) -> Vec<StaticModelMesh> {
    let spawn = StaticModelSpawn {
        position: Vec3::ZERO,
        uniform_scale: 1.0,
    };
    let weapon_path = project_asset_path(weapon.model_asset_path);

    match model::load_static_obj(&weapon_path, spawn) {
        Ok(loaded_model) => {
            tracing::info!(
                path = %loaded_model.source_path.display(),
                mesh_count = loaded_model.meshes.len(),
                material_libraries = loaded_model.material_libraries.len(),
                referenced_diffuse_textures = loaded_model.referenced_diffuse_textures.len(),
                "static OBJ model loaded"
            );

            if !loaded_model.referenced_diffuse_textures.is_empty() {
                tracing::info!(
                    ?loaded_model.referenced_diffuse_textures,
                    "OBJ diffuse textures were detected but are not rendered yet"
                );
            }

            normalize_static_model_meshes(
                loaded_model.meshes.into_iter().map(Into::into).collect(),
                1.0,
            )
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                path = %weapon_path.display(),
                "failed to load weapon OBJ model"
            );
            Vec::new()
        }
    }
}

fn normalize_static_model_meshes(
    meshes: Vec<StaticModelMesh>,
    target_max_extent: f32,
) -> Vec<StaticModelMesh> {
    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);

    for mesh in &meshes {
        for vertex in &mesh.vertices {
            let pos = Vec3::from_array(vertex.position);
            bounds_min = bounds_min.min(pos);
            bounds_max = bounds_max.max(pos);
        }
    }

    if !bounds_min.is_finite() || !bounds_max.is_finite() {
        return meshes;
    }

    let center = (bounds_min + bounds_max) * 0.5;
    let extent = (bounds_max - bounds_min).max(Vec3::splat(0.0001));
    let scale = target_max_extent / extent.max_element().max(0.0001);

    meshes
        .into_iter()
        .map(|mesh| StaticModelMesh {
            label: mesh.label,
            vertices: mesh
                .vertices
                .into_iter()
                .map(|vertex| {
                    let pos = (Vec3::from_array(vertex.position) - center) * scale;
                    let mut out = vertex;
                    out.position = pos.to_array();
                    out
                })
                .collect(),
            indices: mesh.indices,
        })
        .collect()
}

fn transform_viewmodel_mesh(
    mesh: &StaticModelMesh,
    label: impl Into<String>,
    camera: &Camera,
    local_rotation: Quat,
    local_offset: Vec3,
    scale: f32,
) -> StaticModelMesh {
    let forward = camera.forward().normalize_or_zero();
    let right = camera.right().normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let origin =
        camera.position + right * local_offset.x + up * local_offset.y + forward * local_offset.z;

    StaticModelMesh {
        label: label.into(),
        vertices: mesh
            .vertices
            .iter()
            .copied()
            .map(|vertex| {
                let local_position = local_rotation * (Vec3::from_array(vertex.position) * scale);
                let local_normal =
                    (local_rotation * Vec3::from_array(vertex.normal)).normalize_or_zero();

                let world_position = origin
                    + right * local_position.x
                    + up * local_position.y
                    + forward * local_position.z;
                let world_normal =
                    (right * local_normal.x + up * local_normal.y + forward * local_normal.z)
                        .normalize_or_zero();

                StaticModelVertex {
                    position: world_position.to_array(),
                    normal: world_normal.to_array(),
                    uv: vertex.uv,
                    color: vertex.color,
                }
            })
            .collect(),
        indices: mesh.indices.clone(),
    }
}
