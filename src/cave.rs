use {crate::{inventory::{Inventory, Item, Loot},
             model::{self, Piece},
             noise::{self, Roll},
             place::{self, Place},
             player::{Player, View},
             sdf::{self, Bounds, Surface},
             signal::{Cue, FoeKind, FoeSpawn, Notice, Prompt, Prompting, Sound,
                      WordWall},
             sky::{CloseShadows, Daylight},
             stuff::{Stuff, Stuffs},
             terrain::{self, Surfaces}},
     avian3d::prelude::*,
     bevy::{color::Mix, prelude::*},
     fidget::context::Tree,
     std::f32::consts::{FRAC_PI_2, PI, TAU}};

pub(crate) const CARVED: Srgba = Srgba::rgb(0.66, 0.64, 0.60);
pub(crate) const WORN: Srgba = Srgba::rgb(0.56, 0.55, 0.52);
const RUNE: Srgba = Srgba::rgb(0.13, 0.12, 0.11);
pub(crate) const IRON: Srgba = Srgba::rgb(0.24, 0.23, 0.22);
pub(crate) const COAL: Srgba = Srgba::rgb(0.08, 0.07, 0.06);
pub(crate) const OLD_WOOD: Srgba = Srgba::rgb(0.34, 0.23, 0.14);
pub(crate) const BONE: Srgba = Srgba::rgb(0.78, 0.72, 0.58);
const CLAY: Srgba = Srgba::rgb(0.46, 0.41, 0.34);
const WEB: Srgba = Srgba::rgb(0.82, 0.82, 0.80);
pub(crate) const STRAW: Srgba = Srgba::rgb(0.62, 0.52, 0.30);
pub(crate) const PELT: Srgba = Srgba::rgb(0.42, 0.37, 0.31);

const FIRE_BOOST: f32 = 1.6;
const LAMP_BOOST: f32 = 2.2;
const ROCK_LIGHT: LinearRgba = terrain::srgb(0.50, 0.49, 0.47);
const ROCK_DARK: LinearRgba = terrain::srgb(0.30, 0.30, 0.30);
const MOSS: LinearRgba = terrain::srgb(0.30, 0.34, 0.16);
const SNOW: LinearRgba = terrain::srgb(0.92, 0.94, 0.98);

pub(crate) struct Lining {
  pub(crate) wall: LinearRgba,
  pub(crate) floor: LinearRgba,
  pub(crate) roughen: f32,
  pub(crate) crease: f32
}

pub(crate) const MASONRY: Lining = Lining {
  wall: terrain::srgb(0.46, 0.43, 0.39),
  floor: terrain::srgb(0.36, 0.33, 0.29),
  roughen: 0.0,
  crease: 0.8
};

pub(crate) const BURROW: Lining = Lining {
  wall: terrain::srgb(0.40, 0.36, 0.31),
  floor: terrain::srgb(0.33, 0.27, 0.20),
  roughen: 0.45,
  crease: 0.75
};

const DOOR_FLOOR: f32 = 2.0;
const HALL_FLOOR: f32 = -8.0;
const DESCENT_TOP: f32 = 31.0;
const DESCENT_SLOPE: f32 = 0.625;
const CHAMBER: Vec3 = Vec3::new(0.0, HALL_FLOOR, -12.0);
const DAIS: Vec3 = Vec3::new(0.0, -6.8, -16.5);
const DEN_FLOOR: f32 = -5.2;
const DEN_SLOPE: f32 = 0.58;
const DEN_MOUTH: f32 = 21.0;
const SHAFT: Vec2 = Vec2::new(4.5, -3.0);

const HOLLOWCRAG_LOOT: &[Loot] = &[
  Loot::Gold(143),
  Loot::one(Item::IronDagger),
  Loot::one(Item::PotionOfMinorHealing),
  Loot::one(Item::Amethyst),
  Loot::one(Item::AncientNordHelmet)
];
const FELLHOUND_LOOT: &[Loot] = &[
  Loot::Gold(37),
  Loot::one(Item::PotionOfMinorHealing),
  Loot::Goods(Item::Lockpick, 2)
];

fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
  let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

pub(crate) type Parts = Vec<(Stuff, Piece)>;

pub(crate) fn moved(parts: Parts, at: Transform) -> Parts {
  parts.into_iter().map(|(stuff, piece)| (stuff, piece.moved(at))).collect()
}

fn stone(mesh: impl Into<Mesh>, color: Srgba) -> (Stuff, Piece) {
  (Stuff::Stone, Piece::new(mesh, color))
}

pub(crate) fn jitter(roll: &mut Roll, color: Srgba) -> Srgba {
  let tone = roll.range(0.84, 1.1);
  Srgba::rgb(color.red * tone, color.green * tone, color.blue * tone)
}

struct Fire {
  at: Vec3,
  size: f32,
  lumens: f32,
  shadows: bool
}

pub(crate) struct Hoard {
  pub(crate) at: Transform,
  pub(crate) size: f32,
  pub(crate) noun: &'static str,
  pub(crate) loot: &'static [Loot]
}

#[derive(Default)]
pub(crate) struct Works {
  pub(crate) parts: Parts,
  wall: Parts,
  wall_at: Vec3,
  solids: Vec<(Transform, Collider)>,
  fires: Vec<Fire>,
  pub(crate) lamps: Vec<(Vec3, Color, f32, f32)>,
  beams: Vec<(Vec3, f32)>,
  foes: Vec<(Transform, FoeKind, bool)>,
  pub(crate) hoards: Vec<Hoard>,
  pub(crate) rooms: Vec<(Vec3, Vec3)>
}

impl Works {
  pub(crate) fn put(&mut self, parts: Parts, at: Transform) {
    self.parts.extend(moved(parts, at));
  }

  pub(crate) fn solid(&mut self, at: Transform, size: Vec3) {
    self.solids.push((at, Collider::cuboid(size.x, size.y, size.z)));
  }

  pub(crate) fn slab(&mut self, color: Srgba, size: Vec3, at: Transform) {
    self
      .parts
      .push(stone(model::block(size.x, size.y, size.z).transformed_by(at), color));
    self.solid(at, size);
  }

  pub(crate) fn fire(&mut self, at: Vec3, size: f32, lumens: f32, shadows: bool) {
    self.fires.push(Fire { at, size, lumens, shadows });
  }

  pub(crate) fn foe(&mut self, feet: Vec3, facing: Vec3, kind: FoeKind, dormant: bool) {
    self.foes.push((
      Transform::from_translation(feet)
        .looking_to(facing.with_y(0.0).normalize(), Vec3::Y),
      kind,
      dormant
    ));
  }
}

fn gable(half: f32, spring: f32, apex: f32, thick: f32, depth: f32) -> Parts {
  let top = Vec3::new(0.0, apex, 0.0);
  [-1.0, 1.0]
    .into_iter()
    .map(|side| {
      let foot = Vec3::new(side * half, spring, 0.0);
      let reach = (top - foot).normalize() * thick * 0.5;
      Piece::new(model::block(thick, 1.0, depth), CARVED).span(foot - reach, top + reach)
    })
    .map(|piece| (Stuff::Stone, piece))
    .chain([(
      Stuff::Stone,
      Piece::new(model::block(thick * 1.3, thick * 1.3, depth * 1.15), WORN)
        .rolled(PI / 4.0)
        .at(top)
    )])
    .collect()
}

pub(crate) fn rib(half: f32, height: f32, apex: f32, thick: f32, depth: f32) -> Parts {
  [-1.0, 1.0]
    .into_iter()
    .flat_map(|side| {
      [
        Piece::new(model::block(thick, height, depth), CARVED).at_xyz(
          side * half,
          height / 2.0,
          0.0
        ),
        Piece::new(model::block(thick * 1.35, 0.25, depth * 1.2), WORN).at_xyz(
          side * half,
          0.125,
          0.0
        ),
        Piece::new(model::block(thick * 1.3, 0.22, depth * 1.2), WORN).at_xyz(
          side * half,
          height - 0.11,
          0.0
        )
      ]
    })
    .map(|piece| (Stuff::Stone, piece))
    .chain(gable(half, height, apex, thick, depth))
    .collect()
}

pub(crate) fn pillar(height: f32, width: f32) -> Parts {
  let tier = |scale: f32, thick: f32, base: f32, color: Srgba| {
    Piece::new(model::block(width * scale, thick, width * scale), color).at_xyz(
      0.0,
      base + thick / 2.0,
      0.0
    )
  };
  let shaft = height - 1.3;
  [
    tier(1.5, 0.35, 0.0, WORN),
    tier(1.25, 0.2, 0.35, CARVED),
    tier(1.0, shaft, 0.55, CARVED),
    tier(1.12, 0.14, 0.55 + shaft * 0.3, WORN),
    tier(1.12, 0.14, 0.55 + shaft * 0.7, WORN),
    tier(1.2, 0.2, height - 0.75, CARVED),
    tier(1.55, 0.55, height - 0.55, WORN)
  ]
  .into_iter()
  .map(|piece| (Stuff::Stone, piece))
  .collect()
}

fn knot(size: f32, thickness: f32) -> Mesh {
  let path: Vec<Vec3> = (0..=90)
    .map(|step| {
      let t = step as f32 / 90.0 * TAU;
      Vec3::new(
        t.sin() + 2.0 * (2.0 * t).sin(),
        t.cos() - 2.0 * (2.0 * t).cos(),
        -(3.0 * t).sin() * 0.45
      ) * size
        / 3.0
    })
    .collect();
  model::tube(&path, &[thickness], 6)
}

fn ring(radius: f32, thickness: f32) -> Mesh {
  let path: Vec<Vec3> = (0..=48)
    .map(|step| {
      let t = step as f32 / 48.0 * TAU;
      Vec3::new(t.cos(), t.sin(), 0.0) * radius
    })
    .collect();
  model::tube(&path, &[thickness], 6)
}

