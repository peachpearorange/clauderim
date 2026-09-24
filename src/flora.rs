use {crate::{model::{self, Piece},
             noise::{self, Roll},
             place::{self, LAKE_LEVEL, Place, START, START_FACING},
             player::Player,
             stuff::{Stuff, Stuffs},
             terrain::{self, BOUND, Ground, HALF, srgb}},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            camera::visibility::VisibilityRange,
            light::NotShadowCaster,
            mesh::{Indices, PrimitiveTopology},
            pbr::{ExtendedMaterial, MaterialExtension},
            platform::collections::HashMap,
            prelude::*,
            render::render_resource::AsBindGroup,
            shader::ShaderRef,
            tasks::{AsyncComputeTaskPool, Task, futures::check_ready}},
     enum_assoc::Assoc,
     std::f32::consts::{FRAC_PI_2, PI, TAU}};

const MEADOW: LinearRgba = srgb(0.33, 0.36, 0.23);
const TUNDRA: LinearRgba = srgb(0.47, 0.42, 0.30);
const FOREST_FLOOR: LinearRgba = srgb(0.25, 0.24, 0.18);
const MEADOW_TIP: LinearRgba = srgb(0.50, 0.52, 0.33);
const TUNDRA_TIP: LinearRgba = srgb(0.60, 0.54, 0.38);
const NEEDLE_DEEP: LinearRgba = srgb(0.06, 0.11, 0.09);
const NEEDLE: LinearRgba = srgb(0.14, 0.23, 0.18);
const NEEDLE_TIP: LinearRgba = srgb(0.25, 0.35, 0.24);
const SNOW: LinearRgba = srgb(0.90, 0.93, 0.98);
const BARK: LinearRgba = srgb(0.31, 0.25, 0.20);
const DARK_BARK: LinearRgba = srgb(0.18, 0.15, 0.13);
const DEAD_WOOD: LinearRgba = srgb(0.48, 0.44, 0.39);
const HEARTWOOD: LinearRgba = srgb(0.68, 0.56, 0.40);
const BIRCH_BARK: LinearRgba = srgb(0.87, 0.85, 0.80);
const BIRCH_SCAR: LinearRgba = srgb(0.14, 0.13, 0.12);
const LEAVES: [LinearRgba; 4] = [
  srgb(0.90, 0.66, 0.12),
  srgb(0.80, 0.48, 0.08),
  srgb(0.86, 0.74, 0.20),
  srgb(0.70, 0.64, 0.18)
];
const STONE: LinearRgba = srgb(0.47, 0.46, 0.44);
const CRAG: LinearRgba = srgb(0.36, 0.36, 0.37);
const LICHEN: LinearRgba = srgb(0.52, 0.50, 0.36);
const DARK_STONE: LinearRgba = srgb(0.30, 0.30, 0.31);
const MOSS: LinearRgba = srgb(0.27, 0.32, 0.12);
const JUNIPER: LinearRgba = srgb(0.07, 0.14, 0.10);
const JUNIPER_TIP: LinearRgba = srgb(0.20, 0.28, 0.18);
const FERN: LinearRgba = srgb(0.30, 0.44, 0.12);
const FERN_ROOT: LinearRgba = srgb(0.12, 0.18, 0.06);
const STEM: LinearRgba = srgb(0.20, 0.30, 0.10);
const BLOOMS: [LinearRgba; 5] = [
  srgb(0.36, 0.46, 0.94),
  srgb(0.60, 0.34, 0.82),
  srgb(0.86, 0.16, 0.12),
  srgb(0.95, 0.94, 0.90),
  srgb(0.96, 0.80, 0.20)
];
const CAPS: [LinearRgba; 3] =
  [srgb(0.74, 0.14, 0.08), srgb(0.62, 0.48, 0.30), srgb(0.90, 0.87, 0.80)];

const PATCH: f32 = 32.0;
const NEAR_TREE: f32 = 170.0;
const FADE: f32 = 40.0;
const FOREVER: f32 = 1.0e6;
const SWARD_CELL: f32 = 16.0;
const SWARD_REACH: f32 = 80.0;
const TUFT_SPACING: f32 = 0.34;

fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
  let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

fn snow_line(at: Vec2) -> f32 { 150.0 + 40.0 * noise::fbm(at / 200.0, 3, 45) }

fn tundra(at: Vec2) -> f32 { smooth(-0.3, 0.4, noise::fbm(at / 120.0, 4, 41)) }

fn dim(tone: LinearRgba, by: f32) -> LinearRgba { tone.mix(&LinearRgba::BLACK, by) }

fn sheet(rows: &[Vec<Vec3>]) -> Mesh {
  let width = rows[0].len() as u32;
  let height = rows.len() as u32;
  let positions: Vec<Vec3> = rows.iter().flatten().copied().collect();
  let uvs: Vec<Vec2> = (0..height)
    .flat_map(|row| {
      (0..width).map(move |column| {
        Vec2::new(column as f32 / (width - 1) as f32, row as f32 / (height - 1) as f32)
      })
    })
    .collect();
  let indices = (0..height - 1)
    .flat_map(|row| {
      (0..width - 1).flat_map(move |column| {
        let first = row * width + column;
        let next = first + width;
        [first, first + 1, next, first + 1, next + 1, next]
      })
    })
    .collect();
  let mut mesh =
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
      .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
      .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
      .with_inserted_indices(Indices::U32(indices));
  mesh.compute_smooth_normals();
  mesh
}

fn tier(
  roll: &mut Roll,
  radius: f32,
  droop: f32,
  spikes: usize,
  rings: &[(f32, f32)]
) -> Mesh {
  let reach: Vec<f32> = (0..spikes).map(|_| roll.range(0.78, 1.15)).collect();
  let twist = roll.range(0.0, TAU);
  let rows: Vec<Vec<Vec3>> = rings
    .iter()
    .map(|&(out, fall)| {
      (0..=2 * spikes)
        .map(|step| {
          let spike = step % 2 == 0;
          let length = if spike {
            reach[(step / 2) % spikes]
          } else {
            0.62 * (reach[(step / 2) % spikes] + reach[(step / 2 + 1) % spikes]) / 2.0
          };
          let angle = twist + step as f32 / (2 * spikes) as f32 * TAU;
          let span = radius * out * 1.0f32.lerp(length, out);
          let sag = 1.0f32.lerp(if spike { 1.0 } else { 0.8 }, out);
          Vec3::new(angle.cos() * span, -droop * fall * sag, angle.sin() * span)
        })
        .collect()
    })
    .collect();
  sheet(&rows)
}

