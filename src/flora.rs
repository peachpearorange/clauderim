use {crate::{model::{self, Piece},
             noise::{self, Roll},
             place::{self, LAKE_LEVEL, Place, START, START_FACING},
             player::{MainCamera, Player},
             sky,
             stuff::{Stuff, Stuffs},
             terrain::{self, BOUND, Ground, srgb}},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            camera::{primitives::{Aabb, MeshAabb},
                     visibility::VisibilityRange},
            image::{ImageSampler, ImageSamplerDescriptor},
            light::NotShadowCaster,
            mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
            pbr::{ExtendedMaterial, MaterialExtension},
            platform::collections::HashMap,
            prelude::*,
            render::render_resource::{AsBindGroup, Extent3d, TextureDimension,
                                      TextureFormat},
            shader::ShaderRef,
            tasks::{AsyncComputeTaskPool, Task, futures::check_ready}},
     enum_assoc::Assoc,
     std::{f32::consts::{FRAC_PI_2, PI, TAU},
           sync::Arc}};

const MEADOW: LinearRgba = srgb(0.33, 0.36, 0.23);
const TUNDRA: LinearRgba = srgb(0.47, 0.42, 0.30);
const FOREST_FLOOR: LinearRgba = srgb(0.25, 0.24, 0.18);
const MEADOW_TIP: LinearRgba = srgb(0.50, 0.52, 0.33);
const TUNDRA_TIP: LinearRgba = srgb(0.60, 0.54, 0.38);
const NEEDLE_DEEP: LinearRgba = srgb(0.05, 0.10, 0.09);
const NEEDLE: LinearRgba = srgb(0.12, 0.20, 0.17);
const NEEDLE_TIP: LinearRgba = srgb(0.22, 0.31, 0.23);
const SNOW: LinearRgba = srgb(0.90, 0.93, 0.98);
const BARK: LinearRgba = srgb(0.31, 0.25, 0.20);
const DARK_BARK: LinearRgba = srgb(0.18, 0.15, 0.13);
const DEAD_WOOD: LinearRgba = srgb(0.48, 0.44, 0.39);
const HEARTWOOD: LinearRgba = srgb(0.68, 0.56, 0.40);
const GNARL_BARK: LinearRgba = srgb(0.30, 0.27, 0.24);
const SPRAY: LinearRgba = srgb(0.84, 0.70, 0.47);
const SPRAY_DRY: LinearRgba = srgb(0.66, 0.52, 0.40);
const PINE_BARK: LinearRgba = srgb(0.30, 0.26, 0.23);
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
const ROCK_REACH: f32 = 50.0;
const ROCK_DETAIL: [u32; 3] = [15, 6, 3];
const TIER: f32 = 4.0;
const CELL: f32 = 64.0;
const RESTREAM_STEP: f32 = 8.0;
const SLACK: f32 = RESTREAM_STEP + 2.0;
const MERGE_LIMIT: f32 = 360.0;
const MERGE_VERTICES: usize = 1200;
const WOODS_REACH: f32 = 1300.0;
const WOODS_SLACK: f32 = 160.0;
const WOODS_NOW: f32 = 260.0;
const STANDS_PER_FRAME: usize = 6;
const SHADOWLESS: f32 = sky::SHADOW_DISTANCE + 120.0;
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

const ATLAS: u32 = 512;

#[derive(Clone, Copy)]
struct Card {
  corner: Vec2,
  extent: Vec2
}

impl Card {
  const FROND: Card = Card { corner: Vec2::ZERO, extent: Vec2::new(0.5, 1.0) };
  const SPRAY: Card = Card { corner: Vec2::new(0.5, 0.0), extent: Vec2::new(0.5, 0.875) };
  const SOLID: Vec2 = Vec2::new(0.75, 0.94);

  fn texels(self) -> UVec2 { (self.extent * ATLAS as f32).as_uvec2() }

  fn uv(self, across: f32, along: f32) -> Vec2 {
    self.corner + self.extent * Vec2::new(across, along)
  }
}

fn frond_texel(across: f32, along: f32) -> [f32; 4] {
  let side = across.abs();
  let flank = if across < 0.0 { 1 } else { 2 };
  let outline = |along: f32| 0.97 * smooth(0.0, 0.22, along) * (1.0 - along).powf(0.55);
  let spacing = 0.045;
  let root = along - 0.3 * side;
  let twig = (root / spacing).round();
  let offset = (root - twig * spacing).abs();
  let reach =
    outline(twig * spacing) * (0.72 + 0.38 * noise::hash(twig as i32, flank, 131));
  let out = side / reach.max(1e-3);
  let comb = noise::hash((side * 110.0) as i32, twig as i32 * 3 + flank, 133);
  let tuft = spacing * 0.56 * (1.0 - out).max(0.0).powf(0.3) * (0.45 + 0.6 * comb);
  let rachis = side < 0.018 * (1.0 - along) + 0.005 && along < 0.96;
  let stem = offset < 0.004 && out < 0.95;
  let needled = offset < tuft && out < 1.0 && along > 0.04;
  let light = 0.62
    + 0.38 * out.min(1.0)
    + 0.18 * (noise::hash((across * 60.0) as i32, (along * 180.0) as i32, 137) - 0.5);
  (rachis || stem)
    .then_some([0.42, 0.34, 0.26, 1.0])
    .or_else(|| needled.then_some([light, light, light * 0.95, 1.0]))
    .unwrap_or([0.7, 0.7, 0.68, 0.0])
}

struct Sprig {
  from: Vec2,
  to: Vec2,
  width: f32
}