fn medallion(radius: f32) -> Parts {
  vec![
    stone(
      Cylinder::new(radius, 0.12)
        .mesh()
        .resolution(28)
        .build()
        .rotated_by(Quat::from_rotation_x(FRAC_PI_2)),
      WORN
    ),
    stone(ring(radius * 0.96, radius * 0.06).translated_by(Vec3::Z * 0.06), CARVED),
    stone(knot(radius * 0.8, radius * 0.05).translated_by(Vec3::Z * 0.06), CARVED),
  ]
}

fn tusk(from: Vec3, bend: Vec3, to: Vec3, radius: f32) -> (Stuff, Piece) {
  stone(
    model::tube(
      &model::curve(from, bend, to, 14),
      &model::taper(14, radius, radius * 0.15),
      8
    ),
    CARVED
  )
}

fn dragon_head() -> Parts {
  let horns = [-1.0, 1.0].into_iter().flat_map(|side| {
    [
      tusk(
        Vec3::new(side * 0.2, 0.5, -0.05),
        Vec3::new(side * 0.55, 0.95, -0.35),
        Vec3::new(side * 0.35, 1.25, -0.95),
        0.1
      ),
      (
        Stuff::Stone,
        Piece::new(model::block(0.14, 0.1, 0.4), WORN).rolled(side * 0.3).at_xyz(
          side * 0.16,
          0.5,
          0.35
        )
      ),
      (
        Stuff::Frost,
        Piece::new(model::ball(0.045), Srgba::WHITE).at_xyz(side * 0.17, 0.42, 0.5)
      )
    ]
  });
  let teeth = (0..6).map(|index| {
    let x = (index as f32 / 5.0 - 0.5) * 0.28;
    (
      Stuff::Bone,
      Piece::new(model::cone(0.025, 0.1), BONE).pitched(PI).at_xyz(x, 0.14, 0.86)
    )
  });
  [
    Piece::new(model::lump(7, 0.1, 2), CARVED)
      .sized(Vec3::new(0.32, 0.26, 0.42))
      .at_xyz(0.0, 0.34, 0.1),
    Piece::new(model::block(0.34, 0.24, 0.62), CARVED)
      .pitched(-0.08)
      .at_xyz(0.0, 0.3, 0.6),
    Piece::new(model::block(0.3, 0.1, 0.56), WORN).pitched(0.28).at_xyz(0.0, 0.08, 0.55),
    Piece::new(model::block(0.5, 0.12, 0.3), WORN).at_xyz(0.0, 0.0, -0.05)
  ]
  .into_iter()
  .map(|piece| (Stuff::Stone, piece))
  .chain(horns)
  .chain(teeth)
  .collect()
}

fn flame() -> Mesh {
  model::merge([
    Piece::new(model::cone(0.13, 0.55), Srgba::WHITE).at_xyz(0.0, 0.3, 0.0),
    Piece::new(model::cone(0.08, 0.38), Srgba::WHITE)
      .rolled(0.35)
      .at_xyz(0.07, 0.22, 0.0),
    Piece::new(model::cone(0.08, 0.34), Srgba::WHITE)
      .rolled(-0.3)
      .pitched(0.2)
      .at_xyz(-0.06, 0.2, 0.03),
    Piece::new(model::ball(0.12), Srgba::WHITE)
      .sized(Vec3::new(1.0, 0.7, 1.0))
      .at_xyz(0.0, 0.06, 0.0)
  ])
}

pub(crate) fn brazier() -> Parts {
  let legs = (0..3).map(|index| {
    let angle = index as f32 / 3.0 * TAU;
    let (sin, cos) = angle.sin_cos();
    (
      Stuff::Iron,
      Piece::new(model::rod(0.035, 1.0), IRON).span(
        Vec3::new(cos * 0.42, 0.0, sin * 0.42),
        Vec3::new(cos * 0.2, 0.84, sin * 0.2)
      )
    )
  });
  let coals = (0..7).map(|index| {
    let angle = index as f32 * 2.4;
    let stuff = (index % 3 == 0).then_some(Stuff::Ember).unwrap_or(Stuff::Stone);
    (
      stuff,
      Piece::new(model::lump(index + 3, 0.2, 1), COAL).sized(Vec3::splat(0.1)).at_xyz(
        angle.cos() * 0.18,
        0.96,
        angle.sin() * 0.18
      )
    )
  });
  let bowl = model::lathe(
    &[
      Vec2::new(0.0, 0.8),
      Vec2::new(0.3, 0.82),
      Vec2::new(0.46, 0.98),
      Vec2::new(0.49, 1.05),
      Vec2::new(0.43, 1.02),
      Vec2::new(0.3, 0.9),
      Vec2::new(0.0, 0.9)
    ],
    18
  );
  [
    (Stuff::Iron, Piece::new(bowl, IRON)),
    (
      Stuff::Iron,
      Piece::new(ring(0.47, 0.03), IRON).pitched(FRAC_PI_2).at_xyz(0.0, 1.03, 0.0)
    )
  ]
  .into_iter()
  .chain(legs)
  .chain(coals)
  .collect()
}

pub(crate) fn sconce() -> Parts {
  vec![
    (Stuff::Iron, Piece::new(model::block(0.14, 0.3, 0.05), IRON).at_xyz(0.0, 0.0, 0.02)),
    (
      Stuff::Iron,
      Piece::new(model::rod(0.02, 1.0), IRON)
        .span(Vec3::new(0.0, -0.05, 0.02), Vec3::new(0.0, 0.1, 0.3))
    ),
    (
      Stuff::Iron,
      Piece::new(ring(0.05, 0.015), IRON).pitched(FRAC_PI_2).at_xyz(0.0, 0.1, 0.3)
    ),
    (
      Stuff::Wood,
      Piece::new(model::rod(0.03, 1.0), OLD_WOOD)
        .span(Vec3::new(0.0, -0.1, 0.24), Vec3::new(0.0, 0.34, 0.34))
    ),
    (
      Stuff::Cloth,
      Piece::new(model::rod(0.045, 0.12), COAL).pitched(0.2).at_xyz(0.0, 0.3, 0.33)
    ),
  ]
}

pub(crate) fn urn(seed: u32) -> Parts {
  let mut roll = Roll::new(seed);
  let girth = roll.range(0.16, 0.24);
  let tall = roll.range(0.4, 0.62);
  let profile = [
    Vec2::new(0.0, 0.0),
    Vec2::new(girth * 0.6, 0.0),
    Vec2::new(girth * 1.05, tall * 0.35),
    Vec2::new(girth, tall * 0.6),
    Vec2::new(girth * 0.5, tall * 0.88),
    Vec2::new(girth * 0.45, tall),
    Vec2::new(girth * 0.58, tall * 1.04),
    Vec2::new(0.0, tall * 1.04)
  ];
  let color = jitter(&mut roll, CLAY);
  vec![
    stone(model::lathe(&profile, 14), color),
    (
      Stuff::Iron,
      Piece::new(ring(girth * 1.03, 0.012), IRON).pitched(FRAC_PI_2).at_xyz(
        0.0,
        tall * 0.5,
        0.0
      )
    ),
  ]
}

pub(crate) fn bones(seed: u32) -> Parts {
  let mut roll = Roll::new(seed);
  let skull = [
    (
      Stuff::Bone,
      Piece::new(model::ball(0.1), BONE)
        .sized(Vec3::new(0.9, 0.85, 1.15))
        .at_xyz(0.0, 0.08, 0.0)
    ),
    (Stuff::Bone, Piece::new(model::block(0.1, 0.05, 0.1), BONE).at_xyz(0.0, 0.02, 0.1)),
    (Stuff::Stone, Piece::new(model::ball(0.022), COAL).at_xyz(0.035, 0.1, 0.1)),
    (Stuff::Stone, Piece::new(model::ball(0.022), COAL).at_xyz(-0.035, 0.1, 0.1))
  ];
  let yaw = roll.range(0.0, TAU);
  let long = (0..roll.below(4) + 2)
    .map(|_| {
      let angle = roll.range(0.0, TAU);
      let middle = Vec3::new(roll.spread(0.35), 0.03, roll.spread(0.35));
      let reach = Vec3::new(angle.cos(), roll.range(0.0, 0.1), angle.sin())
        * roll.range(0.16, 0.24);
      [
        (
          Stuff::Bone,
          Piece::new(model::rod(0.018, 1.0), BONE).span(middle - reach, middle + reach)
        ),
        (Stuff::Bone, Piece::new(model::ball(0.032), BONE).at(middle - reach)),
        (Stuff::Bone, Piece::new(model::ball(0.032), BONE).at(middle + reach))
      ]
    })
    .collect::<Vec<_>>()
    .into_iter()
    .flatten();
  let offset = Vec3::new(roll.spread(0.2), 0.0, roll.spread(0.2));
  let ribs = (0..4).map(move |index| {
    let z = index as f32 * 0.07;
    (
      Stuff::Bone,
      Piece::new(
        model::tube(
          &model::curve(
            Vec3::new(-0.14, 0.02, z),
            Vec3::new(0.0, 0.2, z),
            Vec3::new(0.14, 0.02, z),
            8
          ),
          &[0.012],
          4
        ),
        BONE
      )
      .at(offset + Vec3::new(0.25, 0.0, -0.2))
    )
  });
  moved(
    skull.into_iter().chain(long).chain(ribs).collect(),
    Transform::from_rotation(Quat::from_rotation_y(yaw))
  )
}

pub(crate) fn sarcophagus() -> Parts {
  let knot_top = moved(medallion(0.28), Transform::from_xyz(0.0, 2.25, 0.4));
  [
    Piece::new(model::block(1.1, 2.5, 0.16), CARVED).at_xyz(0.0, 1.25, -0.36),
    Piece::new(model::block(0.16, 2.5, 0.72), CARVED).at_xyz(-0.55, 1.25, 0.0),
    Piece::new(model::block(0.16, 2.5, 0.72), CARVED).at_xyz(0.55, 1.25, 0.0),
    Piece::new(model::block(1.3, 0.14, 0.8), WORN).at_xyz(0.0, 0.07, 0.0),
    Piece::new(model::block(1.3, 0.32, 0.8), WORN).at_xyz(0.0, 2.6, 0.0)
  ]
  .into_iter()
  .map(|piece| (Stuff::Stone, piece))
  .chain(moved(gable(0.62, 2.72, 3.1, 0.18, 0.8), Transform::IDENTITY))
  .chain(knot_top)
  .collect()
}