fn needle_tone(
  point: Vec3,
  normal: Vec3,
  radius: f32,
  snowy: bool,
  seed: u32
) -> LinearRgba {
  let outer = (point.xz().length() / radius).clamp(0.0, 1.0);
  let lit = smooth(-0.3, 0.7, normal.y);
  let fleck = noise::fbm3(point * 1.7 + Vec3::splat(seed as f32), 2, seed);
  let green = NEEDLE_DEEP
    .mix(&NEEDLE, outer.sqrt() * (0.55 + 0.45 * lit))
    .mix(&NEEDLE_TIP, (outer * outer * lit * (0.7 + 2.0 * fleck)).clamp(0.0, 1.0));
  let cover = snowy
    .then(|| smooth(0.3, 0.7, normal.y + fleck * 1.2) * smooth(0.15, 0.55, outer))
    .unwrap_or(0.0);
  green.mix(&SNOW, cover)
}

fn bark_tone(point: Vec3, base: LinearRgba) -> LinearRgba {
  let angle = point.z.atan2(point.x);
  let furrow = noise::perlin(Vec2::new(angle * 3.0, point.y * 0.6), 91);
  base.mix(&DARK_BARK, smooth(-0.2, 0.5, furrow) * 0.7)
}

fn wood_piece(mesh: Mesh, base: LinearRgba) -> Piece {
  Piece::new(mesh, Srgba::WHITE).shaded(move |point, _| bark_tone(point, base))
}

fn trunk_collider(radius: f32, height: f32) -> Collider {
  Collider::compound(vec![(
    Vec3::Y * height / 2.0,
    Quat::IDENTITY,
    Collider::cylinder(radius, height)
  )])
}

struct Tree {
  wood: Mesh,
  crown: Mesh,
  height: f32,
  girth: f32
}

fn pine(seed: u32, snowy: bool, coarse: bool) -> Tree {
  let mut roll = Roll::new(seed);
  let height = roll.range(13.0, 17.0);
  let base = height * roll.range(0.14, 0.32);
  let width = height * roll.range(0.15, 0.2);
  let girth = 0.018 * height + 0.1;
  let tiers = 9 + roll.below(4);
  let fine_spikes = 7 + roll.below(3);
  let (count, spikes, sag, sides): (usize, usize, f32, u32) =
    if coarse { (tiers / 2 + 1, 5, 1.3, 5) } else { (tiers, fine_spikes, 1.0, 9) };
  let rings: &[(f32, f32)] = if coarse {
    &[(0.04, -0.3), (1.0, 1.0), (0.04, 0.45)]
  } else {
    &[(0.04, -0.35), (0.45, 0.22), (1.0, 1.0), (0.5, 0.62), (0.04, 0.42)]
  };
  let crown = model::merge((0..=count).map(|index| {
    let t = index as f32 / count as f32;
    let y = base + (height - base - 0.6) * (1.0 - (1.0 - t).powf(1.35));
    let radius = width * (1.0 - t).powf(0.9) * roll.range(0.85, 1.12) + 0.3;
    let droop = (radius * 0.75 + 0.35) * sag;
    let skirt = tier(&mut roll, radius, droop, spikes, rings);
    let shift = Vec2::new(roll.spread(0.1), roll.spread(0.1)) * radius;
    Piece::new(skirt, Srgba::WHITE)
      .shaded(|point, normal| needle_tone(point, normal, radius, snowy, seed))
      .at(Vec3::new(shift.x, y, shift.y))
  }));
  let trunk = wood_piece(
    model::lathe(
      &[
        Vec2::new(girth * 1.45, -0.8),
        Vec2::new(girth * 1.15, 0.4),
        Vec2::new(girth, 1.6),
        Vec2::new(girth * 0.7, height * 0.45),
        Vec2::new(girth * 0.35, height * 0.8),
        Vec2::new(0.02, height)
      ],
      sides
    ),
    BARK
  );
  let stubs = (0..(!coarse).then_some(4 + roll.below(5)).unwrap_or(0)).map(|_| {
    let angle = roll.range(0.0, TAU);
    let start = Vec3::Y * roll.range(1.8, base.max(2.2));
    let reach =
      Vec3::new(angle.cos(), roll.range(-0.3, 0.25), angle.sin()) * roll.range(0.5, 1.4);
    wood_piece(model::tube(&[start, start + reach], &[0.05, 0.015], 4), DEAD_WOOD)
  });
  let wood = model::merge(std::iter::once(trunk).chain(stubs.collect::<Vec<_>>()));
  Tree { wood, crown, height, girth }
}

