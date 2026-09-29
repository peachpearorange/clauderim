#import bevy_pbr::{mesh_functions::get_world_from_local, mesh_view_bindings::view, view_transformations::position_world_to_clip}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> toward_light: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> light: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> ambient: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> drift: vec4<f32>;

struct Vertex {
  @builtin(instance_index) instance_index: u32,
  @location(0) center: vec3<f32>,
  @location(1) ground: vec3<f32>,
  @location(2) corner: vec2<f32>,
  @location(3) shape: vec2<f32>,
};

struct Varying {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) corner: vec2<f32>,
  @location(2) shape: vec2<f32>,
  @location(3) ground: vec3<f32>,
  @location(4) center: vec3<f32>,
};

@vertex
fn vertex(in: Vertex) -> Varying {
  let center = (get_world_from_local(in.instance_index) * vec4(in.center, 1.0)).xyz;
  let toward_eye = normalize(view.world_position - center);
  let right = normalize(cross(vec3(0.0, 1.0, 0.0), toward_eye) + vec3(1e-4, 0.0, 0.0));
  let up = cross(toward_eye, right);
  let world = center + (right * in.corner.x * 1.3 + up * in.corner.y * 0.75) * in.shape.x;
  var out: Varying;
  out.clip = position_world_to_clip(world);
  out.world = world;
  out.corner = in.corner;
  out.shape = in.shape;
  out.ground = in.ground;
  out.center = center;
  return out;
}

fn hash(cell: vec2<f32>) -> f32 {
  return fract(sin(dot(cell, vec2(127.1, 311.7))) * 43758.5453);
}

fn value(at: vec2<f32>) -> f32 {
  let base = floor(at);
  let f = at - base;
  let u = f * f * (3.0 - 2.0 * f);
  return mix(
    mix(hash(base), hash(base + vec2(1.0, 0.0)), u.x),
    mix(hash(base + vec2(0.0, 1.0)), hash(base + vec2(1.0, 1.0)), u.x),
    u.y
  );
}

fn fbm(at: vec2<f32>) -> f32 {
  return value(at) * 0.5 + value(at * 2.1 + 5.3) * 0.3 + value(at * 4.3 + 9.1) * 0.2;
}

@fragment
fn fragment(in: Varying) -> @location(0) vec4<f32> {
  let eye = view.world_position;
  let seed = in.shape.y;
  let time = drift.x;
  let billow = fbm(in.corner * 1.6 + vec2(seed, seed * 0.7) + vec2(time * 0.012, 0.0));
  let reach = length(in.corner * vec2(1.0, 1.25)) + (billow - 0.5) * 0.9;
  let body = smoothstep(1.0, 0.25, reach);
  let wisps = smoothstep(0.3, 0.75, fbm(in.corner * in.shape.x / vec2(46.0, 24.0) + vec2(time * 0.04 + seed, seed * 1.3)));
  let density = body * (0.45 + 0.55 * wisps);
  let ground_level = in.ground.x + dot(in.ground.yz, in.world.xz - in.center.xz);
  let above = smoothstep(0.0, 35.0, in.world.y - ground_level);
  let range = length(in.world - eye);
  let close = smoothstep(40.0, 260.0, range);
  let far = 1.0 - smoothstep(6500.0, 9000.0, range);
  let alpha = clamp(density * above * close * far * 0.8, 0.0, 1.0);
  let ray = normalize(in.world - eye);
  let facing = max(dot(ray, toward_light.xyz), 0.0);
  let lit = light.rgb * (0.05 + 0.06 * pow(facing, 6.0)) * (0.55 + 0.45 * billow);
  let radiance = lit + ambient.rgb * (1.25 - 0.3 * body);
  return vec4(clamp(radiance * view.exposure, vec3(0.0), vec3(32.0)) * alpha, alpha);
}
