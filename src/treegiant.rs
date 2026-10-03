use {crate::{combat::{Dead, Fighter, Fighting, Side, Struck, Vitals},
             creature::Foe,
             humanoid::Motion,
             model::{self, Piece},
             noise::{self, Roll},
             place,
             player::Player,
             ragdoll::{Limb, Link, Ragdoll},
             river,
             signal::{Cue, Sound},
             stuff::{Stuff, Stuffs},
             terrain::{self, Ground, height_at},
             walker::{Walker, Walking}},
     avian3d::prelude::*,
     bevy::prelude::*,
     std::{f32::consts::TAU, sync::LazyLock}};

const BARK_TILE: f32 = 1.1;
const BARK: Srgba = Srgba::rgb(0.4, 0.32, 0.24);
const DARK_BARK: Srgba = Srgba::rgb(0.15, 0.12, 0.1);
const MOSS: Srgba = Srgba::rgb(0.25, 0.32, 0.1);
const LEAVES: [Srgba; 4] = [
  Srgba::rgb(0.2, 0.36, 0.1),
  Srgba::rgb(0.3, 0.42, 0.12),
  Srgba::rgb(0.16, 0.3, 0.12),
  Srgba::rgb(0.46, 0.44, 0.14)
];
const EYE: Srgba = Srgba::rgb(1.0, 0.8, 0.3);

const HEIGHT: f32 = 6.8;
const GIRTH: f32 = 0.9;

fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
  let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flank {
  Left,
  Right
}

impl Flank {
  const BOTH: [Flank; 2] = [Flank::Left, Flank::Right];