pub(crate) fn tomb() -> Parts {
  let knot = moved(
    medallion(0.3),
    Transform::from_xyz(0.0, 1.07, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
  );
  [
    Piece::new(model::block(1.3, 0.2, 2.6), WORN).at_xyz(0.0, 0.1, 0.0),
    Piece::new(model::block(1.1, 0.7, 2.4), CARVED).at_xyz(0.0, 0.55, 0.0),
    Piece::new(model::block(1.25, 0.12, 2.5), WORN).at_xyz(0.0, 0.94, 0.0),
    Piece::new(model::block(1.0, 0.08, 2.2), CARVED).at_xyz(0.0, 1.03, 0.0)
  ]
  .into_iter()
  .map(|piece| (Stuff::Stone, piece))
  .chain(knot)
  .collect()
}

pub(crate) fn cobweb(size: f32) -> Parts {
  let spokes = 7;
  let spoke = |index: usize| {
    let angle = index as f32 / (spokes - 1) as f32 * FRAC_PI_2;
    Vec3::new(angle.cos(), angle.sin(), 0.0) * size
  };
  let radials = (0..spokes).map(|index| {
    (Stuff::Cloth, Piece::new(model::tube(&[Vec3::ZERO, spoke(index)], &[0.006], 3), WEB))
  });
  let threads = (1..6).flat_map(move |loop_index| {
    let reach = loop_index as f32 / 6.0;
    (0..spokes - 1).map(move |index| {
      let (from, to) = (spoke(index) * reach, spoke(index + 1) * reach);
      let sag = (from + to) * 0.5 * 0.9;
      (
        Stuff::Cloth,
        Piece::new(model::tube(&model::curve(from, sag, to, 3), &[0.004], 3), WEB)
      )
    })
  });
  radials.chain(threads).collect()
}

pub(crate) fn rubble(seed: u32, count: usize, spread: f32, scale: f32) -> Parts {
  let mut roll = Roll::new(seed);
  (0..count)
    .map(|index| {
      let size = roll.range(0.12, 0.45) * scale;
      let at = Vec3::new(roll.spread(spread), size * 0.3, roll.spread(spread));
      let color = jitter(&mut roll, WORN);
      let piece = (index % 3 == 0)
        .then(|| {
          Piece::new(model::block(size * 1.6, size, size), color)
            .rolled(roll.spread(0.4))
            .yawed(roll.range(0.0, TAU))
        })
        .unwrap_or_else(|| {
          Piece::new(model::lump(seed + index as u32, 0.25, 1), color)
            .sized(Vec3::new(size, size * 0.7, size))
            .yawed(roll.range(0.0, TAU))
        });
      (Stuff::Stone, piece.at(at))
    })
    .collect()
}

pub(crate) fn flagstones(
  seed: u32,
  low: Vec2,
  high: Vec2,
  floor: f32,
  keep: impl Fn(Vec2) -> bool
) -> Parts {
  let mut roll = Roll::new(seed);
  let size = 1.3;
  let (columns, rows) =
    (((high.x - low.x) / size) as usize, ((high.y - low.y) / size) as usize);
  (0..rows)
    .flat_map(|row| (0..columns).map(move |column| (column, row)))
    .filter_map(|(column, row)| {
      let center = low + (Vec2::new(column as f32, row as f32) + 0.5) * size;
      let width = size - roll.range(0.05, 0.14);
      let length = size - roll.range(0.05, 0.14);
      let color = jitter(&mut roll, WORN);
      let lift = roll.range(0.0, 0.03);
      let tilt = roll.spread(0.012);
      keep(center).then(|| {
        (
          Stuff::Stone,
          Piece::new(model::block(width, 0.1, length), color).rolled(tilt).at(Vec3::new(
            center.x,
            floor - 0.03 + lift,
            center.y
          ))
        )
      })
    })
    .collect()
}

fn stairs(works: &mut Works, top: Vec3, bottom: Vec3, width: f32, count: usize) {
  let tread = (bottom - top) / count as f32;
  let mut roll = Roll::new(count as u32 * 7 + width as u32);
  for step in 0..count {
    let center = top + tread * (step as f32 + 0.5);
    let color = jitter(&mut roll, WORN);
    works.parts.push((
      Stuff::Stone,
      Piece::new(model::block(width, 1.2, tread.z.abs() + 0.03), color)
        .at(center - Vec3::Y * 0.6)
    ));
  }
  let slope = bottom - top;
  works.solids.push((
    Transform::from_translation((top + bottom) / 2.0 - Vec3::Y * 0.15).with_rotation(
      Quat::from_rotation_arc(Vec3::Z * slope.z.signum(), slope.normalize())
    ),
    Collider::cuboid(width, 0.3, slope.length())
  ));
}

pub(crate) fn chest(size: f32) -> (Parts, Parts) {
  let (width, height, depth) = (0.9, 0.48, 0.56);
  let band = |x: f32| {
    (
      Stuff::Iron,
      Piece::new(model::block(0.07, height + 0.02, depth + 0.02), IRON).at_xyz(
        x,
        height / 2.0,
        0.0
      )
    )
  };
  let body = [
    (
      Stuff::Wood,
      Piece::new(model::block(width, height, depth), OLD_WOOD).at_xyz(
        0.0,
        height / 2.0,
        0.0
      )
    ),
    (
      Stuff::Iron,
      Piece::new(model::block(width + 0.03, 0.06, depth + 0.03), IRON)
        .at_xyz(0.0, 0.03, 0.0)
    ),
    (
      Stuff::Iron,
      Piece::new(model::block(0.14, 0.16, 0.03), IRON).at_xyz(
        0.0,
        height - 0.08,
        depth / 2.0 + 0.01
      )
    ),
    band(-0.3),
    band(0.3),
    band(-width / 2.0 + 0.02),
    band(width / 2.0 - 0.02)
  ];
  let lid_bulge = |x: f32, stuff: Stuff, color: Srgba, grow: f32| {
    (
      stuff,
      Piece::new(Cylinder::new(depth / 2.0 + grow, x).mesh().resolution(16), color)
        .rolled(FRAC_PI_2)
        .sized(Vec3::new(1.0, 0.45, 1.0))
        .at_xyz(0.0, 0.0, depth / 2.0)
    )
  };
  let lid = [
    lid_bulge(width, Stuff::Wood, OLD_WOOD, 0.0),
    lid_bulge(0.07, Stuff::Iron, IRON, 0.012),
    lid_bulge(0.07, Stuff::Iron, IRON, 0.012),
    lid_bulge(0.05, Stuff::Iron, IRON, 0.012),
    lid_bulge(0.05, Stuff::Iron, IRON, 0.012)
  ]
  .into_iter()
  .zip([0.0, -0.3, 0.3, -width / 2.0 + 0.02, width / 2.0 - 0.02])
  .map(|((stuff, piece), x)| (stuff, piece.at_xyz(x, 0.0, 0.0)))
  .collect();
  (
    moved(body.into_iter().collect(), Transform::from_scale(Vec3::splat(size))),
    moved(lid, Transform::from_scale(Vec3::splat(size)))
  )
}

pub(crate) fn nest(seed: u32) -> Parts {
  let mut roll = Roll::new(seed);
  let straw = (0..140)
    .map(|_| {
      let angle = roll.range(0.0, TAU);
      let reach = roll.range(0.2, 0.95);
      let color = jitter(&mut roll, STRAW);
      (
        Stuff::Cloth,
        Piece::new(model::blade(roll.range(0.35, 0.6), 0.035, 0.01, 0.3), color)
          .pitched(roll.range(1.3, 1.6))
          .yawed(roll.range(0.0, TAU))
          .at(Vec3::new(
            angle.cos() * reach,
            0.02 + (1.0 - reach) * 0.02 + reach * 0.12 * roll.next(),
            angle.sin() * reach
          ))
      )
    })
    .collect::<Vec<_>>();
  let fur = (0..2).map(|index| {
    (
      Stuff::Fur,
      Piece::new(model::lump(seed + index, 0.15, 2), PELT)
        .sized(Vec3::new(0.6, 0.07, 0.4))
        .yawed(index as f32 * 1.3 + seed as f32)
        .at_xyz(index as f32 * 0.25 - 0.1, 0.05, index as f32 * 0.2 - 0.1)
    )
  });
  straw.into_iter().chain(fur).chain(bones(seed + 50)).collect()
}

fn word_wall(works: &mut Works, radius: f32) {
  let segments = 13;
  let spread = 65f32.to_radians();
  let chord = radius * 2.0 * spread / (segments - 1) as f32 * 1.06;
  let thick = 0.8;
  let mut roll = Roll::new(77);
  works.wall_at = DAIS + Vec3::new(0.0, 0.0, -radius + thick / 2.0);
  for index in 0..segments {
    let t = index as f32 / (segments - 1) as f32;
    let angle = (t * 2.0 - 1.0) * spread;
    let height = 5.0 - 1.6 * (t * 2.0 - 1.0).powi(2) + roll.spread(0.3);
    let seat = Transform::from_translation(
      DAIS + Vec3::new(angle.sin() * radius, height / 2.0, -angle.cos() * radius)
    )
    .with_rotation(Quat::from_rotation_y(-angle));
    let color = jitter(&mut roll, CARVED);
    let slab = [
      Piece::new(model::block(chord, height, thick), color),
      Piece::new(model::block(chord + 0.02, 0.4, thick + 0.35), WORN).at_xyz(
        0.0,
        -height / 2.0 + 0.2,
        0.0
      ),
      Piece::new(model::block(chord + 0.04, 0.22, thick + 0.12), WORN)
        .rolled(roll.spread(0.06))
        .at_xyz(0.0, height / 2.0 + 0.08, 0.0)
    ];
    works.wall.extend(slab.map(|piece| (Stuff::Stone, piece.moved(seat))));
    works.solid(seat, Vec3::new(chord, height, thick));
    let rows = ((height - 1.0) / 0.42) as usize;
    for column in 0..2 {
      for row in 0..rows {
        let glowing = (5..=7).contains(&index) && row == 3 && column == 1 - index % 2;
        let stuff = glowing.then_some(Stuff::Frost).unwrap_or(Stuff::Stone);
        let cell = Vec3::new(
          (column as f32 - 0.5) * chord * 0.46,
          -height / 2.0 + 0.72 + row as f32 * 0.42,
          thick / 2.0 + 0.012
        );
        let strokes = glyph(&mut roll);
        works.wall.extend(strokes.into_iter().map(|(offset, tilt, length)| {
          (
            stuff,
            Piece::new(model::block(0.045, length, 0.03), RUNE)
              .rolled(tilt)
              .at(cell + offset.extend(0.0))
              .moved(seat)
          )
        }));
      }
    }
  }
  works.wall.extend(moved(
    dragon_crest(),
    Transform::from_translation(DAIS + Vec3::new(0.0, 5.1, -radius + 0.1))
      .with_scale(Vec3::splat(1.3))
  ));
}

fn dragon_crest() -> Parts {
  moved(medallion(0.55), Transform::from_xyz(0.0, 0.2, 0.45))
    .into_iter()
    .chain([-1.0, 1.0].map(|side| {
      tusk(
        Vec3::new(side * 0.4, 0.0, 0.3),
        Vec3::new(side * 1.3, 0.3, 0.4),
        Vec3::new(side * 1.2, 1.1, 0.2),
        0.14
      )
    }))
    .collect()
}

fn glyph(roll: &mut Roll) -> Vec<(Vec2, f32, f32)> {
  let spine =
    (Vec2::new(roll.spread(0.05), 0.0), roll.spread(0.35), roll.range(0.22, 0.32));
  let chevron = roll.chance(0.6).then(|| {
    let flip = roll.chance(0.5).then_some(1.0).unwrap_or(-1.0);
    [
      (Vec2::new(0.06, 0.03 * flip), 0.7 * flip, 0.17),
      (Vec2::new(-0.06, 0.03 * flip), -0.7 * flip, 0.17)
    ]
  });
  let dot = roll.chance(0.5).then(|| (Vec2::new(roll.spread(0.1), -0.13), 0.0, 0.05));
  let bar = roll.chance(0.35).then(|| (Vec2::new(0.0, 0.15), FRAC_PI_2, 0.17));
  let slash = roll.chance(0.4).then(|| (Vec2::new(0.09, 0.0), 0.5, 0.22));
  [spine]
    .into_iter()
    .chain(chevron.into_iter().flatten())
    .chain(dot)
    .chain(bar)
    .chain(slash)
    .collect()
}

fn barrow_carve() -> Tree {
  let (x, y, z) = Tree::axes();
  let floor = ((z.clone() - f64::from(DESCENT_TOP)) * f64::from(DESCENT_SLOPE)
    + f64::from(DOOR_FLOOR))
  .min(f64::from(DOOR_FLOOR))
  .max(f64::from(HALL_FLOOR))
    - 0.3;
  let corridor = sdf::cuboid(Vec3::new(1.7, 2.25, 12.0)).remap_xyz(
    x.clone(),
    y.clone() - floor - 2.25,
    z.clone() - 26.0
  );
  let hall = sdf::at(
    sdf::cuboid(Vec3::new(5.5, 2.75, 9.0)),
    Vec3::new(0.0, HALL_FLOOR + 2.75, 6.0)
  );
  let vault = sdf::at(
    sdf::along_z(sdf::cylinder(5.5, 9.0)).remap_xyz(
      x.clone(),
      y.clone() / 0.6,
      z.clone()
    ) * 0.6,
    Vec3::new(0.0, HALL_FLOOR + 5.5, 6.0)
  );
  let alcoves = [0.0, 4.0, 8.0, 12.0].into_iter().flat_map(|along| {
    [-1.0, 1.0].into_iter().flat_map(move |side| {
      [
        sdf::at(
          sdf::cuboid(Vec3::new(1.0, 1.35, 0.95)),
          Vec3::new(side * 6.3, HALL_FLOOR + 1.65, along)
        ),
        sdf::at(
          sdf::along_x(sdf::cylinder(0.95, 1.0)),
          Vec3::new(side * 6.3, HALL_FLOOR + 3.0, along)
        )
      ]
    })
  });
  let passage = sdf::at(
    sdf::cuboid(Vec3::new(2.0, 2.2, 2.2)),
    Vec3::new(0.0, HALL_FLOOR + 2.2, -4.0)
  );
  let rotunda = sdf::at(sdf::cylinder(8.5, 3.0), CHAMBER + Vec3::Y * 3.0);
  let dome = sdf::at(sdf::ellipsoid(Vec3::new(8.5, 7.5, 8.5)), CHAMBER + Vec3::Y * 5.5);
  sdf::union([corridor, hall, vault, passage, rotunda, dome].into_iter().chain(alcoves))
}

fn crags(list: &[(Vec3, Vec3)]) -> impl Iterator<Item = Tree> {
  list.iter().map(|&(center, radii)| sdf::at(sdf::ellipsoid(radii), center))
}

fn barrow_solid() -> Tree {
  let (x, y, z) = Tree::axes();
  let hill = sdf::at(
    sdf::rounded_box(Vec3::new(37.0, 22.0, 38.0), 15.0),
    Vec3::new(0.0, 4.0, -2.0)
  );
  let massif = sdf::smooth_unions(
    [hill].into_iter().chain(crags(&[
      (Vec3::new(-14.0, 26.0, -8.0), Vec3::new(9.0, 19.0, 8.0)),
      (Vec3::new(12.0, 24.0, 4.0), Vec3::new(8.0, 12.0, 7.0)),
      (Vec3::new(3.0, 30.0, -18.0), Vec3::new(12.0, 20.0, 10.0)),
      (Vec3::new(-7.0, 36.0, -15.0), Vec3::new(5.0, 18.0, 5.0)),
      (Vec3::new(-24.0, 28.0, -22.0), Vec3::new(6.0, 18.0, 7.0)),
      (Vec3::new(22.0, 20.0, -18.0), Vec3::new(9.0, 14.0, 9.0)),
      (Vec3::new(1.0, 22.0, 21.0), Vec3::new(12.0, 15.0, 10.0)),
      (Vec3::new(-16.0, 7.0, 32.0), Vec3::new(7.0, 14.0, 8.0)),
      (Vec3::new(16.0, 5.0, 31.0), Vec3::new(7.0, 11.0, 8.0)),
      (Vec3::new(-30.0, 4.0, 14.0), Vec3::new(10.0, 12.0, 12.0)),
      (Vec3::new(30.0, 2.0, 12.0), Vec3::new(10.0, 10.0, 12.0))
    ])),
    5.0
  );
  let bay = z - 35.5 - x.square() * 0.06;
  massif.max(bay).max(-(y + 14.0))
}

fn den_carve() -> Tree {
  let (x, y, z) = Tree::axes();
  let floor = ((z.clone() - f64::from(DEN_MOUTH)) * f64::from(DEN_SLOPE))
    .min(0.15)
    .max(f64::from(DEN_FLOOR));
  let wiggle = x.clone() - (z.clone() * 0.25).sin() * 1.2;
  let tunnel = (((wiggle / 2.5).square()
    + ((y.clone() - floor.clone() - 1.0) / 2.5).square())
  .sqrt()
    - 1.0)
    * 2.5;
  let tunnel =
    tunnel.max(z.clone() - 31.0).max(-(z.clone() - 7.0)).max(floor - y.clone());
  let cavern = sdf::smooth_unions(
    crags(&[
      (Vec3::new(0.0, DEN_FLOOR + 1.5, -1.0), Vec3::new(10.0, 6.5, 9.0)),
      (Vec3::new(-8.0, DEN_FLOOR + 0.8, -7.0), Vec3::new(4.5, 3.2, 4.5)),
      (Vec3::new(5.0, DEN_FLOOR + 2.5, 3.0), Vec3::new(5.0, 5.0, 5.0)),
      (Vec3::new(-5.0, DEN_FLOOR + 2.0, 5.0), Vec3::new(5.0, 4.0, 4.0))
    ]),
    2.0
  );
  let cavern = cavern
    .max(f64::from(DEN_FLOOR) - y.clone())
    .max((x.square() + z.square()).sqrt() - 13.2);
  let shaft = sdf::at(sdf::cylinder(1.3, 30.0), Vec3::new(SHAFT.x, 28.0, SHAFT.y));
  sdf::union([sdf::smooth_union(tunnel, cavern, 2.0), shaft])
}

fn den_solid() -> Tree {
  let (_, y, _) = Tree::axes();
  let mound = sdf::at(
    sdf::rounded_box(Vec3::new(26.5, 12.5, 26.5), 10.0),
    Vec3::new(0.0, 1.5, -1.0)
  );
  sdf::smooth_unions(
    [mound].into_iter().chain(crags(&[
      (Vec3::new(6.0, 12.0, -8.0), Vec3::new(9.0, 9.0, 8.0)),
      (Vec3::new(-9.0, 10.0, -2.0), Vec3::new(7.0, 8.0, 9.0)),
      (Vec3::new(-4.0, 8.0, 18.0), Vec3::new(8.0, 7.0, 6.0)),
      (Vec3::new(12.0, 6.0, 14.0), Vec3::new(7.0, 6.0, 7.0))
    ])),
    4.0
  )
  .max(-(y + 12.0))
}

pub(crate) fn field(at: Vec3, scale: f32, seed: u32) -> Vec3 {
  let at = at / scale;
  Vec3::new(
    noise::fbm3(at, 3, seed),
    noise::fbm3(at + Vec3::splat(31.7), 3, seed + 1),
    noise::fbm3(at + Vec3::splat(67.3), 3, seed + 2)
  )
}

fn hew(
  solid: Tree,
  carve: Tree,
  bounds: Bounds,
  level: f32,
  lining: &Lining
) -> (Mesh, Collider) {
  let surface = sdf::surface(sdf::difference(solid, carve.clone()), &bounds).refined(1.3);
  let probed = sdf::probe(carve, &surface.vertices);
  let warped = Surface {
    vertices: surface
      .vertices
      .iter()
      .zip(&probed)
      .map(|(&at, &(reach, slope))| {
        let outer = smooth(0.4, 2.5, reach);
        let crag =
          field(at, 16.0, 3) * 7.0 + field(at, 6.0, 5) * 4.0 + field(at, 1.5, 9) * 0.8;
        let floor = smooth(0.5, 0.8, -slope.y);
        let inner = (field(at, 4.0, 11) * 2.0 + field(at, 1.6, 13))
          * lining.roughen
          * Vec3::new(1.0, 1.0 - floor, 1.0);
        at + crag * outer + inner * (1.0 - outer)
      })
      .collect(),
    triangles: surface.triangles
  };
  let tones: Vec<[f32; 4]> = warped
    .vertices
    .iter()
    .map(|&at| {
      [(7.0, 3, 21), (2.0, 2, 23), (4.0, 3, 25), (30.0, 2, 27)]
        .map(|(scale, octaves, seed)| noise::fbm3(at / scale, octaves, seed))
    })
    .collect();
  let mesh = warped.mesh(0.12, lining.crease, |index, normal| {
    let at = warped.vertices[index];
    let (reach, _) = probed[index];
    let [grain, speckle, lichen, drift] = tones[index];
    (reach < 0.8)
      .then(|| {
        let base = (normal.y > 0.6).then_some(lining.floor).unwrap_or(lining.wall);
        base.mix(
          &ROCK_DARK,
          smooth(-0.2, 0.3, grain) * 0.5 + smooth(0.3, -0.8, normal.y) * 0.25
        )
      })
      .unwrap_or_else(|| {
        let rock = ROCK_LIGHT.mix(&ROCK_DARK, smooth(-0.3, 0.3, grain + speckle * 0.3));
        let moss =
          smooth(0.55, 0.85, normal.y + lichen * 0.6) * (1.0 - smooth(18.0, 34.0, at.y));
        let snow_line = 150.0 + 30.0 * drift;
        let snow = smooth(0.55, 0.8, normal.y)
          * smooth(snow_line, snow_line + 25.0, at.y + level)
            .max(smooth(26.0, 38.0, at.y));
        rock.mix(&MOSS, moss * 0.85).mix(&SNOW, snow)
      })
  });
  let Surface { vertices, triangles } = warped;
  (
    mesh,
    Collider::trimesh(
      vertices,
      triangles.into_iter().map(|corners| corners.map(|index| index as u32)).collect()
    )
  )
}

fn frame(place: Place, toward: Vec2) -> Transform {
  let spot = place.spot();
  let out = (toward - spot).normalize();
  Transform::from_translation(spot.extend(terrain::height_at(spot) + place.sunk()).xzy())
    .with_rotation(Quat::from_rotation_y(f32::atan2(out.x, out.y)))
}

fn boulders(
  works: &mut Works,
  frame: Transform,
  seed: u32,
  count: usize,
  near: f32,
  far: f32,
  gap: f32
) {
  let mut roll = Roll::new(seed);
  for index in 0..count {
    let angle = roll.range(gap, TAU - gap) + FRAC_PI_2;
    let reach = roll.range(near, far);
    let size = roll.range(1.2, 3.8);
    let local = Vec2::new(angle.cos(), angle.sin()) * reach;
    let world = frame.transform_point(local.extend(0.0).xzy());
    let ground = terrain::height_at(world.xz()) - frame.translation.y;
    works.parts.push((
      Stuff::Stone,
      Piece::new(model::lump(seed * 31 + index as u32, 0.3, 2), jitter(&mut roll, WORN))
        .sized(Vec3::new(size, size * roll.range(0.5, 0.8), size * roll.range(0.7, 1.1)))
        .yawed(roll.range(0.0, TAU))
        .at(Vec3::new(local.x, ground - size * 0.25, local.y))
    ));
  }
}

fn ground(frame: Transform, at: Vec2) -> f32 {
  terrain::height_at(frame.transform_point(at.extend(0.0).xzy()).xz())
    - frame.translation.y
}

fn barrow_front(works: &mut Works, frame: Transform) {
  works.slab(
    WORN,
    Vec3::new(16.4, 7.0, 6.2),
    Transform::from_xyz(0.0, DOOR_FLOOR - 3.5, 37.0)
  );
  works.parts.extend(flagstones(
    3,
    Vec2::new(-7.8, 34.2),
    Vec2::new(7.8, 40.0),
    DOOR_FLOOR + 0.02,
    |_| true
  ));
  let (rise, run, width) = (0.3, 0.5, 9.0);
  let lowest = |z: f32| {
    [-width / 2.0, 0.0, width / 2.0]
      .map(|x| ground(frame, Vec2::new(x, z)))
      .into_iter()
      .fold(f32::MAX, f32::min)
  };
  let count = (1..90)
    .find(|&step| {
      DOOR_FLOOR - rise * step as f32 <= lowest(40.0 + run * step as f32) + 0.15
    })
    .unwrap_or(90);
  let mut roll = Roll::new(5);
  for step in 0..count {
    let z = 40.0 + run * (step as f32 + 0.5);
    let top = DOOR_FLOOR - rise * (step as f32 + 0.5);
    let depth = top - lowest(z) + 1.5;
    works.parts.push((
      Stuff::Stone,
      Piece::new(model::block(width, depth, run + 0.03), jitter(&mut roll, WORN)).at_xyz(
        0.0,
        top - depth / 2.0,
        z
      )
    ));
  }
  let (top, bottom) = (
    Vec3::new(0.0, DOOR_FLOOR, 40.0),
    Vec3::new(0.0, DOOR_FLOOR - rise * count as f32, 40.0 + run * count as f32)
  );
  let slope = bottom - top;
  let tilt = Quat::from_rotation_arc(Vec3::Z, slope.normalize());
  works.solid(
    Transform::from_translation((top + bottom) / 2.0 - Vec3::Y * 0.15)
      .with_rotation(tilt),
    Vec3::new(width, 0.3, slope.length())
  );
  for side in [-1.0, 1.0] {
    works.slab(
      CARVED,
      Vec3::new(0.8, 5.0, slope.length()),
      Transform::from_translation(
        (top + bottom) / 2.0 + Vec3::new(side * (width / 2.0 + 0.4), -1.6, 0.0)
      )
      .with_rotation(tilt)
    );
    works.put(
      pillar(2.2, 1.0),
      Transform::from_translation(
        bottom + Vec3::new(side * (width / 2.0 + 0.4), -0.2, 0.0)
      )
    );
    works.slab(
      CARVED,
      Vec3::new(0.9, 1.2, 5.0),
      Transform::from_xyz(side * 7.7, DOOR_FLOOR + 0.6, 36.5)
    );
    works.put(pillar(11.0, 1.4), Transform::from_xyz(side * 3.3, DOOR_FLOOR, 36.6));
    works.solid(
      Transform::from_xyz(side * 3.3, DOOR_FLOOR + 5.5, 36.6),
      Vec3::new(2.1, 11.0, 2.1)
    );
    works.put(pillar(8.5, 1.1), Transform::from_xyz(side * 7.2, DOOR_FLOOR, 39.0));
    works.solid(
      Transform::from_xyz(side * 7.2, DOOR_FLOOR + 4.25, 39.0),
      Vec3::new(1.7, 8.5, 1.7)
    );
    works.put(medallion(1.2), Transform::from_xyz(side * 5.4, DOOR_FLOOR + 5.5, 35.5));
    works.slab(
      CARVED,
      Vec3::new(3.8, 9.8, 0.9),
      Transform::from_xyz(side * 5.4, DOOR_FLOOR + 4.9, 35.0)
    );
    works.put(
      gable(1.6, DOOR_FLOOR + 9.8, DOOR_FLOOR + 10.8, 0.45, 1.0),
      Transform::from_xyz(side * 5.4, 0.0, 35.2)
    );
    works.parts.push(tusk(
      Vec3::new(side * 3.7, DOOR_FLOOR + 11.0, 36.6),
      Vec3::new(side * 8.8, DOOR_FLOOR + 11.2, 37.8),
      Vec3::new(side * 9.8, DOOR_FLOOR + 17.0, 38.2),
      0.55
    ));
    works.parts.push(tusk(
      Vec3::new(side * 7.3, DOOR_FLOOR + 8.5, 39.0),
      Vec3::new(side * 10.2, DOOR_FLOOR + 8.8, 39.6),
      Vec3::new(side * 11.0, DOOR_FLOOR + 12.5, 39.8),
      0.34
    ));
    works.put(brazier(), Transform::from_xyz(side * 5.0, DOOR_FLOOR, 39.2));
    works.fire(Vec3::new(side * 5.0, DOOR_FLOOR + 1.0, 39.2), 1.2, 60000.0, false);
  }
  works.put(
    gable(3.3, DOOR_FLOOR + 11.0, DOOR_FLOOR + 15.0, 1.1, 1.6),
    Transform::from_xyz(0.0, 0.0, 36.6)
  );
  works.put(
    dragon_head(),
    Transform::from_xyz(0.0, DOOR_FLOOR + 14.4, 37.2).with_scale(Vec3::splat(2.4))
  );
  works.slab(
    CARVED,
    Vec3::new(5.4, 0.9, 1.3),
    Transform::from_xyz(0.0, DOOR_FLOOR + 4.65, 35.6)
  );
  works.slab(
    WORN,
    Vec3::new(5.2, 6.2, 0.8),
    Transform::from_xyz(0.0, DOOR_FLOOR + 8.1, 35.2)
  );
  works.put(medallion(1.3), Transform::from_xyz(0.0, DOOR_FLOOR + 8.2, 35.62));
  works.put(
    gable(2.2, DOOR_FLOOR + 4.2, DOOR_FLOOR + 5.4, 0.45, 1.5),
    Transform::from_xyz(0.0, 0.0, 35.7)
  );
  for side in [-1.0, 1.0] {
    let (stuff, leaf) = (Stuff::Wood, Piece::new(model::block(1.7, 4.1, 0.14), OLD_WOOD));
    let hinge = Transform::from_xyz(side * 1.7, DOOR_FLOOR + 2.05, 34.6)
      .with_rotation(Quat::from_rotation_y(-side * 1.2));
    works.parts.push((stuff, leaf.at_xyz(-side * 0.85, 0.0, 0.0).moved(hinge)));
    for height in [-1.3, 0.0, 1.3] {
      works.parts.push((
        Stuff::Iron,
        Piece::new(model::block(1.72, 0.12, 0.17), IRON)
          .at_xyz(-side * 0.85, height, 0.0)
          .moved(hinge)
      ));
    }
  }
  let on_ground = |x: f32, z: f32| Vec3::new(x, ground(frame, Vec2::new(x, z)), z);
  works.parts.extend(moved(
    rubble(41, 14, 2.5, 1.6),
    Transform::from_translation(on_ground(-11.0, 42.0))
  ));
  works.parts.extend(moved(
    rubble(43, 10, 2.0, 1.4),
    Transform::from_translation(on_ground(10.5, 44.0))
  ));
  works.slab(
    WORN,
    Vec3::new(4.5, 3.0, 0.9),
    Transform::from_translation(on_ground(-11.5, 40.0))
      .with_rotation(Quat::from_rotation_y(0.4))
  );
  works.slab(
    WORN,
    Vec3::new(2.0, 5.0, 0.9),
    Transform::from_translation(on_ground(-13.4, 39.0))
      .with_rotation(Quat::from_rotation_y(0.4))
  );
  works.slab(
    CARVED,
    Vec3::new(1.0, 1.0, 5.5),
    Transform::from_translation(on_ground(10.2, 45.5) + Vec3::Y * 0.3)
      .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.9, 0.0, 0.08))
  );
  works.put(
    pillar(3.4, 1.0),
    Transform::from_translation(on_ground(12.0, 41.0) - Vec3::Y * 0.3)
  );
  works.put(
    pillar(1.8, 1.0),
    Transform::from_translation(on_ground(-9.5, 44.5) - Vec3::Y * 0.3)
  );
}