fn foliage(
  roll: &mut Roll,
  center: Vec3,
  size: Vec3,
  count: usize,
  card: f32,
  tone: impl Fn(f32, f32) -> LinearRgba
) -> Piece {
  let (positions, normals, colors) = (0..count).fold(
    (Vec::new(), Vec::new(), Vec::new()),
    |(mut positions, mut normals, mut colors): (Vec<Vec3>, Vec<Vec3>, Vec<[f32; 4]>),
     _| {
      let direction = Vec3::new(roll.spread(1.0), roll.spread(1.0), roll.spread(1.0))
        .normalize_or(Vec3::Y);
      let depth = roll.next().cbrt();
      let offset = direction * depth * size;
      let facing = (direction
        + Vec3::new(roll.spread(0.6), roll.spread(0.6) + 0.3, roll.spread(0.6)))
      .normalize_or(Vec3::Y);
      let turn = Quat::from_rotation_arc(Vec3::Z, facing)
        * Quat::from_rotation_z(roll.range(0.0, TAU));
      let shade = tone(depth, direction.y).to_f32_array();
      let normal = (direction + Vec3::Y * 0.5).normalize();
      positions.extend(
        [
          Vec2::new(-1.0, -1.0),
          Vec2::new(1.0, -1.0),
          Vec2::new(1.0, 1.0),
          Vec2::new(-1.0, 1.0)
        ]
        .map(|corner| center + offset + turn * (corner * card * 0.5).extend(0.0))
      );
      normals.extend([normal; 4]);
      colors.extend([shade; 4]);
      (positions, normals, colors)
    }
  );
  let uvs: Vec<[f32; 2]> =
    (0..count).flat_map(|_| [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).collect();
  let indices = (0..count as u32)
    .flat_map(|card| [0, 1, 2, 0, 2, 3].map(|corner| card * 4 + corner))
    .collect();
  Piece(
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
      .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
      .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
      .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
      .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
      .with_inserted_indices(Indices::U32(indices))
  )
}

fn birch(seed: u32) -> Tree {
  let mut roll = Roll::new(seed);
  let height = roll.range(7.5, 11.0);
  let girth = roll.range(0.14, 0.2);
  let lean = Vec3::new(roll.spread(0.8), 0.0, roll.spread(0.8));
  let path = model::curve(
    Vec3::Y * -0.5,
    Vec3::Y * height * 0.5 + lean * 0.3,
    Vec3::Y * height + lean,
    8
  );
  let trunk =
    Piece::new(model::tube(&path, &model::taper(8, girth, 0.03), 7), Srgba::WHITE)
      .shaded(|point, _| {
        let angle = point.z.atan2(point.x);
        let scar =
          smooth(0.25, 0.5, noise::perlin(Vec2::new(angle * 1.2, point.y * 3.5), seed));
        BIRCH_BARK.mix(&BIRCH_SCAR, scar.max(smooth(1.4, 0.1, point.y) * 0.8))
      });
  let tone = LEAVES[roll.below(LEAVES.len())];
  let branches: Vec<(Piece, Vec3)> = (0..5 + roll.below(3))
    .map(|_| {
      let along = roll.range(0.4, 0.88);
      let start = path[(along * 8.0) as usize];
      let angle = roll.range(0.0, TAU);
      let out =
        Vec3::new(angle.cos(), roll.range(0.5, 1.0), angle.sin()) * roll.range(1.3, 2.4);
      let end = start + out;
      (
        Piece::new(
          model::tube(
            &model::curve(start, start + out * 0.5 + Vec3::Y * 0.3, end, 3),
            &model::taper(3, 0.07, 0.02),
            5
          ),
          Srgba::WHITE
        )
        .shaded(|_, _| BIRCH_BARK.mix(&BIRCH_SCAR, 0.35)),
        end
      )
    })
    .collect();
  let crown = model::merge(
    branches
      .iter()
      .map(|(_, end)| *end)
      .chain([path[8], path[7] + Vec3::X * 0.6, path[6] - Vec3::Z * 0.7])
      .map(|center| {
        let hue = tone.mix(&LEAVES[roll.below(LEAVES.len())], roll.range(0.0, 0.5));
        let size = Vec3::new(1.4, 1.1, 1.4) * roll.range(0.9, 1.3);
        foliage(&mut roll, center, size, 70, 0.55, |depth, up| {
          dim(hue, 0.55 * (1.0 - depth) + 0.2 * smooth(0.3, -0.8, up))
        })
      })
      .collect::<Vec<_>>()
  );
  let wood = model::merge(
    std::iter::once(trunk).chain(branches.into_iter().map(|(piece, _)| piece))
  );
  Tree { wood, crown, height, girth }
}

fn snag(seed: u32) -> (Mesh, Collider) {
  let mut roll = Roll::new(seed);
  let height = roll.range(6.0, 11.0);
  let girth = roll.range(0.22, 0.34);
  let trunk = wood_piece(
    model::lathe(
      &[
        Vec2::new(girth * 1.4, -0.6),
        Vec2::new(girth * 1.1, 0.4),
        Vec2::new(girth, 2.0),
        Vec2::new(girth * 0.7, height * 0.7),
        Vec2::new(girth * 0.55, height - 0.3),
        Vec2::new(girth * 0.3, height),
        Vec2::new(0.0, height - 0.4)
      ],
      8
    ),
    DEAD_WOOD
  );
  let stubs: Vec<Piece> = (0..5 + roll.below(5))
    .map(|_| {
      let angle = roll.range(0.0, TAU);
      let start = Vec3::Y * roll.range(height * 0.3, height * 0.95);
      let reach =
        Vec3::new(angle.cos(), roll.range(-0.4, 0.5), angle.sin()) * roll.range(0.6, 2.2);
      wood_piece(model::tube(&[start, start + reach], &[0.09, 0.02], 5), DEAD_WOOD)
    })
    .collect();
  (model::merge(std::iter::once(trunk).chain(stubs)), trunk_collider(girth * 1.1, height))
}

fn stump(seed: u32) -> (Mesh, Collider) {
  let mut roll = Roll::new(seed);
  let girth = roll.range(0.3, 0.45);
  let height = roll.range(0.35, 0.9);
  let mesh = Piece::new(
    model::lathe(
      &[
        Vec2::new(girth * 1.6, -0.3),
        Vec2::new(girth * 1.25, 0.1),
        Vec2::new(girth * 1.05, 0.35),
        Vec2::new(girth, height),
        Vec2::new(girth * 0.9, height + 0.03),
        Vec2::new(0.0, height + 0.06)
      ],
      10
    ),
    Srgba::WHITE
  )
  .shaded(|point, normal| {
    let rings = (point.xz().length() * 30.0).sin() * 0.5 + 0.5;
    bark_tone(point, BARK).mix(&dim(HEARTWOOD, 0.25 * rings), smooth(0.6, 0.9, normal.y))
  })
  .0;
  (mesh, trunk_collider(girth * 1.1, height))
}

fn log(seed: u32) -> (Mesh, Collider) {
  let mut roll = Roll::new(seed);
  let girth = roll.range(0.26, 0.42);
  let length = roll.range(4.5, 8.0);
  let body = Piece::new(
    model::lathe(
      &[
        Vec2::new(0.0, 0.0),
        Vec2::new(girth * 0.95, 0.02),
        Vec2::new(girth, 0.3),
        Vec2::new(girth * 0.92, length * 0.5),
        Vec2::new(girth * 0.8, length - 0.3),
        Vec2::new(girth * 0.75, length),
        Vec2::new(0.0, length + 0.02)
      ],
      10
    ),
    Srgba::WHITE
  )
  .at(Vec3::Y * -length / 2.0)
  .rolled(FRAC_PI_2)
  .at(Vec3::Y * girth * 0.7);
  let stubs: Vec<Piece> = (0..3 + roll.below(3))
    .map(|_| {
      let angle = roll.range(0.0, PI);
      let start = Vec3::new(roll.spread(length * 0.4), girth * 0.7, 0.0);
      let reach =
        Vec3::new(roll.spread(0.4), angle.sin(), angle.cos()) * roll.range(0.4, 1.1);
      Piece::new(model::tube(&[start, start + reach], &[0.07, 0.02], 5), Srgba::WHITE)
    })
    .collect();
  let mesh = Piece(model::merge(std::iter::once(body).chain(stubs)))
    .shaded(|point, normal| {
      let mossy = smooth(0.35, 0.8, normal.y + 0.4 * noise::fbm3(point * 1.5, 2, seed));
      bark_tone(point * 3.0, DEAD_WOOD.mix(&BARK, 0.5)).mix(&MOSS, mossy * 0.85)
    })
    .0;
  let collider = Collider::compound(vec![(
    Vec3::Y * girth * 0.7,
    Quat::from_rotation_z(FRAC_PI_2),
    Collider::capsule(girth * 0.9, (length - 2.0 * girth).max(0.1))
  )]);
  (mesh, collider)
}

fn stone_tone(point: Vec3, normal: Vec3, snowy: bool, seed: u32) -> LinearRgba {
  let grain = noise::fbm3(point * 1.3, 3, seed);
  let top = smooth(0.4, 0.85, normal.y + 0.5 * noise::fbm3(point * 2.3, 2, seed + 5));
  let rock = STONE.mix(&DARK_STONE, smooth(-0.2, 0.25, grain));
  rock.mix(&if snowy { SNOW } else { MOSS }, top * if snowy { 0.95 } else { 0.8 })
}

fn boulder(seed: u32, snowy: bool, detail: u32) -> (Mesh, Collider) {
  let lump = model::lump(seed, 0.2, detail);
  let collider = lump
    .attribute(Mesh::ATTRIBUTE_POSITION)
    .and_then(|values| values.as_float3())
    .and_then(|points| {
      Collider::convex_hull(points.iter().copied().map(Vec3::from).collect())
    })
    .unwrap_or_else(|| Collider::sphere(1.0));
  let mesh = Piece::new(lump, Srgba::WHITE)
    .shaded(|point, normal| stone_tone(point, normal, snowy, seed))
    .0;
  (mesh, collider)
}

fn crag(seed: u32, snowy: bool) -> (Mesh, Collider) {
  let lump = model::lump(seed, 0.42, 2);
  let collider = lump
    .attribute(Mesh::ATTRIBUTE_POSITION)
    .and_then(|values| values.as_float3())
    .and_then(|points| {
      Collider::convex_hull(points.iter().copied().map(Vec3::from).collect())
    })
    .unwrap_or_else(|| Collider::sphere(1.0));
  let mesh = Piece::new(lump, Srgba::WHITE)
    .shaded(|point, normal| {
      let strata =
        (point.y * 7.0 + 2.0 * noise::fbm3(point * 1.1, 2, seed)).sin() * 0.5 + 0.5;
      let rock = CRAG
        .mix(&DARK_STONE, strata * 0.6)
        .mix(&LICHEN, smooth(0.35, 0.6, noise::fbm3(point * 3.0, 2, seed + 3)) * 0.4);
      let top = smooth(0.55, 0.9, normal.y + 0.4 * noise::fbm3(point * 2.3, 2, seed + 5));
      rock.mix(&if snowy { SNOW } else { MOSS }, top * if snowy { 0.95 } else { 0.5 })
    })
    .0;
  (mesh, collider)
}

fn juniper(seed: u32) -> Mesh {
  let mut roll = Roll::new(seed);
  let clumps: Vec<Piece> = (0..3 + roll.below(3))
    .map(|_| {
      let center = Vec3::new(roll.spread(0.6), roll.range(0.35, 0.6), roll.spread(0.6));
      let size = Vec3::new(0.7, 0.45, 0.7) * roll.range(0.7, 1.1);
      foliage(&mut roll, center, size, 38, 0.42, |depth, up| {
        JUNIPER.mix(&JUNIPER_TIP, depth * depth * smooth(-0.4, 0.8, up))
      })
    })
    .collect();
  model::merge(clumps)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Assoc)]
