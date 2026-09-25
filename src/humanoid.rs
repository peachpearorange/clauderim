use {crate::{model::{self, Hoop, Piece, ball, block, curve, lathe, loft, rod, taper,
                     tube},
             stuff::{Stuff, Stuffs}},
     bevy::{camera::visibility::RenderLayers, light::NotShadowCaster, prelude::*},
     std::f32::consts::{FRAC_PI_2, PI}};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joint {
  Pelvis,
  Chest,
  Head,
  ArmL,
  ElbowL,
  ArmR,
  ElbowR,
  LegL,
  KneeL,
  LegR,
  KneeR
}

pub const JOINTS: usize = 11;

impl Joint {
  pub const ALL: [Joint; JOINTS] = [
    Joint::Pelvis,
    Joint::Chest,
    Joint::Head,
    Joint::ArmL,
    Joint::ElbowL,
    Joint::ArmR,
    Joint::ElbowR,
    Joint::LegL,
    Joint::KneeL,
    Joint::LegR,
    Joint::KneeR
  ];

  const fn parent(self) -> Option<Joint> {
    match self {
      Joint::Pelvis => None,
      Joint::Chest | Joint::LegL | Joint::LegR => Some(Joint::Pelvis),
      Joint::Head | Joint::ArmL | Joint::ArmR => Some(Joint::Chest),
      Joint::ElbowL => Some(Joint::ArmL),
      Joint::ElbowR => Some(Joint::ArmR),
      Joint::KneeL => Some(Joint::LegL),
      Joint::KneeR => Some(Joint::LegR)
    }
  }
}

#[derive(Clone, Copy)]
pub struct Frame {
  pub hip: f32,
  pub waist: f32,
  pub neck: f32,
  pub shoulder: Vec2,
  pub upper_arm: f32,
  pub forearm: f32,
  pub hip_width: f32,
  pub thigh: f32,
  pub shin: f32
}

pub const MAN: Frame = Frame {
  hip: 0.98,
  waist: 0.10,
  neck: 0.50,
  shoulder: Vec2::new(0.215, 0.44),
  upper_arm: 0.30,
  forearm: 0.27,
  hip_width: 0.10,
  thigh: 0.46,
  shin: 0.46
};

impl Frame {
  fn rest(&self, joint: Joint) -> Vec3 {
    match joint {
      Joint::Pelvis => Vec3::Y * self.hip,
      Joint::Chest => Vec3::Y * self.waist,
      Joint::Head => Vec3::Y * self.neck,
      Joint::ArmL => Vec3::new(-self.shoulder.x, self.shoulder.y, 0.0),
      Joint::ArmR => Vec3::new(self.shoulder.x, self.shoulder.y, 0.0),
      Joint::ElbowL | Joint::ElbowR => Vec3::NEG_Y * self.upper_arm,
      Joint::LegL => Vec3::new(-self.hip_width, -0.05, 0.0),
      Joint::LegR => Vec3::new(self.hip_width, -0.05, 0.0),
      Joint::KneeL | Joint::KneeR => Vec3::NEG_Y * self.thigh
    }
  }

  pub fn hand(&self) -> Vec3 { Vec3::NEG_Y * (self.forearm + 0.02) }
}

#[derive(Clone, Copy, Default)]
pub struct Pose {
  pub angles: [Vec3; JOINTS],
  pub drop: f32,
  pub lean: f32
}

impl Pose {
  fn with(mut self, joint: Joint, angles: Vec3) -> Self {
    self.angles[joint as usize] = angles;
    self
  }

  fn add(mut self, joint: Joint, angles: Vec3) -> Self {
    self.angles[joint as usize] += angles;
    self
  }

