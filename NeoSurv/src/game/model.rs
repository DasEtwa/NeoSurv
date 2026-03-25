use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use glam::Vec3;

#[derive(Debug, Clone, Copy)]
pub(crate) struct StaticModelSpawn {
    pub(crate) position: Vec3,
    pub(crate) uniform_scale: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StaticModelVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
}

#[derive(Debug, Clone)]
pub(crate) struct StaticModelMesh {
    pub(crate) label: String,
    pub(crate) vertices: Vec<StaticModelVertex>,
    pub(crate) indices: Vec<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedStaticModel {
    pub(crate) source_path: PathBuf,
    pub(crate) meshes: Vec<StaticModelMesh>,
    pub(crate) material_libraries: Vec<String>,
    pub(crate) referenced_diffuse_textures: Vec<String>,
}

pub(crate) fn load_static_obj(
    path: impl AsRef<Path>,
    spawn: StaticModelSpawn,
) -> Result<LoadedStaticModel> {
    let path = path.as_ref();
    let load_options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };

    let (models, materials_result) = tobj::load_obj(path, &load_options)
        .with_context(|| format!("failed to load OBJ model from {}", path.display()))?;

    let materials = materials_result.unwrap_or_default();
    let material_libraries = find_material_libraries(path)?;
    let mut referenced_diffuse_textures = Vec::new();
    let mut meshes = Vec::with_capacity(models.len());

    for material in &materials {
        if let Some(texture) = extract_diffuse_texture(material) {
            referenced_diffuse_textures.push(texture.to_owned());
        }
    }

    for model in models {
        let mesh = &model.mesh;
        if mesh.positions.is_empty() {
            continue;
        }
        if mesh.indices.is_empty() {
            bail!("OBJ mesh '{}' contains no triangle indices", model.name);
        }

        let base_color = mesh
            .material_id
            .and_then(|id| materials.get(id))
            .and_then(extract_diffuse_color)
            .unwrap_or([0.78, 0.78, 0.78, 1.0]);

        let vertices = build_vertices(mesh, spawn, base_color).with_context(|| {
            format!(
                "failed to convert OBJ mesh '{}' from {}",
                model.name,
                path.display()
            )
        })?;

        meshes.push(StaticModelMesh {
            label: model.name,
            vertices,
            indices: mesh.indices.clone(),
        });
    }

    Ok(LoadedStaticModel {
        source_path: path.to_path_buf(),
        meshes,
        material_libraries,
        referenced_diffuse_textures,
    })
}

fn build_vertices(
    mesh: &tobj::Mesh,
    spawn: StaticModelSpawn,
    color: [f32; 4],
) -> Result<Vec<StaticModelVertex>> {
    let vertex_count = mesh.positions.len() / 3;
    let has_normals = mesh.normals.len() >= vertex_count * 3;
    let has_uvs = mesh.texcoords.len() >= vertex_count * 2;
    let mut vertices = Vec::with_capacity(vertex_count);

    for vertex_index in 0..vertex_count {
        let position_offset = vertex_index * 3;
        let uv_offset = vertex_index * 2;

        let position = Vec3::new(
            mesh.positions[position_offset],
            mesh.positions[position_offset + 1],
            mesh.positions[position_offset + 2],
        ) * spawn.uniform_scale
            + spawn.position;

        let normal = if has_normals {
            [
                mesh.normals[position_offset],
                mesh.normals[position_offset + 1],
                mesh.normals[position_offset + 2],
            ]
        } else {
            [0.0, 1.0, 0.0]
        };

        let uv = if has_uvs {
            [
                mesh.texcoords[uv_offset],
                1.0 - mesh.texcoords[uv_offset + 1],
            ]
        } else {
            [0.0, 0.0]
        };

        vertices.push(StaticModelVertex {
            position: position.to_array(),
            normal,
            uv,
            color,
        });
    }

    Ok(vertices)
}

fn find_material_libraries(path: &Path) -> Result<Vec<String>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to inspect OBJ source {}", path.display()))?;

    Ok(source
        .lines()
        .filter_map(|line| line.strip_prefix("mtllib "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn extract_diffuse_color(material: &tobj::Material) -> Option<[f32; 4]> {
    material
        .diffuse
        .map(|diffuse| [diffuse[0], diffuse[1], diffuse[2], 1.0])
}

fn extract_diffuse_texture(material: &tobj::Material) -> Option<&str> {
    material
        .diffuse_texture
        .as_deref()
        .filter(|texture| !texture.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_obj_asset_loads_with_meshes() {
        let model = load_static_obj(
            "assets/models/pistol_1/Pistol_1.obj",
            StaticModelSpawn {
                position: Vec3::ZERO,
                uniform_scale: 1.0,
            },
        )
        .expect("bundled pistol OBJ should load");

        assert!(!model.meshes.is_empty());
        assert!(model.meshes.iter().all(|mesh| !mesh.vertices.is_empty()));
        assert!(model.meshes.iter().all(|mesh| !mesh.indices.is_empty()));
    }
}
