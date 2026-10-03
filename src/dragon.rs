use {crate::{combat::{Dead, Fighter, Shake, Side, Struck, Vitals},
             fx::Effects,
             humanoid::Motion,
             model::{self, Piece, ball, cone, curve, sculpt, taper, tube},
             opts::opts,
             player::{Player, View},
             ragdoll::{Limb, Link},
             shout::{self, Staggered, WispLook},
             signal::{Cue, Notice, Shouts, Sound},
             sky::Daylight,
             stuff::{Stuff, Stuffs},
             terrain::{self, BOUND, Ground}},
     avian3d::prelude::*,
     bevy::{color::Mix, prelude::*},
     bevy_hanabi::{EffectProperties, EffectSpawner},
     std::f32::consts::{FRAC_PI_2, TAU}};

const ARRIVAL: f32 = 75.0;
const CRUISE: f32 = 24.0;
const ORBIT: f32 = 75.0;
const ALTITUDE: f32 = 48.0;
const STANCE: f32 = 2.5;
const BREATH_REACH: f32 = 24.0;
const BREATH_DPS: f32 = 16.0;
const BREATH_SPEED: f32 = 26.0;
const SHELTERED: f32 = 0.5;
const HEALTH: f32 = 420.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bone {
  Body,
  Neck1,
  Neck2,
  Neck3,
  Head,
  Jaw,
  Tail1,
  Tail2,
  Tail3,
  Tail4,
  WingL,
  WingR,
  TipL,
  TipR,
  LegL,
  LegR,
  ShinL,
  ShinR
}

const BONES: usize = 18;

impl Bone {
  const ALL: [Bone; BONES] = [
    Bone::Body,
    Bone::Neck1,
    Bone::Neck2,
    Bone::Neck3,
    Bone::Head,
    Bone::Jaw,
    Bone::Tail1,
    Bone::Tail2,
    Bone::Tail3,
    Bone::Tail4,
    Bone::WingL,
    Bone::WingR,
    Bone::TipL,
    Bone::TipR,
    Bone::LegL,
    Bone::LegR,
    Bone::ShinL,
    Bone::ShinR
  ];

  const fn parent(self) -> Option<Bone> {
    match self {
      Bone::Body => None,
      Bone::Neck1 | Bone::Tail1 | Bone::WingL | Bone::WingR | Bone::LegL | Bone::LegR => {
        Some(Bone::Body)
      }
      Bone::Neck2 => Some(Bone::Neck1),
      Bone::Neck3 => Some(Bone::Neck2),
      Bone::Head => Some(Bone::Neck3),
      Bone::Jaw => Some(Bone::Head),
      Bone::Tail2 => Some(Bone::Tail1),
      Bone::Tail3 => Some(Bone::Tail2),
      Bone::Tail4 => Some(Bone::Tail3),
      Bone::TipL => Some(Bone::WingL),
      Bone::TipR => Some(Bone::WingR),
      Bone::ShinL => Some(Bone::LegL),
      Bone::ShinR => Some(Bone::LegR)
    }
  }

  const fn rest(self) -> Vec3 {
    match self {
      Bone::Body => Vec3::ZERO,
      Bone::Neck1 => Vec3::new(0.0, 0.45, -1.5),
      Bone::Neck2 | Bone::Neck3 => Vec3::new(0.0, 0.0, -1.05),
      Bone::Head => Vec3::new(0.0, 0.0, -1.0),
      Bone::Jaw => Vec3::new(0.0, -0.18, -0.25),
      Bone::Tail1 => Vec3::new(0.0, 0.1, 2.9),
      Bone::Tail2 | Bone::Tail3 | Bone::Tail4 => Vec3::new(0.0, 0.0, 1.55),
      Bone::WingL => Vec3::new(-0.85, 0.7, -0.8),
      Bone::WingR => Vec3::new(0.85, 0.7, -0.8),
      Bone::TipL => Vec3::new(-4.0, 0.2, 0.3),
      Bone::TipR => Vec3::new(4.0, 0.2, 0.3),
      Bone::LegL => Vec3::new(-0.75, -0.2, 2.2),
      Bone::LegR => Vec3::new(0.75, -0.2, 2.2),
      Bone::ShinL | Bone::ShinR => Vec3::new(0.0, -1.25, 0.15)
    }
  }

  const fn size(self) -> f32 {
    match self {
      Bone::Head => 1.25,
      _ => 1.0
    }
  }

  const fn right_twin(self) -> Option<Bone> {
    match self {
      Bone::WingR => Some(Bone::WingL),
      Bone::TipR => Some(Bone::TipL),
      Bone::LegR => Some(Bone::LegL),
      Bone::ShinR => Some(Bone::ShinL),
      _ => None
    }
  }
}