fn barrow_corridor(works: &mut Works) {
  let floor_at = |z: f32| {
    ((z - DESCENT_TOP) * DESCENT_SLOPE + DOOR_FLOOR).clamp(HALL_FLOOR, DOOR_FLOOR)
  };
  works.parts.extend(flagstones(
    5,
    Vec2::new(-1.7, 31.0),
    Vec2::new(1.7, 35.0),
    DOOR_FLOOR + 0.02,
    |_| true
  ));
  works
    .solid(Transform::from_xyz(0.0, DOOR_FLOOR - 0.15, 34.0), Vec3::new(3.4, 0.3, 6.0));
  let bottom_z = DESCENT_TOP - (DOOR_FLOOR - HALL_FLOOR) / DESCENT_SLOPE;
  stairs(
    works,
    Vec3::new(0.0, DOOR_FLOOR, DESCENT_TOP),
    Vec3::new(0.0, HALL_FLOOR, bottom_z),
    3.4,
    40
  );
  for z in [33.0, 29.0, 25.0, 21.0, 17.0] {
    let floor = floor_at(z + 0.3).max(floor_at(z - 0.3));
    works.put(rib(1.55, 3.1, 3.95, 0.34, 0.45), Transform::from_xyz(0.0, floor - 0.1, z));
  }
  works.put(
    sconce(),
    Transform::from_xyz(-1.7, floor_at(27.0) + 2.0, 27.0)
      .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
  );
  works.fire(Vec3::new(-1.36, floor_at(27.0) + 2.36, 27.0), 0.5, 45000.0, false);
  works.put(
    sconce(),
    Transform::from_xyz(1.7, floor_at(19.0) + 2.0, 19.0)
      .with_rotation(Quat::from_rotation_y(-FRAC_PI_2))
  );
  works.fire(Vec3::new(1.36, floor_at(19.0) + 2.36, 19.0), 0.5, 45000.0, false);
  works.put(
    cobweb(1.0),
    Transform::from_xyz(-1.65, floor_at(23.0) + 3.1, 23.0)
      .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
  );
  works.put(
    cobweb(0.8),
    Transform::from_xyz(1.65, floor_at(30.0) + 3.1, 30.0)
      .with_rotation(Quat::from_euler(EulerRot::YXZ, -FRAC_PI_2, 0.0, FRAC_PI_2))
  );
  works.rooms.push((
    Vec3::new(-2.2, HALL_FLOOR - 1.0, 13.5),
    Vec3::new(2.2, DOOR_FLOOR + 5.0, 32.5)
  ));
}