#[func(const fn variants(self) -> usize)]
enum Growth {
  #[assoc(variants = 6)]
  Pine,
  #[assoc(variants = 6)]
  SnowyPine,
  #[assoc(variants = 4)]
  Birch,
  #[assoc(variants = 3)]
  Snag,
  #[assoc(variants = 3)]
  Stump,
  #[assoc(variants = 3)]
  Log,
  #[assoc(variants = 8)]
  Boulder,
  #[assoc(variants = 8)]
  SnowyBoulder,
  #[assoc(variants = 6)]
  Crag,
  #[assoc(variants = 6)]
  SnowyCrag,
  #[assoc(variants = 4)]
  Juniper
}

impl Growth {
  const ALL: [Growth; 11] = [
    Growth::Pine,
    Growth::SnowyPine,
    Growth::Birch,
    Growth::Snag,
    Growth::Stump,
    Growth::Log,
    Growth::Boulder,
    Growth::SnowyBoulder,
    Growth::Crag,
    Growth::SnowyCrag,
    Growth::Juniper
  ];
}

struct Shape {
  parts: Vec<(Stuff, Mesh)>,
  far: Option<Mesh>,
  collider: Option<Collider>,
  reach: f32
}

fn shape(growth: Growth, variant: usize) -> Shape {
  let seed = 1000 * growth as u32 + variant as u32 + 7;
  match growth {
    Growth::Pine | Growth::SnowyPine => {
      let snowy = growth == Growth::SnowyPine;
      let Tree { wood, crown, height, girth } = pine(variant as u32 + 100, snowy, false);
      let far = pine(variant as u32 + 100, snowy, true);
      Shape {
        parts: vec![(Stuff::Bark, wood), (Stuff::Needles, crown)],
        far: Some(model::merge([Piece(far.wood), Piece(far.crown)])),
        collider: Some(trunk_collider(girth * 1.05, height * 0.9)),
        reach: NEAR_TREE
      }
    }
    Growth::Birch => {
      let Tree { wood, crown, height, girth } = birch(seed);
      Shape {
        parts: vec![(Stuff::Bark, wood), (Stuff::Needles, crown)],
        far: None,
        collider: Some(trunk_collider(girth, height * 0.8)),
        reach: FOREVER
      }
    }
    Growth::Snag => {
      let (mesh, collider) = snag(seed);
      Shape {
        parts: vec![(Stuff::Bark, mesh)],
        far: None,
        collider: Some(collider),
        reach: 600.0
      }
    }
    Growth::Stump => {
      let (mesh, collider) = stump(seed);
      Shape {
        parts: vec![(Stuff::Bark, mesh)],
        far: None,
        collider: Some(collider),
        reach: 220.0
      }
    }
    Growth::Log => {
      let (mesh, collider) = log(seed);
      Shape {
        parts: vec![(Stuff::Bark, mesh)],
        far: None,
        collider: Some(collider),
        reach: 260.0
      }
    }
    Growth::Boulder | Growth::SnowyBoulder => {
      let (mesh, collider) =
        boulder(variant as u32 + 300, growth == Growth::SnowyBoulder, 3);
      Shape {
        parts: vec![(Stuff::Stone, mesh)],
        far: None,
        collider: Some(collider),
        reach: FOREVER
      }
    }
    Growth::Crag | Growth::SnowyCrag => {
      let (mesh, collider) = crag(variant as u32 + 500, growth == Growth::SnowyCrag);
      Shape {
        parts: vec![(Stuff::Stone, mesh)],
        far: None,
        collider: Some(collider),
        reach: FOREVER
      }
    }
    Growth::Juniper => Shape {
      parts: vec![(Stuff::Needles, juniper(seed))],
      far: None,
      collider: None,
      reach: 200.0
    }
  }
}