  fn sign(self) -> f32 {
    match self {
      Flank::Left => -1.0,
      Flank::Right => 1.0
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bone {
  Hips,
  Chest,
  Head,
  Arm(Flank),
  Forearm(Flank),
  Thigh(Flank),
  Shin(Flank)
}

impl Bone {
  const ALL: [Bone; 11] = [
    Bone::Hips,
    Bone::Chest,
    Bone::Head,
    Bone::Arm(Flank::Left),
    Bone::Arm(Flank::Right),
    Bone::Forearm(Flank::Left),
    Bone::Forearm(Flank::Right),
    Bone::Thigh(Flank::Left),
    Bone::Thigh(Flank::Right),
    Bone::Shin(Flank::Left),
    Bone::Shin(Flank::Right)
  ];

  fn parent(self) -> Option<Bone> {
    match self {
      Bone::Hips => None,
      Bone::Chest | Bone::Thigh(_) => Some(Bone::Hips),
      Bone::Head | Bone::Arm(_) => Some(Bone::Chest),
      Bone::Forearm(flank) => Some(Bone::Arm(flank)),
      Bone::Shin(flank) => Some(Bone::Thigh(flank))
    }
  }

  fn pivot(self) -> Vec3 {
    let at = |flank: Flank, x: f32, y: f32, z: f32| Vec3::new(flank.sign() * x, y, z);
    match self {
      Bone::Hips => Vec3::new(0.0, 3.05, 0.0),
      Bone::Chest => Vec3::new(0.0, 3.35, 0.0),
      Bone::Head => Vec3::new(0.0, 5.7, -0.12),
      Bone::Arm(flank) => at(flank, 1.12, 5.25, 0.02),
      Bone::Forearm(flank) => at(flank, 1.42, 3.85, 0.0),
      Bone::Thigh(flank) => at(flank, 0.46, 3.0, 0.0),
      Bone::Shin(flank) => at(flank, 0.56, 1.62, -0.06)
    }
  }

  fn rest(self) -> Vec3 { self.pivot() - self.parent().map_or(Vec3::ZERO, Bone::pivot) }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flesh {
  Wood,
  Leaves,
  Glow
}

impl Flesh {
  fn stuff(self) -> Stuff {
    match self {
      Flesh::Wood => Stuff::Bark,
      Flesh::Leaves => Stuff::Needles,
      Flesh::Glow => Stuff::Ember
    }
  }
}

fn barked(point: Vec3, normal: Vec3) -> LinearRgba {
  let furrow = noise::fbm3(point * Vec3::new(3.5, 0.7, 3.5), 2, 7);
  let mossy = smooth(
    0.35,
    0.85,
    normal.y + 0.5 * noise::fbm3(point * 1.3, 3, 9) + 0.12 * (point.y - 3.0)
  );
  LinearRgba::from(BARK)
    .mix(&DARK_BARK.into(), smooth(-0.15, 0.45, furrow) * 0.75)
    .mix(&MOSS.into(), mossy * 0.85)
}

fn wood(keys: &[(Vec3, f32)], seed: u32) -> Piece {
  let rings: Vec<(Vec3, Vec3)> =
    keys.iter().map(|&(at, girth)| (at, Vec3::splat(girth))).collect();
  let Piece(mesh) =
    Piece::new(model::sculpt(&rings, 5, 11), Srgba::WHITE).followed(BARK_TILE);
  let girth = keys.iter().map(|&(_, girth)| girth).fold(0.0, f32::max);
  Piece(model::ruffled(mesh, girth * 0.22, Vec3::new(2.2, 0.6, 2.2) / girth.sqrt(), seed))
    .shaded(barked)
}

fn twig(from: Vec3, bend: Vec3, to: Vec3, girth: (f32, f32)) -> Piece {
  Piece::new(
    model::tube(&model::curve(from, bend, to, 5), &model::taper(5, girth.0, girth.1), 6),
    Srgba::WHITE
  )
  .followed(BARK_TILE * 0.6)
  .shaded(barked)
}

fn clump(roll: &mut Roll, center: Vec3, (smallest, largest): (f32, f32)) -> Piece {
  let size = roll.range(smallest, largest);
  let tone = LinearRgba::from(LEAVES[roll.below(LEAVES.len())]);
  let seed = roll.below(1000) as u32;
  let tufts: Vec<Piece> = (0..4)
    .map(|tuft| {
      let offset = (tuft > 0)
        .then(|| Vec3::new(roll.spread(0.6), roll.spread(0.35), roll.spread(0.6)) * size)
        .unwrap_or(Vec3::ZERO);
      let stretch =
        Vec3::new(roll.range(0.9, 1.2), roll.range(0.55, 0.8), roll.range(0.9, 1.2));
      Piece::new(model::lump(seed + tuft, 0.5, 2), Srgba::WHITE)
        .sized(stretch * size * (1.0 - 0.15 * tuft as f32))
        .yawed(roll.range(0.0, TAU))
        .at(center + offset)
    })
    .collect();
  Piece(model::merge(tufts)).planar(0.4).shaded(move |point, normal| {
    let speck = noise::fbm3(point * 3.0, 2, seed);
    tone
      .mix(&LinearRgba::BLACK, 0.5 * smooth(0.3, -0.8, normal.y) + 0.2 * speck)
      .mix(&LinearRgba::from(LEAVES[3]), smooth(0.2, 0.6, speck) * 0.35)
  })
}

fn bough(
  roll: &mut Roll,
  from: Vec3,
  toward: Vec3,
  length: f32,
  girth: f32
) -> Vec<(Flesh, Piece)> {
  let droop = Vec3::Y * length * roll.range(0.05, 0.25);
  let to = from + toward.normalize() * length;
  let bend = from.lerp(to, 0.5)
    + droop
    + Vec3::new(roll.spread(0.3), 0.0, roll.spread(0.3)) * length;
  let tip = to + Vec3::Y * length * 0.15;
  let sprigs: Vec<(Flesh, Piece)> = (0..2)
    .map(|_| {
      let start = from.lerp(to, roll.range(0.45, 0.75));
      let out = (toward.normalize()
        + Vec3::new(roll.spread(1.0), roll.range(0.2, 0.9), roll.spread(1.0)))
      .normalize()
        * length
        * roll.range(0.3, 0.5);
      let end = start + out;
      [
        (
          Flesh::Wood,
          twig(
            start,
            start + out * 0.5 + Vec3::Y * 0.1,
            end,
            (girth * 0.45, girth * 0.12)
          )
        ),
        (Flesh::Leaves, clump(roll, end, (length * 0.22, length * 0.32)))
      ]
    })
    .collect::<Vec<_>>()
    .into_iter()
    .flatten()
    .collect();
  [
    (Flesh::Wood, twig(from, bend, tip, (girth, girth * 0.25))),
    (Flesh::Leaves, clump(roll, tip, (length * 0.32, length * 0.42))),
    (
      Flesh::Leaves,
      clump(roll, tip.lerp(bend, 0.5) + Vec3::Y * 0.15, (length * 0.22, length * 0.3))
    )
  ]
  .into_iter()
  .chain(sprigs)
  .collect()
}

pub fn body(seed: u32) -> Vec<(Bone, Stuff, Mesh)> {
  let mut roll = Roll::new(seed * 31 + 7);
  let roll = &mut roll;
  let v = Vec3::new;
  let trunk = [
    (
      Bone::Hips,
      Flesh::Wood,
      wood(
        &[
          (v(-0.62, 3.0, 0.04), 0.38),
          (v(0.0, 3.12, 0.0), 0.56),
          (v(0.62, 3.0, 0.04), 0.38)
        ],
        1
      )
    ),
    (
      Bone::Chest,
      Flesh::Wood,
      wood(
        &[
          (v(0.0, 2.9, 0.04), 0.5),
          (v(0.0, 3.6, 0.08), 0.6),
          (v(0.0, 4.5, 0.1), 0.74),
          (v(0.0, 5.25, 0.04), 0.78),
          (v(0.0, 5.8, -0.06), 0.42)
        ],
        2
      )
    ),
    (
      Bone::Chest,
      Flesh::Wood,
      wood(
        &[
          (v(-1.3, 5.12, 0.06), 0.3),
          (v(-0.6, 5.42, 0.04), 0.44),
          (v(0.6, 5.42, 0.04), 0.44),
          (v(1.3, 5.12, 0.06), 0.3)
        ],
        3
      )
    ),
    (
      Bone::Head,
      Flesh::Wood,
      wood(
        &[
          (v(0.0, 5.55, -0.08), 0.34),
          (v(0.0, 6.05, -0.2), 0.4),
          (v(0.0, 6.55, -0.18), 0.34),
          (v(0.0, 6.85, -0.1), 0.18)
        ],
        4
      )
    ),
    (
      Bone::Head,
      Flesh::Wood,
      wood(
        &[
          (v(-0.36, 6.22, -0.46), 0.08),
          (v(0.0, 6.3, -0.56), 0.12),
          (v(0.36, 6.22, -0.46), 0.08)
        ],
        5
      )
    ),
    (
      Bone::Head,
      Flesh::Wood,
      wood(
        &[
          (v(0.0, 6.2, -0.54), 0.07),
          (v(0.0, 6.0, -0.62), 0.09),
          (v(0.0, 5.88, -0.58), 0.05)
        ],
        6
      )
    )
  ];
  let eyes = Flank::BOTH.map(|flank| {
    (
      Bone::Head,
      Flesh::Glow,
      Piece::new(model::ball(0.075), EYE).sized(v(1.3, 0.75, 1.0)).at(v(
        flank.sign() * 0.16,
        6.12,
        -0.52
      ))
    )
  });
  let beard: Vec<(Bone, Flesh, Piece)> = (0..7)
    .map(|strand| {
      let across = (strand as f32 / 6.0 - 0.5) * 0.5;
      let from = v(across, 5.86, -0.5 + across.abs() * 0.4);
      let to = from + v(across * 0.4 + roll.spread(0.08), -roll.range(0.5, 0.9), -0.12);
      (
        Bone::Head,
        Flesh::Wood,
        twig(from, from.lerp(to, 0.5) + v(0.0, 0.0, -0.08), to, (0.05, 0.015))
      )
    })
    .collect();
  let crown: Vec<(Bone, Flesh, Piece)> = (0..6)
    .flat_map(|branch| {
      let angle = branch as f32 / 6.0 * TAU + roll.spread(0.4);
      let toward = v(angle.cos() * 0.7, roll.range(0.8, 1.3), angle.sin() * 0.55 + 0.25);
      let from = v(angle.cos() * 0.12, 6.6, -0.15 + angle.sin() * 0.1);
      let length = roll.range(1.2, 1.9);
      bough(roll, from, toward, length, 0.11)
        .into_iter()
        .map(|(flesh, piece)| (Bone::Head, flesh, piece))
        .collect::<Vec<_>>()
    })
    .collect();
  let shoulders: Vec<(Bone, Flesh, Piece)> = Flank::BOTH
    .into_iter()
    .flat_map(|flank| {
      let k = flank.sign();
      [
        (v(k * 0.95, 5.35, 0.12), v(k * 0.7, 1.0, 0.45), 1.8, 0.14),
        (v(k * 0.4, 5.1, 0.45), v(k * 0.35, 0.9, 0.9), 1.5, 0.12),
        (v(k * 0.2, 4.2, 0.55), v(k * 0.6, 0.45, 1.0), 1.1, 0.09)
      ]
      .into_iter()
      .flat_map(|(from, toward, length, girth)| bough(roll, from, toward, length, girth))
      .map(|(flesh, piece)| (Bone::Chest, flesh, piece))
      .collect::<Vec<_>>()
    })
    .collect();
  let limbs: Vec<(Bone, Flesh, Piece)> = Flank::BOTH
    .into_iter()
    .flat_map(|flank| {
      let k = flank.sign();
      let at = |x: f32, y: f32, z: f32| v(k * x, y, z);
      let seed = (flank == Flank::Right) as u32 * 20;
      let fingers = (0..4).map(|finger| {
        let spread = (finger as f32 - 1.5) * 0.38;
        let thumb = finger == 0;
        let from = at(1.5, 2.5, -0.12);
        let out = at(
          spread.sin() * 0.25 + thumb as u8 as f32 * -0.15,
          -0.75 + thumb as u8 as f32 * 0.35,
          -spread.cos() * 0.3
        );
        (
          Bone::Forearm(flank),
          Flesh::Wood,
          twig(
            from,
            from + out * 0.75 + at(0.0, 0.0, -0.2),
            from + out * 1.55 + at(0.0, 0.0, 0.2),
            (0.13, 0.035)
          )
        )
      });
      let roots: Vec<_> = (0..5)
        .map(|toe| {
          let spread = (toe as f32 - 2.0) * 0.5 + (toe == 4) as u8 as f32 * 0.9;
          let ankle = at(0.62, 0.45, -0.02);
          let reach = roll.range(0.75, 1.05);
          let out = v(spread.sin() * k, 0.0, -spread.cos());
          (
            Bone::Shin(flank),
            Flesh::Wood,
            twig(
              ankle,
              ankle + out * reach * 0.45 - Vec3::Y * 0.2,
              ankle + out * reach - Vec3::Y * 0.42,
              (0.15, 0.04)
            )
          )
        })
        .collect();
      [
        (
          Bone::Arm(flank),
          Flesh::Wood,
          wood(
            &[
              (at(1.0, 5.32, 0.04), 0.36),
              (at(1.22, 4.8, 0.03), 0.32),
              (at(1.38, 4.1, 0.0), 0.27),
              (at(1.43, 3.75, 0.0), 0.3)
            ],
            seed + 7
          )
        ),
        (
          Bone::Forearm(flank),
          Flesh::Wood,
          wood(
            &[
              (at(1.42, 3.95, 0.0), 0.3),
              (at(1.47, 3.3, -0.06), 0.25),
              (at(1.5, 2.65, -0.12), 0.22),
              (at(1.5, 2.4, -0.12), 0.24)
            ],
            seed + 8
          )
        ),
        (
          Bone::Thigh(flank),
          Flesh::Wood,
          wood(
            &[
              (at(0.42, 3.15, 0.02), 0.42),
              (at(0.5, 2.4, 0.0), 0.38),
              (at(0.56, 1.75, -0.06), 0.34),
              (at(0.56, 1.5, -0.06), 0.36)
            ],
            seed + 9
          )
        ),
        (
          Bone::Shin(flank),
          Flesh::Wood,
          wood(
            &[
              (at(0.56, 1.72, -0.06), 0.34),
              (at(0.58, 1.1, -0.02), 0.3),
              (at(0.62, 0.5, -0.02), 0.36),
              (at(0.62, 0.15, -0.02), 0.44)
            ],
            seed + 10
          )
        )
      ]
      .into_iter()
      .chain(fingers)
      .chain(roots)
      .chain(
        bough(roll, at(1.2, 4.6, 0.15), at(0.8, 0.9, 0.35), 0.9, 0.08)
          .into_iter()
          .map(move |(flesh, piece)| (Bone::Arm(flank), flesh, piece))
      )
      .collect::<Vec<_>>()
    })
    .collect();
  let parts: Vec<(Bone, Flesh, Piece)> = trunk
    .into_iter()
    .chain(eyes)
    .chain(beard)
    .chain(crown)
    .chain(shoulders)
    .chain(limbs)
    .collect();
  Bone::ALL
    .into_iter()
    .flat_map(|bone| [Flesh::Wood, Flesh::Leaves, Flesh::Glow].map(|flesh| (bone, flesh)))
    .filter_map(|(bone, flesh)| {
      let pieces: Vec<Piece> = parts
        .iter()
        .filter(|(owner, each, _)| *owner == bone && *each == flesh)
        .map(|(_, _, Piece(mesh))| Piece(mesh.clone()).at(-bone.pivot()))
        .collect();
      (!pieces.is_empty()).then(|| {
        let stuff = flesh.stuff();
        (bone, stuff, stuff.fitted(model::merge(pieces)))
      })
    })
    .collect()
}

pub fn limbs() -> [Option<Limb>; 11] {
  let limb =
    |radius: f32, to: Vec3, link: Link| Some(Limb { radius, from: Vec3::ZERO, to, link });
  let socket = Link::Socket { swing: 0.9, twist: 0.3 };
  let hinge = Link::Hinge { low: -1.5, high: 0.3 };
  Bone::ALL.map(|bone| match bone {
    Bone::Hips => Some(Limb {
      radius: 0.5,
      from: Vec3::new(-0.3, 0.0, 0.0),
      to: Vec3::new(0.3, 0.0, 0.0),
      link: Link::Root
    }),
    Bone::Chest => limb(0.7, Vec3::Y * 2.1, socket),
    Bone::Head => limb(0.36, Vec3::new(0.0, 0.8, -0.1), socket),
    Bone::Arm(flank) => limb(0.3, Vec3::new(flank.sign() * 0.3, -1.3, 0.0), socket),
    Bone::Forearm(flank) => limb(0.26, Vec3::new(flank.sign() * 0.08, -1.5, -0.1), hinge),
    Bone::Thigh(flank) => limb(0.38, Vec3::new(flank.sign() * 0.1, -1.3, -0.05), socket),
    Bone::Shin(flank) => limb(0.36, Vec3::new(flank.sign() * 0.06, -1.25, 0.04), hinge)
  })
}

#[derive(Component)]
pub struct Striding {
  pub bones: Vec<(Bone, Entity)>,
  stride: f32,
  offset: f32,
  shift: f32
}

fn rig(
  commands: &mut Commands,
  parts: &[(Bone, Handle<StandardMaterial>, Handle<Mesh>)],
  (root, owner): (Entity, Entity),
  (offset, shift): (f32, f32)
) {
  let bones =
    Bone::ALL.into_iter().fold(Vec::new(), |mut bones: Vec<(Bone, Entity)>, bone| {
      let parent = bone
        .parent()
        .and_then(|parent| bones.iter().find(|(each, _)| *each == parent))
        .map_or(root, |&(_, entity)| entity);
      let entity = commands
        .spawn((
          Transform::from_translation(bone.rest()),
          Visibility::Inherited,
          ChildOf(parent)
        ))
        .id();
      for (_, material, mesh) in parts.iter().filter(|(owner, ..)| *owner == bone) {
        commands.spawn((
          Transform::IDENTITY,
          Visibility::Inherited,
          Mesh3d(mesh.clone()),
          MeshMaterial3d(material.clone()),
          ChildOf(entity)
        ));
      }
      bones.push((bone, entity));
      bones
    });
  commands.entity(owner).insert(Striding { bones, stride: 0.0, offset, shift });
}

fn shaped(
  seed: u32,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs
) -> Vec<(Bone, Handle<StandardMaterial>, Handle<Mesh>)> {
  body(seed)
    .into_iter()
    .map(|(bone, stuff, mesh)| (bone, stuffs.of(stuff), meshes.add(mesh)))
    .collect()
}

pub fn spawn(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  holder: Entity,
  (seed, offset): (u32, f32)
) {
  rig(commands, &shaped(seed, meshes, stuffs), (holder, holder), (offset, offset))
}

fn rise(from: f32, to: f32, at: f32) -> f32 {
  let t = ((at - from) / (to - from)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

fn posed(
  bone: Bone,
  motion: &Motion,
  time: f32,
  stride: f32,
  shift: f32
) -> (Vec3, Quat) {
  let walk = (motion.speed / 1.4).clamp(0.0, 1.0);
  let still = 1.0 - walk;
  let (step, lift) = stride.sin_cos();
  let breath = (time * 0.7).sin();
  let attack = motion.swing.map_or(0.0, |progress| (progress + shift).fract());
  let swinging = motion.swing.is_some() as u8 as f32;
  let (raise, slam, settle) =
    (rise(0.0, 0.38, attack), rise(0.38, 0.52, attack), rise(0.7, 1.0, attack));
  let lifted = swinging * (raise - slam);
  let struck = swinging * slam * (1.0 - settle);
  let euler = |x: f32, y: f32, z: f32| Quat::from_euler(EulerRot::YXZ, y, x, z);
  match bone {
    Bone::Hips => (
      Vec3::Y * (0.03 * breath * still - 0.12 * walk * step.abs() - 0.15 * struck),
      euler(
        0.0,
        0.07 * walk * step,
        0.04 * walk * step + 0.02 * (time * 0.3).sin() * still
      )
    ),
    Bone::Chest => (
      Vec3::ZERO,
      euler(
        0.02 * breath * still - 0.08 * walk + 0.22 * lifted - 0.4 * struck,
        -0.1 * walk * step + 0.05 * (time * 0.23).sin() * still,
        0.03 * (time * 0.31).sin() * still
      )
    ),
    Bone::Head => (
      Vec3::ZERO,
      euler(
        0.04 * walk - 0.03 * breath * still - 0.1 * lifted + 0.15 * struck,
        0.3 * (time * 0.21).sin() * still + 0.05 * walk * step,
        0.04 * (time * 0.17).sin() * still
      )
    ),
    Bone::Arm(flank) => {
      let k = flank.sign();
      (
        Vec3::ZERO,
        euler(
          -0.28 * walk * step * k
            + 0.04 * (time * 0.6 + k).sin() * still
            + 2.7 * lifted
            + 0.9 * struck,
          0.0,
          k * (0.05 + 0.015 * breath * still) - k * 0.2 * lifted
        )
      )
    }
    Bone::Forearm(_) => (
      Vec3::ZERO,
      euler(
        0.15 + 0.15 * walk * (-step).max(0.0) + 0.9 * lifted - 0.1 * struck,
        0.0,
        0.0
      )
    ),
    Bone::Thigh(flank) => {
      let k = flank.sign();
      (Vec3::ZERO, euler(0.36 * walk * step * k + 0.2 * struck, 0.0, 0.0))
    }
    Bone::Shin(flank) => {
      let k = flank.sign();
      (Vec3::ZERO, euler(-0.7 * walk * (lift * k).max(0.0) - 0.3 * struck, 0.0, 0.0))
    }
  }
}

fn pace(speed: f32) -> f32 { TAU * (0.4 + 0.12 * speed) }

pub fn animate(
  time: Res<Time>,
  mut frames: Query<(&mut Striding, &Motion), (Without<Dead>, Without<Ragdoll>)>,
  mut transforms: Query<&mut Transform, Without<Striding>>
) {
  for (mut striding, motion) in &mut frames {
    striding.stride += time.delta_secs() * pace(motion.speed);
    let &Striding { stride, offset, shift, .. } = &*striding;
    for &(bone, entity) in &striding.bones {
      if let Ok(mut transform) = transforms.get_mut(entity) {
        let (lift, turn) = posed(
          bone,
          motion,
          time.elapsed_secs() + offset,
          stride + offset * pace(motion.speed),
          shift
        );
        *transform = Transform::from_translation(bone.rest() + lift).with_rotation(turn)
      }
    }
  }
}

const GROVES: usize = 10;
const ROAM: f32 = 70.0;
const SENSE: f32 = 45.0;
const UNWATCHED: f32 = 320.0;
const GRUDGE: f32 = 20.0;
const WALK: f32 = 1.3;
const RUN: f32 = 3.4;

fn grove(at: Vec2) -> bool {
  let height = height_at(at);
  let slope = Vec2::new(
    height_at(at + Vec2::X * 6.0) - height,
    height_at(at + Vec2::Y * 6.0) - height
  ) / 6.0;
  terrain::forest(at) > 0.25
    && slope.length() < 0.3
    && height < 200.0
    && place::route_distance(at) > 20.0
    && river::water_level(at).is_none()
}

static HOMES: LazyLock<Vec<Vec2>> = LazyLock::new(|| {
  let reach = terrain::WORLD - 150.0;
  let mut spots: Vec<Vec2> = (-70..=70)
    .flat_map(|row| {
      (-70..=70)
        .map(move |column| place::START + Vec2::new(column as f32, row as f32) * 40.0)
    })
    .filter(|spot| {
      spot.abs().max_element() < reach
        && (160.0..2900.0).contains(&spot.distance(place::START))
    })
    .filter(|&spot| grove(spot))
    .collect();
  spots.sort_by(|a, b| a.distance(place::START).total_cmp(&b.distance(place::START)));
  spots.into_iter().fold(Vec::new(), |mut homes, spot| {
    if homes.len() < GROVES && homes.iter().all(|home: &Vec2| home.distance(spot) > 500.0)
    {
      homes.push(spot)
    }
    homes
  })
});

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tending {
  Rest(f32),
  Wander(Vec3, f32),
  Fight(Entity)
}

#[derive(Component)]
pub struct Warden {
  home: Vec3,
  mind: Tending,
  cooldown: f32,
  roll: Roll
}

#[derive(Component)]
struct Sprouting(u32);

fn haunt(mut commands: Commands) {
  for (index, &home) in HOMES.iter().enumerate() {
    let seed = index as u32 * 13 + 5;
    let ground = home.extend(height_at(home)).xzy();
    commands.spawn((
      Name::new("Tree Giant"),
      Warden {
        home: ground,
        mind: Tending::Rest(Roll::new(seed).range(0.0, 8.0)),
        cooldown: 0.0,
        roll: Roll::new(seed)
      },
      Sprouting(index as u32 % 3),
      Walker::default(),
      Motion::default(),
      Vitals::new(420.0, 200.0),
      Side::Hero,
      Fighter {
        reach: 3.3,
        damage: 24.0,
        swing_time: 1.5,
        cone: 0.9,
        girth: GIRTH,
        tall: HEIGHT
      },
      Collider::capsule(GIRTH, HEIGHT - 2.0 * GIRTH),
      Transform::from_translation(ground + Vec3::Y * (HEIGHT / 2.0 + 0.05))
        .with_rotation(Quat::from_rotation_y(Roll::new(seed + 1).range(0.0, TAU)))
    ));
  }
}

fn sprout(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  mut kinds: Local<Vec<(u32, Vec<(Bone, Handle<StandardMaterial>, Handle<Mesh>)>)>>,
  sprouting: Query<(Entity, &Sprouting)>
) {
  for (entity, &Sprouting(kind)) in &sprouting {
    if !kinds.iter().any(|(each, _)| *each == kind) {
      kinds.push((kind, shaped(kind, &mut meshes, &stuffs)))
    }
    let body = commands
      .spawn((
        Transform::from_xyz(0.0, -HEIGHT / 2.0 - 0.05, 0.0),
        Visibility::Inherited,
        ChildOf(entity)
      ))
      .id();
    let offset = Roll::new(entity.index().index()).range(0.0, 10.0);
    if let Some((_, parts)) = kinds.iter().find(|(each, _)| *each == kind) {
      rig(&mut commands, parts, (body, entity), (offset, 0.0))
    }
    commands.entity(entity).remove::<Sprouting>();
  }
}

fn tend(
  time: Res<Time>,
  mut struck: MessageReader<Struck>,
  mut sounds: MessageWriter<Sound>,
  mut grudges: Local<Vec<(Entity, f32)>>,
  player: Single<(Entity, &Transform), With<Player>>,
  sides: Query<&Side>,
  hostiles: Query<
    (Entity, &Transform, &Side, Option<&Foe>),
    (Without<Dead>, Without<Warden>)
  >,
  mut wardens: Query<
    (Entity, &mut Warden, &Transform, &mut Walker, &mut Motion, &Fighter),
    Without<Dead>
  >
) {
  let delta = time.delta_secs();
  let now = time.elapsed_secs();
  let (hero, hero_at) = *player;
  grudges.extend(
    struck
      .read()
      .filter(|hit| {
        sides.get(hit.attacker).is_ok_and(|&side| side == Side::Wild)
          && (hit.target == hero || wardens.contains(hit.target))
      })
      .map(|hit| (hit.attacker, now))
  );
  grudges.retain(|&(_, at)| now - at < GRUDGE);
  let foes: Vec<(Entity, Vec3)> = hostiles
    .iter()
    .filter(|&(entity, _, &side, foe)| {
      side == Side::Wild
        && (foe.is_some_and(Foe::hunting)
          || grudges.iter().any(|&(each, _)| each == entity))
    })
    .map(|(entity, transform, ..)| (entity, transform.translation))
    .collect();
  for (_, mut warden, transform, mut walker, mut motion, fighter) in &mut wardens {
    let at = transform.translation;
    let nearest = foes
      .iter()
      .filter(|(_, foe)| foe.distance(at) < SENSE)
      .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)))
      .map(|&(entity, _)| entity);
    let quarry = |entity: Entity| {
      hostiles
        .get(entity)
        .ok()
        .map(|(_, transform, ..)| transform.translation)
        .filter(|foe| foe.distance(at) < SENSE * 2.0)
    };
    warden.cooldown -= delta;
    let mind = match warden.mind {
      Tending::Fight(foe) if quarry(foe).is_some() => Tending::Fight(foe),
      _ if let Some(foe) = nearest => {
        if !matches!(warden.mind, Tending::Fight(_)) {
          sounds.write(Sound::here(Cue::TreeGroan, at));
          warden.cooldown = 0.8;
        }
        Tending::Fight(foe)
      }
      Tending::Fight(_) => Tending::Rest(3.0),
      Tending::Rest(wait)
        if wait <= 0.0 && hero_at.translation.distance(at) < UNWATCHED =>
      {
        let home = warden.home;
        let spot = home.xz()
          + Vec2::from_angle(warden.roll.range(0.0, TAU)) * warden.roll.range(10.0, ROAM);
        (grove(spot) || spot.distance(home.xz()) < 25.0)
          .then(|| Tending::Wander(spot.extend(height_at(spot)).xzy(), 60.0))
          .unwrap_or(Tending::Rest(2.0))
      }
      Tending::Rest(wait) => Tending::Rest(wait - delta),
      Tending::Wander(spot, left)
        if left <= 0.0 || (spot - at).with_y(0.0).length() < 2.5 =>
      {
        Tending::Rest(warden.roll.range(6.0, 18.0))
      }
      Tending::Wander(spot, left) => Tending::Wander(spot, left - delta)
    };
    warden.mind = mind;
    let busy = motion.swing.is_some() || motion.flinch > 0.4;
    let (wish, facing) = match mind {
      Tending::Rest(_) => (Vec3::ZERO, None),
      Tending::Wander(spot, _) => {
        ((spot - at).with_y(0.0).normalize_or_zero() * WALK, None)
      }
      Tending::Fight(foe) => {
        let gap = quarry(foe).map_or(Vec3::ZERO, |foe| (foe - at).with_y(0.0));
        let toward = gap.normalize_or_zero();
        let closing = (gap.length() > fighter.reach * 0.8)
          .then_some(toward * RUN)
          .unwrap_or(Vec3::ZERO);
        if gap.length() < fighter.reach + 0.4 && warden.cooldown <= 0.0 && !busy {
          motion.swing = Some(0.0);
          motion.power = warden.roll.chance(0.25);
          warden.cooldown = fighter.swing_time
            * motion.power.then_some(1.45).unwrap_or(1.0)
            + warden.roll.range(0.4, 1.2);
          sounds.write(Sound::here(Cue::PowerSwing, at));
        }
        (busy.then_some(closing * 0.2).unwrap_or(closing), Some(toward))
      }
    };
    walker.wish = wish;
    walker.facing = facing;
  }
}

fn fell(
  fallen: Query<&Transform, (With<Warden>, Added<Dead>)>,
  mut sounds: MessageWriter<Sound>
) {
  for transform in &fallen {
    sounds.write(Sound::here(Cue::TimberFall, transform.translation));
  }
}

fn specimen(
  mut commands: Commands,
  ground: Res<Ground>,
  player: Single<&Transform, Added<Player>>
) {
  if crate::opts::opts().foe.as_deref() == Some("treegiant") {
    let gap = crate::opts::opts().gap.unwrap_or(9.0);
    let ahead = player.translation + player.forward().as_vec3() * gap;
    commands.spawn((
      Motion::default(),
      Sprouting(crate::opts::opts().seed.unwrap_or(0)),
      Transform::from_translation(
        ground.surface(ahead.xz()) + Vec3::Y * (HEIGHT / 2.0 + 0.05)
      )
      .looking_to(player.right().as_vec3().with_y(0.0), Vec3::Y),
      Visibility::Inherited
    ));
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(Startup, haunt)
    .add_systems(PostStartup, specimen)
    .add_systems(Update, (sprout, tend.before(Walking), animate.after(Fighting), fell));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tree_giants_roam_woods_around_the_start() {
    println!(
      "{:?}",
      HOMES
        .iter()
        .map(|home| (
          home.round(),
          height_at(*home).round(),
          home.distance(place::START).round()
        ))
        .collect::<Vec<_>>()
    );
    assert_eq!(HOMES.len(), GROVES);
    assert!(HOMES.iter().all(|&home| grove(home)));
  }
}
