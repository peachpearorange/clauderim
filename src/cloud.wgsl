#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> toward_light: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> light: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> ambient: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> drift: vec4<f32>;

const PI: f32 = 3.14159265;
const TURN: mat2x2<f32> = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);

fn scramble(cell: vec2<i32>) -> vec2<u32> {
  var v = bitcast<vec2<u32>>(cell) * 1664525u + 1013904223u;
  v.x += v.y * 1664525u;
  v.y += v.x * 1664525u;
  v = v ^ (v >> vec2<u32>(16u));
  v.x += v.y * 1664525u;
  v.y += v.x * 1664525u;
  return v ^ (v >> vec2<u32>(16u));
}

fn jitter(cell: vec2<i32>) -> vec2<f32> {
  return vec2<f32>(scramble(cell) & vec2<u32>(0xffffu)) / 65535.0;
}

fn gradient(cell: vec2<i32>, offset: vec2<f32>) -> f32 {
  return dot(jitter(cell) * 2.0 - 1.0, offset);
}

fn perlin(at: vec2<f32>) -> f32 {
  let base = floor(at);
  let f = at - base;
  let c = vec2<i32>(base);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(
    mix(gradient(c, f), gradient(c + vec2(1, 0), f - vec2(1.0, 0.0)), u.x),
    mix(gradient(c + vec2(0, 1), f - vec2(0.0, 1.0)), gradient(c + vec2(1, 1), f - vec2(1.0, 1.0)), u.x),
    u.y
  );
}

fn fbm(at: vec2<f32>, octaves: i32) -> f32 {
  var sum = 0.0;
  var amplitude = 0.5;
  var p = at;
  for (var i = 0; i < octaves; i++) {
    sum += amplitude * perlin(p);
    p = TURN * p * 2.03 + vec2(13.7, 5.1);
    amplitude *= 0.5;
  }
  return sum;
}

fn cells(at: vec2<f32>) -> f32 {
  let base = floor(at);
  let c = vec2<i32>(base);
  var nearest = 2.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let neighbour = c + vec2(x, y);
      let point = vec2<f32>(neighbour) + jitter(neighbour + vec2(71, 29));
      nearest = min(nearest, length(point - at));
    }
  }
  return nearest;
}

fn density(at: vec2<f32>, detail: i32) -> f32 {
  let q = at + drift.xy;
  let stretched = vec2(q.x * 0.8, q.y);
  let warp = vec2(fbm(q / 1400.0, 2), fbm(q / 1400.0 + vec2(17.0, 3.0), 2)) * 420.0;
  let broad = fbm(q / 6500.0 + vec2(4.0, 9.0), 3);
  let puffs = 1.0 - cells((stretched + warp) / 620.0);
  let fine = fbm((q + warp) / 240.0, detail) + fbm(q / 90.0, detail - 1) * 0.35;
  let shape = puffs * 0.62 + fine * 0.55 + broad * 1.3 + drift.z;
  return clamp((shape - 0.5) * 2.4, 0.0, 1.0);
}

fn scatter(cosine: f32, g: f32) -> f32 {
  let g2 = g * g;
  return (1.0 - g2) / (4.0 * PI * pow(1.0 + g2 - 2.0 * g * cosine, 1.5));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
  let eye = view.world_position;
  let at = in.world_position.xyz;
  let ray = normalize(at - eye);
  let reach = length(at.xz - eye.xz);
  let fade = 1.0 - smoothstep(7000.0, 24000.0, reach);
  let thick = density(at.xz, 4);
  let sun_flat = toward_light.xz / max(length(toward_light.xz), 1e-3);
  let shadow = density(at.xz + sun_flat * 90.0, 2)
    + density(at.xz + sun_flat * 230.0, 2)
    + density(at.xz + sun_flat * 430.0, 1)
    + density(at.xz + sun_flat * 700.0, 1);
  let slant = mix(1.6, 0.6, clamp(toward_light.y, 0.0, 1.0));
  let through = exp(-(shadow * 0.7 * slant + thick * 1.6));
  let powder = 1.0 - exp(-thick * 5.0);
  let cosine = dot(ray, toward_light.xyz);
  let phase = 0.07 + scatter(cosine, 0.6) * 0.8 + scatter(cosine, -0.2) * 0.4;
  let sunlit = light.rgb * through * mix(0.35, 1.0, powder) * phase;
  let underside = ambient.rgb * (1.1 - 0.7 * thick);
  let haze = smoothstep(3000.0, 22000.0, reach) * 0.55;
  let radiance = mix(sunlit + underside, ambient.rgb * 1.3, haze);
  let alpha = smoothstep(0.0, 0.45, thick) * fade * smoothstep(0.0, 0.03, ray.y);
  return vec4(radiance * view.exposure, alpha);
}