struct Form {
  parts: Vec<(Handle<StandardMaterial>, Handle<Mesh>)>,
  far: Option<Handle<Mesh>>,
  collider: Option<Collider>,
  reach: f32
}

struct Plant {
  growth: Growth,
  variant: usize,
  place: Transform
}

struct Site {
  spot: Vec3,
  normal: Vec3,
  forest: f32,
  road: f32,
  snow: f32,
  treeline: f32,
  open: bool
}

fn survey(ground: &Ground, at: Vec2) -> Site {
  let height = ground.height(at);
  let line = snow_line(at);
  Site {
    spot: at.extend(height).xzy(),
    normal: ground.normal(at),
    forest: terrain::forest(at),
    road: place::road_distance(at),
    snow: smooth(line - 45.0, line, height),
    treeline: smooth(line + 130.0, line + 60.0, height),
    open: at.abs().max_element() < BOUND
      && height > LAKE_LEVEL + 0.8
      && at.distance(START) > 6.0
      && Place::ALL.into_iter().all(|place| {
        at.distance(place.spot()) > place.flat() * 1.4
          && at.distance(place.spot() + Vec2::Y * (place.flat() * 1.6 + 6.0)) > 7.0
      })
  }
}

fn upright(spot: Vec3, yaw: f32, lean: Vec2, scale: Vec3) -> Transform {
  Transform::from_translation(spot)
    .with_rotation(
      Quat::from_rotation_y(yaw) * Quat::from_euler(EulerRot::XYZ, lean.x, 0.0, lean.y)
    )
    .with_scale(scale)
}

fn rock(roll: &mut Roll, site: &Site, size: f32) -> Plant {
  let scale =
    Vec3::new(roll.range(0.8, 1.3), roll.range(0.55, 0.9), roll.range(0.8, 1.3)) * size;
  let snowy = roll.next() < site.snow;
  Plant {
    growth: if snowy { Growth::SnowyBoulder } else { Growth::Boulder },
    variant: roll.below(Growth::Boulder.variants()),
    place: upright(
      site.spot - Vec3::Y * scale.y * roll.range(0.2, 0.45),
      roll.range(0.0, TAU),
      Vec2::new(roll.spread(0.3), roll.spread(0.3)),
      scale
    )
  }
}

