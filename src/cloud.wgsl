#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> toward_light: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> light: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> ambient: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> drift: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> night: vec4<f32>;

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
  return (1.0 - g2) / (4.0 * PI * pow(max(1.0 + g2 - 2.0 * g * cosine, 1e-4), 1.5));
}

fn hash3(cell: vec3<i32>) -> vec3<f32> {
  let a = scramble(cell.xy + vec2(cell.z * 31, cell.z * 17));
  let b = scramble(cell.zx + vec2<i32>(i32(a.x & 1023u), 7));
  return vec3<f32>(vec3<u32>(a.x & 0xffffu, a.y & 0xffffu, b.x & 0xffffu)) / 65535.0;
}

fn stars(ray: vec3<f32>) -> vec3<f32> {
  let scaled = ray * 260.0;
  let cell = vec3<i32>(floor(scaled));
  let pick = hash3(cell);
  let spot = normalize(vec3<f32>(cell) + 0.25 + 0.5 * hash3(cell + vec3(11, 5, 3)));
  let gap = length(ray - spot) * 260.0;
  let band = abs(dot(ray, normalize(vec3(0.35, 0.55, -0.75))));
  let galaxy = exp(-band * band * 60.0);
  let chance = 0.022 + 0.1 * galaxy;
  let lit = f32(pick.x < chance);
  let magnitude = pick.y * pick.y * pick.y * pick.y * pick.y * pick.y * pick.y * pick.y * pick.y * 12.0 + 0.2 + 0.3 * pick.y;
  let twinkle = 0.75 + 0.25 * sin(night.z * (2.0 + pick.z * 5.0) + pick.x * 90.0);
  let tint = mix(vec3(1.0, 0.82, 0.66), vec3(0.72, 0.84, 1.0), pick.z);
  let point = exp(-gap * gap * 9.0) * lit * magnitude * twinkle;
  let dust = galaxy * (0.35 + 0.65 * smoothstep(-0.2, 0.5, fbm(ray.xz * 9.0 + ray.y * 5.0, 4)))
    * (1.0 - 0.6 * smoothstep(0.1, 0.45, fbm(ray.xz * 22.0 + 3.0, 3)));
  return tint * point * 90.0 + vec3(0.55, 0.6, 0.8) * dust * 2.2;
}

fn aurora(eye: vec3<f32>, ray: vec3<f32>) -> vec3<f32> {
  var glow = vec3(0.0);
  let steps = 20;
  for (var i = 0; i < steps; i++) {
    let lift = f32(i) / f32(steps - 1);
    let height = 4200.0 + lift * 5200.0;
    let at = eye.xz + ray.xz * (height - eye.y) / max(ray.y, 0.02);
    let along = at.x / 5200.0 + night.z * 0.012;
    let sway = fbm(vec2(along, 1.7), 3) * 3600.0 + sin(along * 1.7 + night.z * 0.03) * 1400.0;
    let line = abs(at.y - (eye.z - 7500.0 + sway)) / 380.0;
    let fold = 0.55 + 0.45 * sin(at.x / 170.0 + fbm(vec2(along * 6.0, night.z * 0.02), 2) * 9.0);
    let curtain = exp(-line * line) * fold * exp2(1.6 * log2(max(1.0 - lift, 1e-4)));
    let colour = mix(vec3(0.15, 1.0, 0.45), vec3(0.55, 0.2, 0.9), smoothstep(0.35, 0.95, lift));
    glow += colour * curtain;
  }
  return glow * 420.0 / f32(steps);
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
  var above = vec3(0.0);
  if (night.x > 0.001) {
    let horizon = smoothstep(0.04, 0.3, ray.y);
    above = (stars(ray) * night.x + aurora(eye, ray) * night.y) * horizon;
  }
  return vec4(clamp((radiance * alpha + above * (1.0 - alpha)) * view.exposure, vec3(0.0), vec3(64.0)), alpha);
}