fn barrow_hall(works: &mut Works) {
  works.parts.extend(flagstones(
    7,
    Vec2::new(-5.5, -3.0),
    Vec2::new(5.5, 14.6),
    HALL_FLOOR,
    |_| true
  ));
  for z in [-2.0, 2.0, 6.0, 10.0, 14.0] {
    works.put(rib(5.2, 4.7, 7.6, 0.55, 0.6), Transform::from_xyz(0.0, HALL_FLOOR, z));
    for side in [-1.0, 1.0] {
      works.solid(
        Transform::from_xyz(side * 5.2, HALL_FLOOR + 2.4, z),
        Vec3::new(0.6, 4.8, 0.6)
      )
    }
  }
  let mut roll = Roll::new(19);
  for (slot, z) in [0.0, 4.0, 8.0, 12.0].into_iter().enumerate() {
    for side in [-1.0, 1.0] {
      let niche = Vec3::new(side * 6.3, HALL_FLOOR + 0.3, z);
      let facing = Vec3::new(-side, 0.0, 0.0);
      let inward = Transform::from_translation(niche).looking_to(-facing, Vec3::Y);
      works.parts.push((
        Stuff::Stone,
        Piece::new(model::block(0.4, 0.3, 2.0), WORN).at(Vec3::new(
          side * 5.55,
          HALL_FLOOR + 0.15,
          z
        ))
      ));
      let kind = (slot + (side > 0.0) as usize) % 2;
      (kind == 1)
        .then(|| {
          works
            .put(sarcophagus(), inward.with_translation(niche + Vec3::X * side * 0.45));
          works.foe(niche + Vec3::X * side * 0.35, facing, FoeKind::Draugr, true);
        })
        .unwrap_or_else(|| {
          works.put(
            urn(slot as u32 * 2 + (side > 0.0) as u32),
            Transform::from_translation(niche + Vec3::new(side * 0.4, 0.0, -0.45))
          );
          works.put(
            urn(slot as u32 * 2 + 9),
            Transform::from_translation(niche + Vec3::new(side * 0.5, 0.0, 0.5))
          );
          works.put(
            bones(slot as u32 + 30),
            Transform::from_translation(niche + Vec3::new(side * 0.1, 0.0, 0.0))
          );
          works.put(
            cobweb(0.9),
            Transform::from_translation(niche + Vec3::new(side * 0.9, 2.1, -0.95))
              .with_rotation(Quat::from_rotation_y(roll.spread(0.2)))
          );
        });
    }
  }
  works.put(tomb(), Transform::from_xyz(0.0, HALL_FLOOR, 5.5));
  works.solid(Transform::from_xyz(0.0, HALL_FLOOR + 0.5, 5.5), Vec3::new(1.3, 1.0, 2.6));
  for side in [-1.0, 1.0] {
    works.put(brazier(), Transform::from_xyz(side * 2.4, HALL_FLOOR, 1.2));
    works.solid(
      Transform::from_xyz(side * 2.4, HALL_FLOOR + 0.5, 1.2),
      Vec3::new(0.8, 1.0, 0.8)
    );
    works.fire(Vec3::new(side * 2.4, HALL_FLOOR + 1.0, 1.2), 1.1, 160000.0, side < 0.0);
    works.put(
      sconce(),
      Transform::from_xyz(side * 4.9, HALL_FLOOR + 2.4, 13.4)
        .with_rotation(Quat::from_rotation_y(-side * FRAC_PI_2))
    );
    works.fire(Vec3::new(side * 4.56, HALL_FLOOR + 2.76, 13.4), 0.5, 40000.0, false);
  }
  works
    .parts
    .extend(moved(rubble(51, 16, 1.4, 1.2), Transform::from_xyz(4.3, HALL_FLOOR, -1.8)));
  works
    .parts
    .extend(moved(rubble(53, 9, 1.0, 1.0), Transform::from_xyz(-4.4, HALL_FLOOR, 9.8)));
  works.parts.push((
    Stuff::Stone,
    Piece::new(model::block(0.55, 3.2, 0.6), CARVED)
      .rolled(1.35)
      .yawed(0.3)
      .at(Vec3::new(3.0, HALL_FLOOR + 0.35, -0.8))
  ));
  works.put(bones(61), Transform::from_xyz(-1.2, HALL_FLOOR, 9.5));
  works.put(bones(62), Transform::from_xyz(1.6, HALL_FLOOR, -0.5));
  for (x, z, side) in [(-4.9, 10.0, 1.0), (4.9, 2.0, -1.0), (4.9, 14.0, -1.0)].into_iter()
  {
    works.put(
      cobweb(1.3),
      Transform::from_xyz(x, HALL_FLOOR + 4.6, z + 0.3).with_rotation(Quat::from_euler(
        EulerRot::YXZ,
        0.0,
        0.0,
        (side < 0.0) as u8 as f32 * FRAC_PI_2
      ))
    );
  }
  works.foe(Vec3::new(0.0, HALL_FLOOR, -0.8), Vec3::Z, FoeKind::Draugr, false);
  works.rooms.push((Vec3::new(-7.6, HALL_FLOOR - 1.0, -6.3), Vec3::new(7.6, 2.0, 15.5)));
}

