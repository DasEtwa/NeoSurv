struct CameraUniform {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var block_atlas: texture_2d<f32>;
@group(1) @binding(1) var block_sampler: sampler;

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

fn atlas_uv(material_id: u32, uv: vec2<f32>) -> vec2<f32> {
    let atlas_columns = 4.0;
    let atlas_rows = 2.0;
    let tile_size = vec2<f32>(1.0 / atlas_columns, 1.0 / atlas_rows);
    let tile = f32(material_id);
    let tile_x = tile % atlas_columns;
    let tile_y = floor(tile / atlas_columns);
    let tiled_uv = fract(uv);
    return vec2<f32>(tile_x, tile_y) * tile_size + tiled_uv * tile_size;
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
    let base_color = textureSample(block_atlas, block_sampler, atlas_uv(input.material_id, input.uv)).rgb;
    let light_dir = normalize(vec3<f32>(0.35, 0.8, 0.2));
    let diffuse = max(dot(normalize(input.normal), light_dir), 0.15);
    return vec4<f32>(base_color * diffuse, 1.0);
}