fn sprigs(
  roll: &mut Roll,
  from: Vec2,
  heading: f32,
  length: f32,
  width: f32,
  depth: u32
) -> (Vec<Sprig>, Vec<(Vec2, f32)>) {
  let to = from + Vec2::from_angle(heading) * length;
  let leaves: Vec<(Vec2, f32)> = (0..if depth >= 4 { depth - 2 } else { 0 })
    .map(|_| {
      let along = from.lerp(to, roll.range(0.4, 1.0));
      (along + Vec2::new(roll.spread(4.0), roll.spread(4.0)), roll.range(1.2, 2.6))
    })
    .collect();
  let sprig = Sprig { from, to, width };
  (0..if depth < 5 { 2 + roll.below(2) } else { 0 })
    .map(|_| {
      let heading = heading + roll.spread(0.75);
      let length = length * roll.range(0.55, 0.8);
      sprigs(roll, to, heading, length, (width * 0.68).max(0.6), depth + 1)
    })
    .fold((vec![sprig], leaves), |(mut twigs, mut leaves), (more, buds)| {
      twigs.extend(more);
      leaves.extend(buds);
      (twigs, leaves)
    })
}

fn spray_texel(at: Vec2, twigs: &[Sprig], leaves: &[(Vec2, f32)]) -> [f32; 4] {
  let wood = twigs.iter().any(|&Sprig { from, to, width }| {
    let along = to - from;
    let t = ((at - from).dot(along) / along.length_squared()).clamp(0.0, 1.0);
    at.distance(from + along * t) < width * 0.5
  });
  let leaf = leaves.iter().any(|&(center, radius)| at.distance(center) < radius);
  let fleck = noise::hash(at.x as i32, at.y as i32, 139);
  leaf
    .then_some([0.95 + 0.05 * fleck, 0.9 + 0.1 * fleck, 0.78, 1.0])
    .or_else(|| wood.then_some([0.62, 0.56, 0.48, 1.0]))
    .unwrap_or([0.8, 0.76, 0.6, 0.0])
}

fn coverage(texels: &[[f32; 4]], scale: f32) -> f32 {
  texels.iter().filter(|texel| texel[3] * scale >= 0.5).count() as f32
    / texels.len() as f32
}

fn halved(texels: &[[f32; 4]], size: u32) -> Vec<[f32; 4]> {
  let half = size / 2;
  (0..half * half)
    .map(|index| {
      let (x, y) = (index % half * 2, index / half * 2);
      [(0, 0), (1, 0), (0, 1), (1, 1)]
        .map(|(dx, dy)| texels[((y + dy) * size + x + dx) as usize])
        .into_iter()
        .fold([0.0; 4], |sum, texel| {
          std::array::from_fn(|channel| sum[channel] + texel[channel] / 4.0)
        })
    })
    .collect()
}

fn frond_atlas() -> Image {
  let mut roll = Roll::new(141);
  let spray = Card::SPRAY.texels().as_vec2();
  let (twigs, leaves) =
    (0..3).fold((Vec::new(), Vec::new()), |(mut twigs, mut leaves), stem| {
      let (more, buds) = sprigs(
        &mut roll,
        Vec2::new(spray.x * (0.4 + 0.1 * stem as f32), 2.0),
        FRAC_PI_2 + (stem as f32 - 1.0) * 0.35,
        spray.y * 0.34,
        5.0,
        0
      );
      twigs.extend(more);
      leaves.extend(buds);
      (twigs, leaves)
    });
  let frond = Card::FROND.texels().as_vec2();
  let base: Vec<[f32; 4]> = (0..ATLAS * ATLAS)
    .map(|index| {
      let at = Vec2::new((index % ATLAS) as f32, (index / ATLAS) as f32) + 0.5;
      let across = at.x - frond.x;
      (at.x < frond.x)
        .then(|| frond_texel(at.x / frond.x * 2.0 - 1.0, at.y / frond.y))
        .or_else(|| {
          (at.y < spray.y).then(|| spray_texel(Vec2::new(across, at.y), &twigs, &leaves))
        })
        .unwrap_or([1.0; 4])
    })
    .collect();
  let target = coverage(&base, 1.0);
  let levels = ATLAS.ilog2() + 1;
  let data: Vec<u8> = (1..levels)
    .scan((base.clone(), ATLAS), |(texels, size), _| {
      *texels = halved(texels, *size);
      *size /= 2;
      let scale = (0..14)
        .fold((1.0f32, 4.0f32), |(low, high), _| {
          let middle = (low + high) / 2.0;
          if coverage(texels, middle) < target { (middle, high) } else { (low, middle) }
        })
        .1;
      Some(
        texels
          .iter()
          .map(|&[r, g, b, a]| [r, g, b, (a * scale).min(1.0)])
          .collect::<Vec<_>>()
      )
    })
    .fold(vec![base], |mut chain, level| {
      chain.push(level);
      chain
    })
    .into_iter()
    .flatten()
    .flat_map(|texel| texel.map(|channel| (channel.clamp(0.0, 1.0) * 255.0) as u8))
    .collect();
  let mut image = Image::new_uninit(
    Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
    TextureDimension::D2,
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::RENDER_WORLD
  );
  image.data = Some(data);
  image.texture_descriptor.mip_level_count = levels;
  image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
    anisotropy_clamp: 8,
    ..ImageSamplerDescriptor::linear()
  });
  image
}

#[derive(Default)]
struct Cards {
  positions: Vec<Vec3>,
  normals: Vec<Vec3>,
  uvs: Vec<Vec2>,
  colors: Vec<[f32; 4]>,
  indices: Vec<u32>
}