fn barrow_chamber(works: &mut Works) {
  works.put(rib(1.9, 4.0, 5.2, 0.6, 0.8), Transform::from_xyz(0.0, HALL_FLOOR, -3.4));
  works.parts.extend(flagstones(
    9,
    Vec2::new(-8.5, -20.5),
    Vec2::new(8.5, -3.5),
    HALL_FLOOR,
    |at| at.distance(CHAMBER.xz()) < 8.2 && at.distance(DAIS.xz()) > 6.0
  ));
  let mut roll = Roll::new(29);
  for (radius, top) in
    [(6.4, HALL_FLOOR + 0.4), (5.7, HALL_FLOOR + 0.8), (5.0, DAIS.y)].into_iter()
  {
    let height = top - HALL_FLOOR + 0.5;
    works.parts.push((
      Stuff::Stone,
      Piece::new(
        Cylinder::new(radius, height).mesh().resolution(56),
        jitter(&mut roll, WORN)
      )
      .at(Vec3::new(DAIS.x, top - height / 2.0, DAIS.z))
    ));
  }
  let rim = |radius: f32, y: f32| {
    (0..24).map(move |index| {
      let angle = index as f32 / 24.0 * TAU;
      DAIS.with_y(y) + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius
    })
  };
  works.solids.push((
    Transform::IDENTITY,
    Collider::convex_hull(rim(6.6, HALL_FLOOR - 0.2).chain(rim(5.0, DAIS.y)).collect())
      .expect("dais hull")
  ));
  word_wall(works, 4.0);
  let toward_room = |at: Vec3| (CHAMBER + Vec3::Z * 6.0 - at).with_y(0.0).normalize();
  let crypt = DAIS + Vec3::new(-3.5, 0.0, 1.2);
  works.put(
    sarcophagus(),
    Transform::from_translation(crypt).looking_to(-toward_room(crypt), Vec3::Y)
  );
  works.foe(
    crypt + toward_room(crypt) * 0.1,
    toward_room(crypt),
    FoeKind::DraugrOverlord,
    true
  );
  let hoard = DAIS + Vec3::new(3.1, 0.0, 1.5);
  works.hoards.push(Hoard {
    at: Transform::from_translation(hoard).looking_to(-toward_room(hoard), Vec3::Y),
    size: 1.0,
    noun: "Ancient Nord Chest",
    loot: HOLLOWCRAG_LOOT
  });
  for side in [-1.0, 1.0] {
    let foot = Vec3::new(side * 4.6, HALL_FLOOR, -8.6);
    works.put(brazier(), Transform::from_translation(foot));
    works
      .solid(Transform::from_translation(foot + Vec3::Y * 0.5), Vec3::new(0.8, 1.0, 0.8));
    works.fire(foot + Vec3::Y * 1.0, 1.1, 160000.0, side > 0.0);
    let angle = side * 1.75;
    let wall = CHAMBER + Vec3::new(angle.sin() * 8.45, 2.4, -angle.cos() * 8.45);
    let inward = (CHAMBER - wall).with_y(0.0).normalize();
    works.put(sconce(), Transform::from_translation(wall).looking_to(-inward, Vec3::Y));
    works.fire(wall + inward * 0.34 + Vec3::Y * 0.36, 0.5, 40000.0, false);
    works.put(
      urn(70 + side as u32),
      Transform::from_translation(DAIS + Vec3::new(side * 5.3, -0.4, 3.2))
    );
    works.put(
      urn(80 + side as u32),
      Transform::from_translation(DAIS + Vec3::new(side * 5.9, -0.8, 2.2))
    );
  }
  works.lamps.push((
    DAIS + Vec3::new(0.0, 2.0, -1.8),
    Color::srgb(0.45, 0.7, 1.0),
    120000.0,
    14.0
  ));
  works.put(bones(91), Transform::from_translation(DAIS + Vec3::new(-1.6, 0.0, 2.6)));
  works.put(bones(92), Transform::from_xyz(3.5, HALL_FLOOR, -5.5));
  works
    .parts
    .extend(moved(rubble(93, 18, 1.6, 1.3), Transform::from_xyz(-6.4, HALL_FLOOR, -7.0)));
  works.put(
    cobweb(1.5),
    Transform::from_xyz(-1.8, HALL_FLOOR + 3.95, -3.0)
      .with_rotation(Quat::from_rotation_z(0.0))
  );
  works.put(
    cobweb(1.2),
    Transform::from_xyz(1.8, HALL_FLOOR + 3.95, -3.0)
      .with_rotation(Quat::from_rotation_z(FRAC_PI_2))
  );
  works.foe(
    Vec3::new(-2.0, HALL_FLOOR, -7.0),
    Vec3::new(0.3, 0.0, 1.0),
    FoeKind::Draugr,
    false
  );
  works.rooms.push((Vec3::new(-9.0, HALL_FLOOR - 1.0, -21.0), Vec3::new(9.0, 6.0, -3.0)));
}