fn scatter(ground: &Ground, patch: IVec2) -> Vec<Plant> {
  let mut roll = Roll::new(
    (patch.x as u32).wrapping_mul(73_856_093) ^ (patch.y as u32).wrapping_mul(19_349_663)
  );
  let origin = patch.as_vec2() * PATCH - BOUND;
  let anywhere = |roll: &mut Roll| origin + Vec2::new(roll.next(), roll.next()) * PATCH;
  let trees: Vec<Plant> = (0..36)
    .filter_map(|slot| {
      let at = origin
        + (Vec2::new((slot % 6) as f32, (slot / 6) as f32)
          + Vec2::new(roll.next(), roll.next()))
          * PATCH
          / 6.0;
      let site = survey(ground, at);
      let chance = (site.forest * 0.9 + 0.03) * site.treeline;
      let birchy = smooth(0.2, 0.45, noise::fbm(at / 240.0, 3, 61))
        * smooth(110.0, 70.0, site.spot.y);
      let (pick, kind, snowy) = (roll.next(), roll.next(), roll.next());
      let (yaw, lean) =
        (roll.range(0.0, TAU), Vec2::new(roll.spread(0.04), roll.spread(0.04)));
      let size = roll.range(0.7, 1.35) * (0.85 + 0.25 * site.forest);
      (site.open && site.road > 4.5 && site.normal.y > 0.78 && pick < chance).then(|| {
        let growth = if kind < 0.04 {
          Growth::Snag
        } else if kind < 0.04 + birchy * 0.85 {
          Growth::Birch
        } else if snowy < site.snow {
          Growth::SnowyPine
        } else {
          Growth::Pine
        };
        Plant {
          growth,
          variant: ((kind * 7919.0) as usize) % growth.variants(),
          place: upright(site.spot, yaw, lean, Vec3::splat(size))
        }
      })
    })
    .collect();
  let deadwood: Vec<Plant> = (0..3)
    .filter_map(|_| {
      let site = survey(ground, anywhere(&mut roll));
      let (pick, kind, variant, yaw) =
        (roll.next(), roll.next(), roll.below(3), roll.range(0.0, TAU));
      (site.open && site.road > 4.0 && site.normal.y > 0.82 && pick < site.forest * 0.35)
        .then(|| {
          if kind < 0.55 {
            Plant {
              growth: Growth::Stump,
              variant,
              place: upright(site.spot, yaw, Vec2::ZERO, Vec3::ONE)
            }
          } else {
            Plant {
              growth: Growth::Log,
              variant,
              place: Transform::from_translation(site.spot).with_rotation(
                Quat::from_rotation_arc(Vec3::Y, site.normal)
                  * Quat::from_rotation_y(yaw)
              )
            }
          }
        })
    })
    .collect();
  let boulders: Vec<Plant> = (0..3)
    .flat_map(|_| {
      let at = anywhere(&mut roll);
      let site = survey(ground, at);
      let rocky = smooth(0.1, 0.5, noise::fbm(at / 180.0, 3, 67));
      let chance = (0.05 + 0.35 * rocky + 0.1 * smooth(0.9, 0.7, site.normal.y))
        * (1.0 - 0.7 * site.snow);
      let size =
        if roll.chance(0.07) { roll.range(4.5, 8.0) } else { roll.range(1.0, 3.2) };
      let crowd = 2 + roll.below(5);
      (site.open && site.road > 3.0 && roll.next() < chance)
        .then(|| {
          let main = rock(&mut roll, &site, size);
          let satellites: Vec<Plant> = (0..crowd)
            .filter_map(|_| {
              let angle = roll.range(0.0, TAU);
              let near = at + Vec2::from_angle(angle) * size * roll.range(0.8, 1.8);
              let site = survey(ground, near);
              let small = size * roll.range(0.15, 0.5);
              (site.open && site.road > 3.0).then(|| rock(&mut roll, &site, small))
            })
            .collect();
          std::iter::once(main).chain(satellites).collect::<Vec<_>>()
        })
        .unwrap_or_default()
    })
    .collect();
  let outcrops: Vec<Plant> = (0..2)
    .flat_map(|_| {
      let at = anywhere(&mut roll);
      let site = survey(ground, at);
      let steep = smooth(0.84, 0.62, site.normal.y);
      let size = roll.range(3.0, 8.0);
      let (pick, crowd) = (roll.next(), 1 + roll.below(3));
      (site.spot.x.abs() < HALF - 20.0
        && site.spot.z.abs() < HALF - 20.0
        && site.road > 6.0
        && pick < 0.4 * steep)
        .then(|| {
          (0..crowd)
            .map(|_| {
              let site =
                survey(ground, at + Vec2::new(roll.spread(size), roll.spread(size)));
              let big = size * roll.range(0.5, 1.0);
              let scale =
                Vec3::new(big * roll.range(1.2, 1.8), big * roll.range(0.45, 0.7), big);
              let snowy = roll.next() < site.snow;
              Plant {
                growth: if snowy { Growth::SnowyCrag } else { Growth::Crag },
                variant: roll.below(6),
                place: Transform::from_translation(
                  site.spot - site.normal * scale.y * 0.5
                )
                .with_rotation(
                  Quat::from_rotation_arc(
                    Vec3::Y,
                    site.normal.lerp(Vec3::Y, 0.25).normalize()
                  ) * Quat::from_rotation_y(roll.range(0.0, TAU))
                )
                .with_scale(scale)
              }
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    })
    .collect();
  let shrubs: Vec<Plant> = (0..8)
    .filter_map(|_| {
      let at = anywhere(&mut roll);
      let site = survey(ground, at);
      let chance = 0.03 + site.forest * (1.0 - site.forest) * 1.2 + 0.06 * tundra(at);
      let (pick, variant, yaw, size) =
        (roll.next(), roll.below(4), roll.range(0.0, TAU), roll.range(0.6, 1.4));
      (site.open
        && site.road > 3.0
        && site.normal.y > 0.7
        && site.snow < 0.8
        && pick < chance)
        .then(|| Plant {
          growth: Growth::Juniper,
          variant,
          place: upright(
            site.spot - Vec3::Y * 0.1,
            yaw,
            Vec2::ZERO,
            Vec3::new(size, size * roll.range(0.7, 1.2), size)
          )
        })
    })
    .collect();
  [trees, deadwood, boulders, outcrops, shrubs].into_iter().flatten().collect()
}

fn foreground(ground: &Ground) -> Vec<Plant> {
  let ahead = START_FACING.normalize();
  let right = Vec2::new(-ahead.y, ahead.x);
  let spot =
    |forward: f32, side: f32| ground.surface(START + ahead * forward + right * side);
  [
    (Growth::Pine, 2, 16.0, -10.0, Vec3::splat(1.2)),
    (Growth::Pine, 4, 27.0, 12.0, Vec3::splat(1.35)),
    (Growth::Pine, 0, -7.0, -9.0, Vec3::splat(1.1)),
    (Growth::Pine, 5, 3.0, 12.5, Vec3::splat(0.95)),
    (Growth::Pine, 1, 36.0, -17.0, Vec3::splat(1.4)),
    (Growth::Pine, 3, 44.0, 19.0, Vec3::splat(1.25)),
    (Growth::Snag, 1, 21.0, -15.0, Vec3::ONE),
    (Growth::Boulder, 3, 9.0, -6.5, Vec3::new(2.3, 1.3, 1.9)),
    (Growth::Boulder, 5, 10.8, -4.9, Vec3::new(0.8, 0.5, 0.7)),
    (Growth::Boulder, 1, 7.6, -8.4, Vec3::new(1.0, 0.7, 0.9)),
    (Growth::Boulder, 6, 5.0, 7.5, Vec3::new(1.3, 0.8, 1.1)),
    (Growth::Boulder, 2, 18.0, 8.0, Vec3::new(3.0, 1.8, 2.6)),
    (Growth::Juniper, 0, 6.0, 6.2, Vec3::splat(1.1)),
    (Growth::Juniper, 2, 12.5, -7.5, Vec3::splat(1.3)),
    (Growth::Juniper, 1, 19.0, 10.5, Vec3::splat(1.0)),
    (Growth::Stump, 0, 2.0, -7.0, Vec3::ONE)
  ]
  .into_iter()
  .enumerate()
  .map(|(index, (growth, variant, forward, side, scale))| {
    let sink = if growth == Growth::Boulder { scale.y * 0.3 } else { 0.0 };
    Plant {
      growth,
      variant,
      place: upright(
        spot(forward, side) - Vec3::Y * sink,
        index as f32 * 2.4,
        Vec2::ZERO,
        scale
      )
    }
  })
  .collect()
}

fn sow(ground: &Ground) -> Vec<Plant> {
  let patches = (2.0 * BOUND / PATCH).ceil() as i32;
  let bands = std::thread::available_parallelism().map_or(8, usize::from) as i32;
  let rows = (patches + bands - 1) / bands;
  std::thread::scope(|scope| {
    (0..bands)
      .map(|band| {
        scope.spawn(move || {
          (band * rows..((band + 1) * rows).min(patches))
            .flat_map(|row| {
              (0..patches)
                .flat_map(move |column| scatter(ground, IVec2::new(column, row)))
            })
            .collect::<Vec<_>>()
        })
      })
      .collect::<Vec<_>>()
      .into_iter()
      .flat_map(|handle| handle.join().expect("flora band"))
      .chain(foreground(ground))
      .collect()
  })
}

fn shapes() -> Vec<((Growth, usize), Shape)> {
  std::thread::scope(|scope| {
    Growth::ALL
      .into_iter()
      .flat_map(|growth| (0..growth.variants()).map(move |variant| (growth, variant)))
      .map(|key| (key, scope.spawn(move || shape(key.0, key.1))))
      .collect::<Vec<_>>()
      .into_iter()
      .map(|(key, handle)| (key, handle.join().expect("flora shape")))
      .collect()
  })
}

fn spawn_flora(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  ground: Res<Ground>
) {
  let (made, plants) = std::thread::scope(|scope| {
    let made = scope.spawn(shapes);
    let plants = sow(&ground);
    (made.join().expect("flora shapes"), plants)
  });
  let forms: HashMap<(Growth, usize), Form> = made
    .into_iter()
    .map(|(key, Shape { parts, far, collider, reach })| {
      (key, Form {
        parts: parts
          .into_iter()
          .map(|(stuff, mesh)| (stuffs.of(stuff), meshes.add(mesh)))
          .collect(),
        far: far.map(|mesh| meshes.add(mesh)),
        collider,
        reach
      })
    })
    .collect();
  let needles = stuffs.of(Stuff::Needles);
  plants.iter().for_each(|&Plant { growth, variant, place }| {
    let form = &forms[&(growth, variant)];
    let near = VisibilityRange {
      start_margin: 0.0..0.0,
      end_margin: form.reach..form.reach + FADE,
      use_aabb: false
    };
    let (material, mesh) = &form.parts[0];
    let mut root = commands.spawn((
      Mesh3d(mesh.clone()),
      MeshMaterial3d(material.clone()),
      place,
      near.clone()
    ));
    form.parts[1..].iter().for_each(|(material, mesh)| {
      root.with_child((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        near.clone()
      ));
    });
    if let Some(collider) = &form.collider
      && place.scale.max_element() > 0.5
    {
      root.insert((RigidBody::Static, collider.clone()));
    }
    if let Some(far) = &form.far {
      commands.spawn((
        Mesh3d(far.clone()),
        MeshMaterial3d(needles.clone()),
        place,
        VisibilityRange {
          start_margin: form.reach..form.reach + FADE,
          end_margin: FOREVER..FOREVER,
          use_aabb: false
        }
      ));
    }
  });
}

#[derive(Default)]
struct Tufts {
  positions: Vec<Vec3>,
  normals: Vec<Vec3>,
  uvs: Vec<Vec2>,
  colors: Vec<[f32; 4]>,
  indices: Vec<u32>
}

impl Tufts {
  fn strand(
    &mut self,
    root: Vec3,
    spine: &[(Vec3, f32, LinearRgba)],
    side: Vec3,
    normal: Vec3,
    tall: f32
  ) {
    let first = self.positions.len() as u32;
    spine.iter().for_each(|&(point, half, tone)| {
      let rise = ((point - root).length() / tall).clamp(0.0, 1.0);
      self.positions.extend([point - side * half, point + side * half]);
      self.normals.extend([normal; 2]);
      self.uvs.extend([Vec2::new(tall, rise); 2]);
      self.colors.extend([tone.to_f32_array(); 2]);
    });
    self.indices.extend((0..spine.len() as u32 - 1).flat_map(|step| {
      let at = first + step * 2;
      [at, at + 1, at + 2, at + 1, at + 3, at + 2]
    }));
  }

  fn tuft(
    &mut self,
    roll: &mut Roll,
    root: Vec3,
    up: Vec3,
    tall: f32,
    base: LinearRgba,
    tip: LinearRgba
  ) {
    (0..4 + roll.below(5)).for_each(|_| {
      let angle = roll.range(0.0, TAU);
      let out = Vec3::new(angle.cos(), 0.0, angle.sin());
      let side = Vec3::new(-out.z, 0.0, out.x);
      let height = tall * roll.range(0.55, 1.1);
      let lean = roll.range(0.1, 0.5) * height;
      let start = root + out * roll.range(0.0, 0.09);
      let width = roll.range(0.012, 0.022);
      let tint = tip.mix(&base, roll.range(0.0, 0.35));
      self.strand(
        start,
        &[
          (start, width, base),
          (
            start + up * height * 0.5 + out * lean * 0.25,
            width * 0.75,
            base.mix(&tint, 0.6)
          ),
          (start + up * height + out * lean, 0.0, tint)
        ],
        side,
        (up + out * 0.35).normalize(),
        height
      );
    });
  }

  fn fern(&mut self, roll: &mut Roll, root: Vec3, size: f32) {
    let fronds = 6 + roll.below(4);
    (0..fronds).for_each(|index| {
      let angle = index as f32 / fronds as f32 * TAU + roll.spread(0.3);
      let out = Vec3::new(angle.cos(), 0.0, angle.sin());
      let length = size * roll.range(0.7, 1.1);
      let spine: Vec<(Vec3, f32, LinearRgba)> = (0..=8)
        .map(|step| {
          let t = step as f32 / 8.0;
          let point =
            root + out * length * t * 0.8 + Vec3::Y * length * (1.1 * t - 0.8 * t * t);
          let serration = if step % 2 == 0 { 1.0 } else { 0.4 };
          (
            point,
            length * 0.13 * ((t * PI).sin() + 0.1) * serration,
            FERN_ROOT.mix(&FERN, t.sqrt())
          )
        })
        .collect();
      self.strand(
        root,
        &spine,
        Vec3::new(-out.z, 0.0, out.x),
        (Vec3::Y + out * 0.3).normalize(),
        length
      );
    });
  }

  fn bloom(&mut self, roll: &mut Roll, root: Vec3, tall: f32, petal: LinearRgba) {
    (0..3 + roll.below(4)).for_each(|_| {
      let angle = roll.range(0.0, TAU);
      let out = Vec3::new(angle.cos(), 0.0, angle.sin());
      let side = Vec3::new(-out.z, 0.0, out.x);
      let start = root + out * roll.range(0.0, 0.25);
      let height = tall * roll.range(0.6, 1.0);
      let top = start + Vec3::Y * height + out * height * 0.15;
      let size = roll.range(0.012, 0.024);
      let tone = petal.mix(&LinearRgba::WHITE, roll.range(0.0, 0.2));
      self.strand(
        start,
        &[(start, 0.006, STEM), (top, 0.004, STEM)],
        side,
        Vec3::Y,
        height
      );
      self.strand(
        start,
        &[(top - out * size, size, tone), (top + out * size, size, tone)],
        side,
        Vec3::Y,
        height
      );
      self.strand(
        start,
        &[
          (top - Vec3::Y * size, size, dim(tone, 0.2)),
          (top + Vec3::Y * size, size, tone)
        ],
        side,
        Vec3::Y,
        height
      );
    });
  }

  fn mesh(self) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
      .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
      .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
      .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
      .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
      .with_inserted_indices(Indices::U32(self.indices))
  }
}

fn still(Piece(mut mesh): Piece) -> Mesh {
  let count = mesh.count_vertices();
  mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; count]);
  mesh
}

fn mushrooms(roll: &mut Roll, root: Vec3) -> Vec<Mesh> {
  let cap = CAPS[roll.below(CAPS.len())];
  (0..1 + roll.below(4))
    .flat_map(|_| {
      let spot = root + Vec3::new(roll.spread(0.2), 0.0, roll.spread(0.2));
      let height = roll.range(0.04, 0.12);
      let width = roll.range(0.03, 0.07);
      [
        still(
          Piece::new(model::rod(width * 0.25, height), Srgba::from(CAPS[2]))
            .at(spot + Vec3::Y * height / 2.0)
        ),
        still(
          Piece::new(model::ball(width), Srgba::from(cap))
            .sized(Vec3::new(1.0, 0.55, 1.0))
            .at(spot + Vec3::Y * height)
        )
      ]
    })
    .collect()
}

fn sward(ground: &Ground, cell: IVec2) -> Option<(Vec3, Mesh)> {
  let origin = cell.as_vec2() * SWARD_CELL;
  let center = ground.surface(origin + SWARD_CELL / 2.0);
  let mut roll = Roll::new(
    (cell.x as u32).wrapping_mul(2_654_435_761) ^ (cell.y as u32).wrapping_mul(40_503)
  );
  let across = (SWARD_CELL / TUFT_SPACING) as i32;
  let mut tufts = Tufts::default();
  let extras: Vec<Mesh> = (0..across * across)
    .flat_map(|slot| {
      let at = origin
        + (Vec2::new((slot % across) as f32, (slot / across) as f32)
          + Vec2::new(roll.next(), roll.next()))
          * TUFT_SPACING;
      let height = ground.height(at);
      let normal = ground.normal(at);
      let forest = terrain::forest(at);
      let road = place::road_distance(at);
      let dry = tundra(at);
      let line = snow_line(at);
      let clump = smooth(-0.35, 0.25, noise::fbm(at / 7.0, 2, 83));
      let lush = (1.0 - forest * 0.7)
        * smooth(1.9, 3.4, road + noise::fbm(at / 3.0, 2, 85))
        * smooth(0.7, 0.84, normal.y)
        * smooth(line + 5.0, line - 30.0, height)
        * smooth(LAKE_LEVEL + 0.5, LAKE_LEVEL + 1.8, height)
        * (0.25 + 0.75 * clump)
        * f32::from(u8::from(at.abs().max_element() < HALF - 2.0))
        * f32::from(u8::from(Place::ALL.into_iter().all(|place| {
          place.sunk() <= 0.0 || at.distance(place.spot()) > place.flat() * 0.95
        })));
      let root = at.extend(height).xzy() - center;
      let (pick, flower, fungus, pebble) =
        (roll.next(), roll.next(), roll.next(), roll.next());
      let blooming = smooth(0.25, 0.6, noise::fbm(at / 28.0, 2, 87)) * (1.0 - forest);
      let ground_tone = MEADOW.mix(&TUNDRA, dry).mix(&FOREST_FLOOR, forest * 0.8);
      if forest > 0.3 && pick < forest * 0.05 && lush > 0.05 {
        let size = roll.range(0.45, 0.85);
        tufts.fern(&mut roll, root, size);
      } else if pick < lush && flower < blooming * 0.18 {
        let petal = BLOOMS[(noise::hash((at.x / 9.0) as i32, (at.y / 9.0) as i32, 89)
          * 5.0) as usize
          % 5];
        let tall = roll.range(0.2, 0.4);
        tufts.bloom(&mut roll, root, tall, petal);
      } else if pick < lush {
        let tall =
          (0.2 + 0.36 * clump * (1.0 - 0.4 * dry) - 0.08 * forest) * roll.range(0.8, 1.2);
        let tip = dim(
          MEADOW_TIP.mix(&TUNDRA_TIP, dry).mix(&FOREST_FLOOR, forest * 0.5),
          roll.range(0.0, 0.25)
        );
        tufts.tuft(
          &mut roll,
          root,
          normal.lerp(Vec3::Y, 0.6).normalize(),
          tall,
          dim(ground_tone, 0.45),
          tip
        );
      }
      let pebbly = 0.002 + 0.02 * smooth(2.0, 3.0, road) * smooth(5.5, 3.5, road);
      [
        (forest > 0.45 && fungus < 0.004).then(|| mushrooms(&mut roll, root)),
        (lush > 0.0 && pebble < pebbly).then(|| {
          let size = roll.range(0.06, 0.25);
          vec![still(
            Piece::new(model::lump(roll.below(50) as u32, 0.25, 0), Srgba::WHITE)
              .shaded(|point, normal| {
                stone_tone(point, normal, false, 3).mix(&DARK_STONE, 0.4)
              })
              .sized(Vec3::new(size, size * 0.6, size))
              .at(root - Vec3::Y * size * 0.2)
          )]
        })
      ]
      .into_iter()
      .flatten()
      .flatten()
      .collect::<Vec<_>>()
    })
    .collect();
  (!tufts.positions.is_empty()).then(|| {
    let mesh = extras.into_iter().fold(tufts.mesh(), |mut all, piece| {
      all.merge(&piece).expect("sward pieces share attributes");
      all
    });
    (center, mesh)
  })
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
struct Sway {}

impl MaterialExtension for Sway {
  fn vertex_shader() -> ShaderRef { "shaders/sway.wgsl".into() }
}

type SwayMaterial = ExtendedMaterial<StandardMaterial, Sway>;

#[derive(Resource)]
struct Sward(Handle<SwayMaterial>);

#[derive(Resource, Default)]
struct Meadow {
  grown: HashMap<IVec2, Option<Entity>>,
  growing: HashMap<IVec2, Task<Option<(Vec3, Mesh)>>>
}

fn prepare_sward(mut commands: Commands, mut materials: ResMut<Assets<SwayMaterial>>) {
  commands.insert_resource(Sward(materials.add(ExtendedMaterial {
    base: StandardMaterial {
      perceptual_roughness: 0.85,
      reflectance: 0.2,
      cull_mode: None,
      ..default()
    },
    extension: Sway {}
  })));
}

fn tend_meadow(
  mut commands: Commands,
  mut meadow: ResMut<Meadow>,
  mut meshes: ResMut<Assets<Mesh>>,
  sward_material: Res<Sward>,
  ground: Res<Ground>,
  players: Query<&Transform, With<Player>>
) {
  if let Ok(player) = players.single() {
    let here = player.translation.xz();
    let distance = |cell: IVec2| ((cell.as_vec2() + 0.5) * SWARD_CELL).distance(here);
    let Meadow { grown, growing } = &mut *meadow;
    let span = (SWARD_REACH / SWARD_CELL).ceil() as i32 + 1;
    let middle = (here / SWARD_CELL).floor().as_ivec2();
    let wanted: Vec<IVec2> = (-span..=span)
      .flat_map(|dz| (-span..=span).map(move |dx| middle + IVec2::new(dx, dz)))
      .filter(|&cell| {
        distance(cell) < SWARD_REACH
          && !grown.contains_key(&cell)
          && !growing.contains_key(&cell)
      })
      .collect();
    wanted.into_iter().for_each(|cell| {
      let ground = ground.clone();
      growing.insert(
        cell,
        AsyncComputeTaskPool::get().spawn(async move { sward(&ground, cell) })
      );
    });
    growing.retain(|&cell, task| {
      check_ready(task)
        .map(|made| {
          let entity = made.map(|(center, mesh)| {
            commands
              .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(sward_material.0.clone()),
                Transform::from_translation(center),
                NotShadowCaster
              ))
              .id()
          });
          grown.insert(cell, entity);
        })
        .is_none()
    });
    grown.retain(|&cell, entity| {
      let keep = distance(cell) < SWARD_REACH + SWARD_CELL;
      if !keep && let Some(entity) = *entity {
        commands.entity(entity).despawn();
      }
      keep
    });
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_plugins(MaterialPlugin::<SwayMaterial>::default())
    .init_resource::<Meadow>()
    .add_systems(Startup, (spawn_flora, prepare_sward))
    .add_systems(Update, tend_meadow);
}