struct Rib {
  center: Vec3,
  side: Vec3,
  drop: Vec3,
  normal: Vec3,
  tone: LinearRgba
}

impl Cards {
  fn ribbon(&mut self, card: Card, ribs: &[Rib]) {
    let first = self.positions.len() as u32;
    let last = (ribs.len() - 1) as f32;
    ribs.iter().enumerate().for_each(
      |(row, &Rib { center, side, drop, normal, tone })| {
        let along = row as f32 / last;
        self.positions.extend([center - side - drop, center, center + side - drop]);
        self.normals.extend([
          (normal - side.normalize_or_zero() * 0.4).normalize(),
          normal,
          (normal + side.normalize_or_zero() * 0.4).normalize()
        ]);
        self.uvs.extend([0.0, 0.5, 1.0].map(|across| card.uv(across, along)));
        self.colors.extend([tone.to_f32_array(); 3]);
      }
    );
    self.indices.extend((0..ribs.len() as u32 - 1).flat_map(|row| {
      let at = first + row * 3;
      [0, 1].into_iter().flat_map(move |column| {
        let corner = at + column;
        [corner, corner + 3, corner + 1, corner + 1, corner + 3, corner + 4]
      })
    }));
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

fn solid(Piece(mut mesh): Piece) -> Piece {
  let count = mesh.count_vertices();
  mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Card::SOLID; count]);
  Piece(mesh)
}

fn pine(seed: u32, snowy: bool, coarse: bool) -> Tree {
  let mut roll = Roll::new(seed);
  let height = roll.range(14.0, 18.0);
  let base = height * roll.range(0.16, 0.3);
  let width = height * roll.range(0.16, 0.2);
  let girth = 0.02 * height + 0.12;
  let whorls = if coarse { 6 + roll.below(2) } else { 12 + roll.below(4) };
  let twist = roll.range(0.0, TAU);
  let mut cards = Cards::default();
  (0..whorls).for_each(|whorl| {
    let t = whorl as f32 / whorls as f32;
    let y =
      base + (height - base - 1.2) * (1.0 - (1.0 - t).powf(1.25)) + roll.spread(0.15);
    let reach = width * (1.0 - t).powf(0.85) * roll.range(0.85, 1.1) + 0.6;
    let boughs = if coarse { 4 } else { 5 + roll.below(3) };
    (0..boughs).for_each(|bough| {
      let angle = twist
        + whorl as f32 * 2.4
        + (bough as f32 + roll.spread(0.3)) / boughs as f32 * TAU;
      let out = Vec3::new(angle.cos(), 0.0, angle.sin());
      let tilt = roll.spread(0.35);
      let across = Vec3::new(-out.z, 0.0, out.x) * tilt.cos() + Vec3::Y * tilt.sin();
      let lift = roll.spread(0.35);
      let length = reach * roll.range(0.8, 1.15);
      let droop = (0.3 + 0.4 * (1.0 - t)) * roll.range(0.8, 1.25);
      let half = (length * 0.45 + 0.3).min(1.2) * if coarse { 1.3 } else { 1.0 };
      let hue = NEEDLE_DEEP
        .mix(&NEEDLE, roll.range(0.3, 0.8))
        .mix(&LinearRgba::BLACK, 0.35 * (1.0 - t));
      let frost = if snowy { roll.range(0.35, 0.8) } else { 0.0 };
      let ribs: Vec<Rib> = (0..=3)
        .map(|step| {
          let s = step as f32 / 3.0;
          Rib {
            center: out * (girth * 0.5 + length * s)
              + Vec3::Y * (y + lift + length * (0.18 * s - droop * s * s)),
            side: across * half,
            drop: Vec3::Y * half * (0.3 + 0.3 * s),
            normal: (Vec3::Y * 0.7 + out * (0.5 + 0.5 * s)).normalize(),
            tone: hue
              .mix(&NEEDLE_TIP, s * s * 0.45)
              .mix(&SNOW, frost * smooth(0.1, 0.6, s))
          }
        })
        .collect();
      cards.ribbon(Card::FROND, &ribs);
      let hanging = !coarse && t < 0.8 && roll.chance(0.6);
      let from = ribs[1].center;
      let sway = roll.spread(0.6);
      let hang = (out + across * sway - Vec3::Y * roll.range(0.5, 0.9)).normalize();
      let span = length * roll.range(0.45, 0.65);
      let under: Vec<Rib> = (0..=2)
        .map(|step| {
          let s = step as f32 / 2.0;
          Rib {
            center: from + hang * span * s - Vec3::Y * 0.15,
            side: (across - out * sway).normalize() * half * 0.7,
            drop: Vec3::Y * half * 0.25,
            normal: (Vec3::Y * 0.5 + out).normalize(),
            tone: dim(hue, 0.25).mix(&NEEDLE_TIP, s * 0.3)
          }
        })
        .collect();
      if hanging {
        cards.ribbon(Card::FROND, &under);
      }
    });
  });
  [Vec3::X, Vec3::Z].into_iter().for_each(|across| {
    let ribs: Vec<Rib> = (0..=1)
      .map(|step| Rib {
        center: Vec3::Y * (height - 2.6 + 3.0 * step as f32),
        side: across * 0.55,
        drop: Vec3::ZERO,
        normal: Vec3::Y,
        tone: NEEDLE.mix(&SNOW, if snowy { 0.4 } else { 0.0 })
      })
      .collect();
    cards.ribbon(Card::FROND, &ribs);
  });
  let trunk = wood_piece(
    model::lathe(
      &[
        Vec2::new(girth * 1.5, -0.8),
        Vec2::new(girth * 1.15, 0.4),
        Vec2::new(girth, 1.6),
        Vec2::new(girth * 0.72, height * 0.45),
        Vec2::new(girth * 0.38, height * 0.8),
        Vec2::new(0.03, height)
      ],
      if coarse { 5 } else { 9 }
    ),
    PINE_BARK
  );
  let stubs = (0..if coarse { 0 } else { 4 + roll.below(5) }).map(|_| {
    let angle = roll.range(0.0, TAU);
    let start = Vec3::Y * roll.range(1.8, base.max(2.2));
    let reach =
      Vec3::new(angle.cos(), roll.range(-0.3, 0.25), angle.sin()) * roll.range(0.5, 1.4);
    wood_piece(model::tube(&[start, start + reach], &[0.05, 0.015], 4), DEAD_WOOD)
  });
  let wood = model::merge(std::iter::once(trunk).chain(stubs.collect::<Vec<_>>()));
  Tree { wood, crown: cards.mesh(), height, girth }
}

