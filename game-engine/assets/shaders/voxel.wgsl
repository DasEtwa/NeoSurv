struct CameraUniform {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material_id: u32,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) material_id: u32,
};

fn material_color(material_id: u32) -> vec3<f32> {
    switch material_id {
        case 1u: {
            return vec3<f32>(0.35, 0.8, 0.25); // grass
        }
        case 2u: {
            return vec3<f32>(0.50, 0.35, 0.20); // dirt
        }
        case 3u: {
            return vec3<f32>(0.55, 0.55, 0.60); // stone
        }
        case 4u: {
            return vec3<f32>(0.82, 0.72, 0.45); // sand
        }
        case 5u: {
            return vec3<f32>(0.12, 0.20, 0.85); // border wall
        }
        case 6u: {
            return vec3<f32>(0.90, 0.15, 0.15); // dummy target
        }
        default: {
            return vec3<f32>(0.85, 0.2, 0.85); // fallback/missing
        }
    }
}

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_position = camera.projection * camera.view * vec4<f32>(input.position, 1.0);
    out.normal = normalize(input.normal);
    out.uv = input.uv;
    out.material_id = input.material_id;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let base_color = material_color(input.material_id);
    let light_dir = normalize(vec3<f32>(0.35, 0.8, 0.2));
    let diffuse = max(dot(normalize(input.normal), light_dir), 0.15);
    let checker = select(0.94, 1.0, ((u32(floor(input.uv.x * 2.0)) + u32(floor(input.uv.y * 2.0))) % 2u) == 0u);
    return vec4<f32>(base_color * diffuse * checker, 1.0);
}
