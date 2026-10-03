use {crate::{model::{self, Piece},
             stuff::Stuff},
     bevy::{mesh::VertexAttributeValues, prelude::*}};

const ICE: Srgba = Srgba::rgb(0.5, 0.82, 0.9);

fn shard(seed: u32, from: Vec3, to: Vec3, girth: impl Fn(f32) -> Vec2) -> Piece {
  let mut mesh = model::hewn(seed, 11, 0.0, 2);
  let raw: Vec<Vec3> = mesh
    .attribute(Mesh::ATTRIBUTE_POSITION)
    .and_then(VertexAttributeValues::as_float3)
    .map(|values| values.iter().copied().map(Vec3::from).collect())
    .unwrap_or_default();
  let (low, high) = raw.iter().fold((Vec3::MAX, Vec3::MIN), |(low, high), &point| {
    (low.min(point), high.max(point))
  });
  let (centre, half) = ((low + high) / 2.0, (high - low) / 2.0);
  let turn = Quat::from_rotation_arc(Vec3::Y, (to - from).normalize());
  let length = from.distance(to);
  let positions: Vec<Vec3> = raw
    .into_iter()
    .map(|point| {
      let unit = (point - centre) / half;
      let along = (unit.y + 1.0) / 2.0;
      let Vec2 { x: wide, y: deep } = girth(along) / 2.0;
      let placed = from + turn * Vec3::new(unit.x * wide, along * length, unit.z * deep);
      placed.with_y(placed.y.max(0.0))
    })
    .collect();
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh.compute_flat_normals();
  Piece::new(mesh, ICE)
}

fn tapered(from: Vec2, to: Vec2) -> impl Fn(f32) -> Vec2 {
  move |along| from.lerp(to, along)
}

fn swelling(ends: Vec2, middle: Vec2, at: f32) -> impl Fn(f32) -> Vec2 {
  move |along| {
    let reach = (1.0 - ((along - at) / at.max(1.0 - at)).abs()).clamp(0.0, 1.0);
    ends.lerp(middle, reach.sqrt())
  }
}

pub fn body() -> Vec<(Stuff, Mesh)> {
  let core = [
    shard(1, Vec3::new(0.0, 2.3, 0.02), Vec3::new(0.0, 3.25, 0.12), |along| {
      Vec2::new(0.78, 0.5).lerp(Vec2::new(0.02, 0.02), along.powf(0.75))
    }),
    shard(
      2,
      Vec3::new(0.0, 1.45, 0.0),
      Vec3::new(0.0, 2.62, 0.02),
      tapered(Vec2::new(0.36, 0.32), Vec2::new(1.3, 0.62))
    ),
    shard(
      3,
      Vec3::new(0.0, 1.1, 0.0),
      Vec3::new(0.0, 1.65, 0.0),
      swelling(Vec2::new(0.36, 0.3), Vec2::new(0.44, 0.36), 0.4)
    ),
    shard(
      4,
      Vec3::new(0.0, 0.98, -0.02),
      Vec3::new(0.0, 1.3, 0.0),
      swelling(Vec2::new(0.5, 0.36), Vec2::new(0.62, 0.42), 0.5)
    )
  ];
  let side = |side: f32| {
    let at = |x: f32, y: f32, z: f32| Vec3::new(side * x, y, z);
    let seed = (side > 0.0) as u32 * 20;
    [
      shard(seed + 5, at(0.1, 2.3, 0.0), at(1.0, 2.62, 0.0), |along| {
        Vec2::new(0.62, 0.62).lerp(Vec2::new(0.12, 0.2), along.powf(1.6))
      }),
      shard(
        seed + 6,
        at(0.72, 2.6, 0.0),
        at(0.96, 1.55, 0.08),
        swelling(Vec2::new(0.4, 0.4), Vec2::new(0.44, 0.42), 0.5)
      ),
      shard(seed + 7, at(0.95, 1.85, 0.08), at(1.14, 0.1, 0.22), |along| {
        swelling(Vec2::new(0.44, 0.4), Vec2::new(0.72, 0.52), 0.35)(along)
          * (1.0 - along.powf(2.2)).max(0.05)
      }),
      shard(
        seed + 8,
        at(0.15, 1.25, 0.0),
        at(0.33, 0.82, 0.02),
        swelling(Vec2::new(0.36, 0.36), Vec2::new(0.42, 0.42), 0.45)
      ),
      shard(seed + 9, at(0.34, 1.0, 0.02), at(0.44, -0.12, 0.06), |along| {
        swelling(Vec2::new(0.42, 0.44), Vec2::new(0.48, 0.5), 0.3)(along)
          .lerp(Vec2::new(0.64, 0.76), along.powf(2.0))
      })
    ]
  };
  vec![(
    Stuff::Ice,
    Stuff::Ice.fitted(model::merge(
      core
        .into_iter()
        .chain(side(1.0))
        .chain(side(-1.0))
        .map(|piece| piece.unwrapped(0.6))
    ))
  )]
}