fn limb(
  roll: &mut Roll,
  from: Vec3,
  heading: Vec3,
  length: f32,
  radius: f32,
  depth: u32
) -> (Vec<Piece>, Vec<(Vec3, Vec3)>) {
  let wobble = Vec3::new(roll.spread(0.3), roll.spread(0.15), roll.spread(0.3)) * length;
  let to = from + heading * length + wobble * 0.4 + Vec3::Y * length * 0.1;
  let bend = (from + to) / 2.0 + wobble;
  let branch = wood_piece(
    model::tube(
      &model::curve(from, bend, to, 3),
      &model::taper(3, radius, radius * 0.66),
      if depth < 2 { 6 } else { 4 }
    ),
    GNARL_BARK
  );
  let direction = (to - bend).normalize_or(heading);
  let tips = (depth >= 1).then_some((to, direction)).into_iter().collect();
  (0..if depth < 3 { 2 + roll.below(2) } else { 0 })
    .map(|_| {
      let splay = Vec3::new(roll.spread(1.0), roll.spread(0.4), roll.spread(1.0));
      let heading = (direction + splay * 1.1 + Vec3::Y * 0.15).normalize_or(direction);
      let length = length * roll.range(0.6, 0.8);
      limb(roll, to, heading, length, radius * 0.62, depth + 1)
    })
    .fold((vec![branch], tips), |(mut pieces, mut tips), (more, ends)| {
      pieces.extend(more);
      tips.extend(ends);
      (pieces, tips)
    })
}

fn gnarl(seed: u32) -> Tree {
  let mut roll = Roll::new(seed);
  let height = roll.range(5.0, 7.5);
  let girth = roll.range(0.2, 0.3);
  let lean = Vec3::new(roll.spread(1.0), 0.0, roll.spread(1.0)).normalize_or(Vec3::X);
  let across = Vec3::Y.cross(lean);
  let path = model::spline(
    &[
      Vec3::Y * -0.4,
      Vec3::Y * height * 0.16 + lean * 0.5 + across * roll.spread(0.4),
      Vec3::Y * height * 0.3 + lean * height * 0.2 + across * roll.spread(0.7),
      Vec3::Y * height * 0.42 + lean * height * 0.24 + across * roll.spread(0.8)
    ],
    3
  );
  let steps = path.len() - 1;
  let radii: Vec<f32> = (0..=steps)
    .map(|step| {
      let t = step as f32 / steps as f32;
      girth * (0.75 + 0.9 * smooth(0.25, 0.0, t) - 0.2 * t)
    })
    .collect();
  let trunk = wood_piece(model::tube(&path, &radii, 8), GNARL_BARK);
  let fork = path[steps];
  let heading = (fork - path[steps - 1]).normalize();
  let (limbs, tips) = (0..2 + roll.below(2))
    .map(|_| {
      let splay = Vec3::new(roll.spread(1.0), 0.0, roll.spread(1.0));
      let direction = (heading * 0.6 + splay * 1.4 + Vec3::Y * 0.3).normalize_or(Vec3::Y);
      let length = height * roll.range(0.28, 0.4);
      limb(&mut roll, fork, direction, length, girth * 0.62, 0)
    })
    .fold((vec![trunk], Vec::new()), |(mut pieces, mut tips), (more, ends)| {
      pieces.extend(more);
      tips.extend(ends);
      (pieces, tips)
    });
  let mut cards = Cards::default();
  tips.iter().for_each(|&(tip, direction)| {
    let rise = (direction + Vec3::Y * 0.3).normalize();
    let size = roll.range(1.1, 1.8);
    let flat = rise.any_orthonormal_vector();
    let turn = |angle: f32| Quat::from_axis_angle(rise, angle) * flat;
    let hue =
      SPRAY.mix(&SPRAY_DRY, roll.next()).mix(&LinearRgba::BLACK, roll.range(0.0, 0.25));
    [0.0, PI / 3.0, 2.0 * PI / 3.0].map(turn).into_iter().for_each(|side| {
      let ribs: Vec<Rib> = (0..=1)
        .map(|step| Rib {
          center: tip - rise * size * 0.35 + rise * size * 1.75 * step as f32,
          side: side * size * 0.5,
          drop: Vec3::ZERO,
          normal: (Vec3::Y + direction * 0.5).normalize(),
          tone: hue
        })
        .collect();
      cards.ribbon(Card::SPRAY, &ribs);
    });
  });
  Tree { wood: model::merge(limbs), crown: cards.mesh(), height, girth }
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
  let strata = (point.y * 9.0 + 2.0 * noise::fbm3(point * 0.9, 2, seed + 2)).sin();
  let top = smooth(0.55, 0.85, normal.y + 0.4 * noise::fbm3(point * 2.3, 2, seed + 5));
  let rock = STONE
    .mix(&DARK_STONE, smooth(-0.2, 0.25, grain))
    .mix(&CRAG, smooth(0.3, 0.9, strata) * 0.35)
    .mix(&LICHEN, smooth(0.3, 0.6, noise::fbm3(point * 3.0, 2, seed + 3)) * 0.12);
  rock.mix(&if snowy { SNOW } else { MOSS }, top * if snowy { 0.95 } else { 0.35 })
}