fn barrow_works(frame: Transform) -> Works {
  let mut works = Works::default();
  barrow_front(&mut works, frame);
  barrow_corridor(&mut works);
  barrow_hall(&mut works);
  barrow_chamber(&mut works);
  boulders(&mut works, frame, 3, 26, 36.0, 48.0, 0.45);
  works
}

fn den_works(frame: Transform) -> Works {
  let mut works = Works::default();
  let floor = DEN_FLOOR + 0.05;
  for (at, seed) in [
    (Vec3::new(-6.0, floor, -5.0), 1),
    (Vec3::new(4.0, floor, -7.0), 2),
    (Vec3::new(-2.0, floor, 3.5), 3)
  ]
  .into_iter()
  {
    works.put(nest(seed), Transform::from_translation(at));
    works.foe(
      at + Vec3::new(0.6, 0.0, 0.6),
      Vec3::new(0.2, 0.0, 1.0),
      FoeKind::Wolf,
      false
    );
  }
  works.foe(
    Vec3::new(0.3, (18.0 - DEN_MOUTH) * DEN_SLOPE + 0.1, 18.0),
    Vec3::Z,
    FoeKind::Wolf,
    false
  );
  for index in 0..12 {
    let angle = index as f32 * 2.1;
    let reach = 3.0 + (index % 4) as f32 * 2.0;
    works.put(
      bones(200 + index),
      Transform::from_xyz(angle.cos() * reach, floor, angle.sin() * reach - 1.0)
    );
  }
  works.put(bones(230), Transform::from_xyz(0.8, (14.0 - DEN_MOUTH) * DEN_SLOPE, 14.0));
  let satchel = Vec3::new(-8.8, floor, -8.2);
  works.hoards.push(Hoard {
    at: Transform::from_translation(satchel)
      .looking_to(-Vec3::new(1.0, 0.0, 0.8).normalize(), Vec3::Y),
    size: 0.7,
    noun: "Weathered Chest",
    loot: FELLHOUND_LOOT
  });
  works.put(bones(240), Transform::from_translation(satchel + Vec3::new(1.0, 0.0, 0.4)));
  works
    .parts
    .extend(moved(rubble(250, 20, 3.0, 1.8), Transform::from_xyz(7.0, floor, 2.0)));
  works.parts.extend(moved(
    rubble(251, 12, 1.5, 1.4),
    Transform::from_xyz(-1.5, (12.0 - DEN_MOUTH) * DEN_SLOPE, 12.0)
  ));
  works.beams.push((Vec3::new(SHAFT.x, 13.0, SHAFT.y), 13.0 - DEN_FLOOR));
  works.lamps.push((
    Vec3::new(0.0, DEN_FLOOR + 3.5, -1.0),
    Color::srgb(0.7, 0.8, 1.0),
    500000.0,
    26.0
  ));
  works.lamps.push((
    Vec3::new(0.0, -1.5, 15.0),
    Color::srgb(0.85, 0.85, 0.9),
    120000.0,
    16.0
  ));
  works.rooms.push((Vec3::new(-4.5, DEN_FLOOR - 1.0, 7.0), Vec3::new(4.5, 4.0, 22.0)));
  works
    .rooms
    .push((Vec3::new(-14.0, DEN_FLOOR - 1.0, -14.5), Vec3::new(14.0, 8.0, 10.0)));
  boulders(&mut works, frame, 7, 14, 25.0, 34.0, 0.5);
  works
}

#[derive(Component)]
pub(crate) struct Hollow {
  pub(crate) rooms: Vec<(Vec3, Vec3)>
}

#[derive(Component)]
struct Flicker {
  lumens: f32,
  seed: f32
}

#[derive(Component)]
struct Flame {
  seed: f32,
  size: f32
}

#[derive(Component)]
struct Chest {
  noun: &'static str,
  loot: &'static [Loot],
  opened: bool
}

#[derive(Component)]
struct Lid {
  openness: f32
}

fn grouped(parts: Parts) -> Vec<(Stuff, Mesh)> {
  parts
    .into_iter()
    .fold(Vec::<(Stuff, Vec<Piece>)>::new(), |mut groups, (stuff, piece)| {
      if let Some((_, pieces)) = groups.iter_mut().find(|(each, _)| *each == stuff) {
        pieces.push(piece);
      } else {
        groups.push((stuff, vec![piece]));
      }
      groups
    })
    .into_iter()
    .map(|(stuff, pieces)| (stuff, stuff.fitted(model::merge(pieces))))
    .collect()
}

pub(crate) struct Kit<'a, 'w, 's> {
  pub(crate) commands: &'a mut Commands<'w, 's>,
  pub(crate) meshes: &'a mut Assets<Mesh>,
  pub(crate) materials: &'a mut Assets<StandardMaterial>,
  pub(crate) stuffs: &'a Stuffs,
  pub(crate) surfaces: &'a Surfaces
}