  pub fn blend(&self, other: &Pose, amount: f32) -> Pose {
    Pose {
      angles: std::array::from_fn(|index| {
        self.angles[index].lerp(other.angles[index], amount)
      }),
      drop: self.drop.lerp(other.drop, amount),
      lean: self.lean.lerp(other.lean, amount)
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grip {
  Blade,
  Axe
}

#[derive(Component, Clone)]
pub struct Rig {
  pub bones: [Entity; JOINTS],
  pub frame: Frame,
  pub grip: Grip,
  pub hunch: f32
}

#[derive(Component, Default)]
pub struct Motion {
  pub speed: f32,
  pub stride: f32,
  pub swing: Option<f32>,
  pub power: bool,
  pub guard: f32,
  pub fallen: f32,
  pub flinch: f32,
  pub airborne: f32,
  pub swim: f32,
  pub shout: f32,
  pub breath: f32
}

fn ready(rig: &Rig) -> Pose {
  let hunch = rig.hunch;
  Pose::default()
    .with(Joint::Chest, Vec3::new(0.08 + hunch, 0.0, 0.0))
    .with(Joint::Head, Vec3::new(-0.05 - hunch * 0.8, 0.0, 0.0))
    .with(Joint::ArmR, Vec3::new(0.35, 0.0, 0.14))
    .with(Joint::ElbowR, Vec3::new(0.95, 0.0, 0.0))
    .with(Joint::ArmL, Vec3::new(0.25, 0.0, -0.18))
    .with(Joint::ElbowL, Vec3::new(0.85, 0.0, 0.0))
    .with(Joint::LegL, Vec3::new(0.08, 0.0, -0.04))
    .with(Joint::KneeL, Vec3::new(-0.14, 0.0, 0.0))
    .with(Joint::LegR, Vec3::new(-0.12, 0.0, 0.04))
    .with(Joint::KneeR, Vec3::new(-0.1, 0.0, 0.0))
}

fn walking(base: Pose, motion: &Motion) -> Pose {
  let pace = (motion.speed / 5.5).min(1.4);
  let (wave, lift) = (motion.stride.sin(), motion.stride.cos());
  let knee = |phase: f32| -(0.15 + 0.9 * pace) * (0.5 + 0.5 * phase).powi(2);
  base
    .add(Joint::LegL, Vec3::X * wave * 0.62 * pace)
    .add(Joint::LegR, Vec3::X * -wave * 0.62 * pace)
    .add(Joint::KneeL, Vec3::X * knee(-lift))
    .add(Joint::KneeR, Vec3::X * knee(lift))
    .add(Joint::ArmL, Vec3::X * -wave * 0.4 * pace)
    .add(Joint::ArmR, Vec3::X * wave * 0.22 * pace)
    .add(Joint::Chest, Vec3::new(0.12 * pace.powi(2), wave * 0.12 * pace, 0.0))
    .add(Joint::Pelvis, Vec3::Y * -wave * 0.1 * pace)
}

fn swinging(base: Pose, progress: f32, power: bool, grip: Grip) -> Pose {
  let heave = power.then_some(1.25).unwrap_or(1.0);
  let windup = base
    .with(Joint::ArmR, Vec3::new(2.3 * heave, -0.4, 0.75))
    .with(Joint::ElbowR, Vec3::new(1.5, 0.0, 0.0))
    .with(Joint::Chest, Vec3::new(-0.05, -0.55 * heave, 0.0))
    .with(Joint::Pelvis, Vec3::new(0.0, -0.18, 0.0));
  let strike = base
    .with(Joint::ArmR, Vec3::new(0.9, 0.9, -0.55))
    .with(Joint::ElbowR, Vec3::new(0.2, 0.0, 0.0))
    .with(Joint::Chest, Vec3::new(0.3, 0.6 * heave, 0.0))
    .with(Joint::Pelvis, Vec3::new(0.0, 0.25, 0.0))
    .with(Joint::LegR, Vec3::new(-0.35, 0.0, 0.05))
    .with(Joint::LegL, Vec3::new(0.35, 0.0, -0.05))
    .with(Joint::KneeL, Vec3::new(-0.45, 0.0, 0.0));
  let chop = (grip == Grip::Axe).then(|| {
    strike
      .with(Joint::ArmR, Vec3::new(0.6, 0.3, -0.15))
      .with(Joint::Chest, Vec3::new(0.45, 0.25 * heave, 0.0))
  });
  let strike = chop.unwrap_or(strike);
  let (gather, release) = (0.38, 0.62);
  match progress {
    early if early < gather => base.blend(&windup, ease(early / gather)),
    mid if mid < release => {
      windup.blend(&strike, ease((mid - gather) / (release - gather)))
    }
    late => strike.blend(&base, ease((late - release) / (1.0 - release)))
  }
}

fn ease(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

fn guarding(base: Pose) -> Pose {
  base
    .with(Joint::ArmL, Vec3::new(0.4, -0.4, 0.0))
    .with(Joint::ElbowL, Vec3::new(1.25, -0.75, 0.0))
    .with(Joint::ArmR, Vec3::new(0.6, 0.0, 0.2))
    .with(Joint::Chest, Vec3::new(0.18, -0.12, 0.0))
    .with(Joint::KneeL, Vec3::new(-0.35, 0.0, 0.0))
    .with(Joint::KneeR, Vec3::new(-0.35, 0.0, 0.0))
}

fn shouting(base: Pose) -> Pose {
  base
    .with(Joint::Chest, Vec3::new(-0.1, 0.0, 0.0))
    .with(Joint::Head, Vec3::new(-0.35, 0.0, 0.0))
    .with(Joint::ArmL, Vec3::new(0.3, 0.0, -0.9))
    .with(Joint::ArmR, Vec3::new(0.3, 0.0, 0.9))
    .with(Joint::ElbowL, Vec3::new(0.2, 0.0, 0.0))
    .with(Joint::ElbowR, Vec3::new(0.5, 0.0, 0.0))
}

fn swimming(base: Pose, motion: &Motion) -> Pose {
  let pace = (motion.speed / 2.5).min(1.0);
  let stroke = motion.stride;
  let reach = |phase: f32| Vec3::new(0.5 + pace - (0.4 + pace) * phase.cos(), 0.0, 0.0);
  let bend = |phase: f32| Vec3::X * (0.3 + 0.9 * phase.sin().max(0.0));
  let kick = |phase: f32| Vec3::X * (0.05 + 0.3 * (3.0 * phase).sin());
  Pose { drop: -0.2 - 0.1 * pace, lean: -0.15 - 1.2 * pace, ..base }
    .with(Joint::Chest, Vec3::new(0.05, 0.15 * stroke.sin(), 0.0))
    .with(Joint::Head, Vec3::new(-0.3 - 0.6 * pace, 0.0, 0.0))
    .with(Joint::ArmL, reach(stroke) + Vec3::Z * -0.25)
    .with(Joint::ArmR, reach(stroke + std::f32::consts::PI) + Vec3::Z * 0.25)
    .with(Joint::ElbowL, bend(stroke))
    .with(Joint::ElbowR, bend(stroke + std::f32::consts::PI))
    .with(Joint::LegL, kick(stroke) + Vec3::Z * -0.06)
    .with(Joint::LegR, kick(stroke + std::f32::consts::PI) + Vec3::Z * 0.06)
    .with(Joint::KneeL, Vec3::X * -0.25)
    .with(Joint::KneeR, Vec3::X * -0.25)
}

fn fallen(base: Pose) -> Pose {
  Pose { drop: 0.82, lean: -1.45, ..base }
    .with(Joint::ArmL, Vec3::new(2.4, 0.0, -0.9))
    .with(Joint::ArmR, Vec3::new(2.2, 0.0, 1.1))
    .with(Joint::ElbowL, Vec3::new(0.4, 0.0, 0.0))
    .with(Joint::ElbowR, Vec3::new(0.7, 0.0, 0.0))
    .with(Joint::Head, Vec3::new(0.4, 0.5, 0.0))
    .with(Joint::LegL, Vec3::new(0.35, 0.0, -0.2))
    .with(Joint::LegR, Vec3::new(0.1, 0.0, 0.25))
    .with(Joint::KneeL, Vec3::new(-0.5, 0.0, 0.0))
    .with(Joint::KneeR, Vec3::new(-0.1, 0.0, 0.0))
}

pub fn posed(rig: &Rig, motion: &Motion) -> Pose {
  let base = ready(rig).add(Joint::Chest, Vec3::X * (motion.breath.sin() * 0.025));
  let walked = walking(base, motion);
  let airborne = walked
    .with(Joint::LegL, Vec3::new(0.6, 0.0, 0.0))
    .with(Joint::KneeL, Vec3::new(-1.1, 0.0, 0.0))
    .with(Joint::LegR, Vec3::new(-0.1, 0.0, 0.0))
    .with(Joint::KneeR, Vec3::new(-0.5, 0.0, 0.0))
    .with(Joint::ArmL, Vec3::new(0.2, 0.0, -0.6));
  let moved =
    walked.blend(&airborne, motion.airborne).blend(&swimming(base, motion), motion.swim);
  let guarded = moved.blend(&guarding(moved), motion.guard);
  let swung = motion
    .swing
    .map(|progress| swinging(guarded, progress, motion.power, rig.grip))
    .unwrap_or(guarded);
  let shouted = swung.blend(&shouting(swung), motion.shout);
  let flinched = shouted
    .add(Joint::Chest, Vec3::new(-0.4, 0.2, 0.0) * motion.flinch)
    .add(Joint::Head, Vec3::new(-0.4, 0.0, 0.0) * motion.flinch);
  flinched.blend(&fallen(flinched), motion.fallen)
}

fn animate(
  time: Res<Time>,
  mut rigs: Query<(&Rig, &mut Motion)>,
  mut bones: Query<&mut Transform>
) {
  let blend = 1.0 - (-18.0 * time.delta_secs()).exp();
  rigs.iter_mut().for_each(|(rig, mut motion)| {
    motion.breath += time.delta_secs() * 1.6;
    let pose = posed(rig, &motion);
    rig.bones.iter().zip(Joint::ALL).for_each(|(&bone, joint)| {
      if let Ok(mut transform) = bones.get_mut(bone) {
        let angles = pose.angles[joint as usize];
        let target = Quat::from_euler(EulerRot::YXZ, angles.y, angles.x, angles.z);
        let target = (joint == Joint::Pelvis)
          .then(|| Quat::from_rotation_x(pose.lean) * target)
          .unwrap_or(target);
        transform.rotation = transform.rotation.slerp(target, blend);
        if joint == Joint::Pelvis {
          let height = rig.frame.hip - pose.drop;
          transform.translation.y = transform.translation.y.lerp(height, blend);
        }
      }
    });
  });
}

pub struct Kit(Vec<(Joint, Stuff, Piece)>);

impl Kit {
  pub fn new() -> Self { Self(Vec::new()) }

  pub fn add(&mut self, joint: Joint, stuff: Stuff, piece: Piece) -> &mut Self {
    self.0.push((joint, stuff, piece));
    self
  }

  pub fn both(&mut self, joint: Joint, stuff: Stuff, piece: Piece) -> &mut Self {
    let mirrored = piece.mirrored();
    let twin = match joint {
      Joint::ArmR => Joint::ArmL,
      Joint::ElbowR => Joint::ElbowL,
      Joint::LegR => Joint::LegL,
      Joint::KneeR => Joint::KneeL,
      other => other
    };
    self.add(joint, stuff, piece).add(twin, stuff, mirrored)
  }
}

#[derive(Component)]
pub struct Hidden1st;

pub const SHADOW_ONLY: RenderLayers = RenderLayers::layer(1);

pub fn shadowing() -> RenderLayers { RenderLayers::layer(0).union(&SHADOW_ONLY) }

pub fn spawn_body(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  owner: Entity,
  frame: Frame,
  grip: Grip,
  hunch: f32,
  kit: Kit
) -> Rig {
  let mut bones = [Entity::PLACEHOLDER; JOINTS];
  Joint::ALL.into_iter().for_each(|joint| {
    let parent = joint.parent().map_or(owner, |parent| bones[parent as usize]);
    bones[joint as usize] = commands
      .spawn((
        Transform::from_translation(frame.rest(joint)),
        Visibility::Inherited,
        ChildOf(parent)
      ))
      .id();
  });
  let Kit(pieces) = kit;
  let groups = pieces.into_iter().fold(
    Vec::<(Joint, Stuff, Vec<Piece>)>::new(),
    |mut groups, (joint, stuff, piece)| {
      match groups.iter_mut().find(|(each, kind, _)| *each == joint && *kind == stuff) {
        Some((_, _, list)) => list.push(piece),
        None => groups.push((joint, stuff, vec![piece]))
      }
      groups
    }
  );
  groups.into_iter().for_each(|(joint, stuff, list)| {
    let mut part = commands.spawn((
      Mesh3d(meshes.add(model::merge(list))),
      MeshMaterial3d(stuffs.of(stuff)),
      ChildOf(bones[joint as usize])
    ));
    if matches!(stuff, Stuff::Frost | Stuff::Ember) {
      part.insert(NotShadowCaster);
    }
  });
  Rig { bones, frame, grip, hunch }
}

fn srgb(red: f32, green: f32, blue: f32) -> Srgba { Srgba::new(red, green, blue, 1.0) }

fn shell(profile: &[(f32, f32)], sides: u32) -> bevy::mesh::Mesh {
  lathe(
    &profile
      .iter()
      .map(|&(radius, height)| Vec2::new(radius, height))
      .collect::<Vec<_>>(),
    sides
  )
}

#[derive(Clone, Copy)]
pub struct Build {
  pub limbs: f32,
  pub chest: f32,
  pub waist: f32,
  pub cheeks: f32
}

impl Build {
  pub const HALE: Build = Build { limbs: 1.0, chest: 1.0, waist: 1.0, cheeks: 1.0 };
  pub const WITHERED: Build =
    Build { limbs: 0.74, chest: 0.9, waist: 0.76, cheeks: 0.84 };
}

const LIMB_SIDES: u32 = 20;
const TRUNK_SIDES: u32 = 28;

fn hoop_at(hoops: &[Hoop], y: f32) -> Hoop {
  hoops
    .windows(2)
    .find(|pair| (pair[0].at.y - y) * (pair[1].at.y - y) <= 0.0)
    .map(|pair| {
      let span = pair[1].at.y - pair[0].at.y;
      pair[0].lerp(pair[1], ((y - pair[0].at.y) / span).clamp(0.0, 1.0))
    })
    .unwrap_or(hoops[0])
}

fn sheath(hoops: &[Hoop], from: f32, to: f32, scale: f32) -> bevy::mesh::Mesh {
  let (low, high) = (from.min(to), from.max(to));
  let inner = hoops.iter().copied().filter(|hoop| hoop.at.y > low && hoop.at.y < high);
  let ordered: Vec<Hoop> = std::iter::once(hoop_at(hoops, from))
    .chain(inner)
    .chain(std::iter::once(hoop_at(hoops, to)))
    .collect();
  let ordered = ((ordered[0].at.y < ordered[ordered.len() - 1].at.y)
    != (hoops[0].at.y < hoops[hoops.len() - 1].at.y))
    .then(|| ordered.iter().rev().copied().collect())
    .unwrap_or(ordered);
  loft(&ordered.iter().map(|hoop| hoop.scaled(scale)).collect::<Vec<_>>(), TRUNK_SIDES)
}

fn head_hoops(build: &Build) -> Vec<Hoop> {
  [
    Hoop::pole(0.03).shifted(0.0, -0.02),
    Hoop::new(0.045, 0.036, 0.068, 0.035).shifted(0.0, -0.03),
    Hoop::new(0.075, 0.058, 0.085, 0.062).shifted(0.0, -0.02),
    Hoop::new(0.11, 0.068, 0.095, 0.085).shifted(0.0, -0.015),
    Hoop::new(0.15, 0.075, 0.098, 0.1).shifted(0.0, -0.01),
    Hoop::new(0.185, 0.078, 0.098, 0.105).shifted(0.0, -0.005),
    Hoop::new(0.225, 0.072, 0.086, 0.1),
    Hoop::new(0.255, 0.056, 0.064, 0.076).shifted(0.0, 0.005),
    Hoop::new(0.275, 0.031, 0.034, 0.041).shifted(0.0, 0.008),
    Hoop::pole(0.284).shifted(0.0, 0.008)
  ]
  .into_iter()
  .map(|hoop| {
    let hollow =
      (hoop.at.y > 0.06 && hoop.at.y < 0.13).then_some(build.cheeks).unwrap_or(1.0);
    Hoop { wide: hoop.wide * hollow, ..hoop }
  })
  .collect()
}

fn chest_hoops(build: &Build) -> Vec<Hoop> {
  [
    Hoop::new(-0.06, 0.145, 0.1, 0.095),
    Hoop::new(0.02, 0.14, 0.1, 0.092),
    Hoop::new(0.1, 0.15, 0.108, 0.098),
    Hoop::new(0.2, 0.166, 0.122, 0.104),
    Hoop::new(0.28, 0.18, 0.133, 0.11),
    Hoop::new(0.34, 0.19, 0.13, 0.115),
    Hoop::new(0.4, 0.183, 0.11, 0.11),
    Hoop::new(0.445, 0.148, 0.085, 0.092),
    Hoop::new(0.48, 0.085, 0.06, 0.066),
    Hoop::new(0.505, 0.055, 0.05, 0.056),
    Hoop::pole(0.515)
  ]
  .into_iter()
  .map(|hoop| {
    hoop.scaled(build.waist.lerp(build.chest, (hoop.at.y / 0.3).clamp(0.0, 1.0)))
  })
  .collect()
}

fn hip_hoops(build: &Build) -> Vec<Hoop> {
  [
    Hoop::pole(-0.17),
    Hoop::new(-0.155, 0.07, 0.06, 0.07),
    Hoop::new(-0.11, 0.15, 0.1, 0.115),
    Hoop::new(-0.05, 0.163, 0.105, 0.12),
    Hoop::new(0.0, 0.155, 0.1, 0.105),
    Hoop::new(0.06, 0.145, 0.1, 0.095),
    Hoop::pole(0.08)
  ]
  .into_iter()
  .map(|hoop| hoop.scaled(build.waist))
  .collect()
}

fn limb_hoops(hoops: &[Hoop], build: &Build) -> Vec<Hoop> {
  hoops.iter().map(|hoop| hoop.scaled(build.limbs)).collect()
}

const UPPER_ARM: [Hoop; 9] = [
  Hoop::pole(0.06).shifted(0.01, 0.0),
  Hoop::new(0.035, 0.052, 0.055, 0.055).shifted(0.012, 0.0),
  Hoop::new(0.0, 0.066, 0.062, 0.062).shifted(0.01, 0.0),
  Hoop::new(-0.05, 0.062, 0.06, 0.058).shifted(0.006, 0.0),
  Hoop::new(-0.1, 0.052, 0.054, 0.05),
  Hoop::new(-0.16, 0.046, 0.056, 0.048),
  Hoop::new(-0.23, 0.042, 0.046, 0.044),
  Hoop::new(-0.29, 0.04, 0.04, 0.042),
  Hoop::pole(-0.32)
];

const FOREARM: [Hoop; 7] = [
  Hoop::pole(0.035),
  Hoop::new(0.0, 0.042, 0.04, 0.044),
  Hoop::new(-0.05, 0.047, 0.046, 0.044),
  Hoop::new(-0.11, 0.041, 0.04, 0.036),
  Hoop::new(-0.19, 0.031, 0.031, 0.028),
  Hoop::new(-0.255, 0.024, 0.03, 0.028),
  Hoop::pole(-0.27)
];

const FIST: [Hoop; 7] = [
  Hoop::pole(-0.245),
  Hoop::new(-0.26, 0.026, 0.036, 0.032),
  Hoop::new(-0.285, 0.03, 0.048, 0.04),
  Hoop::new(-0.315, 0.03, 0.05, 0.04),
  Hoop::new(-0.335, 0.024, 0.042, 0.032),
  Hoop::new(-0.345, 0.012, 0.022, 0.016),
  Hoop::pole(-0.348)
];

const THIGH: [Hoop; 9] = [
  Hoop::pole(0.04),
  Hoop::new(0.0, 0.086, 0.086, 0.09).shifted(0.008, 0.0),
  Hoop::new(-0.07, 0.092, 0.095, 0.09).shifted(0.005, 0.0),
  Hoop::new(-0.17, 0.087, 0.093, 0.08),
  Hoop::new(-0.28, 0.075, 0.08, 0.068),
  Hoop::new(-0.38, 0.058, 0.062, 0.055),
  Hoop::new(-0.45, 0.052, 0.056, 0.05),
  Hoop::new(-0.49, 0.048, 0.05, 0.046),
  Hoop::pole(-0.51)
];

const SHIN: [Hoop; 8] = [
  Hoop::pole(0.04),
  Hoop::new(0.0, 0.05, 0.054, 0.05),
  Hoop::new(-0.06, 0.052, 0.05, 0.064),
  Hoop::new(-0.13, 0.054, 0.046, 0.074),
  Hoop::new(-0.22, 0.045, 0.04, 0.058),
  Hoop::new(-0.31, 0.036, 0.036, 0.038),
  Hoop::new(-0.39, 0.033, 0.034, 0.032),
  Hoop::pole(-0.41)
];

const FOOT: [Hoop; 7] = [
  Hoop::pole(-0.055),
  Hoop::new(-0.04, 0.032, 0.035, 0.035),
  Hoop::new(0.0, 0.04, 0.035, 0.062),
  Hoop::new(0.06, 0.046, 0.035, 0.046),
  Hoop::new(0.12, 0.049, 0.03, 0.03),
  Hoop::new(0.165, 0.042, 0.026, 0.022),
  Hoop::pole(0.19)
];

fn horned_helmet(kit: &mut Kit, iron: Srgba, horn: Srgba, horn_size: f32) {
  let stretch = Vec3::new(1.0, 1.0, 1.2);
  let dome = shell(
    &[
      (0.093, 0.15),
      (0.097, 0.19),
      (0.092, 0.235),
      (0.074, 0.275),
      (0.04, 0.302),
      (0.0, 0.311)
    ],
    TRUNK_SIDES
  );
  let crest: Vec<Vec3> = (0..=16)
    .map(|step| {
      let angle = -1.25 + step as f32 / 16.0 * 2.35;
      Vec3::new(0.0, 0.16 + 0.152 * angle.cos(), 0.118 * angle.sin())
    })
    .collect();
  kit
    .add(Joint::Head, Stuff::Iron, Piece::new(dome, iron).sized(stretch))
    .add(
      Joint::Head,
      Stuff::Iron,
      Piece::new(rod(0.098, 0.03), iron * 0.8).sized(stretch).at_xyz(0.0, 0.16, 0.0)
    )
    .add(Joint::Head, Stuff::Iron, Piece::new(tube(&crest, &[0.011], 8), iron * 0.9))
    .add(
      Joint::Head,
      Stuff::Iron,
      Piece::new(block(0.018, 0.05, 0.012), iron).at_xyz(0.0, 0.155, -0.118)
    )
    .both(
      Joint::Head,
      Stuff::Iron,
      Piece::new(block(0.012, 0.085, 0.07), iron * 0.85)
        .rolled(0.08)
        .at_xyz(0.09, 0.12, -0.02)
    );
  let base = Vec3::new(0.085, 0.24, 0.0);
  let horn_path =
    curve(base, Vec3::new(0.22, 0.26, 0.03), Vec3::new(0.25, 0.4, -0.08), 12)
      .into_iter()
      .map(|point| base + (point - base) * horn_size)
      .collect::<Vec<_>>();
  let horn_mesh = tube(&horn_path, &taper(12, 0.03 * horn_size, 0.003), 12);
  kit.both(Joint::Head, Stuff::Bone, Piece::new(horn_mesh, horn));
}

fn face(
  kit: &mut Kit,
  build: &Build,
  skin: Srgba,
  beard: Srgba,
  eyes: Stuff,
  eye_color: Srgba
) {
  let nose = loft(
    &[
      Hoop::new(0.17, 0.008, 0.006, 0.0).shifted(0.0, -0.098),
      Hoop::new(0.125, 0.011, 0.03, 0.0).shifted(0.0, -0.098),
      Hoop::new(0.106, 0.018, 0.024, 0.0).shifted(0.0, -0.098),
      Hoop::pole(0.098).shifted(0.0, -0.11)
    ],
    12
  );
  let neck = loft(
    &[
      Hoop::new(-0.04, 0.06, 0.055, 0.06).shifted(0.0, 0.005),
      Hoop::new(0.02, 0.055, 0.05, 0.055).shifted(0.0, -0.005),
      Hoop::new(0.07, 0.052, 0.047, 0.052).shifted(0.0, -0.015),
      Hoop::new(0.1, 0.045, 0.04, 0.045).shifted(0.0, -0.02)
    ]
    .map(|hoop| hoop.scaled(build.limbs.sqrt())),
    LIMB_SIDES
  );
  let beard_mass = loft(
    &[
      Hoop::new(0.098, 0.074, 0.1, 0.03).shifted(0.0, -0.015),
      Hoop::new(0.06, 0.066, 0.104, 0.04).shifted(0.0, -0.02),
      Hoop::new(0.025, 0.046, 0.078, 0.035).shifted(0.0, -0.04),
      Hoop::new(-0.005, 0.022, 0.044, 0.02).shifted(0.0, -0.062),
      Hoop::pole(-0.015).shifted(0.0, -0.072)
    ],
    LIMB_SIDES
  );
  let moustache = curve(
    Vec3::new(-0.04, 0.078, -0.098),
    Vec3::new(0.0, 0.108, -0.132),
    Vec3::new(0.04, 0.078, -0.098),
    10
  );
  kit
    .add(
      Joint::Head,
      Stuff::Skin,
      Piece::new(loft(&head_hoops(build), TRUNK_SIDES), skin)
    )
    .add(Joint::Head, Stuff::Skin, Piece::new(nose, skin))
    .add(Joint::Head, Stuff::Skin, Piece::new(neck, skin))
    .add(
      Joint::Head,
      Stuff::Skin,
      Piece::new(ball(1.0), skin * 0.95)
        .sized(Vec3::new(0.068, 0.014, 0.02))
        .at_xyz(0.0, 0.172, -0.098)
    )
    .both(
      Joint::Head,
      Stuff::Skin,
      Piece::new(ball(1.0), skin * 0.92)
        .sized(Vec3::new(0.012, 0.03, 0.019))
        .at_xyz(0.076, 0.14, 0.012)
    )
    .add(Joint::Head, Stuff::Fur, Piece::new(beard_mass, beard))
    .add(
      Joint::Head,
      Stuff::Fur,
      Piece::new(tube(&moustache, &[0.004, 0.012, 0.004], 8), beard)
    )
    .both(
      Joint::Head,
      eyes,
      Piece::new(ball(0.012), eye_color).at_xyz(0.033, 0.152, -0.094)
    );
}

fn hair(kit: &mut Kit, color: Srgba) {
  let cap = loft(
    &[
      Hoop::new(0.1, 0.078, 0.02, 0.106),
      Hoop::new(0.19, 0.084, 0.108, 0.113).shifted(0.0, -0.005),
      Hoop::new(0.23, 0.078, 0.095, 0.108),
      Hoop::new(0.26, 0.061, 0.072, 0.083).shifted(0.0, 0.005),
      Hoop::new(0.28, 0.034, 0.037, 0.046).shifted(0.0, 0.008),
      Hoop::pole(0.29).shifted(0.0, 0.008)
    ],
    TRUNK_SIDES
  );
  kit.add(Joint::Head, Stuff::Fur, Piece::new(cap, color));
}

pub fn iron_sword(kit: &mut Kit, frame: &Frame) {
  let hand = frame.hand();
  let steel = srgb(0.78, 0.8, 0.83);
  let grip_forward = |piece: Piece| piece.yawed(FRAC_PI_2).pitched(-FRAC_PI_2).at(hand);
  kit
    .add(
      Joint::ElbowR,
      Stuff::Steel,
      grip_forward(
        Piece::new(model::blade(0.82, 0.058, 0.014, 0.14), steel).at_xyz(0.0, 0.12, 0.0)
      )
    )
    .add(
      Joint::ElbowR,
      Stuff::Iron,
      grip_forward(
        Piece::new(block(0.2, 0.028, 0.04), srgb(0.4, 0.4, 0.42)).at_xyz(0.0, 0.11, 0.0)
      )
    )
    .add(
      Joint::ElbowR,
      Stuff::Leather,
      grip_forward(
        Piece::new(rod(0.019, 0.17), srgb(0.28, 0.18, 0.1)).at_xyz(0.0, 0.02, 0.0)
      )
    )
    .add(
      Joint::ElbowR,
      Stuff::Iron,
      grip_forward(
        Piece::new(ball(0.028), srgb(0.4, 0.4, 0.42)).at_xyz(0.0, -0.075, 0.0)
      )
    );
}

pub fn war_axe(kit: &mut Kit, frame: &Frame, metal: Srgba) {
  let hand = frame.hand();
  let grip_forward = |piece: Piece| piece.pitched(-FRAC_PI_2).at(hand);
  let head = model::fan(
    &[
      Vec2::new(0.0, 0.0),
      Vec2::new(0.05, -0.02),
      Vec2::new(0.17, -0.08),
      Vec2::new(0.2, 0.02),
      Vec2::new(0.2, 0.13),
      Vec2::new(0.15, 0.1),
      Vec2::new(0.05, 0.06),
      Vec2::new(0.0, 0.06)
    ],
    0.022
  );
  kit
    .add(
      Joint::ElbowR,
      Stuff::Wood,
      grip_forward(
        Piece::new(rod(0.02, 0.62), srgb(0.3, 0.22, 0.14)).at_xyz(0.0, 0.2, 0.0)
      )
    )
    .add(
      Joint::ElbowR,
      Stuff::Iron,
      grip_forward(
        Piece::new(head, metal).yawed(FRAC_PI_2).rolled(0.0).at_xyz(0.0, 0.42, -0.01)
      )
    )
    .add(
      Joint::ElbowR,
      Stuff::Leather,
      grip_forward(
        Piece::new(rod(0.024, 0.14), srgb(0.2, 0.14, 0.09)).at_xyz(0.0, 0.0, 0.0)
      )
    );
}

pub fn round_shield(kit: &mut Kit, frame: &Frame) {
  let center = Vec3::new(-0.08, -frame.forearm * 0.55, 0.0);
  let facing = |piece: Piece| piece.rolled(FRAC_PI_2).at(center);
  let wood = srgb(0.5, 0.36, 0.22);
  let iron = srgb(0.36, 0.36, 0.38);
  let rim: Vec<Vec3> = (0..=32)
    .map(|step| {
      let angle = step as f32 / 32.0 * 2.0 * PI;
      Vec3::new(angle.cos() * 0.34, 0.0, angle.sin() * 0.34)
    })
    .collect();
  kit
    .add(Joint::ElbowL, Stuff::Wood, facing(Piece::new(model::rod(0.34, 0.035), wood)))
    .add(Joint::ElbowL, Stuff::Iron, facing(Piece::new(tube(&rim, &[0.018], 6), iron)))
    .add(
      Joint::ElbowL,
      Stuff::Iron,
      facing(
        Piece::new(
          shell(&[(0.09, 0.0), (0.085, 0.03), (0.05, 0.06), (0.0, 0.07)], 14),
          iron
        )
        .at_xyz(0.0, 0.015, 0.0)
      )
    )
    .add(
      Joint::ElbowL,
      Stuff::Iron,
      facing(Piece::new(block(0.66, 0.012, 0.05), iron).at_xyz(0.0, 0.02, 0.0))
    )
    .add(
      Joint::ElbowL,
      Stuff::Iron,
      facing(Piece::new(block(0.05, 0.012, 0.66), iron).at_xyz(0.0, 0.02, 0.0))
    );
}

fn torso(
  kit: &mut Kit,
  build: &Build,
  under: (Stuff, Srgba),
  plate: Option<Srgba>,
  fur: Srgba,
  belt: Srgba
) {
  let hoops = chest_hoops(build);
  let (stuff, color) = under;
  kit.add(Joint::Chest, stuff, Piece::new(loft(&hoops, TRUNK_SIDES), color));
  if let Some(plate) = plate {
    kit.add(
      Joint::Chest,
      Stuff::Iron,
      Piece::new(sheath(&hoops, 0.07, 0.43, 1.08), plate)
    );
    [0.11, 0.18, 0.25].into_iter().for_each(|height| {
      kit.add(
        Joint::Chest,
        Stuff::Iron,
        Piece::new(sheath(&hoops, height - 0.009, height + 0.009, 1.12), plate * 0.75)
      );
    });
  }
  let front = hoop_at(&hoops, 0.03).front;
  kit
    .add(
      Joint::Chest,
      Stuff::Fur,
      Piece::new(model::lump(3, 0.12, 2), fur)
        .sized(Vec3::new(0.23, 0.075, 0.15) * build.chest)
        .at_xyz(0.0, 0.455, 0.005)
    )
    .add(
      Joint::Chest,
      Stuff::Fur,
      Piece::new(sheath(&hoops, 0.35, 0.47, 1.17), fur * 0.9)
    )
    .add(
      Joint::Chest,
      Stuff::Leather,
      Piece::new(sheath(&hoops, 0.005, 0.055, 1.1), belt)
    )
    .add(
      Joint::Chest,
      Stuff::Gold,
      Piece::new(block(0.06, 0.045, 0.02), srgb(0.75, 0.6, 0.3)).at_xyz(
        0.0,
        0.03,
        -front * 1.1 - 0.006
      )
    )
    .add(
      Joint::Chest,
      Stuff::Leather,
      Piece::new(block(0.05, 0.5, 0.012), belt).rolled(0.7).at_xyz(
        0.0,
        0.26,
        -0.152 * build.chest
      )
    );
}

fn skirt(kit: &mut Kit, build: &Build, leather: Srgba, cloth: Srgba) {
  kit.add(
    Joint::Pelvis,
    Stuff::Cloth,
    Piece::new(loft(&hip_hoops(build), TRUNK_SIDES), cloth)
  );
  [
    (0.0, -1.0, 0.16),
    (0.9, -0.7, 0.13),
    (-0.9, -0.7, 0.13),
    (0.0, 1.0, 0.16),
    (1.3, 0.0, 0.12),
    (-1.3, 0.0, 0.12)
  ]
  .into_iter()
  .for_each(|(angle, depth, width)| {
    let outward = Vec3::new((angle as f32).sin(), 0.0, depth);
    kit.add(
      Joint::Pelvis,
      Stuff::Leather,
      Piece::new(block(width, 0.34, 0.015), leather)
        .pitched(-0.12 * depth.signum())
        .yawed(angle * 1.2)
        .at(outward * Vec3::new(0.18, 0.0, 0.13) * build.waist + Vec3::Y * -0.14)
    );
  });
}

fn arms(
  kit: &mut Kit,
  build: &Build,
  sleeve: (Stuff, Srgba),
  bracer: Srgba,
  hand: (Stuff, Srgba),
  pauldron: Option<Srgba>
) {
  let (sleeve_stuff, sleeve_color) = sleeve;
  let (hand_stuff, hand_color) = hand;
  let thumb = curve(
    Vec3::new(-0.018, -0.262, -0.026),
    Vec3::new(-0.036, -0.285, -0.046),
    Vec3::new(-0.024, -0.305, -0.052),
    8
  );
  kit
    .both(
      Joint::ArmR,
      sleeve_stuff,
      Piece::new(loft(&limb_hoops(&UPPER_ARM, build), LIMB_SIDES), sleeve_color)
    )
    .both(
      Joint::ElbowR,
      Stuff::Leather,
      Piece::new(loft(&limb_hoops(&FOREARM, build), LIMB_SIDES), bracer)
    )
    .both(
      Joint::ElbowR,
      hand_stuff,
      Piece::new(loft(&limb_hoops(&FIST, build), LIMB_SIDES), hand_color)
    )
    .both(
      Joint::ElbowR,
      hand_stuff,
      Piece::new(tube(&thumb, &taper(8, 0.012, 0.009), 8), hand_color)
    );
  if let Some(plate) = pauldron {
    let cap =
      shell(&[(0.105, -0.07), (0.1, -0.02), (0.08, 0.03), (0.04, 0.06), (0.0, 0.07)], 16);
    kit
      .both(
        Joint::ArmR,
        Stuff::Iron,
        Piece::new(cap.clone(), plate)
          .sized(Vec3::new(0.8, 0.62, 0.84))
          .rolled(-0.5)
          .at_xyz(0.025, 0.0, 0.0)
      )
      .both(
        Joint::ArmR,
        Stuff::Iron,
        Piece::new(cap, plate * 0.85)
          .sized(Vec3::new(0.7, 0.52, 0.74))
          .rolled(-0.7)
          .at_xyz(0.05, -0.085, 0.0)
      )
      .both(
        Joint::ElbowR,
        Stuff::Iron,
        Piece::new(sheath(&limb_hoops(&FOREARM, build), -0.13, -0.03, 1.15), plate)
      );
  }
}

fn legs(
  kit: &mut Kit,
  frame: &Frame,
  build: &Build,
  pants: Srgba,
  boot: Srgba,
  cuff: Option<Srgba>
) {
  let shin = limb_hoops(&SHIN, build);
  let top = -0.2;
  kit
    .both(
      Joint::LegR,
      Stuff::Cloth,
      Piece::new(loft(&limb_hoops(&THIGH, build), LIMB_SIDES), pants)
    )
    .both(Joint::KneeR, Stuff::Cloth, Piece::new(loft(&shin, LIMB_SIDES), pants))
    .both(
      Joint::KneeR,
      Stuff::Leather,
      Piece::new(sheath(&shin, top, -0.405, 1.14), boot)
    )
    .both(
      Joint::KneeR,
      Stuff::Leather,
      Piece::new(sheath(&shin, top + 0.015, top - 0.015, 1.22), boot * 0.8)
    )
    .both(
      Joint::KneeR,
      Stuff::Leather,
      Piece::new(loft(&FOOT, LIMB_SIDES), boot * 0.9).pitched(-FRAC_PI_2).at_xyz(
        0.0,
        -frame.shin + 0.035,
        0.0
      )
    );
  if let Some(cuff) = cuff {
    kit.both(
      Joint::KneeR,
      Stuff::Fur,
      Piece::new(model::lump(5, 0.14, 2), cuff)
        .sized(Vec3::new(0.068, 0.045, 0.078) * build.limbs)
        .at_xyz(0.0, top, 0.005)
    );
  }
}

pub fn dragonborn() -> Kit {
  let frame = MAN;
  let build = Build::HALE;
  let mut kit = Kit::new();
  let iron = srgb(0.5, 0.5, 0.52);
  let fur = srgb(0.42, 0.3, 0.2);
  let leather = srgb(0.3, 0.2, 0.13);
  let locks = srgb(0.36, 0.22, 0.12);
  face(
    &mut kit,
    &build,
    srgb(0.78, 0.6, 0.48),
    locks,
    Stuff::Gloss,
    srgb(0.1, 0.12, 0.14)
  );
  hair(&mut kit, locks);
  horned_helmet(&mut kit, iron, srgb(0.82, 0.76, 0.64), 1.0);
  torso(&mut kit, &build, (Stuff::Cloth, srgb(0.3, 0.26, 0.2)), Some(iron), fur, leather);
  skirt(&mut kit, &build, leather, srgb(0.25, 0.21, 0.17));
  arms(
    &mut kit,
    &build,
    (Stuff::Fur, fur),
    leather,
    (Stuff::Leather, srgb(0.26, 0.18, 0.12)),
    Some(iron)
  );
  legs(&mut kit, &frame, &build, srgb(0.24, 0.2, 0.16), srgb(0.3, 0.22, 0.15), Some(fur));
  iron_sword(&mut kit, &frame);
  round_shield(&mut kit, &frame);
  kit
}

pub fn draugr(seed: u32) -> Kit {
  let frame = MAN;
  let build = Build::WITHERED;
  let mut kit = Kit::new();
  let flesh = srgb(0.36, 0.33, 0.27);
  let rags = srgb(0.22, 0.2, 0.17);
  let ancient = srgb(0.3, 0.33, 0.3);
  face(&mut kit, &build, flesh, srgb(0.7, 0.68, 0.62), Stuff::Frost, srgb(0.5, 0.8, 1.0));
  kit.add(
    Joint::Head,
    Stuff::Fur,
    Piece::new(model::lump(seed, 0.2, 2), srgb(0.62, 0.6, 0.55))
      .sized(Vec3::new(0.09, 0.19, 0.07))
      .at_xyz(0.0, 0.08, 0.085)
  );
  (seed % 2 == 0).then(|| horned_helmet(&mut kit, ancient, srgb(0.55, 0.5, 0.42), 0.6));
  torso(
    &mut kit,
    &build,
    (Stuff::Skin, flesh * 0.85),
    (seed % 3 != 0).then_some(ancient),
    rags,
    srgb(0.18, 0.15, 0.12)
  );
  skirt(&mut kit, &build, rags, rags * 0.8);
  arms(
    &mut kit,
    &build,
    (Stuff::Skin, flesh),
    rags,
    (Stuff::Skin, flesh * 0.9),
    (seed % 3 == 1).then_some(ancient)
  );
  legs(&mut kit, &frame, &build, rags, srgb(0.2, 0.17, 0.14), None);
  war_axe(&mut kit, &frame, srgb(0.28, 0.3, 0.28));
  kit
}

pub fn bandit(seed: u32) -> Kit {
  let frame = MAN;
  let build = Build::HALE;
  let mut kit = Kit::new();
  let fur = srgb(0.5, 0.4, 0.3);
  let leather = srgb(0.34, 0.24, 0.16);
  let locks = [srgb(0.3, 0.2, 0.1), srgb(0.6, 0.45, 0.25), srgb(0.15, 0.1, 0.08)]
    [seed as usize % 3];
  face(&mut kit, &build, srgb(0.8, 0.62, 0.5), locks, Stuff::Gloss, srgb(0.1, 0.1, 0.1));
  hair(&mut kit, locks);
  torso(&mut kit, &build, (Stuff::Cloth, srgb(0.36, 0.3, 0.22)), None, fur, leather);
  skirt(&mut kit, &build, leather, srgb(0.3, 0.25, 0.2));
  arms(
    &mut kit,
    &build,
    (Stuff::Cloth, srgb(0.35, 0.3, 0.24)),
    leather,
    (Stuff::Leather, leather),
    None
  );
  legs(&mut kit, &frame, &build, srgb(0.3, 0.26, 0.2), leather, Some(fur));
  (seed % 2 == 0).then(|| iron_sword(&mut kit, &frame));
  (seed % 2 == 1).then(|| war_axe(&mut kit, &frame, srgb(0.45, 0.45, 0.47)));
  kit
}

pub fn plugin(app: &mut App) {
  app.add_systems(PostUpdate, animate.before(TransformSystems::Propagate));
}
