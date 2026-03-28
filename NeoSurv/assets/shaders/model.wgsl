struct CameraUniform {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct ModelUniform {
    model: mat4x4<f32>,
    tint: vec4<f32>,
};

@group(1) @binding(0) var<uniform> model: ModelUniform;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let world_position = model.model * vec4<f32>(input.position, 1.0);
    out.clip_position = camera.projection * camera.view * world_position;
    out.normal = normalize((model.model * vec4<f32>(input.normal, 0.0)).xyz);
    out.uv = input.uv;
    out.color = input.color * model.tint;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.35, 0.75, 0.25));
    let diffuse = max(dot(normalize(input.normal), light_dir), 0.18);
    let uv_tint = select(0.96, 1.0, ((u32(floor(input.uv.x * 4.0)) + u32(floor(input.uv.y * 4.0))) % 2u) == 0u);
    return vec4<f32>(input.color.rgb * diffuse * uv_tint, input.color.a);
}