impl Kit<'_, '_, '_> {
  fn dress(&mut self, parent: Entity, parts: Parts) {
    for (stuff, mesh) in grouped(parts) {
      self.commands.spawn((
        Mesh3d(self.meshes.add(mesh)),
        MeshMaterial3d(self.stuffs.of(stuff)),
        Transform::IDENTITY,
        ChildOf(parent)
      ));
    }
  }

  pub(crate) fn raise(
    &mut self,
    name: &'static str,
    frame: Transform,
    (rock, collider): (Mesh, Collider),
    works: Works
  ) -> (Entity, Vec<Entity>) {
    let Works { parts, wall, wall_at, solids, fires, lamps, beams, foes, hoards, rooms } =
      works;
    let root = self
      .commands
      .spawn((
        Name::new(name),
        Hollow { rooms },
        RigidBody::Static,
        frame,
        Visibility::default()
      ))
      .id();
    self.commands.spawn((
      Name::new("Rock"),
      Mesh3d(self.meshes.add(rock)),
      MeshMaterial3d(self.surfaces.rock.clone()),
      collider,
      Transform::IDENTITY,
      ChildOf(root)
    ));
    self.dress(root, parts);
    for (at, collider) in solids {
      self.commands.spawn((collider, at, ChildOf(root)));
    }
    (!wall.is_empty()).then(|| {
      let entity = self
        .commands
        .spawn((
          Name::new("Word Wall"),
          WordWall,
          frame.mul_transform(Transform::from_translation(wall_at)),
          Visibility::default()
        ))
        .id();
      self.dress(entity, moved(wall, Transform::from_translation(-wall_at)));
    });
    let flame = self.meshes.add(flame());
    for (index, Fire { at, size, lumens, shadows }) in fires.into_iter().enumerate() {
      let seed = index as f32 * 1.7 + at.x;
      let fire = self
        .commands
        .spawn((Transform::from_translation(at), Visibility::default(), ChildOf(root)))
        .id();
      self.commands.spawn((
        Flame { seed, size },
        Mesh3d(flame.clone()),
        MeshMaterial3d(self.stuffs.of(Stuff::Ember)),
        Transform::from_scale(Vec3::splat(size)),
        ChildOf(fire)
      ));
      let mut light = self.commands.spawn((
        Flicker { lumens: lumens * FIRE_BOOST, seed },
        PointLight {
          color: Color::srgb(1.0, 0.62, 0.3),
          intensity: lumens * FIRE_BOOST,
          range: 8.0 + lumens.sqrt() * 0.06,
          radius: 0.1 * size,
          ..default()
        },
        crate::humanoid::shadowing(),
        Transform::from_xyz(0.0, 0.45 * size, 0.0),
        ChildOf(fire)
      ));
      if shadows {
        light.insert(CloseShadows);
      }
    }
    for (at, color, lumens, range) in lamps {
      self.commands.spawn((
        PointLight {
          color,
          intensity: lumens * LAMP_BOOST,
          range: range * 1.2,
          ..default()
        },
        Transform::from_translation(at),
        ChildOf(root)
      ));
    }
    for (top, length) in beams {
      self.commands.spawn((
        SpotLight {
          color: Color::srgb(1.0, 0.95, 0.85),
          intensity: 4.0e6,
          range: length + 6.0,
          outer_angle: 0.2,
          inner_angle: 0.08,
          ..default()
        },
        Transform::from_translation(top).looking_to(Vec3::NEG_Y, Vec3::X),
        ChildOf(root)
      ));
      self.commands.spawn((
        Mesh3d(
          self.meshes.add(Cone { radius: 1.5, height: length }.mesh().resolution(20))
        ),
        MeshMaterial3d(self.materials.add(StandardMaterial {
          base_color: Color::srgba(1.0, 0.95, 0.8, 0.025),
          unlit: true,
          alpha_mode: AlphaMode::Add,
          double_sided: true,
          cull_mode: None,
          ..default()
        })),
        Transform::from_translation(top - Vec3::Y * length / 2.0),
        ChildOf(root)
      ));
    }
    let spawned = foes
      .into_iter()
      .map(|(at, kind, dormant)| {
        self.commands.spawn((FoeSpawn { kind, dormant }, frame.mul_transform(at))).id()
      })
      .collect();
    for Hoard { at, size, noun, loot } in hoards {
      let (body, lid) = chest(size);
      let entity = self
        .commands
        .spawn((
          Name::new(noun),
          Chest { noun, loot, opened: false },
          RigidBody::Static,
          Collider::cuboid(0.9 * size, 0.5 * size, 0.56 * size),
          at,
          Visibility::default(),
          ChildOf(root)
        ))
        .id();
      let shell = self
        .commands
        .spawn((
          Transform::from_xyz(0.0, -0.25 * size, 0.0),
          Visibility::default(),
          ChildOf(entity)
        ))
        .id();
      self.dress(shell, body);
      let hinge = self
        .commands
        .spawn((
          Lid { openness: 0.0 },
          Transform::from_xyz(0.0, 0.48 * size, -0.28 * size),
          Visibility::default(),
          ChildOf(shell)
        ))
        .id();
      self.dress(hinge, lid);
    }
    (root, spawned)
  }
}

fn raise_hollows(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  stuffs: Res<Stuffs>,
  surfaces: Res<Surfaces>
) {
  let barrow = frame(Place::HOLLOWCRAG, place::ROAD[place::ROAD.len() - 1]);
  let den = frame(Place::FELLHOUND, place::TRAIL[place::TRAIL.len() - 1]);
  let (barrow_rock, den_rock) = crate::par::scope(|scope| {
    let barrow_job = scope.spawn(|| {
      hew(
        barrow_solid(),
        barrow_carve(),
        Bounds { center: Vec3::new(0.0, 16.0, 0.0), half_extent: 54.0, depth: 8 },
        barrow.translation.y,
        &MASONRY
      )
    });
    let den_job = scope.spawn(|| {
      hew(
        den_solid(),
        den_carve(),
        Bounds { center: Vec3::new(0.0, 8.0, 0.0), half_extent: 36.0, depth: 7 },
        den.translation.y,
        &BURROW
      )
    });
    (barrow_job.join().expect("barrow meshing"), den_job.join().expect("den meshing"))
  });
  let mut kit = Kit {
    commands: &mut commands,
    meshes: &mut meshes,
    materials: &mut materials,
    stuffs: &stuffs,
    surfaces: &surfaces
  };
  kit.raise("Hollowcrag Barrow", barrow, barrow_rock, barrow_works(barrow));
  kit.raise("Fellhound Den", den, den_rock, den_works(den));
}

fn shelter(
  time: Res<Time>,
  player: Single<&Transform, With<Player>>,
  hollows: Query<(&Transform, &Hollow), Without<Player>>,
  mut daylight: ResMut<Daylight>
) {
  let at = player.translation;
  let inside = hollows.iter().any(|(frame, hollow)| {
    let local = frame.compute_affine().inverse().transform_point3(at);
    hollow
      .rooms
      .iter()
      .any(|&(low, high)| local.cmpge(low).all() && local.cmple(high).all())
  });
  let pace =
    (daylight.snap > 0).then_some(1.0).unwrap_or(1.0 - (-1.3 * time.delta_secs()).exp());
  daylight.shelter = daylight.shelter.lerp(inside as u8 as f32, pace);
}

fn flicker(
  time: Res<Time>,
  mut lights: Query<(&Flicker, &mut PointLight)>,
  mut flames: Query<(&Flame, &mut Transform)>
) {
  let now = time.elapsed_secs();
  let wave = |seed: f32| {
    0.82
      + 0.1 * (now * 9.3 + seed).sin()
      + 0.06 * (now * 23.1 + seed * 3.1).sin()
      + 0.12 * noise::perlin(Vec2::new(now * 5.0, seed * 7.0), 3)
  };
  for (&Flicker { lumens, seed }, mut light) in lights.iter_mut() {
    light.intensity = lumens * wave(seed)
  }
  for (&Flame { seed, size }, mut transform) in flames.iter_mut() {
    let pulse = wave(seed + 0.4);
    transform.scale = Vec3::new(
      size * (1.6 - pulse * 0.6),
      size * pulse * 1.15,
      size * (1.6 - pulse * 0.6)
    );
    transform.rotation = Quat::from_rotation_y(now * 1.7 + seed);
  }
}

fn open_chests(
  keys: Res<ButtonInput<KeyCode>>,
  view: Res<View>,
  player: Single<(&Transform, &mut Inventory), With<Player>>,
  mut chests: Query<(&GlobalTransform, &mut Chest)>,
  mut prompt: ResMut<Prompt>,
  mut notices: MessageWriter<Notice>,
  mut sounds: MessageWriter<Sound>
) {
  let (hero, mut inventory) = player.into_inner();
  let feet = hero.translation - Vec3::Y * 0.9;
  let forward = view.flat_forward();
  if let Some((spot, mut chest)) = chests
    .iter_mut()
    .filter(|(_, chest)| !chest.opened)
    .map(|(place, chest)| (place.translation(), chest))
    .find(|&(spot, _)| {
      let gap = spot - feet;
      let flat = gap.with_y(0.0);
      flat.length() < 2.2
        && gap.y.abs() < 1.5
        && flat.normalize_or_zero().dot(forward) > 0.45
    })
  {
    prompt.0 = Some(Prompting { verb: "Open".into(), noun: chest.noun.into() });
    if keys.just_pressed(KeyCode::KeyE) {
      chest.opened = true;
      for &loot in chest.loot.iter() {
        notices.write(Notice(loot.to_string()));
        inventory.take(loot);
      }
      sounds.write(Sound::here(Cue::ChestOpen, spot));
    }
  }
}

fn swing_lids(
  time: Res<Time>,
  chests: Query<&Chest>,
  parents: Query<&ChildOf>,
  mut lids: Query<(Entity, &mut Lid, &mut Transform)>
) {
  for (entity, mut lid, mut transform) in lids.iter_mut() {
    let opened = parents
      .iter_ancestors(entity)
      .find_map(|ancestor| chests.get(ancestor).ok())
      .is_some_and(|chest| chest.opened);
    lid.openness = (lid.openness
      + time.delta_secs() * 1.6 * if opened { 1.0 } else { -1.0 })
    .clamp(0.0, 1.0);
    transform.rotation = Quat::from_rotation_x(-1.9 * smooth(0.0, 1.0, lid.openness));
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(Startup, raise_hollows)
    .add_systems(Update, (shelter, flicker, open_chests, swing_lids));
}
