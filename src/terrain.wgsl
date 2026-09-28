#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var tones: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var tones_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var relief: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var relief_sampler: sampler;

const GRASS: i32 = 0;
const LITTER: i32 = 1;
const DIRT: i32 = 2;
const GRAVEL: i32 = 3;
const ROCK: i32 = 4;
const SNOW: i32 = 5;

struct Layer {
    tone: vec4<f32>,
    normal: vec3<f32>,
    rough: f32,
}

fn span(layer: i32) -> f32 {
    switch layer {
        case 0: { return 2.6; }
        case 1: { return 2.8; }
        case 2: { return 3.2; }
        case 3: { return 1.4; }
        case 4: { return 7.0; }
        default: { return 8.0; }
    }
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i), hash2(i + vec2(1.0, 0.0)), s.x),
        mix(hash2(i + vec2(0.0, 1.0)), hash2(i + vec2(1.0, 1.0)), s.x),
        s.y,
    );
}

fn fetch(layer: i32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> Layer {
    let tone = textureSampleGrad(tones, tones_sampler, uv, layer, dx, dy);
    let bump = textureSampleGrad(relief, relief_sampler, uv, layer, dx, dy);
    return Layer(tone, bump.xyz * 2.0 - 1.0, bump.a);
}

fn lay_flat(n: vec3<f32>, up: vec3<f32>) -> vec3<f32> {
    return normalize(vec3(n.x + up.x, abs(n.z) * up.y, n.y + up.z));
}

// Two samples at different scale and rotation, mixed by a world-space noise so the tiling doesn't show.
fn ground(layer: i32, at: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, up: vec3<f32>, mask: f32) -> Layer {
    let size = span(layer);
    let a = fetch(layer, at / size, dx / size, dy / size);
    let turn = mat2x2<f32>(0.766, -0.643, 0.643, 0.766);
    let wide = size * 2.3;
    let b = fetch(layer, turn * at / wide + 0.37, turn * dx / wide, turn * dy / wide);
    let pick = clamp((mask - 0.5) * 3.0 + (b.tone.a - a.tone.a) * 1.5 + 0.5, 0.0, 1.0);
    let b_normal = vec3(transpose(turn) * b.normal.xy, b.normal.z);
    return Layer(
        mix(a.tone, b.tone, pick),
        lay_flat(mix(a.normal, b_normal, pick), up),
        mix(a.rough, b.rough, pick),
    );
}

fn triplanar(layer: i32, at: vec3<f32>, dx: vec3<f32>, dy: vec3<f32>, up: vec3<f32>) -> Layer {
    let size = span(layer);
    var blend = pow(abs(up), vec3(4.0));
    blend = blend / (blend.x + blend.y + blend.z);
    let x = fetch(layer, at.zy / size, dx.zy / size, dy.zy / size);
    let y = fetch(layer, at.xz / size, dx.xz / size, dy.xz / size);
    let z = fetch(layer, at.xy / size, dx.xy / size, dy.xy / size);
    let nx = vec3(x.normal.xy + up.zy, abs(x.normal.z) * up.x).zyx;
    let ny = vec3(y.normal.xy + up.xz, abs(y.normal.z) * up.y).xzy;
    let nz = vec3(z.normal.xy + up.xy, abs(z.normal.z) * up.z);
    return Layer(
        x.tone * blend.x + y.tone * blend.y + z.tone * blend.z,
        normalize(nx * blend.x + ny * blend.y + nz * blend.z),
        x.rough * blend.x + y.rough * blend.y + z.rough * blend.z,
    );
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let at = in.world_position.xyz;
    let up = normalize(in.world_normal);
    let dx = dpdx(at);
    let dy = dpdy(at);
    var weights: array<f32, 6>;
    weights[LITTER] = in.uv.x;
    weights[DIRT] = in.uv.y;
    weights[GRAVEL] = in.uv_b.x;
    weights[ROCK] = in.uv_b.y;
    weights[SNOW] = in.color.a;
    weights[GRASS] = max(1.0 - in.uv.x - in.uv.y - in.uv_b.x - in.uv_b.y - in.color.a, 0.0);
    let mask = value_noise(at.xz / 9.0) * 0.65 + value_noise(at.xz / 2.7) * 0.35;

    var layers: array<Layer, 6>;
    var scores: array<f32, 6>;
    var top = -1.0;
    for (var layer = 0; layer < 6; layer++) {
        if weights[layer] > 0.004 {
            if layer == ROCK {
                layers[layer] = triplanar(layer, at, dx, dy, up);
            } else {
                layers[layer] = ground(layer, at.xz, dx.xz, dy.xz, up, mask);
            }
            let cover = select(0.0, 0.7, layer == SNOW);
            scores[layer] = weights[layer] + layers[layer].tone.a * 0.5 + cover * weights[layer];
            top = max(top, scores[layer]);
        }
    }
    var tone = vec3(0.0);
    var normal = vec3(0.0);
    var rough = 0.0;
    var total = 0.0;
    for (var layer = 0; layer < 6; layer++) {
        if weights[layer] > 0.004 {
            let share = max(scores[layer] - top + 0.18, 0.0);
            tone += layers[layer].tone.rgb * share;
            normal += layers[layer].normal * share;
            rough += layers[layer].rough * share;
            total += share;
        }
    }
    tone /= total;
    rough /= total;

    let macro_tone = 0.9 + 0.2 * value_noise(at.xz / 31.0);
    pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * tone * macro_tone, 1.0);
    pbr_input.material.perceptual_roughness = rough;
    pbr_input.N = normalize(normal);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