pub fn limbs() -> [Option<Limb>; BONES] {
  let along = |radius: f32, reach: Vec3, link: Link| {
    Some(Limb { radius, from: Vec3::ZERO, to: reach, link })
  };
  let spine = Link::Socket { swing: 0.55, twist: 0.4 };
  Bone::ALL.map(|bone| match bone {
    Bone::Body => Some(Limb {
      radius: 0.85,
      from: Vec3::new(0.0, 0.2, -1.4),
      to: Vec3::new(0.0, 0.1, 2.5),
      link: Link::Root
    }),
    Bone::Neck1 => along(0.5, Vec3::Z * -1.0, spine),
    Bone::Neck2 => along(0.42, Vec3::Z * -1.0, spine),
    Bone::Neck3 => along(0.36, Vec3::Z * -0.95, spine),
    Bone::Head => along(0.34, Vec3::Z * -0.8, Link::Socket { swing: 0.7, twist: 0.5 }),
    Bone::Tail1 => along(0.5, Vec3::Z * 1.5, spine),
    Bone::Tail2 => along(0.4, Vec3::Z * 1.5, spine),
    Bone::Tail3 => along(0.3, Vec3::Z * 1.5, spine),
    Bone::Tail4 => along(0.2, Vec3::Z * 1.6, spine),
    Bone::WingL => {
      along(0.28, Vec3::new(-3.8, 0.2, 0.3), Link::Socket { swing: 1.3, twist: 0.8 })
    }
    Bone::WingR => {
      along(0.28, Vec3::new(3.8, 0.2, 0.3), Link::Socket { swing: 1.3, twist: 0.8 })
    }
    Bone::LegL | Bone::LegR => {
      along(0.34, Vec3::new(0.0, -1.2, 0.15), Link::Socket { swing: 1.2, twist: 0.4 })
    }
    Bone::ShinL | Bone::ShinR => {
      along(0.24, Vec3::new(0.0, -1.1, -0.3), Link::Hinge { low: -0.5, high: 1.6 })
    }
    Bone::Jaw | Bone::TipL | Bone::TipR => None
  })
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flight {
  Waiting,
  Roosting(f32),
  Soaring(f32),
  Homing,
  Arriving,
  Circling(f32),
  Strafing { from: Vec3, to: Vec3, low: f32 },
  Landing(Vec3),
  Grounded(f32),
  Rising(f32),
  Posing { aloft: bool },
  Falling,
  Slain(f32)
}

#[derive(Component)]
pub struct Dragon {
  flight: Flight,
  pub bones: [Entity; BONES],
  pub velocity: Vec3,
  flap: f32,
  flap_rate: f32,
  breath: f32,
  breath_cooldown: f32,
  bite_cooldown: f32,
  passes: u32,
  scorch: f32,
  roared: bool,
  spent: bool,
  awake: bool,
  lair: Option<Vec3>,
  parts: Vec<Entity>,
  streamed: u32,
  absorbed: bool
}

impl Dragon {
  pub fn roost(&self) -> Option<Vec2> {
    self
      .lair
      .filter(|_| !matches!(self.flight, Flight::Falling | Flight::Slain(_)))
      .map(|lair| lair.xz())
  }
}

#[derive(Component)]
struct Breath(Entity);

#[derive(Component)]
pub struct FoldsAway;

const fn srgb(red: f32, green: f32, blue: f32) -> Srgba {
  Srgba::new(red, green, blue, 1.0)
}

const SCALE_TILE: f32 = 2.6;
const RIBS: f32 = 3.0;
const FAN_TURN: f32 = 0.47;
const FOLDED_TOWARD: Vec3 = Vec3::new(1.4, 1.6, 0.6);

struct Hide {
  flank: Srgba,
  dorsal: Srgba,
  belly: Srgba
}

impl Hide {
  const BRONZE: Hide = Hide {
    flank: srgb(0.3, 0.33, 0.24),
    dorsal: srgb(0.11, 0.11, 0.09),
    belly: srgb(0.95, 0.93, 0.8)
  };

  fn paint(&self, piece: Piece) -> Piece {
    let (flank, dorsal, belly) = (
      LinearRgba::from(self.flank),
      LinearRgba::from(self.dorsal),
      LinearRgba::from(self.belly)
    );
    piece.shaded(move |position, normal| {
      let under = ((-normal.y - 0.1) * 2.4).clamp(0.0, 1.0);
      let top = ((normal.y - 0.2) * 1.8).clamp(0.0, 1.0);
      let rib = 0.4 + 0.6 * ((position.z * RIBS).rem_euclid(1.0) * 2.5).min(1.0);
      let mottle = crate::noise::value3(position * 1.6, 13) * 0.35 + 0.82;
      (flank * mottle)
        .mix(&(dorsal * mottle), top)
        .mix(&(belly * rib), under)
        .with_alpha(1.0)
    })
  }
}

fn plated(mut mesh: Mesh) -> Mesh {
  let read = |mesh: &Mesh, attribute| -> Vec<Vec3> {
    mesh
      .attribute(attribute)
      .and_then(bevy::mesh::VertexAttributeValues::as_float3)
      .map(|values| values.iter().copied().map(Vec3::from).collect())
      .unwrap_or_default()
  };
  let (positions, normals): (Vec<Vec3>, Vec<Vec3>) =
    read(&mesh, Mesh::ATTRIBUTE_POSITION)
      .into_iter()
      .zip(read(&mesh, Mesh::ATTRIBUTE_NORMAL))
      .map(|(position, normal)| {
        let belly = ((-normal.y - 0.2) * 2.5).clamp(0.0, 1.0);
        let back = ((normal.y - 0.5) * 2.5).clamp(0.0, 1.0);
        let lift = 0.05 * belly * (position.z * RIBS).rem_euclid(1.0)
          + 0.04 * back * (position.z * 2.2).rem_euclid(1.0);
        let slope = 0.05 * RIBS * belly + 0.09 * back;
        (position + normal * lift, (normal - Vec3::Z * slope).normalize_or(normal))
      })
      .unzip();
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
  mesh
}

fn tiled(mut piece: Piece, around: f32, along: f32) -> Piece {
  if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
    piece.0.attribute_mut(Mesh::ATTRIBUTE_UV_0)
  {
    for uv in uvs.iter_mut() {
      *uv = [uv[0] * around.max(0.5).round(), uv[1] * along]
    }
  }
  piece
}

fn mirrored(turn: Quat) -> Quat { Quat::from_xyzw(turn.x, -turn.y, -turn.z, turn.w) }

fn anatomy() -> Vec<(Bone, Stuff, Mesh)> {
  let hide = Hide::BRONZE.flank;
  let horn = srgb(0.38, 0.35, 0.29);
  let spike = srgb(0.2, 0.19, 0.16);
  let claw = srgb(0.16, 0.14, 0.12);
  let key = |x: f32, y: f32, z: f32, wide: f32, high: f32, deep: f32| {
    (Vec3::new(x, y, z), Vec3::new(wide, high, deep))
  };
  let flesh = |keys: &[(Vec3, Vec3)], seed: u32| {
    let length: f32 = keys.windows(2).map(|pair| pair[0].0.distance(pair[1].0)).sum();
    let girth = keys
      .iter()
      .map(|&(_, Vec3 { x, y, z })| (x + (y + z) * 0.5) * 0.5 * TAU)
      .fold(0.0, f32::max);
    let mesh = plated(model::ruffled(sculpt(keys, 12, 24), 0.03, Vec3::splat(2.6), seed));
    Hide::BRONZE.paint(tiled(
      Piece::new(mesh, hide),
      girth / SCALE_TILE,
      length / SCALE_TILE
    ))
  };
  let fins = |from: Vec2, to: Vec2, count: usize, height: f32, shrink: f32| {
    (0..count)
      .map(|index| {
        let t = index as f32 / (count.max(2) - 1) as f32;
        let spot = from.lerp(to, t);
        let tall = height * (1.0 - shrink * t);
        Piece::new(cone(tall * 0.42, tall), spike)
          .sized(Vec3::new(0.28, 1.0, 1.0))
          .pitched(0.8)
          .at_xyz(0.0, spot.y, spot.x)
      })
      .collect::<Vec<_>>()
  };
  let mut parts: Vec<(Bone, Stuff, Vec<Piece>)> = vec![
    (Bone::Body, Stuff::Scales, vec![flesh(
      &[
        key(0.0, 0.5, -2.05, 0.02, 0.02, 0.02),
        key(0.0, 0.48, -1.85, 0.56, 0.5, 0.56),
        key(0.0, 0.36, -1.35, 0.8, 0.66, 0.85),
        key(0.0, 0.2, -0.65, 1.02, 0.8, 1.2),
        key(0.0, 0.1, 0.25, 0.95, 0.74, 1.02),
        key(0.0, 0.08, 1.15, 0.76, 0.64, 0.72),
        key(0.0, 0.12, 2.05, 0.9, 0.72, 0.78),
        key(0.0, 0.12, 2.8, 0.7, 0.6, 0.6),
        key(0.0, 0.1, 3.3, 0.52, 0.46, 0.46),
        key(0.0, 0.1, 3.5, 0.02, 0.02, 0.02)
      ],
      1
    )]),
    (
      Bone::Body,
      Stuff::Bone,
      fins(Vec2::new(-1.6, 0.98), Vec2::new(-0.4, 1.0), 4, 0.75, 0.0)
        .into_iter()
        .chain(fins(Vec2::new(0.1, 0.84), Vec2::new(3.0, 0.72), 7, 0.7, 0.35))
        .collect()
    ),
  ];
  let segment = |length: f32, girth: f32, next: f32, seed: u32| {
    let lap = length.signum() * 0.3;
    let ring =
      |z: f32, radius: f32| key(0.0, 0.0, z, radius, radius * 0.86, radius * 0.94);
    vec![flesh(
      &[
        ring(-lap, girth * 0.05),
        ring(-lap * 0.6, girth * 0.96),
        ring(length * 0.5, (girth + next) * 0.51),
        ring(length, next),
        ring(length + lap * 0.5, next * 0.9),
        ring(length + lap * 0.8, next * 0.05)
      ],
      seed
    )]
  };
  for (index, (bone, girth, next)) in
    [(Bone::Neck1, 0.6, 0.48), (Bone::Neck2, 0.48, 0.4), (Bone::Neck3, 0.4, 0.34)]
      .into_iter()
      .enumerate()
  {
    parts.push((bone, Stuff::Scales, segment(-1.05, girth, next, 10 + index as u32)));
    parts.push((
      bone,
      Stuff::Bone,
      fins(Vec2::new(-0.1, girth * 0.82), Vec2::new(-0.85, next * 0.85), 3, 0.42, 0.2)
    ));
  }
  let crown: Vec<Piece> = (0..11)
    .map(|index| {
      let t = index as f32 / 5.0 - 1.0;
      let angle = t * 1.8;
      let outward = Vec3::new(angle.sin(), angle.cos(), 0.0);
      let long = 0.5 + 0.75 * (1.0 - t.abs()).powf(0.6);
      let root = Vec3::new(outward.x * 0.3, 0.1 + outward.y * 0.24, 0.1);
      let tip = root + (Vec3::Z * 0.9 + outward * 0.85).normalize() * long;
      let bend = root + outward * long * 0.4 + Vec3::Z * long * 0.25;
      Piece::new(
        tube(
          &curve(root, bend, tip, 10),
          &taper(10, 0.05 + 0.05 * (1.0 - t.abs()), 0.005),
          8
        ),
        horn
      )
    })
    .collect();
  let brow_horns: Vec<Piece> = (0..7)
    .map(|index| {
      let t = index as f32 / 3.0 - 1.0;
      let angle = t * 1.4;
      let outward = Vec3::new(angle.sin(), angle.cos(), 0.0);
      let long = 0.28 + 0.2 * (1.0 - t.abs());
      let root = Vec3::new(outward.x * 0.3, 0.12 + outward.y * 0.18, -0.2);
      let tip = root + (Vec3::Z + outward * 0.8).normalize() * long;
      Piece::new(
        tube(
          &curve(root, root.lerp(tip, 0.5) + outward * 0.06, tip, 6),
          &taper(6, 0.045, 0.004),
          7
        ),
        spike
      )
    })
    .collect();
  let cheek_spikes: Vec<Piece> = [1.0, -1.0]
    .into_iter()
    .flat_map(|side| {
      [
        (0.1, -0.02, 0.34),
        (-0.12, -0.06, 0.26),
        (-0.34, -0.08, 0.2),
        (-0.55, -0.08, 0.14)
      ]
      .map(|(z, y, long)| {
        let root = Vec3::new(side * 0.27, y, z);
        let tip = root + Vec3::new(side * 0.55, -0.35, 1.0).normalize() * long;
        Piece::new(
          tube(&curve(root, root.lerp(tip, 0.5), tip, 4), &taper(4, 0.04, 0.004), 6),
          spike
        )
      })
    })
    .collect();
  let chin_spikes: Vec<Piece> = [(0.0, 0.2), (-0.25, 0.16), (-0.5, 0.12)]
    .into_iter()
    .flat_map(|(z, long)| {
      [1.0, -1.0].map(|side| {
        Piece::new(cone(long * 0.25, long), spike)
          .pitched(2.3)
          .rolled(-side * 0.35)
          .at_xyz(side * 0.14, -0.08, z)
      })
    })
    .collect();
  let fangs = |row: f32, down: f32| {
    (0..6)
      .flat_map(|index| {
        let z = -0.5 - index as f32 * 0.1;
        let spread = 0.2 - index as f32 * 0.018;
        let long = (index == 1).then_some(0.2).unwrap_or(0.12);
        [1.0, -1.0].map(|side| {
          Piece::new(cone(0.03, long), srgb(0.9, 0.86, 0.75)).pitched(down).at_xyz(
            side * spread,
            row,
            z
          )
        })
      })
      .collect::<Vec<_>>()
  };
  let brows: Vec<Piece> = [1.0, -1.0]
    .into_iter()
    .map(|side| {
      flesh(
        &[
          key(side * 0.2, 0.26, 0.05, 0.02, 0.02, 0.02),
          key(side * 0.21, 0.27, -0.05, 0.08, 0.07, 0.05),
          key(side * 0.24, 0.26, -0.35, 0.075, 0.06, 0.04),
          key(side * 0.22, 0.22, -0.6, 0.04, 0.03, 0.03),
          key(side * 0.2, 0.2, -0.7, 0.02, 0.02, 0.02)
        ],
        30
      )
    })
    .collect();
  let skull = flesh(
    &[
      key(0.0, 0.08, 0.32, 0.02, 0.02, 0.02),
      key(0.0, 0.1, 0.18, 0.34, 0.3, 0.28),
      key(0.0, 0.1, -0.2, 0.36, 0.3, 0.24),
      key(0.0, 0.07, -0.5, 0.3, 0.24, 0.16),
      key(0.0, 0.03, -0.85, 0.22, 0.17, 0.12),
      key(0.0, 0.0, -1.12, 0.17, 0.13, 0.1),
      key(0.0, -0.01, -1.24, 0.1, 0.08, 0.08),
      key(0.0, -0.02, -1.28, 0.02, 0.02, 0.02)
    ],
    20
  );
  let jaw = flesh(
    &[
      key(0.0, 0.0, 0.42, 0.02, 0.02, 0.02),
      key(0.0, 0.0, 0.3, 0.28, 0.1, 0.16),
      key(0.0, -0.02, -0.15, 0.24, 0.08, 0.13),
      key(0.0, 0.0, -0.65, 0.17, 0.06, 0.09),
      key(0.0, 0.02, -0.92, 0.12, 0.05, 0.06),
      key(0.0, 0.02, -1.0, 0.02, 0.02, 0.02)
    ],
    23
  );
  parts.extend([
    (Bone::Head, Stuff::Scales, [skull].into_iter().chain(brows).collect()),
    (
      Bone::Head,
      Stuff::Bone,
      crown
        .into_iter()
        .chain(brow_horns)
        .chain(cheek_spikes)
        .chain(fangs(-0.07, std::f32::consts::PI))
        .collect()
    ),
    (Bone::Head, Stuff::Ember, vec![
      Piece::new(ball(0.055), srgb(1.0, 0.7, 0.2)).at_xyz(0.25, 0.17, -0.42),
      Piece::new(ball(0.055), srgb(1.0, 0.7, 0.2)).at_xyz(-0.25, 0.17, -0.42),
    ]),
    (Bone::Jaw, Stuff::Scales, vec![jaw]),
    (Bone::Jaw, Stuff::Bone, fangs(0.06, 0.0).into_iter().chain(chin_spikes).collect())
  ]);
  for (index, (bone, girth, next)) in [
    (Bone::Tail1, 0.64, 0.46),
    (Bone::Tail2, 0.46, 0.32),
    (Bone::Tail3, 0.32, 0.2),
    (Bone::Tail4, 0.2, 0.05)
  ]
  .into_iter()
  .enumerate()
  {
    parts.push((bone, Stuff::Scales, segment(1.55, girth, next, 40 + index as u32)));
    parts.push((
      bone,
      Stuff::Bone,
      fins(
        Vec2::new(0.15, girth * 0.82),
        Vec2::new(1.35, next * 0.85),
        3,
        0.46 - index as f32 * 0.09,
        0.2
      )
    ));
  }
  parts.push((Bone::Tail4, Stuff::Bone, vec![
    Piece::new(
      model::fan(
        &[
          Vec2::new(0.0, 0.1),
          Vec2::new(0.3, -0.25),
          Vec2::new(0.58, -0.62),
          Vec2::new(0.22, -0.62),
          Vec2::new(0.0, -1.45),
          Vec2::new(-0.22, -0.62),
          Vec2::new(-0.58, -0.62),
          Vec2::new(-0.3, -0.25)
        ],
        0.06
      ),
      spike
    )
    .pitched(-FRAC_PI_2)
    .at_xyz(0.0, 0.0, 1.3),
  ]));
  let arm_spine = model::spline(
    &[
      Vec3::new(-0.3, -0.1, 0.0),
      Vec3::ZERO,
      Vec3::new(0.95, 0.28, 0.35),
      Vec3::new(1.8, 0.34, 0.55),
      Vec3::new(3.0, 0.26, 0.42),
      Vec3::new(4.0, 0.2, 0.3),
      Vec3::new(4.15, 0.2, 0.3)
    ],
    6
  );
  let arm_girth = model::spline(
    &[
      Vec3::splat(0.36),
      Vec3::new(0.34, 0.38, 0.4),
      Vec3::new(0.26, 0.3, 0.3),
      Vec3::new(0.2, 0.2, 0.2),
      Vec3::new(0.14, 0.15, 0.15),
      Vec3::new(0.16, 0.17, 0.17),
      Vec3::splat(0.02)
    ],
    6
  );
  let wing_arm = Hide::BRONZE.paint(tiled(
    Piece::new(model::sweep(&arm_spine, &arm_girth, 14), hide),
    1.0,
    4.5 / SCALE_TILE
  ));
  let finger_tips = [
    Vec3::new(3.1, 0.0, 1.0),
    Vec3::new(2.2, 0.0, 2.9),
    Vec3::new(0.8, 0.0, 3.9),
    Vec3::new(-1.0, 0.0, 4.2)
  ];
  let finger_path = |tip: Vec3| curve(Vec3::ZERO, tip * 0.5 + Vec3::Y * 0.15, tip, 10);
  let strut = hide * 0.62;
  let fingers: Vec<Piece> = finger_tips
    .iter()
    .flat_map(|&tip| {
      let path = finger_path(tip);
      let knuckles = [0.3, 0.6].map(|t| {
        Piece::new(ball(0.085 * (1.25 - t)), strut).at(path[(t * 10.0) as usize])
      });
      let talon = Piece::new(cone(0.035, 1.0), claw)
        .span(tip, tip + (tip - path[8]).normalize() * 0.35);
      [Piece::new(tube(&path, &taper(10, 0.12, 0.025), 8), strut), talon]
        .into_iter()
        .chain(knuckles)
    })
    .chain([
      Piece::new(cone(0.07, 0.4), claw).pitched(-1.4).at_xyz(0.05, 0.12, -0.2),
      Piece::new(ball(0.2), strut)
    ])
    .collect();
  let skin = LinearRgba::from(srgb(0.52, 0.54, 0.44));
  let webbing = move |grid: Vec<Vec<Vec3>>| {
    Piece::new(model::sheet(&grid), srgb(0.52, 0.54, 0.44)).shaded(move |position, _| {
      let reach = position.length();
      let veins = (f32::atan2(position.z, position.x) * 11.0
        + crate::noise::value3(position * 0.8, 31) * 3.0)
        .sin()
        .abs()
        .powf(16.0);
      let blotch = crate::noise::value3(position * 1.7, 29) * 0.3 + 0.82;
      (skin * blotch * (1.0 - reach * 0.035).max(0.7) * (1.0 - veins * 0.4))
        .with_alpha(1.0)
    })
  };
  let across =
    |columns: usize| (0..=columns).map(move |column| column as f32 / columns as f32);
  let membranes: Vec<Piece> = finger_tips
    .windows(2)
    .map(|pair| {
      let (lead, trail) = (finger_path(pair[0]), finger_path(pair[1]));
      webbing(
        lead
          .iter()
          .zip(&trail)
          .enumerate()
          .map(|(row, (&lead, &trail))| {
            let along = row as f32 / 10.0;
            across(10)
              .map(|s| {
                let bow = (s * std::f32::consts::PI).sin();
                lead.lerp(trail, s) * (1.0 - 0.34 * bow * along.powi(2))
                  - Vec3::Y * 0.16 * bow * along
              })
              .collect()
          })
          .collect()
      )
    })
    .collect();
  let fan_frame = Quat::from_rotation_y(-FAN_TURN);
  let wrist = Bone::TipR.rest();
  let (root, far) = (Vec3::new(0.2, -0.05, 3.3), wrist + finger_tips[3]);
  let inner = webbing(
    arm_spine[6..arm_spine.len() - 6]
      .iter()
      .enumerate()
      .map(|(row, &lead)| {
        let along = row as f32 / (arm_spine.len() - 13) as f32;
        let edge = root.lerp(far, along)
          + Vec3::NEG_Z * 0.7 * (along * std::f32::consts::PI).sin();
        across(10)
          .map(|s| lead.lerp(edge, s) - Vec3::Y * 0.2 * (s * std::f32::consts::PI).sin())
          .collect()
      })
      .collect()
  );
  let thigh = flesh(
    &[
      key(0.0, 0.45, 0.05, 0.02, 0.02, 0.02),
      key(0.0, 0.3, 0.05, 0.36, 0.42, 0.46),
      key(0.0, -0.2, 0.12, 0.38, 0.44, 0.48),
      key(0.0, -0.8, 0.14, 0.24, 0.26, 0.28),
      key(0.0, -1.25, 0.15, 0.2, 0.2, 0.2),
      key(0.0, -1.4, 0.15, 0.02, 0.02, 0.02)
    ],
    40
  );
  let shin = flesh(
    &[
      key(0.0, 0.14, 0.0, 0.02, 0.02, 0.02),
      key(0.0, 0.0, 0.0, 0.2, 0.2, 0.2),
      key(0.0, -0.45, 0.32, 0.14, 0.16, 0.13),
      key(0.0, -0.8, 0.22, 0.12, 0.12, 0.12),
      key(0.0, -1.02, 0.08, 0.14, 0.12, 0.12),
      key(0.0, -1.1, -0.05, 0.12, 0.1, 0.1),
      key(0.0, -1.14, -0.22, 0.02, 0.02, 0.02)
    ],
    41
  );
  let toes: Vec<Piece> = [-0.16, 0.0, 0.16]
    .into_iter()
    .map(|x| {
      Piece::new(
        tube(
          &curve(
            Vec3::new(x * 0.4, -1.04, 0.0),
            Vec3::new(x, -1.08, -0.25),
            Vec3::new(x * 1.3, -1.14, -0.45),
            6
          ),
          &taper(6, 0.08, 0.05),
          8
        ),
        hide
      )
    })
    .collect();
  let claws: Vec<Piece> = [-0.16, 0.0, 0.16]
    .into_iter()
    .map(|x| {
      Piece::new(cone(0.05, 0.22), claw).pitched(-1.75).at_xyz(x * 1.3, -1.15, -0.5)
    })
    .collect();
  let shoulder = flesh(
    &[
      key(-0.3, 0.1, 0.05, 0.02, 0.02, 0.02),
      key(-0.2, 0.1, 0.05, 0.38, 0.38, 0.4),
      key(0.35, 0.18, 0.15, 0.34, 0.3, 0.34),
      key(0.7, 0.24, 0.25, 0.02, 0.02, 0.02)
    ],
    45
  );
  let in_fan = |pieces: Vec<Piece>| -> Vec<Piece> {
    pieces.into_iter().map(|piece| piece.turned(fan_frame)).collect()
  };
  let right_side: Vec<(Bone, Stuff, Vec<Piece>)> = vec![
    (Bone::WingR, Stuff::Scales, vec![wing_arm, shoulder]),
    (Bone::WingR, Stuff::Membrane, vec![inner]),
    (Bone::TipR, Stuff::Leather, in_fan(fingers)),
    (Bone::TipR, Stuff::Membrane, in_fan(membranes)),
    (Bone::LegR, Stuff::Scales, vec![thigh]),
    (Bone::ShinR, Stuff::Scales, [shin].into_iter().chain(toes).collect()),
    (Bone::ShinR, Stuff::Bone, claws),
  ];
  for (bone, stuff, pieces) in right_side {
    let mirrored: Vec<Piece> = pieces.iter().map(Piece::mirrored).collect();
    parts.push((bone.right_twin().unwrap_or(bone), stuff, mirrored));
    parts.push((bone, stuff, pieces));
  }
  parts
    .into_iter()
    .map(|(bone, stuff, pieces)| (bone, stuff, stuff.fitted(model::merge(pieces))))
    .collect()
}

#[derive(Resource)]
struct Anatomy(Vec<(Bone, Stuff, Handle<Mesh>)>);

fn build(
  commands: &mut Commands,
  anatomy: &Anatomy,
  stuffs: &Stuffs,
  owner: Entity
) -> ([Entity; BONES], Vec<Entity>) {
  let mut bones = [Entity::PLACEHOLDER; BONES];
  for bone in Bone::ALL {
    let parent = bone.parent().map_or(owner, |parent| bones[parent as usize]);
    bones[bone as usize] = commands
      .spawn((
        Transform::from_translation(bone.rest()).with_scale(Vec3::splat(bone.size())),
        Visibility::Inherited,
        ChildOf(parent)
      ))
      .id();
  }
  let parts = anatomy
    .0
    .iter()
    .map(|(bone, stuff, mesh)| {
      let part = commands
        .spawn((
          Mesh3d(mesh.clone()),
          MeshMaterial3d(stuffs.of(*stuff)),
          ChildOf(bones[*bone as usize])
        ))
        .id();
      if matches!(bone, Bone::WingL | Bone::WingR) && *stuff == Stuff::Membrane {
        commands.entity(part).insert(FoldsAway);
      }
      part
    })
    .collect();
  (bones, parts)
}

const LAIRS: usize = 7;
#[cfg(test)]
const LAIR_APART: f32 = 1300.0;
#[cfg(test)]
const LAIR_CLEAR_OF_START: f32 = 900.0;
const WAKE: f32 = 260.0;
const SNEAK_WAKE: f32 = 130.0;
const LEASH: f32 = 850.0;
const SOAR_RADIUS: f32 = 170.0;
const SOAR_HEIGHT: f32 = 90.0;

pub const LAIR_SPOTS: [Vec2; LAIRS] = [
  Vec2::new(960.0, -2592.0),
  Vec2::new(-240.0, -2064.0),
  Vec2::new(2256.0, -2736.0),
  Vec2::new(1392.0, -1248.0),
  Vec2::new(2640.0, -768.0),
  Vec2::new(-2688.0, 2544.0),
  Vec2::new(-432.0, -672.0)
];

#[cfg(test)]
fn highest_peaks() -> Vec<Vec2> {
  let step = 48.0;
  let span = (BOUND - 200.0) / step;
  let cells = (-span as i32..=span as i32).flat_map(|x| {
    (-span as i32..=span as i32).map(move |z| Vec2::new(x as f32, z as f32) * step)
  });
  let mut peaks: Vec<(Vec2, f32)> = cells
    .map(|at| (at, terrain::natural_height(at)))
    .filter(|&(at, height)| {
      height > 300.0
        && at.distance(crate::place::START) > LAIR_CLEAR_OF_START
        && (0..8).all(|index| {
          terrain::natural_height(at + Vec2::from_angle(index as f32 / 8.0 * TAU) * step)
            <= height
        })
    })
    .collect();
  peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
  peaks.into_iter().fold(Vec::new(), |mut kept, (at, _)| {
    if kept.len() < LAIRS
      && kept.iter().all(|other: &Vec2| other.distance(at) > LAIR_APART)
    {
      kept.push(at);
    }
    kept
  })
}

#[cfg(test)]
mod tests {
  #[test]
  fn lairs_on_the_highest_peaks() {
    assert_eq!(
      super::highest_peaks(),
      super::LAIR_SPOTS,
      "terrain changed: paste these into LAIR_SPOTS"
    );
  }
}

fn spawn_dragon(
  commands: &mut Commands,
  anatomy: &Anatomy,
  stuffs: &Stuffs,
  effects: &Effects,
  lair: Option<Vec3>,
  seed: u32
) -> Entity {
  let mut roll = crate::noise::Roll::new(seed);
  let start = lair.map_or(Transform::from_xyz(0.0, -500.0, 0.0), |lair| {
    Transform::from_translation(lair + Vec3::Y * STANCE)
      .with_rotation(Quat::from_rotation_y(roll.range(0.0, TAU)))
  });
  let dragon = commands
    .spawn((
      Name::new("Dragon"),
      Side::Wild,
      Vitals::new(HEALTH, 200.0),
      Motion::default(),
      Fighter { reach: 7.5, damage: 26.0, swing_time: 1.1, cone: 0.6, girth: 3.0 },
      RigidBody::Kinematic,
      Collider::sphere(2.0),
      start,
      lair.map_or(Visibility::Hidden, |_| Visibility::Inherited)
    ))
    .id();
  let (bones, parts) = build(commands, anatomy, stuffs, dragon);
  commands.spawn((
    Breath(dragon),
    effects.emit(&effects.breath),
    EffectSpawner::new(&bevy_hanabi::SpawnerSettings::rate(420.0.into()))
      .with_active(false),
    EffectProperties::default(),
    Transform::from_xyz(0.0, -0.05, -1.3),
    ChildOf(bones[Bone::Head as usize])
  ));
  commands.entity(dragon).insert(Dragon {
    flight: lair
      .map(|_| {
        roll
          .chance(0.5)
          .then(|| Flight::Roosting(roll.range(0.0, 60.0)))
          .unwrap_or(Flight::Soaring(roll.range(0.0, 30.0)))
      })
      .unwrap_or(Flight::Waiting),
    bones,
    velocity: Vec3::ZERO,
    flap: 0.0,
    flap_rate: 2.0,
    breath: 0.0,
    breath_cooldown: 4.0,
    bite_cooldown: 2.0,
    passes: 0,
    scorch: 0.0,
    roared: false,
    spent: false,
    awake: lair.is_none(),
    lair,
    parts,
    streamed: 0,
    absorbed: false
  });
  dragon
}

fn spawn_dragons(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  effects: Res<Effects>
) {
  let anatomy = Anatomy(
    anatomy()
      .into_iter()
      .map(|(bone, stuff, mesh)| (bone, stuff, meshes.add(mesh)))
      .collect()
  );
  spawn_dragon(&mut commands, &anatomy, &stuffs, &effects, None, 0);
  let lairs = opts().foe.as_deref().is_none_or(|foe| !foe.starts_with("dragon"));
  for (index, &lair) in LAIR_SPOTS.iter().enumerate().filter(|_| lairs) {
    let lair = lair.extend(terrain::height_at(lair)).xzy();
    spawn_dragon(
      &mut commands,
      &anatomy,
      &stuffs,
      &effects,
      Some(lair),
      index as u32 + 1
    );
  }
}

pub fn specimen(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  effects: &Effects,
  holders: &[Entity],
  aloft: bool
) {
  let anatomy = Anatomy(
    anatomy()
      .into_iter()
      .map(|(bone, stuff, mesh)| (bone, stuff, meshes.add(mesh)))
      .collect()
  );
  let lift = Vec3::Y * aloft.then_some(14.0).unwrap_or(STANCE);
  for &holder in holders {
    let dragon = spawn_dragon(commands, &anatomy, stuffs, effects, None, 0);
    commands
      .entity(dragon)
      .insert((Transform::from_translation(lift), Visibility::Inherited, ChildOf(holder)))
      .entry::<Dragon>()
      .and_modify(move |mut dragon| dragon.flight = Flight::Posing { aloft });
  }
}

const LATERAL_PULL: f32 = 9.0;
const GRAVITY: f32 = 9.8;

fn steer_toward(
  velocity: Vec3,
  goal: Vec3,
  at: Vec3,
  speed: f32,
  agility: f32,
  delta: f32
) -> Vec3 {
  let wanted = (goal - at).normalize_or_zero();
  let current = velocity.normalize_or(wanted);
  let pace = velocity.length().lerp(speed, 1.0 - (-agility * 0.8 * delta).exp());
  let most = (agility * LATERAL_PULL / pace.max(4.0)).min(3.0) * delta;
  let angle = current.angle_between(wanted);
  let axis = current.cross(wanted).try_normalize().unwrap_or(Vec3::Y);
  let turned = (angle > 1e-4)
    .then(|| Quat::from_axis_angle(axis, angle.min(most)) * current)
    .unwrap_or(current);
  turned * pace
}

fn fly(
  time: Res<Time>,
  ground: Res<Ground>,
  daylight: Res<Daylight>,
  view: Res<View>,
  stealth: Res<crate::combat::Stealth>,
  mut sounds: MessageWriter<Sound>,
  mut struck: MessageWriter<Struck>,
  mut shake: ResMut<Shake>,
  player: Single<
    (Entity, &Transform, Has<Dead>, &Motion),
    (With<Player>, Without<Dragon>)
  >,
  mut player_vitals: Query<&mut Vitals, (With<Player>, Without<Dragon>)>,
  mut dragons: Query<
    (
      Entity,
      &mut Dragon,
      &mut Transform,
      &mut Visibility,
      &mut Motion,
      &Vitals,
      Has<Dead>,
      Has<Staggered>
    ),
    Without<Player>
  >
) {
  let delta = time.delta_secs().min(0.05);
  let (hero, hero_at, hero_dead, hero_motion) = *player;
  let target = hero_at.translation;
  let hidden = daylight.shelter > SHELTERED;
  let exposed = !hero_dead && !hidden;
  let arrival = opts().dragon.unwrap_or(ARRIVAL);
  for (
    entity,
    mut dragon,
    mut transform,
    mut visibility,
    mut motion,
    vitals,
    dead,
    staggered
  ) in dragons.iter_mut()
  {
    let at = transform.translation;
    let before = dragon.velocity;
    let floor = |spot: Vec3| ground.height(spot.xz());
    let flat_gap = (target - at).with_y(0.0);
    let distance = flat_gap.length();
    dragon.breath_cooldown -= delta;
    dragon.bite_cooldown -= delta;
    dragon.breath = (dragon.breath - delta).max(0.0);
    let aloft = at.y - floor(at) > STANCE + 1.5;
    let wake = stealth.sneaking.then_some(SNEAK_WAKE).unwrap_or(WAKE);
    let stirred = exposed && distance < wake;
    let strayed = dragon.lair.is_some_and(|lair| {
      hero_dead || hidden || (target - lair).with_y(0.0).length() > LEASH
    });
    let flight = match dragon.flight {
      Flight::Falling | Flight::Slain(_) => dragon.flight,
      _ if dead && aloft => Flight::Falling,
      _ if dead => Flight::Slain(0.0),
      Flight::Arriving
      | Flight::Circling(_)
      | Flight::Strafing { .. }
      | Flight::Landing(_)
      | Flight::Grounded(_)
        if strayed =>
      {
        dragon.awake = false;
        matches!(dragon.flight, Flight::Grounded(_))
          .then_some(Flight::Rising(0.0))
          .unwrap_or(Flight::Homing)
      }
      other => other
    };
    let mut rouse = |dragon: &mut Dragon| {
      dragon.awake = true;
      dragon.roared = true;
      sounds.write(Sound::here(Cue::DragonRoar, at));
    };
    let next = match flight {
      Flight::Roosting(_) if stirred => {
        rouse(&mut dragon);
        Flight::Rising(0.0)
      }
      Flight::Roosting(rested) => {
        dragon.velocity = Vec3::ZERO;
        (rested > 80.0)
          .then_some(Flight::Rising(0.0))
          .unwrap_or(Flight::Roosting(rested + delta))
      }
      Flight::Soaring(_) | Flight::Homing if stirred => {
        rouse(&mut dragon);
        Flight::Arriving
      }
      Flight::Soaring(soared) => {
        let lair = dragon.lair.unwrap_or(at);
        let bearing = f32::atan2(at.z - lair.z, at.x - lair.x) + 0.3;
        let ahead = Vec3::new(bearing.cos(), 0.0, bearing.sin()) * SOAR_RADIUS + lair;
        let goal = ahead.with_y(lair.y + SOAR_HEIGHT);
        dragon.velocity =
          steer_toward(dragon.velocity, goal, at, CRUISE * 0.9, 1.0, delta);
        (soared > 50.0)
          .then_some(Flight::Homing)
          .unwrap_or(Flight::Soaring(soared + delta))
      }
      Flight::Homing => {
        let lair = dragon.lair.unwrap_or(at);
        let horizontal = (lair - at).with_y(0.0).length();
        let goal = (horizontal > 12.0)
          .then(|| lair + Vec3::Y * (STANCE + 14.0 + (horizontal * 0.15).min(60.0)))
          .unwrap_or(lair + Vec3::Y * STANCE);
        let speed = (horizontal * 0.8 + 4.0).min(CRUISE);
        dragon.velocity = steer_toward(dragon.velocity, goal, at, speed, 1.5, delta);
        let touched = at.y - (floor(at) + STANCE) < 0.6 && horizontal < 4.0;
        touched
          .then(|| {
            dragon.velocity = Vec3::ZERO;
            Flight::Roosting(0.0)
          })
          .unwrap_or(Flight::Homing)
      }
      Flight::Waiting
        if let Some(rest) =
          opts().foe.as_deref().and_then(|foe| foe.strip_prefix("dragon"))
          && time.elapsed_secs() > 1.0 =>
      {
        let aloft = rest == "aloft";
        if rest == "slain" {
          struck.write(Struck {
            target: entity,
            attacker: hero,
            damage: HEALTH * 2.0,
            power: true,
            blocked: false,
            at
          });
        }
        let ahead = target + view.flat_forward() * aloft.then_some(26.0).unwrap_or(13.0);
        transform.translation =
          ground.surface(ahead.xz()) + Vec3::Y * aloft.then_some(14.0).unwrap_or(STANCE);
        transform.look_to(view.flat_forward().cross(Vec3::Y), Vec3::Y);
        *visibility = Visibility::Inherited;
        Flight::Posing { aloft }
      }
      Flight::Posing { .. } => {
        dragon.breath = (opts().foe.as_deref() == Some("dragonfire"))
          .then_some(1.0)
          .unwrap_or(dragon.breath);
        flight
      }
      Flight::Waiting => {
        *visibility = Visibility::Hidden;
        let ready = time.elapsed_secs() > arrival && daylight.shelter < 0.3;
        ready
          .then(|| {
            let away = Vec3::new(1.0, 0.0, -0.6).normalize() * 420.0;
            transform.translation = target + away + Vec3::Y * 120.0;
            *visibility = Visibility::Inherited;
            dragon.velocity = -away.normalize() * CRUISE;
            Flight::Arriving
          })
          .unwrap_or(Flight::Waiting)
      }
      Flight::Arriving => {
        let goal = target + Vec3::Y * ALTITUDE;
        dragon.velocity =
          steer_toward(dragon.velocity, goal, at, CRUISE * 1.2, 0.8, delta);
        if !dragon.roared && distance < 260.0 {
          dragon.roared = true;
          sounds.write(Sound::here(Cue::DragonRoar, at));
        }
        (distance < ORBIT * 1.1)
          .then_some(Flight::Circling(0.0))
          .unwrap_or(Flight::Arriving)
      }
      Flight::Circling(circled) => {
        let bearing = f32::atan2(at.z - target.z, at.x - target.x) + 0.35;
        let ahead = Vec3::new(bearing.cos(), 0.0, bearing.sin()) * ORBIT + target;
        let goal = ahead.with_y(floor(ahead).max(floor(target)) + ALTITUDE);
        dragon.velocity = steer_toward(dragon.velocity, goal, at, CRUISE, 1.4, delta);
        if circled > 10.0 && exposed {
          dragon.passes += 1;
          let heading = flat_gap.normalize_or(Vec3::X);
          let (from, to) = (target - heading * 110.0, target + heading * 120.0);
          if dragon.passes % 3 == 0 {
            Flight::Landing(ground.surface((target - heading * 16.0).xz()))
          } else {
            Flight::Strafing { from, to, low: 11.0 }
          }
        } else {
          Flight::Circling(circled + delta)
        }
      }
      Flight::Strafing { from, to, low } => {
        let along = (to - from).with_y(0.0).normalize_or_zero();
        let progress = (at - from).with_y(0.0).dot(along);
        let pass_height = floor(at).max(floor(target)) + low;
        let lookahead = from + along * (progress + 40.0);
        let goal =
          lookahead.with_y(pass_height + (progress - 110.0).abs().min(60.0) * 0.25);
        dragon.velocity =
          steer_toward(dragon.velocity, goal, at, CRUISE * 1.1, 1.6, delta);
        let facing_player =
          dragon.velocity.normalize_or_zero().dot((target - at).normalize_or_zero())
            > 0.6;
        if exposed && facing_player && distance < 60.0 && distance > 8.0 {
          if dragon.breath <= 0.0 {
            sounds.write(Sound::here(Cue::FireBreath, at));
          }
          dragon.breath = 0.3;
        }
        (hidden || progress > (to - from).with_y(0.0).length())
          .then_some(Flight::Circling(0.0))
          .unwrap_or(flight)
      }
      Flight::Landing(spot) => {
        let high = spot + Vec3::Y * (STANCE + 14.0);
        let horizontal = (spot - at).with_y(0.0).length();
        let goal = if horizontal > 12.0 { high } else { spot + Vec3::Y * STANCE };
        let speed = (horizontal * 0.8 + 4.0).min(CRUISE);
        dragon.velocity = steer_toward(dragon.velocity, goal, at, speed, 1.5, delta);
        let touched = at.y - (floor(at) + STANCE) < 0.6 && horizontal < 4.0;
        if hidden {
          Flight::Rising(0.0)
        } else if touched {
          shake.0 = shake.0.max(0.6);
          sounds.write(Sound::here(Cue::DragonRoar, at));
          dragon.velocity = Vec3::ZERO;
          Flight::Grounded(0.0)
        } else {
          flight
        }
      }
      Flight::Grounded(time_down) => {
        let facing = flat_gap.normalize_or_zero();
        let walking =
          distance > 9.5 && dragon.breath <= 0.0 && motion.swing.is_none() && !staggered;
        dragon.velocity = walking.then_some(facing * 3.2).unwrap_or(Vec3::ZERO);
        if exposed
          && !staggered
          && distance < 8.5
          && dragon.bite_cooldown <= 0.0
          && motion.swing.is_none()
        {
          motion.swing = Some(0.0);
          dragon.bite_cooldown = 2.6;
        }
        if exposed
          && !staggered
          && (8.5..BREATH_REACH).contains(&distance)
          && dragon.breath_cooldown <= 0.0
        {
          dragon.breath = 2.6;
          dragon.breath_cooldown = 7.0;
          sounds.write(Sound::here(Cue::FireBreath, at));
        }
        let restless = time_down > 28.0
          || (hero_dead && time_down > 4.0)
          || (hidden && time_down > 1.5)
          || (vitals.health < HEALTH * 0.45 && !dragon.spent);
        if restless {
          dragon.spent = dragon.spent || vitals.health < HEALTH * 0.45;
          Flight::Rising(0.0)
        } else {
          Flight::Grounded(time_down + delta)
        }
      }
      Flight::Rising(lifted) => {
        dragon.velocity = Vec3::Y * 9.0
          + transform.forward().as_vec3().with_y(0.0).normalize_or_zero() * lifted * 6.0;
        (lifted > 2.5)
          .then_some(
            dragon.awake.then_some(Flight::Circling(0.0)).unwrap_or(Flight::Soaring(0.0))
          )
          .unwrap_or(Flight::Rising(lifted + delta))
      }
      Flight::Falling => {
        dragon.velocity += Vec3::NEG_Y * 12.0 * delta;
        dragon.velocity = dragon.velocity.with_y(dragon.velocity.y.max(-25.0));
        (at.y <= floor(at) + STANCE)
          .then(|| {
            shake.0 = 1.0;
            Flight::Slain(0.0)
          })
          .unwrap_or(Flight::Falling)
      }
      Flight::Slain(since) => {
        dragon.velocity = Vec3::ZERO;
        Flight::Slain(since + delta)
      }
    };
    dragon.flight = next;

    let grounded = matches!(
      next,
      Flight::Grounded(_)
        | Flight::Roosting(_)
        | Flight::Slain(_)
        | Flight::Posing { aloft: false }
    );
    let moved = transform.translation + dragon.velocity * delta;
    let clamped = moved.clamp(
      Vec3::new(-BOUND * 1.4, -400.0, -BOUND * 1.4),
      Vec3::new(BOUND * 1.4, 3000.0, BOUND * 1.4)
    );
    let placed = if grounded {
      let settle = matches!(next, Flight::Slain(_)).then_some(1.2).unwrap_or(0.0);
      clamped
        .with_y((floor(clamped) + STANCE - settle).lerp(clamped.y, (-6.0 * delta).exp()))
    } else {
      clamped.with_y(clamped.y.max(floor(clamped) + 2.0))
    };
    if !dead {
      transform.translation = placed;
    }
    let forward = transform.forward().as_vec3();
    let hovering = match next {
      Flight::Landing(spot) => (spot - at).with_y(0.0).length() < 12.0,
      _ => false
    };
    let settling = match next {
      Flight::Roosting(_) => true,
      Flight::Homing => {
        dragon.lair.is_some_and(|lair| (lair - at).with_y(0.0).length() < 12.0)
      }
      _ => false
    };
    let heading = if settling {
      forward.with_y(0.0).normalize_or(Vec3::X)
    } else if grounded || hovering {
      flat_gap.normalize_or(forward)
    } else if matches!(next, Flight::Rising(_)) {
      forward.with_y(0.0).normalize_or(Vec3::X).with_y(0.35).normalize()
    } else {
      dragon.velocity.normalize_or(forward)
    };
    let (was, now) = (before.with_y(0.0), dragon.velocity.with_y(0.0));
    let yaw_rate = (was.length() > 1.0 && now.length() > 1.0)
      .then(|| {
        was.normalize().cross(now.normalize()).y.clamp(-1.0, 1.0).asin() / delta.max(1e-3)
      })
      .unwrap_or(0.0);
    let bank = (!grounded && !hovering && !settling)
      .then(|| (yaw_rate * now.length() / GRAVITY).atan())
      .unwrap_or(0.0)
      .clamp(-0.9, 0.9);
    let pitch = (!grounded).then_some(heading.y.asin() * 0.8).unwrap_or(0.0);
    let yaw = f32::atan2(-heading.x, -heading.z);
    let aim = Quat::from_euler(EulerRot::YXZ, yaw, pitch, bank);
    if !dead
      && !matches!(next, Flight::Slain(_) | Flight::Waiting | Flight::Posing { .. })
    {
      let agility = grounded.then_some(1.6).unwrap_or(2.2);
      transform.rotation = transform.rotation.slerp(aim, 1.0 - (-agility * delta).exp());
    }

    dragon.flap_rate = match next {
      Flight::Landing(_) | Flight::Rising(_) => 3.4,
      Flight::Grounded(_)
      | Flight::Roosting(_)
      | Flight::Slain(_)
      | Flight::Waiting
      | Flight::Posing { aloft: false } => 0.0,
      Flight::Homing if settling => 3.4,
      Flight::Falling => 5.0,
      _ => (dragon.velocity.y.max(0.0) * 0.3 + 1.4).min(3.0)
    };
    let before = dragon.flap;
    dragon.flap += dragon.flap_rate * delta;
    let beat = (before / TAU).floor() != (dragon.flap / TAU).floor();
    if beat && distance < 140.0 && dragon.flap_rate > 0.0 {
      sounds.write(Sound::here(Cue::Wingbeat, at));
    }

    let breathing = dragon.breath > 0.0 && !dead;
    if breathing && exposed {
      let mouth = at + transform.forward().as_vec3() * 5.5;
      let chest = target + Vec3::Y;
      let reach = target - mouth;
      let earthed = (1..12).any(|step| {
        let probe = mouth.lerp(chest, step as f32 / 12.0);
        probe.y < floor(probe) - 0.2
      });
      let aimed = transform
        .forward()
        .as_vec3()
        .with_y(0.0)
        .normalize_or_zero()
        .dot(reach.with_y(0.0).normalize_or_zero())
        > 0.85;
      if aimed
        && !earthed
        && reach.with_y(0.0).length() < BREATH_REACH
        && reach.y.abs() < 16.0
      {
        let blocking = hero_motion.guard > 0.6;
        dragon.scorch += delta;
        if let Ok(mut vitals) = player_vitals.get_mut(hero) {
          vitals.health -= BREATH_DPS * delta * blocking.then_some(0.4).unwrap_or(1.0);
        }
        if dragon.scorch > 0.45 {
          dragon.scorch = 0.0;
          struck.write(Struck {
            target: hero,
            attacker: entity,
            damage: 0.0,
            power: false,
            blocked: blocking,
            at: target + Vec3::Y * 0.5
          });
        }
      }
    }
  }
}

fn kindle(
  dragons: Query<(&Dragon, &Transform)>,
  heroes: Query<&Transform, (With<Player>, Without<Dragon>)>,
  mut breaths: Query<(
    &Breath,
    &mut EffectSpawner,
    &mut EffectProperties,
    &GlobalTransform
  )>
) {
  for (&Breath(owner), mut spawner, mut properties, mouth) in breaths.iter_mut() {
    let breathing = dragons.get(owner).ok().filter(|(dragon, _)| {
      dragon.breath > 0.0 && !matches!(dragon.flight, Flight::Slain(_) | Flight::Falling)
    });
    spawner.active = breathing.is_some();
    if let Some((&Dragon { velocity, .. }, transform)) = breathing
      && let ahead = transform.forward().as_vec3().with_y(0.0).normalize_or(Vec3::NEG_Z)
      && let toward =
        heroes.single().map_or(ahead * BREATH_REACH - Vec3::Y * 2.0, |hero| {
          hero.translation + Vec3::Y - mouth.translation()
        })
    {
      let aim = (ahead * toward.dot(ahead).max(6.0) + Vec3::Y * toward.y).normalize();
      properties.set("thrust", (aim * BREATH_SPEED + velocity).into())
    }
  }
}

pub fn pose(
  time: Res<Time>,
  dragons: Query<(&Dragon, &Motion), Without<Dead>>,
  mut bones: Query<&mut Transform, (Without<Dragon>, Without<FoldsAway>)>,
  mut folding: Query<&mut Transform, (With<FoldsAway>, Without<Dragon>)>
) {
  let delta = time.delta_secs();
  let blend = 1.0 - (-8.0 * delta).exp();
  let clock = time.elapsed_secs();
  for (dragon, motion) in dragons.iter() {
    let beat = dragon.flap.sin();
    let lag = (dragon.flap - 0.9).sin();
    let flying = !matches!(
      dragon.flight,
      Flight::Grounded(_)
        | Flight::Roosting(_)
        | Flight::Slain(_)
        | Flight::Waiting
        | Flight::Posing { aloft: false }
    );
    let slain = matches!(dragon.flight, Flight::Slain(_));
    let breathing = dragon.breath > 0.0;
    let bite =
      motion.swing.map_or(0.0, |progress| (progress * std::f32::consts::PI).sin());
    let sway = (clock * 1.3).sin();
    let angles = |bone: Bone| -> Vec3 {
      match bone {
        Bone::Body if slain => Vec3::new(0.05, 0.0, 0.5),
        Bone::Body if flying => Vec3::ZERO,
        Bone::Body => Vec3::new(-0.08 + 0.02 * sway, 0.0, 0.0),
        Bone::Neck1 if slain => Vec3::new(0.35, 0.3, 0.0),
        Bone::Neck1 if breathing => Vec3::new(-0.25, 0.0, 0.0),
        Bone::Neck1 if flying => Vec3::new(-0.05, 0.05 * sway, 0.0),
        Bone::Neck1 => Vec3::new(-0.55 + 0.4 * bite, 0.0, 0.0),
        Bone::Neck2 if slain => Vec3::new(0.2, 0.3, 0.0),
        Bone::Neck2 if flying => Vec3::new(0.05, 0.05 * sway, 0.0),
        Bone::Neck2 => Vec3::new(0.15 + 0.3 * bite, 0.0, 0.0),
        Bone::Neck3 if slain => Vec3::new(0.1, 0.4, 0.0),
        Bone::Neck3 => Vec3::new(0.25 + 0.2 * bite, 0.0, 0.0),
        Bone::Head if breathing => Vec3::new(-0.3, 0.0, 0.0),
        Bone::Head if flying => Vec3::new(0.05, 0.0, 0.0),
        Bone::Head => Vec3::new(0.3 - 0.2 * bite, 0.0, 0.0),
        Bone::Jaw if slain => Vec3::new(0.35, 0.0, 0.0),
        Bone::Jaw => {
          Vec3::new(0.05 + breathing as u8 as f32 * 0.55 + bite * 0.7, 0.0, 0.0)
        }
        Bone::Tail1 | Bone::Tail2 | Bone::Tail3 | Bone::Tail4 => {
          let wave = (clock * 1.7 + bone as u8 as f32 * 0.8).sin();
          if slain {
            Vec3::new(0.0, 0.25, 0.0)
          } else if flying {
            Vec3::new(0.06 * lag, 0.08 * wave, 0.0)
          } else {
            Vec3::new(0.12, 0.2 * wave, 0.0)
          }
        }
        Bone::WingR if slain => Vec3::new(0.0, 0.3, -0.75),
        Bone::WingL if slain => Vec3::new(0.0, -0.3, 0.45),
        Bone::WingR if flying => {
          Vec3::new(0.0, 0.0, beat * 0.75 * (dragon.flap_rate > 0.1) as u8 as f32 + 0.05)
        }
        Bone::WingL if flying => {
          Vec3::new(0.0, 0.0, -beat * 0.75 * (dragon.flap_rate > 0.1) as u8 as f32 - 0.05)
        }
        Bone::WingR => Vec3::new(0.2, 0.6, -1.15),
        Bone::WingL => Vec3::new(0.2, -0.6, 1.15),
        Bone::TipR | Bone::TipL => Vec3::ZERO,
        Bone::LegL | Bone::LegR if flying => Vec3::new(-1.1, 0.0, 0.0),
        Bone::ShinL | Bone::ShinR if flying => Vec3::new(1.0, 0.0, 0.0),
        Bone::LegL => Vec3::new(
          0.35 * (dragon.velocity.length() > 0.5) as u8 as f32 * (clock * 3.0).sin(),
          0.0,
          0.0
        ),
        Bone::LegR => Vec3::new(
          -0.35 * (dragon.velocity.length() > 0.5) as u8 as f32 * (clock * 3.0).sin(),
          0.0,
          0.0
        ),
        Bone::ShinL | Bone::ShinR => Vec3::new(-0.1, 0.0, 0.0)
      }
    };
    let euler = |bone: Bone| {
      let Vec3 { x, y, z } = angles(bone);
      Quat::from_euler(EulerRot::YXZ, y, x, z)
    };
    let fan = Quat::from_rotation_y(FAN_TURN);
    let (tip, spread, furl) = if flying {
      (Quat::from_rotation_z(lag * 0.5) * fan, Vec3::ONE, 1.0)
    } else if slain {
      (Quat::from_rotation_z(0.3) * fan, Vec3::splat(0.85), 1.0)
    } else {
      let wing = euler(Bone::WingR);
      let wrist = Bone::WingR.rest() + wing * Bone::TipR.rest();
      let along = (FOLDED_TOWARD - wrist).normalize();
      let outward = Vec3::X.reject_from_normalized(along).normalize();
      let folded =
        Quat::from_mat3(&Mat3::from_cols(outward.cross(along), outward, along));
      (wing.inverse() * folded, Vec3::new(0.13, 1.0, 0.95), 0.3)
    };
    for (&entity, bone) in dragon.bones.iter().zip(Bone::ALL) {
      if let Ok(mut transform) = bones.get_mut(entity) {
        let target = match bone {
          Bone::TipR => tip,
          Bone::TipL => mirrored(tip),
          _ => euler(bone)
        };
        transform.rotation = transform.rotation.slerp(target, blend);
        if matches!(bone, Bone::TipL | Bone::TipR) {
          transform.scale = transform.scale.lerp(spread, blend);
        }
      }
    }
    for mut transform in folding.iter_mut() {
      transform.scale = transform.scale.lerp(Vec3::new(1.0, 1.0, furl), blend)
    }
  }
}

fn absorb(
  time: Res<Time>,
  look: Res<WispLook>,
  stuffs: Res<Stuffs>,
  mut shouts: ResMut<Shouts>,
  mut sounds: MessageWriter<Sound>,
  mut notices: MessageWriter<Notice>,
  mut commands: Commands,
  player: Single<Entity, With<Player>>,
  mut dragons: Query<(&mut Dragon, &Transform)>,
  mut parts: Query<&mut MeshMaterial3d<StandardMaterial>>
) {
  for (index, (mut dragon, transform)) in dragons.iter_mut().enumerate() {
    if let Flight::Slain(since) = dragon.flight
      && !dragon.absorbed
    {
      let flowing = (since - 2.0).clamp(0.0, 5.0);
      let due = (flowing * 40.0) as u32;
      let seed = index as u32 * 1000 + 999;
      for wisp in dragon.streamed..due {
        let mut roll = crate::noise::Roll::new(wisp * 97 + 5 + seed);
        let from = transform.transform_point(Vec3::new(
          roll.spread(1.5),
          roll.spread(1.0),
          roll.range(-3.0, 4.0)
        ));
        shout::stream(&mut commands, &look, from, *player, wisp + seed, true);
      }
      dragon.streamed = due;
      let mut clad = |stuff: Stuff| {
        let material = stuffs.of(stuff);
        for &part in dragon.parts.iter() {
          if let Ok(mut worn) = parts.get_mut(part) {
            worn.0 = material.clone()
          }
        }
      };
      if since > 2.0 && since - time.delta_secs() <= 2.0 {
        clad(Stuff::Cinder);
      }
      if since > 7.5 {
        clad(Stuff::Bone);
        dragon.absorbed = true;
        shouts.cooldown = 0.0;
        sounds.write(Sound::flat(Cue::WordLearned));
        notices.write(Notice("Dragon Soul Absorbed".into()));
      }
    }
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(Startup, spawn_dragons)
    .add_systems(Update, (fly, kindle, absorb).chain().after(crate::walker::Walking))
    .add_systems(PostUpdate, pose.before(TransformSystems::Propagate));
}
