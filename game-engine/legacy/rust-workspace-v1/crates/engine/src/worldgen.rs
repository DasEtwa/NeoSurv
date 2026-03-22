use noise::{NoiseFn, Perlin};

pub fn terrain_height(seed: u32, x: f64, z: f64) -> f32 {
    let noise = Perlin::new(seed);
    let value = noise.get([x * 0.01, z * 0.01]);
    ((value as f32) * 32.0) + 64.0
}