fn rock_shape(seed: u32, cuts: u32, ledges: f32, snowy: bool) -> Shape {
  let [near, far @ ..] = ROCK_DETAIL.map(|detail| {
    Piece::new(model::hewn(seed, cuts, ledges, detail), Srgba::WHITE)
      .shaded(|point, normal| stone_tone(point, normal, snowy, seed))
      .0
  });
  let collider = near
    .attribute(Mesh::ATTRIBUTE_POSITION)
    .and_then(|values| values.as_float3())
    .and_then(|points| {
      Collider::convex_hull(points.iter().copied().map(Vec3::from).collect())
    })
    .unwrap_or_else(|| Collider::sphere(1.0));
  Shape {
    parts: vec![(Coat::Plain(Stuff::Stone), near)],
    far: far.into_iter().map(|mesh| (Coat::Plain(Stuff::Stone), mesh)).collect(),
    collider: Some(collider),
    reach: Reach::PerSize(ROCK_REACH)
  }
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
  #[assoc(variants = 5)]
  Gnarl,
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
  const ALL: [Growth; 12] = [
    Growth::Pine,
    Growth::SnowyPine,
    Growth::Birch,
    Growth::Gnarl,
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

#[derive(Clone, Copy)]
enum Coat {
  Plain(Stuff),
  Fronds
}

#[derive(Clone, Copy)]
enum Reach {
  Fixed(f32),
  PerSize(f32)
}

impl Reach {
  fn at(self, scale: Vec3) -> f32 {
    match self {
      Reach::Fixed(reach) => reach,
      Reach::PerSize(reach) => reach * scale.max_element()
    }
  }
}

fn fade(reach: f32) -> f32 { FADE.min(reach * 0.25) }

struct Shape {
  parts: Vec<(Coat, Mesh)>,
  far: Vec<(Coat, Mesh)>,
  collider: Option<Collider>,
  reach: Reach
}

fn shape(growth: Growth, variant: usize) -> Shape {
  let seed = 1000 * growth as u32 + variant as u32 + 7;
  match growth {
    Growth::Pine | Growth::SnowyPine => {
      let snowy = growth == Growth::SnowyPine;
      let Tree { wood, crown, height, girth } = pine(variant as u32 + 100, snowy, false);
      let far = pine(variant as u32 + 100, snowy, true);
      Shape {
        parts: vec![(Coat::Plain(Stuff::Bark), wood), (Coat::Fronds, crown)],
        far: vec![(
          Coat::Fronds,
          model::merge([solid(Piece(far.wood)), Piece(far.crown)])
        )],
        collider: Some(trunk_collider(girth * 1.05, height * 0.9)),
        reach: Reach::Fixed(NEAR_TREE)
      }
    }
    Growth::Birch => {
      let Tree { wood, crown, height, girth } = birch(seed);
      Shape {
        parts: vec![
          (Coat::Plain(Stuff::Bark), wood),
          (Coat::Plain(Stuff::Needles), crown),
        ],
        far: vec![],
        collider: Some(trunk_collider(girth, height * 0.8)),
        reach: Reach::Fixed(FOREVER)
      }
    }
    Growth::Gnarl => {
      let Tree { wood, crown, height, girth } = gnarl(seed);
      Shape {
        parts: vec![(Coat::Plain(Stuff::Bark), wood), (Coat::Fronds, crown)],
        far: vec![],
        collider: Some(trunk_collider(girth, height * 0.4)),
        reach: Reach::Fixed(700.0)
      }
    }
    Growth::Snag => {
      let (mesh, collider) = snag(seed);
      Shape {
        parts: vec![(Coat::Plain(Stuff::Bark), mesh)],
        far: vec![],
        collider: Some(collider),
        reach: Reach::Fixed(600.0)
      }
    }
    Growth::Stump => {
      let (mesh, collider) = stump(seed);
      Shape {
        parts: vec![(Coat::Plain(Stuff::Bark), mesh)],
        far: vec![],
        collider: Some(collider),
        reach: Reach::Fixed(220.0)
      }
    }
    Growth::Log => {
      let (mesh, collider) = log(seed);
      Shape {
        parts: vec![(Coat::Plain(Stuff::Bark), mesh)],
        far: vec![],
        collider: Some(collider),
        reach: Reach::Fixed(260.0)
      }
    }
    Growth::Boulder | Growth::SnowyBoulder => {
      rock_shape(variant as u32 + 300, 13, 0.07, growth == Growth::SnowyBoulder)
    }
    Growth::Crag | Growth::SnowyCrag => {
      rock_shape(variant as u32 + 500, 9, 0.14, growth == Growth::SnowyCrag)
    }
    Growth::Juniper => Shape {
      parts: vec![(Coat::Plain(Stuff::Needles), juniper(seed))],
      far: vec![],
      collider: None,
      reach: Reach::Fixed(200.0)
    }
  }
}

struct Form {
  parts: Vec<(Handle<StandardMaterial>, Handle<Mesh>)>,
  far: Vec<(Handle<StandardMaterial>, Handle<Mesh>)>,
  collider: Option<Collider>,
  reach: Reach
}

#[derive(Clone)]
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
      && crate::river::course_distance(at) > 11.0
      && at.distance(START) > 6.0
      && Place::ALL.into_iter().all(|place| {
        at.distance(place.spot()) > place.flat() * place.clearance()
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
  let origin = patch.as_vec2() * PATCH;
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
      (site.spot.x.abs() < BOUND
        && site.spot.z.abs() < BOUND
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
  let gnarls: Vec<Plant> = (0..2)
    .filter_map(|_| {
      let at = anywhere(&mut roll);
      let site = survey(ground, at);
      let chance = 0.3
        * smooth(0.22, 0.04, site.forest)
        * (0.35 + tundra(at))
        * site.treeline
        * (1.0 - site.snow);
      let (pick, variant, yaw, size) = (
        roll.next(),
        roll.below(Growth::Gnarl.variants()),
        roll.range(0.0, TAU),
        roll.range(0.8, 1.25)
      );
      (site.open && site.road > 4.5 && site.normal.y > 0.75 && pick < chance).then(|| {
        Plant {
          growth: Growth::Gnarl,
          variant,
          place: upright(site.spot, yaw, Vec2::ZERO, Vec3::splat(size))
        }
      })
    })
    .collect();
  [trees, deadwood, boulders, outcrops, shrubs, gnarls].into_iter().flatten().collect()
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

fn sow(ground: &Ground, cell: IVec2, near: &[Plant]) -> Vec<Plant> {
  ground.chunk_at((cell.as_vec2() + 0.5) * CELL);
  let per = (CELL / PATCH) as i32;
  (0..per)
    .flat_map(|row| (0..per).map(move |column| cell * per + IVec2::new(column, row)))
    .flat_map(|patch| scatter(ground, patch))
    .chain(near.iter().filter(|plant| cell_of(plant.place.translation) == cell).cloned())
    .collect()
}

fn cell_of(at: Vec3) -> IVec2 { (at.xz() / CELL).floor().as_ivec2() }

fn shapes() -> Vec<((Growth, usize), Shape)> {
  crate::par::scope(|scope| {
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

struct Look {
  mesh: Handle<Mesh>,
  material: Handle<StandardMaterial>,
  place: Transform,
  range: VisibilityRange,
  shown: Option<Entity>,
  shadowless: bool
}

impl Look {
  fn stream(&mut self, commands: &mut Commands, eye: Vec3, wanted: bool) {
    let distance = eye.distance(self.place.translation);
    let live = wanted
      && distance >= self.range.start_margin.start - SLACK
      && distance < self.range.end_margin.end + SLACK;
    let shadowless = distance > SHADOWLESS;
    match (live, self.shown) {
      (true, None) => {
        let mut shown = commands.spawn((
          Mesh3d(self.mesh.clone()),
          MeshMaterial3d(self.material.clone()),
          self.place,
          self.range.clone()
        ));
        if shadowless {
          shown.insert(NotShadowCaster);
        }
        self.shown = Some(shown.id());
      }
      (false, Some(entity)) => {
        commands.entity(entity).despawn();
        self.shown = None;
      }
      (true, Some(entity)) if shadowless != self.shadowless => {
        match shadowless {
          true => commands.entity(entity).insert(NotShadowCaster),
          false => commands.entity(entity).remove::<NotShadowCaster>()
        };
      }
      _ => {}
    }
    self.shadowless = shadowless;
  }
}

fn bake(looks: &[&Look], origin: Vec3, meshes: &HashMap<AssetId<Mesh>, Mesh>) -> Mesh {
  let floats = |mesh: &Mesh, attribute| match mesh.attribute(attribute) {
    Some(VertexAttributeValues::Float32x3(values)) => values.clone(),
    _ => Vec::new()
  };
  let (positions, normals, uvs, colors, indices) = looks.iter().fold(
    (Vec::<[f32; 3]>::new(), Vec::<[f32; 3]>::new(), Vec::new(), Vec::new(), Vec::new()),
    |(mut positions, mut normals, mut uvs, mut colors, mut indices), look| {
      let mesh = &meshes[&look.mesh.id()];
      let first = positions.len() as u32;
      let count = mesh.count_vertices();
      positions.extend(
        floats(mesh, Mesh::ATTRIBUTE_POSITION)
          .into_iter()
          .map(|point| (look.place.transform_point(point.into()) - origin).to_array())
      );
      normals.extend(floats(mesh, Mesh::ATTRIBUTE_NORMAL).into_iter().map(|normal| {
        (look.place.rotation * (Vec3::from(normal) / look.place.scale))
          .normalize_or_zero()
          .to_array()
      }));
      match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(values)) => uvs.extend(values),
        _ => uvs.extend(vec![[0.0f32; 2]; count])
      }
      match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(values)) => colors.extend(values),
        _ => colors.extend(vec![[1.0f32; 4]; count])
      }
      match mesh.indices() {
        Some(list) => indices.extend(list.iter().map(|index| first + index as u32)),
        None => indices.extend(first..first + count as u32)
      }
      (positions, normals, uvs, colors, indices)
    }
  );
  Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

struct Stand {
  bounds: Rect,
  origin: Vec3,
  settled: f32,
  looks: Vec<Look>,
  distant: Vec<Look>,
  batches: Vec<(Handle<StandardMaterial>, Handle<Mesh>, Aabb)>,
  merged: Vec<Entity>,
  shadowless: bool,
  body: Option<Entity>
}

impl Stand {
  fn gap(&self, eye: Vec2) -> f32 {
    (self.bounds.min - eye).max(eye - self.bounds.max).max(Vec2::ZERO).length()
  }

  fn tend(&mut self, commands: &mut Commands, eye: Vec3) {
    let gap = self.gap(eye.xz());
    let far = gap
      > self.settled
        + RESTREAM_STEP
        + (self.merged.is_empty() as u8 as f32) * CELL * 0.25;
    let shadowless = gap > SHADOWLESS;
    match (far, self.merged.is_empty()) {
      (true, true) => {
        self.merged = self
          .batches
          .iter()
          .map(|(material, mesh, aabb)| {
            let mut batch = commands.spawn((
              Mesh3d(mesh.clone()),
              MeshMaterial3d(material.clone()),
              Transform::from_translation(self.origin),
              *aabb
            ));
            if shadowless {
              batch.insert(NotShadowCaster);
            }
            batch.id()
          })
          .collect();
      }
      (false, false) => {
        self.merged.drain(..).for_each(|entity| commands.entity(entity).despawn());
      }
      _ => {}
    }
    if shadowless != self.shadowless {
      self.merged.iter().for_each(|&entity| {
        match shadowless {
          true => commands.entity(entity).insert(NotShadowCaster),
          false => commands.entity(entity).remove::<NotShadowCaster>()
        };
      });
    }
    self.shadowless = shadowless;
    self.looks.iter_mut().for_each(|look| look.stream(commands, eye, true));
    self.distant.iter_mut().for_each(|look| look.stream(commands, eye, !far));
  }

  fn fell(mut self, commands: &mut Commands) {
    self
      .looks
      .iter_mut()
      .chain(self.distant.iter_mut())
      .for_each(|look| look.stream(commands, Vec3::splat(FOREVER), false));
    self
      .merged
      .drain(..)
      .chain(self.body)
      .for_each(|entity| commands.entity(entity).despawn());
  }
}

#[derive(Resource)]
struct Forms {
  kinds: HashMap<(Growth, usize), Form>,
  cpu: HashMap<AssetId<Mesh>, Mesh>
}

#[derive(Resource, Default)]
struct Woods {
  stands: HashMap<IVec2, Stand>,
  growing: HashMap<IVec2, Task<Vec<Plant>>>,
  near: Arc<Vec<Plant>>,
  eye: Option<Vec3>
}

fn stand(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  forms: &Forms,
  plants: Vec<Plant>
) -> Stand {
  let origin = plants.iter().map(|plant| plant.place.translation).sum::<Vec3>()
    / plants.len().max(1) as f32;
  let shapes: Vec<_> = plants
    .iter()
    .filter(|plant| plant.place.scale.max_element() > 0.5)
    .filter_map(|&Plant { growth, variant, place }| {
      forms.kinds[&(growth, variant)].collider.clone().map(|mut collider| {
        collider.set_scale(place.scale, 8);
        let shape = collider.shape_scaled();
        shape
          .as_compound()
          .map_or_else(
            || vec![(Vec3::ZERO, Quat::IDENTITY, shape.clone())],
            |compound| {
              compound
                .shapes()
                .iter()
                .map(|(pose, shape)| (pose.translation, pose.rotation, shape.clone()))
                .collect()
            }
          )
          .into_iter()
          .map(move |(offset, turn, shape)| {
            (
              Position(place.translation - origin + place.rotation * offset),
              place.rotation * turn,
              Collider::from(shape)
            )
          })
      })
    })
    .flatten()
    .collect();
  let body = (!shapes.is_empty()).then(|| {
    commands
      .spawn((
        RigidBody::Static,
        Collider::compound(shapes),
        Transform::from_translation(origin)
      ))
      .id()
  });
  let (distant, looks): (Vec<Look>, Vec<Look>) = plants
    .iter()
    .flat_map(|&Plant { growth, variant, place }| {
      let form = &forms.kinds[&(growth, variant)];
      let bound = |tier: usize| {
        let reach = form.reach.at(place.scale) * TIER.powi(tier as i32);
        reach..reach + fade(reach)
      };
      let look = |(material, mesh): &(Handle<StandardMaterial>, Handle<Mesh>),
                  start_margin,
                  end_margin| Look {
        mesh: mesh.clone(),
        material: material.clone(),
        place,
        range: VisibilityRange { start_margin, end_margin, use_aabb: false },
        shown: None,
        shadowless: false
      };
      form
        .parts
        .iter()
        .map(move |part| look(part, 0.0..0.0, bound(0)))
        .chain(form.far.iter().enumerate().map(move |(tier, part)| {
          look(
            part,
            bound(tier),
            (tier + 1 < form.far.len())
              .then(|| bound(tier + 1))
              .unwrap_or(FOREVER..FOREVER)
          )
        }))
        .collect::<Vec<_>>()
    })
    .partition(|look| {
      look.range.end_margin.start >= FOREVER
        && look.range.start_margin.end <= MERGE_LIMIT
        && forms.cpu[&look.mesh.id()].count_vertices() <= MERGE_VERTICES
    });
  let batches = distant
    .iter()
    .fold(Vec::<(Handle<StandardMaterial>, Vec<&Look>)>::new(), |mut batches, look| {
      match batches.iter_mut().find(|(material, _)| *material == look.material) {
        Some((_, list)) => list.push(look),
        None => batches.push((look.material.clone(), vec![look]))
      }
      batches
    })
    .into_iter()
    .map(|(material, list)| {
      let mesh = bake(&list, origin, &forms.cpu);
      let aabb = mesh.compute_aabb().unwrap_or_default();
      (material, meshes.add(mesh), aabb)
    })
    .collect();
  Stand {
    bounds: plants.iter().fold(Rect::EMPTY, |bounds, plant| {
      bounds.union_point(plant.place.translation.xz())
    }),
    origin,
    settled: distant.iter().map(|look| look.range.start_margin.end).fold(0.0, f32::max),
    looks,
    distant,
    batches,
    merged: Vec::new(),
    shadowless: false,
    body
  }
}

fn cell_gap(cell: IVec2, eye: Vec2) -> f32 {
  let low = cell.as_vec2() * CELL;
  (low - eye).max(eye - low - CELL).max(Vec2::ZERO).length()
}

fn tend_woods(
  mut commands: Commands,
  mut woods: ResMut<Woods>,
  mut meshes: ResMut<Assets<Mesh>>,
  forms: Option<Res<Forms>>,
  ground: Res<Ground>,
  camera: Single<&Transform, With<MainCamera>>
) {
  if let Some(forms) = forms {
    let eye = camera.translation;
    let Woods { stands, growing, near, eye: last } = &mut *woods;
    let span = (WOODS_REACH / CELL).ceil() as i32 + 1;
    let middle = (eye.xz() / CELL).floor().as_ivec2();
    let wanted: Vec<IVec2> = (-span..=span)
      .flat_map(|dz| (-span..=span).map(move |dx| middle + IVec2::new(dx, dz)))
      .filter(|&cell| {
        cell_gap(cell, eye.xz()) < WOODS_REACH
          && (cell.as_vec2() * CELL).abs().max_element() < BOUND + CELL
          && !stands.contains_key(&cell)
          && !growing.contains_key(&cell)
      })
      .collect();
    let (urgent, later): (Vec<IVec2>, Vec<IVec2>) = wanted
      .into_iter()
      .partition(|&cell| stands.is_empty() && cell_gap(cell, eye.xz()) < WOODS_NOW);
    terrain::in_parallel(&urgent, |&cell| (cell, sow(&ground, cell, near)))
      .into_iter()
      .for_each(|(cell, plants)| {
        let mut made = stand(&mut commands, &mut meshes, &forms, plants);
        made.tend(&mut commands, eye);
        stands.insert(cell, made);
      });
    later.into_iter().for_each(|cell| {
      let (ground, near) = (ground.clone(), near.clone());
      growing.insert(
        cell,
        AsyncComputeTaskPool::get().spawn(async move { sow(&ground, cell, &near) })
      );
    });
    let grown: Vec<(IVec2, Vec<Plant>)> = growing
      .iter_mut()
      .filter_map(|(&cell, task)| check_ready(task).map(|plants| (cell, plants)))
      .take(STANDS_PER_FRAME)
      .collect();
    grown.into_iter().for_each(|(cell, plants)| {
      growing.remove(&cell);
      let mut made = stand(&mut commands, &mut meshes, &forms, plants);
      made.tend(&mut commands, eye);
      stands.insert(cell, made);
    });
    growing.retain(|&cell, _| cell_gap(cell, eye.xz()) < WOODS_REACH + WOODS_SLACK);
    stands
      .extract_if(|&cell, _| cell_gap(cell, eye.xz()) > WOODS_REACH + WOODS_SLACK)
      .for_each(|(_, stand)| stand.fell(&mut commands));
    if last.is_none_or(|last| last.distance(eye) > RESTREAM_STEP) {
      *last = Some(eye);
      stands.values_mut().for_each(|stand| stand.tend(&mut commands, eye));
    }
  }
}

fn spawn_flora(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut woods: ResMut<Woods>,
  stuffs: Res<Stuffs>,
  ground: Res<Ground>
) {
  let (made, atlas) = crate::par::scope(|scope| {
    let made = scope.spawn(shapes);
    let atlas = scope.spawn(frond_atlas);
    (made.join().expect("flora shapes"), atlas.join().expect("frond atlas"))
  });
  let fronds = materials.add(StandardMaterial {
    base_color_texture: Some(images.add(atlas)),
    alpha_mode: AlphaMode::Mask(0.5),
    perceptual_roughness: 0.9,
    reflectance: 0.2,
    double_sided: true,
    cull_mode: None,
    ..default()
  });
  let mut cpu = HashMap::default();
  let mut coated = |coats: Vec<(Coat, Mesh)>| -> Vec<_> {
    coats
      .into_iter()
      .map(|(coat, mesh)| {
        let material = match coat {
          Coat::Plain(stuff) => stuffs.of(stuff),
          Coat::Fronds => fronds.clone()
        };
        let handle = meshes.add(mesh.clone());
        cpu.insert(handle.id(), mesh);
        (material, handle)
      })
      .collect()
  };
  let forms: HashMap<(Growth, usize), Form> = made
    .into_iter()
    .map(|(key, Shape { parts, far, collider, reach })| {
      (key, Form { parts: coated(parts), far: coated(far), collider, reach })
    })
    .collect();
  woods.near = Arc::new(foreground(&ground));
  commands.insert_resource(Forms { kinds: forms, cpu });
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
        * (1.0 - crate::river::bank(at))
        * (0.25 + 0.75 * clump)
        * f32::from(u8::from(at.abs().max_element() < BOUND))
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
    .init_resource::<Woods>()
    .add_systems(Startup, (spawn_flora, prepare_sward))
    .add_systems(Update, (tend_meadow, tend_woods));
}
