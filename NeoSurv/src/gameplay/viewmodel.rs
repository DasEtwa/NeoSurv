use std::path::PathBuf;

use glam::{Mat4, Quat, Vec3};

use crate::{
    game::model::{self, StaticModelSpawn},
    renderer::{MeshInstance, StaticModelMesh, StaticModelVertex},
    ui::camera_basis_matrix,
    world::camera::Camera,
};

use super::weapons::{WeaponDefinition, WeaponEffects};

const MUZZLE_FLASH_TEMPLATE_LABEL: &str = "viewmodel-muzzle-flash-template";
const HAND_PALM_TEMPLATE_LABEL: &str = "viewmodel-hand-palm-template";
const HAND_FOREARM_TEMPLATE_LABEL: &str = "viewmodel-hand-forearm-template";

#[derive(Debug)]
pub(crate) struct ViewmodelAssets {
    weapon_source_meshes: Vec<StaticModelMesh>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
struct ViewmodelComposition {
    weapon: Vec<MeshInstance>,
    hand: Vec<MeshInstance>,
    muzzle: Option<MeshInstance>,
}

impl ViewmodelComposition {
    fn flatten(self) -> Vec<MeshInstance> {
        let mut instances =
            Vec::with_capacity(self.weapon.len() + self.hand.len() + usize::from(self.muzzle.is_some()));
        instances.extend(self.weapon);
        instances.extend(self.hand);
        if let Some(muzzle) = self.muzzle {
            instances.push(muzzle);
        }
        instances
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
struct ViewmodelAnchors {
    viewmodel_root: Mat4,
    grip_anchor: Mat4,
    barrel_anchor: Mat4,
    weapon_transform: Mat4,
    hand_transform: Mat4,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
struct WeaponAnchorProfile {
    root_offset_bias: Vec3,
    weapon_local_offset: Vec3,
    hand_local_offset: Vec3,
    barrel_local_offset: Vec3,
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

        meshes.push(transform_viewmodel_mesh(
            &build_box_mesh(
                HAND_FOREARM_TEMPLATE_LABEL,
                Vec3::new(-0.06, -0.18, -0.18),
                Vec3::new(0.05, 0.00, 0.06),
                [0.32, 0.24, 0.20, 1.0],
            ),
            "viewmodel-hand-forearm",
            camera,
            model_rotation * Quat::from_rotation_z(0.18) * Quat::from_rotation_x(-0.10),
            Vec3::new(
                weapon_offset.x - 0.05,
                weapon_offset.y - 0.06,
                weapon_offset.z - 0.02,
            ),
            weapon.viewmodel_scale * 0.92,
        ));
        meshes.push(transform_viewmodel_mesh(
            &build_box_mesh(
                HAND_PALM_TEMPLATE_LABEL,
                Vec3::new(-0.05, -0.05, -0.10),
                Vec3::new(0.05, 0.04, 0.08),
                [0.56, 0.44, 0.34, 1.0],
            ),
            "viewmodel-hand-palm",
            camera,
            model_rotation * Quat::from_rotation_z(0.18) * Quat::from_rotation_x(-0.10),
            Vec3::new(
                weapon_offset.x - 0.03,
                weapon_offset.y - 0.01,
                weapon_offset.z + 0.03,
            ),
            weapon.viewmodel_scale * 0.86,
        ));

        if weapon_effects.muzzle_flash_active() {
            meshes.push(transform_viewmodel_mesh(
                &build_box_mesh(
                    MUZZLE_FLASH_TEMPLATE_LABEL,
                    Vec3::new(-0.06, -0.06, -0.02),
                    Vec3::new(0.06, 0.06, 0.14),
                    [1.0, 0.92, 0.55, 1.0],
                ),
                "viewmodel-muzzle-flash",
                camera,
                model_rotation,
                Vec3::new(
                    weapon_offset.x + 0.02,
                    weapon_offset.y + 0.02,
                    weapon_offset.z + 0.28,
                ),
                weapon.viewmodel_scale,
            ));
        }

        meshes
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn build_template_meshes(&self, weapon: WeaponDefinition) -> Vec<StaticModelMesh> {
        let mut meshes = if !self.weapon_source_meshes.is_empty() {
            let mut meshes = Vec::with_capacity(self.weapon_source_meshes.len() + 3);
            meshes.extend(
                self.weapon_source_meshes
                    .iter()
                    .enumerate()
                    .map(|(index, mesh)| StaticModelMesh {
                        label: format!("viewmodel-{}-{index}", weapon.id),
                        vertices: mesh.vertices.clone(),
                        indices: mesh.indices.clone(),
                    }),
            );
            meshes
        } else {
            vec![
            build_box_mesh(
                "fallback-sidearm-slide",
                Vec3::new(-0.11, -0.05, -0.20),
                Vec3::new(0.11, 0.05, 0.16),
                [0.08, 0.08, 0.10, 1.0],
            ),
            build_box_mesh(
                "fallback-sidearm-grip",
                Vec3::new(-0.05, -0.22, -0.02),
                Vec3::new(0.05, -0.02, 0.09),
                [0.22, 0.12, 0.08, 1.0],
            ),
            build_box_mesh(
                "fallback-sidearm-barrel",
                Vec3::new(-0.03, -0.01, 0.16),
                Vec3::new(0.03, 0.03, 0.28),
                [0.20, 0.20, 0.22, 1.0],
            ),
            ]
        };

        meshes.push(build_box_mesh(
            MUZZLE_FLASH_TEMPLATE_LABEL,
            Vec3::new(-0.05, -0.05, -0.02),
            Vec3::new(0.05, 0.05, 0.20),
            [1.0, 1.0, 1.0, 1.0],
        ));
        meshes.extend(build_hand_template_meshes());
        meshes
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn build_instances(
        &self,
        camera: &Camera,
        weapon_effects: &WeaponEffects,
        weapon: WeaponDefinition,
    ) -> Vec<MeshInstance> {
        self.build_composition(camera, weapon_effects, weapon).flatten()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn build_composition(
        &self,
        camera: &Camera,
        weapon_effects: &WeaponEffects,
        weapon: WeaponDefinition,
    ) -> ViewmodelComposition {
        let anchors = build_viewmodel_anchors(camera, weapon_effects, weapon);
        let mut weapon_instances = Vec::new();
        if !self.weapon_source_meshes.is_empty() {
            weapon_instances.extend(
                self.weapon_source_meshes
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        MeshInstance::new(
                            format!("viewmodel-{}-{index}", weapon.id),
                            anchors.weapon_transform,
                            [1.0, 1.0, 1.0, 1.0],
                        )
                    }),
            );
        } else {
            weapon_instances.push(MeshInstance::new(
                "fallback-sidearm-slide",
                anchors.weapon_transform,
                [1.0, 1.0, 1.0, 1.0],
            ));
            weapon_instances.push(MeshInstance::new(
                "fallback-sidearm-grip",
                anchors.weapon_transform,
                [1.0, 1.0, 1.0, 1.0],
            ));
            weapon_instances.push(MeshInstance::new(
                "fallback-sidearm-barrel",
                anchors.weapon_transform,
                [1.0, 1.0, 1.0, 1.0],
            ));
        }

        let hand_instances = vec![
            MeshInstance::new(
                HAND_FOREARM_TEMPLATE_LABEL,
                anchors.hand_transform,
                [0.30, 0.22, 0.18, 1.0],
            ),
            MeshInstance::new(
                HAND_PALM_TEMPLATE_LABEL,
                anchors.hand_transform,
                [0.58, 0.44, 0.34, 1.0],
            ),
        ];

        let muzzle = weapon_effects.muzzle_flash_active().then(|| {
            MeshInstance::new(
                MUZZLE_FLASH_TEMPLATE_LABEL,
                anchors.barrel_anchor,
                [1.0, 0.92, 0.55, 1.0],
            )
        });

        ViewmodelComposition {
            weapon: weapon_instances,
            hand: hand_instances,
            muzzle,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn build_viewmodel_anchors(
    camera: &Camera,
    weapon_effects: &WeaponEffects,
    weapon: WeaponDefinition,
) -> ViewmodelAnchors {
    let recoil_ratio = weapon_effects.recoil_ratio(weapon);
    let profile = anchor_profile(weapon);
    let viewmodel_offset = Vec3::new(
        weapon.viewmodel_right_offset,
        -weapon.viewmodel_down_offset,
        weapon.viewmodel_distance,
    );
    let viewmodel_offset = viewmodel_offset + profile.root_offset_bias;
    let root_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
        * Quat::from_rotation_z(-0.24)
        * Quat::from_rotation_x(0.14);
    let grip_anchor = camera_basis_matrix(camera)
        * Mat4::from_translation(viewmodel_offset)
        * Mat4::from_quat(root_rotation);

    let weapon_scale = weapon.viewmodel_scale * 0.82;
    let weapon_kick = Vec3::new(
        recoil_ratio * 0.006,
        recoil_ratio * 0.012,
        -recoil_ratio * weapon.viewmodel_recoil_distance,
    );
    let hand_follow = Vec3::new(
        recoil_ratio * 0.002,
        recoil_ratio * 0.004,
        -recoil_ratio * weapon.viewmodel_recoil_distance * 0.28,
    );
    let hand_rotation =
        Quat::from_rotation_z(0.22) * Quat::from_rotation_y(-0.12) * Quat::from_rotation_x(-0.20);

    let weapon_transform = grip_anchor
        * Mat4::from_translation(profile.weapon_local_offset + weapon_kick)
        * Mat4::from_scale(Vec3::splat(weapon_scale));
    let hand_transform = grip_anchor
        * Mat4::from_translation(profile.hand_local_offset + hand_follow)
        * Mat4::from_quat(hand_rotation)
        * Mat4::from_scale(Vec3::splat(weapon_scale * 0.92));
    let barrel_anchor = grip_anchor
        * Mat4::from_translation(profile.barrel_local_offset + weapon_kick)
        * Mat4::from_scale(Vec3::splat(weapon_scale * 0.85));

    ViewmodelAnchors {
        viewmodel_root: camera_basis_matrix(camera) * Mat4::from_translation(viewmodel_offset),
        grip_anchor,
        barrel_anchor,
        weapon_transform,
        hand_transform,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn anchor_profile(weapon: WeaponDefinition) -> WeaponAnchorProfile {
    match weapon.id {
        "launcher" => WeaponAnchorProfile {
            root_offset_bias: Vec3::new(0.18, -0.09, 0.12),
            weapon_local_offset: Vec3::new(0.02, 0.10, 0.24),
            hand_local_offset: Vec3::new(-0.04, -0.05, -0.02),
            barrel_local_offset: Vec3::new(0.20, 0.11, 0.40),
        },
        "grenade" => WeaponAnchorProfile {
            root_offset_bias: Vec3::new(0.14, -0.11, 0.10),
            weapon_local_offset: Vec3::new(-0.02, 0.02, 0.12),
            hand_local_offset: Vec3::new(-0.03, -0.06, -0.03),
            barrel_local_offset: Vec3::new(0.10, 0.08, 0.18),
        },
        _ => WeaponAnchorProfile {
            root_offset_bias: Vec3::new(0.22, -0.10, 0.12),
            weapon_local_offset: Vec3::new(0.00, 0.09, 0.22),
            hand_local_offset: Vec3::new(-0.02, -0.05, -0.02),
            barrel_local_offset: Vec3::new(0.18, 0.10, 0.36),
        },
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn build_hand_template_meshes() -> [StaticModelMesh; 2] {
    [
        build_box_mesh(
            HAND_FOREARM_TEMPLATE_LABEL,
            Vec3::new(-0.06, -0.18, -0.18),
            Vec3::new(0.05, 0.00, 0.06),
            [0.32, 0.24, 0.20, 1.0],
        ),
        build_box_mesh(
            HAND_PALM_TEMPLATE_LABEL,
            Vec3::new(-0.05, -0.05, -0.10),
            Vec3::new(0.05, 0.04, 0.08),
            [0.56, 0.44, 0.34, 1.0],
        ),
    ]
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

#[cfg(test)]
mod tests {
    use glam::Vec4Swizzles;

    use super::*;

    fn translation(instance: &MeshInstance) -> Vec3 {
        Mat4::from_cols_array_2d(&instance.model).w_axis.xyz()
    }

    fn instance<'a>(instances: &'a [MeshInstance], label: &str) -> &'a MeshInstance {
        instances
            .iter()
            .find(|instance| instance.template_label == label)
            .unwrap_or_else(|| panic!("missing instance {label}"))
    }

    #[test]
    fn templates_include_hand_and_muzzle_geometry() {
        let assets = ViewmodelAssets::new(WeaponDefinition::sidearm());
        let templates = assets.build_template_meshes(WeaponDefinition::sidearm());
        let labels: Vec<_> = templates.iter().map(|mesh| mesh.label.as_str()).collect();

        assert!(labels.contains(&HAND_FOREARM_TEMPLATE_LABEL));
        assert!(labels.contains(&HAND_PALM_TEMPLATE_LABEL));
        assert!(labels.contains(&MUZZLE_FLASH_TEMPLATE_LABEL));
    }

    #[test]
    fn build_instances_separates_weapon_hand_and_muzzle_anchors() {
        let assets = ViewmodelAssets::new(WeaponDefinition::sidearm());
        let camera = Camera::default();
        let mut effects = WeaponEffects::new();
        effects.register_shot(WeaponDefinition::sidearm());

        let instances = assets.build_instances(&camera, &effects, WeaponDefinition::sidearm());
        let weapon = instance(&instances, "viewmodel-sidearm-0");
        let hand = instance(&instances, HAND_PALM_TEMPLATE_LABEL);
        let muzzle = instance(&instances, MUZZLE_FLASH_TEMPLATE_LABEL);

        assert_ne!(weapon.model, hand.model);
        assert_ne!(weapon.model, muzzle.model);
        assert_ne!(hand.model, muzzle.model);
    }

    #[test]
    fn weapon_switch_keeps_hand_instances_present() {
        let assets = ViewmodelAssets::new(WeaponDefinition::sidearm());
        let camera = Camera::default();
        let effects = WeaponEffects::new();

        let instances = assets.build_instances(&camera, &effects, WeaponDefinition::launcher());

        assert!(instances
            .iter()
            .any(|instance| instance.template_label.starts_with("viewmodel-launcher-")));
        assert!(instances
            .iter()
            .any(|instance| instance.template_label == HAND_FOREARM_TEMPLATE_LABEL));
        assert!(instances
            .iter()
            .any(|instance| instance.template_label == HAND_PALM_TEMPLATE_LABEL));
    }

    #[test]
    fn recoil_moves_weapon_more_than_hand() {
        let assets = ViewmodelAssets::new(WeaponDefinition::sidearm());
        let camera = Camera::default();
        let idle_effects = WeaponEffects::new();
        let mut recoil_effects = WeaponEffects::new();
        recoil_effects.register_shot(WeaponDefinition::sidearm());

        let idle_instances = assets.build_instances(&camera, &idle_effects, WeaponDefinition::sidearm());
        let recoil_instances =
            assets.build_instances(&camera, &recoil_effects, WeaponDefinition::sidearm());

        let idle_weapon = translation(instance(&idle_instances, "viewmodel-sidearm-0"));
        let recoil_weapon = translation(instance(&recoil_instances, "viewmodel-sidearm-0"));
        let idle_hand = translation(instance(&idle_instances, HAND_PALM_TEMPLATE_LABEL));
        let recoil_hand = translation(instance(&recoil_instances, HAND_PALM_TEMPLATE_LABEL));

        assert!(recoil_weapon.distance(idle_weapon) > recoil_hand.distance(idle_hand));
    }

    #[test]
    fn muzzle_flash_follows_barrel_anchor_instead_of_weapon_root() {
        let camera = Camera::default();
        let mut effects = WeaponEffects::new();
        effects.register_shot(WeaponDefinition::sidearm());
        let anchors = build_viewmodel_anchors(&camera, &effects, WeaponDefinition::sidearm());

        let grip_position = anchors.grip_anchor.w_axis.xyz();
        let barrel_position = anchors.barrel_anchor.w_axis.xyz();
        let weapon_position = anchors.weapon_transform.w_axis.xyz();

        assert!(barrel_position.distance(grip_position) > 0.05);
        assert!(barrel_position.distance(weapon_position) > 0.02);
    }
}
