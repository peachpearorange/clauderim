#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Blurring {
    near: f32,
    from: f32,
    to: f32,
    spread: f32,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var smooth_sampler: sampler;
@group(0) @binding(3) var<uniform> blurring: Blurring;

const TAPS: u32 = 12u;
const GOLDEN_ANGLE: f32 = 2.39996323;
const REFERENCE_HEIGHT: f32 = 1080.0;

fn softness(pixel: vec2<f32>, size: vec2<f32>) -> f32 {
    let raw = textureLoad(depth, vec2<i32>(clamp(pixel, vec2(0.0), size - 1.0)), 0);
    let distance = blurring.near / max(raw, 1e-9);
    return smoothstep(blurring.from, blurring.to, distance);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(screen));
    let centre = textureSampleLevel(screen, smooth_sampler, in.uv, 0.0);
    let radius = softness(in.position.xy, size) * blurring.spread * size.y / REFERENCE_HEIGHT;
    var total = centre.rgb;
    var weight = 1.0;
    for (var tap = 0u; tap < TAPS; tap++) {
        let angle = f32(tap) * GOLDEN_ANGLE;
        let reach = sqrt((f32(tap) + 0.5) / f32(TAPS)) * radius;
        let at = in.position.xy + vec2(cos(angle), sin(angle)) * reach;
        let share = softness(at, size);
        total += textureSampleLevel(screen, smooth_sampler, at / size, 0.0).rgb * share;
        weight += share;
    }
    return vec4(total / weight, centre.a);
}
