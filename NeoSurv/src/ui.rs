use glam::Vec3;

use crate::{
    renderer::{StaticModelMesh, StaticModelVertex},
    world::camera::Camera,
};

pub(crate) fn sanitize_text(text: &str) -> String {
    let upper = text.to_ascii_uppercase();
    if upper.is_empty() {
        return " ".to_string();
    }

    upper
        .chars()
        .map(|ch| if glyph_rows(ch).is_some() { ch } else { ' ' })
        .collect()
}

pub(crate) fn text_width(text: &str, scale: f32) -> f32 {
    let glyph_width = 5.0 * scale;
    let spacing = scale;
    text.chars().count() as f32 * (glyph_width + spacing)
}

pub(crate) fn build_text_mesh(
    label: impl Into<String>,
    text: &str,
    origin: Vec3,
    scale: f32,
    color: [f32; 4],
) -> StaticModelMesh {
    let mut mesh = StaticModelMesh {
        label: label.into(),
        vertices: Vec::new(),
        indices: Vec::new(),
    };

    let sanitized = sanitize_text(text);
    let mut cursor_x = origin.x;
    for ch in sanitized.chars() {
        if let Some(rows) = glyph_rows(ch) {
            for (row_index, bits) in rows.iter().enumerate() {
                for column in 0..5 {
                    let mask = 1 << (4 - column);
                    if bits & mask == 0 {
                        continue;
                    }

                    let min = Vec3::new(
                        cursor_x + column as f32 * scale,
                        origin.y - row_index as f32 * scale,
                        origin.z,
                    );
                    let max = min + Vec3::new(scale * 0.86, scale * 0.86, scale * 0.16);
                    append_box(&mut mesh, min, max, color);
                }
            }
        }

        cursor_x += 6.0 * scale;
    }

    mesh
}

pub(crate) fn build_box_mesh(
    label: impl Into<String>,
    min: Vec3,
    max: Vec3,
    color: [f32; 4],
) -> StaticModelMesh {
    let mut mesh = StaticModelMesh {
        label: label.into(),
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    append_box(&mut mesh, min, max, color);
    mesh
}

pub(crate) fn transform_overlay_mesh(
    mesh: &StaticModelMesh,
    label: impl Into<String>,
    camera: &Camera,
    offset: Vec3,
) -> StaticModelMesh {
    let forward = camera.forward().normalize_or_zero();
    let right = camera.right().normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let origin = camera.position + right * offset.x + up * offset.y + forward * offset.z;

    StaticModelMesh {
        label: label.into(),
        vertices: mesh
            .vertices
            .iter()
            .copied()
            .map(|vertex| {
                let local = Vec3::from_array(vertex.position);
                let world_position = origin + right * local.x + up * local.y + forward * local.z;
                let local_normal = Vec3::from_array(vertex.normal);
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

fn append_box(mesh: &mut StaticModelMesh, min: Vec3, max: Vec3, color: [f32; 4]) {
    let base_index = mesh.vertices.len() as u32;
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
        let face_base = base_index + (face_index * 4) as u32;
        let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

        for (corner_index, uv) in quad.into_iter().zip(uv) {
            mesh.vertices.push(StaticModelVertex {
                position: corners[corner_index].to_array(),
                normal,
                uv,
                color,
            });
        }

        mesh.indices.extend_from_slice(&[
            face_base,
            face_base + 1,
            face_base + 2,
            face_base,
            face_base + 2,
            face_base + 3,
        ]);
    }
}

fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    match ch {
        'A' => Some([
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'B' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ]),
        'C' => Some([
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ]),
        'D' => Some([
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ]),
        'E' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ]),
        'F' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'G' => Some([
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ]),
        'H' => Some([
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'I' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ]),
        'J' => Some([
            0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
        ]),
        'K' => Some([
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ]),
        'L' => Some([
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ]),
        'M' => Some([
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ]),
        'N' => Some([
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ]),
        'O' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'P' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'Q' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ]),
        'R' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ]),
        'S' => Some([
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        'T' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'U' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'V' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ]),
        'W' => Some([
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ]),
        'X' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ]),
        'Y' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'Z' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ]),
        '0' => Some([
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ]),
        '1' => Some([
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        '2' => Some([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ]),
        '3' => Some([
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        '4' => Some([
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ]),
        '5' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ]),
        '6' => Some([
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ]),
        '7' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ]),
        '8' => Some([
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ]),
        '9' => Some([
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ]),
        '<' => Some([
            0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001,
        ]),
        '>' => Some([
            0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000,
        ]),
        '/' => Some([
            0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000,
        ]),
        ':' => Some([
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ]),
        '.' => Some([
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100,
        ]),
        '-' => Some([
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ]),
        '!' => Some([
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ]),
        ' ' => Some([0, 0, 0, 0, 0, 0, 0]),
        _ => None,
    }
}
