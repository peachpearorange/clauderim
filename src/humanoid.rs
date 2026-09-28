use {crate::{combat::Dead,
             face::{self, Beard, Person, Race, Style},
             model::{self, Hoop, Piece, ball, block, curve, lathe, loft, rod, taper,
                     tube},
             ragdoll::{Limb, Link},
             stuff::{Stuff, Stuffs}},
     bevy::{camera::visibility::{DynamicSkinnedMeshBounds, RenderLayers},
            light::NotShadowCaster,
            mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
            prelude::*},
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
  Axe,
  Bare,
  Torch
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
  pub breath: f32,
  pub crouch: f32
}

fn relaxed(rig: &Rig) -> Pose {
  let hunch = rig.hunch;
  Pose::default()
    .with(Joint::Chest, Vec3::new(0.02 + hunch, 0.0, 0.0))
    .with(Joint::Head, Vec3::new(-hunch * 0.8, 0.0, 0.0))
    .with(Joint::ArmR, Vec3::new(0.04, 0.0, 0.1))
    .with(Joint::ElbowR, Vec3::new(0.22, 0.0, 0.0))
    .with(Joint::ArmL, Vec3::new(0.04, 0.0, -0.1))
    .with(Joint::ElbowL, Vec3::new(0.22, 0.0, 0.0))
    .with(Joint::KneeL, Vec3::new(-0.04, 0.0, 0.0))
    .with(Joint::KneeR, Vec3::new(-0.04, 0.0, 0.0))
}

fn ready(rig: &Rig) -> Pose {
  let hunch = rig.hunch;
  let raised = || {
    relaxed(rig)
      .with(Joint::ArmL, Vec3::new(0.55, 0.0, -0.22))
      .with(Joint::ElbowL, Vec3::new(1.15, 0.0, 0.0))
  };
  match rig.grip {
    Grip::Bare => Some(relaxed(rig)),
    Grip::Torch => Some(raised()),
    _ => None
  }
  .unwrap_or_else(|| {
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
  })
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

fn crouched(base: Pose) -> Pose {
  Pose { drop: 0.3, lean: -0.12, ..base }
    .add(Joint::Chest, Vec3::new(0.3, 0.0, 0.0))
    .add(Joint::Head, Vec3::new(-0.35, 0.0, 0.0))
    .add(Joint::LegL, Vec3::new(0.85, 0.0, -0.08))
    .add(Joint::LegR, Vec3::new(0.6, 0.0, 0.08))
    .add(Joint::KneeL, Vec3::new(-1.2, 0.0, 0.0))
    .add(Joint::KneeR, Vec3::new(-1.0, 0.0, 0.0))
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
  let upright = ready(rig).add(Joint::Chest, Vec3::X * (motion.breath.sin() * 0.025));
  let base = upright.blend(&crouched(upright), motion.crouch);
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
  mut rigs: Query<(&Rig, &mut Motion), Without<Dead>>,
  mut bones: Query<&mut Transform>
) {
  let blend = 1.0 - (-18.0 * time.delta_secs()).exp();
  for (rig, mut motion) in rigs.iter_mut() {
    motion.breath += time.delta_secs() * 1.6;
    let pose = posed(rig, &motion);
    for (&bone, joint) in rig.bones.iter().zip(Joint::ALL) {
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
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hold {
  Supple,
  Rigid
}

pub struct Kit(Vec<(Joint, Stuff, Piece, Hold)>);

impl Kit {
  pub fn new() -> Self { Self(Vec::new()) }

  fn put(&mut self, joint: Joint, stuff: Stuff, piece: Piece, hold: Hold) -> &mut Self {
    self.0.push((joint, stuff, piece, hold));
    self
  }

  pub fn add(&mut self, joint: Joint, stuff: Stuff, piece: Piece) -> &mut Self {
    self.put(joint, stuff, piece, Hold::Supple)
  }

  pub fn held(&mut self, joint: Joint, stuff: Stuff, piece: Piece) -> &mut Self {
    self.put(joint, stuff, piece, Hold::Rigid)
  }

  pub fn both(&mut self, joint: Joint, stuff: Stuff, piece: Piece) -> &mut Self {
    let mirrored = piece.mirrored();
    self.add(joint, stuff, piece).add(joint.twin(), stuff, mirrored)
  }
}

impl Joint {
  const fn twin(self) -> Joint {
    match self {
      Joint::ArmR => Joint::ArmL,
      Joint::ArmL => Joint::ArmR,
      Joint::ElbowR => Joint::ElbowL,
      Joint::ElbowL => Joint::ElbowR,
      Joint::LegR => Joint::LegL,
      Joint::LegL => Joint::LegR,
      Joint::KneeR => Joint::KneeL,
      Joint::KneeL => Joint::KneeR,
      other => other
    }
  }
}

#[derive(Component)]
pub struct Hidden1st;

pub const SHADOW_ONLY: RenderLayers = RenderLayers::layer(1);

pub fn shadowing() -> RenderLayers { RenderLayers::layer(0).union(&SHADOW_ONLY) }

pub fn skeleton(
  commands: &mut Commands,
  owner: Entity,
  frame: Frame
) -> [Entity; JOINTS] {
  let mut bones = [Entity::PLACEHOLDER; JOINTS];
  for joint in Joint::ALL {
    let parent = joint.parent().map_or(owner, |parent| bones[parent as usize]);
    bones[joint as usize] = commands
      .spawn((
        Transform::from_translation(frame.rest(joint)),
        Visibility::Inherited,
        ChildOf(parent)
      ))
      .id();
  }
  bones
}

pub fn limbs(frame: &Frame) -> [Option<Limb>; JOINTS] {
  let bone = |radius: f32, from: f32, to: f32, link: Link| {
    Some(Limb { radius, from: Vec3::Y * from, to: Vec3::Y * to, link })
  };
  Joint::ALL.map(|joint| match joint {
    Joint::Pelvis => Some(Limb {
      radius: 0.12,
      from: Vec3::new(-0.06, -0.04, 0.0),
      to: Vec3::new(0.06, -0.04, 0.0),
      link: Link::Root
    }),
    Joint::Chest => bone(0.14, 0.1, 0.34, Link::Socket { swing: 0.7, twist: 0.6 }),
    Joint::Head => bone(0.1, 0.08, 0.18, Link::Socket { swing: 0.8, twist: 1.0 }),
    Joint::ArmL | Joint::ArmR => {
      bone(0.05, -0.04, 0.04 - frame.upper_arm, Link::Socket { swing: 2.4, twist: 1.2 })
    }
    Joint::ElbowL | Joint::ElbowR => {
      bone(0.045, -0.03, -0.04 - frame.forearm, Link::Hinge { low: -0.15, high: 2.4 })
    }
    Joint::LegL | Joint::LegR => {
      bone(0.07, -0.06, 0.06 - frame.thigh, Link::Socket { swing: 1.3, twist: 0.4 })
    }
    Joint::KneeL | Joint::KneeR => {
      bone(0.055, -0.05, -frame.shin, Link::Hinge { low: -2.3, high: 0.15 })
    }
  })
}

impl Frame {
  fn bind(&self) -> [Vec3; JOINTS] {
    Joint::ALL.iter().fold([Vec3::ZERO; JOINTS], |mut placed, &joint| {
      placed[joint as usize] =
        joint.parent().map_or(Vec3::ZERO, |parent| placed[parent as usize])
          + self.rest(joint);
      placed
    })
  }
}

struct Seam {
  axis: Vec3,
  width: f32,
  reach: f32
}

impl Joint {
  fn seam(self) -> Seam {
    let seam =
      |axis: Vec3, width: f32, reach: f32| Seam { axis: axis.normalize(), width, reach };
    match self {
      Joint::Pelvis => seam(Vec3::Y, 0.0, 0.0),
      Joint::Chest => seam(Vec3::Y, 0.07, 0.34),
      Joint::Head => seam(Vec3::Y, 0.03, 0.14),
      Joint::ArmL => seam(Vec3::new(-0.6, -0.8, 0.0), 0.045, 0.13),
      Joint::ArmR => seam(Vec3::new(0.6, -0.8, 0.0), 0.045, 0.13),
      Joint::ElbowL | Joint::ElbowR => seam(Vec3::NEG_Y, 0.04, 0.1),
      Joint::LegL | Joint::LegR => seam(Vec3::NEG_Y, 0.06, 0.17),
      Joint::KneeL | Joint::KneeR => seam(Vec3::NEG_Y, 0.045, 0.1)
    }
  }
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
  ease(((value - from) / (to - from)).clamp(0.0, 1.0))
}

fn influences(bind: &[Vec3; JOINTS], owner: Joint, at: Vec3) -> ([u16; 4], [f32; 4]) {
  let mut shares = [0.0f32; JOINTS];
  shares[owner as usize] = 1.0;
  for &child in Joint::ALL.iter() {
    if let Some(parent) = child.parent()
      && (owner == parent || owner == child)
      && let Seam { axis, width, reach } = child.seam()
      && let offset = at - bind[child as usize]
      && let near = 1.0 - smoothstep(reach * 0.45, reach, offset.length())
      && let beyond = smoothstep(-width, width, offset.dot(axis))
    {
      let (from, to, moved) = match owner == child {
        true => (child, parent, (1.0 - beyond) * near),
        false => (parent, child, beyond * near)
      };
      let amount = shares[from as usize] * moved;
      shares[from as usize] -= amount;
      shares[to as usize] += amount;
    }
  }
  let mut ranked: Vec<(usize, f32)> = shares.into_iter().enumerate().collect();
  ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
  let total: f32 = ranked.iter().take(4).map(|&(_, share)| share).sum();
  (
    std::array::from_fn(|slot| ranked[slot].0 as u16),
    std::array::from_fn(|slot| ranked[slot].1 / total)
  )
}

fn bound(piece: Piece, bind: &[Vec3; JOINTS], joint: Joint, hold: Hold) -> Piece {
  let Piece(mesh) = piece.at(bind[joint as usize]);
  let positions: Vec<Vec3> = mesh
    .attribute(Mesh::ATTRIBUTE_POSITION)
    .and_then(|values| values.as_float3())
    .map(|values| values.iter().copied().map(Vec3::from).collect())
    .unwrap_or_default();
  let (indices, weights): (Vec<[u16; 4]>, Vec<[f32; 4]>) = positions
    .iter()
    .map(|&at| match hold {
      Hold::Supple => influences(bind, joint, at),
      Hold::Rigid => ([joint as u16, 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
    })
    .unzip();
  Piece(
    mesh
      .with_inserted_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        bevy::mesh::VertexAttributeValues::Uint16x4(indices)
      )
      .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights)
  )
}

pub fn tailor(Kit(pieces): Kit, frame: &Frame) -> Vec<(usize, Stuff, Mesh)> {
  let bind = frame.bind();
  pieces
    .into_iter()
    .fold(
      Vec::<(Joint, Stuff, Vec<Piece>)>::new(),
      |mut groups, (joint, stuff, piece, hold)| {
        let anchor =
          (joint == Joint::Head).then_some(Joint::Head).unwrap_or(Joint::Pelvis);
        let piece = bound(piece, &bind, joint, hold);
        match groups.iter_mut().find(|(each, kind, _)| *each == anchor && *kind == stuff)
        {
          Some((_, _, list)) => list.push(piece),
          None => groups.push((anchor, stuff, vec![piece]))
        }
        groups
      }
    )
    .into_iter()
    .map(|(anchor, stuff, list)| {
      let mut mesh = model::merge(list);
      if stuff.grain() == crate::stuff::Grain::Scales {
        mesh.generate_tangents().ok();
      }
      mesh.generate_skinned_mesh_bounds().ok();
      (anchor as usize, stuff, mesh)
    })
    .collect()
}

pub fn fasten(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  bones: &[Entity],
  parts: Vec<(usize, Stuff, Mesh)>
) {
  for (bone, stuff, mesh) in parts {
    let mut part = commands.spawn((
      Mesh3d(meshes.add(mesh)),
      MeshMaterial3d(stuffs.of(stuff)),
      ChildOf(bones[bone])
    ));
    if matches!(stuff, Stuff::Frost | Stuff::Ember) {
      part.insert(NotShadowCaster);
    }
  }
}

pub fn dress(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  poses: &mut Assets<SkinnedMeshInverseBindposes>,
  stuffs: &Stuffs,
  rig: &Rig,
  parts: Vec<(usize, Stuff, Mesh)>
) -> Vec<Entity> {
  let inverse =
    poses.add(rig.frame.bind().map(|at| Mat4::from_translation(-at)).to_vec());
  parts
    .into_iter()
    .filter_map(|(anchor, stuff, mesh)| {
      let mut part = commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(stuffs.of(stuff)),
        SkinnedMesh { inverse_bindposes: inverse.clone(), joints: rig.bones.to_vec() },
        DynamicSkinnedMeshBounds,
        ChildOf(rig.bones[Joint::Pelvis as usize])
      ));
      if matches!(stuff, Stuff::Frost | Stuff::Ember) {
        part.insert(NotShadowCaster);
      }
      (anchor == Joint::Head as usize).then_some(part.id())
    })
    .collect()
}

pub fn spawn_body(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  poses: &mut Assets<SkinnedMeshInverseBindposes>,
  stuffs: &Stuffs,
  owner: Entity,
  frame: Frame,
  grip: Grip,
  hunch: f32,
  kit: Kit
) -> (Rig, Vec<Entity>) {
  let bones = skeleton(commands, owner, frame);
  let rig = Rig { bones, frame, grip, hunch };
  let head = dress(commands, meshes, poses, stuffs, &rig, tailor(kit, &frame));
  (rig, head)
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
      (0.094, 0.172),
      (0.097, 0.2),
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
      Piece::new(rod(0.099, 0.026), iron * 0.8).sized(stretch).at_xyz(0.0, 0.185, 0.0)
    )
    .add(Joint::Head, Stuff::Iron, Piece::new(tube(&crest, &[0.011], 8), iron * 0.9))
    .add(
      Joint::Head,
      Stuff::Iron,
      Piece::new(block(0.014, 0.05, 0.012), iron).at_xyz(0.0, 0.16, -0.118)
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

pub fn torch(kit: &mut Kit, frame: &Frame) {
  let hand = frame.hand();
  let upright = |piece: Piece| piece.pitched(-FRAC_PI_2).at(hand);
  kit
    .held(
      Joint::ElbowL,
      Stuff::Wood,
      upright(Piece::new(rod(0.025, 0.62), srgb(0.3, 0.2, 0.12)).at_xyz(0.0, 0.12, 0.0))
    )
    .held(
      Joint::ElbowL,
      Stuff::Cloth,
      upright(Piece::new(rod(0.042, 0.14), srgb(0.2, 0.16, 0.12)).at_xyz(0.0, 0.4, 0.0))
    )
    .held(
      Joint::ElbowL,
      Stuff::Ember,
      upright(
        Piece::new(model::ball(0.045), srgb(1.0, 0.62, 0.3)).at_xyz(0.0, 0.47, 0.0)
      )
    );
}

pub fn iron_sword(kit: &mut Kit, frame: &Frame) {
  let hand = frame.hand();
  let steel = srgb(0.78, 0.8, 0.83);
  let grip_forward = |piece: Piece| piece.yawed(FRAC_PI_2).pitched(-FRAC_PI_2).at(hand);
  kit
    .held(
      Joint::ElbowR,
      Stuff::Steel,
      grip_forward(
        Piece::new(model::blade(0.82, 0.058, 0.014, 0.14), steel).at_xyz(0.0, 0.12, 0.0)
      )
    )
    .held(
      Joint::ElbowR,
      Stuff::Iron,
      grip_forward(
        Piece::new(block(0.2, 0.028, 0.04), srgb(0.4, 0.4, 0.42)).at_xyz(0.0, 0.11, 0.0)
      )
    )
    .held(
      Joint::ElbowR,
      Stuff::Leather,
      grip_forward(
        Piece::new(rod(0.019, 0.17), srgb(0.28, 0.18, 0.1)).at_xyz(0.0, 0.02, 0.0)
      )
    )
    .held(
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
    .held(
      Joint::ElbowR,
      Stuff::Wood,
      grip_forward(
        Piece::new(rod(0.02, 0.62), srgb(0.3, 0.22, 0.14)).at_xyz(0.0, 0.2, 0.0)
      )
    )
    .held(
      Joint::ElbowR,
      Stuff::Iron,
      grip_forward(
        Piece::new(head, metal).yawed(FRAC_PI_2).rolled(0.0).at_xyz(0.0, 0.42, -0.01)
      )
    )
    .held(
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
    .held(Joint::ElbowL, Stuff::Wood, facing(Piece::new(model::rod(0.34, 0.035), wood)))
    .held(Joint::ElbowL, Stuff::Iron, facing(Piece::new(tube(&rim, &[0.018], 6), iron)))
    .held(
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
    .held(
      Joint::ElbowL,
      Stuff::Iron,
      facing(Piece::new(block(0.66, 0.012, 0.05), iron).at_xyz(0.0, 0.02, 0.0))
    )
    .held(
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
    for height in [0.11, 0.18, 0.25] {
      kit.add(
        Joint::Chest,
        Stuff::Iron,
        Piece::new(sheath(&hoops, height - 0.009, height + 0.009, 1.12), plate * 0.75)
      );
    }
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
  for (angle, depth, width) in [
    (0.0, -1.0, 0.16),
    (0.9, -0.7, 0.13),
    (-0.9, -0.7, 0.13),
    (0.0, 1.0, 0.16),
    (1.3, 0.0, 0.12),
    (-1.3, 0.0, 0.12)
  ]
  .into_iter()
  {
    let outward = Vec3::new((angle as f32).sin(), 0.0, depth);
    kit.add(
      Joint::Pelvis,
      Stuff::Leather,
      Piece::new(block(width, 0.34, 0.015), leather)
        .pitched(-0.12 * depth.signum())
        .yawed(angle * 1.2)
        .at(outward * Vec3::new(0.18, 0.0, 0.13) * build.waist + Vec3::Y * -0.14)
    );
  }
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
  let person = Person {
    skin: srgb(0.78, 0.6, 0.48),
    mane: srgb(0.36, 0.22, 0.12),
    iris: srgb(0.3, 0.4, 0.45),
    style: Style::Cropped,
    beard: Some(Beard::Full),
    paint: None,
    ..Person::roll(Race::Nord, false, 4)
  };
  face::head(&mut kit, &person, &build);
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
  let helmed = seed % 2 == 0;
  let person = Person::draugr(seed);
  let person =
    Person { style: helmed.then_some(Style::Cropped).unwrap_or(person.style), ..person };
  face::head(&mut kit, &person, &build);
  helmed.then(|| horned_helmet(&mut kit, ancient, srgb(0.55, 0.5, 0.42), 0.6));
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
  let person = Person::roll(Race::of(seed), false, seed);
  face::head(&mut kit, &person, &build);
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

pub const WOMAN: Frame = Frame {
  hip: 0.93,
  waist: 0.1,
  neck: 0.47,
  shoulder: Vec2::new(0.19, 0.42),
  upper_arm: 0.28,
  forearm: 0.25,
  hip_width: 0.095,
  thigh: 0.44,
  shin: 0.44
};

impl Build {
  pub const FAIR: Build = Build { limbs: 0.86, chest: 0.88, waist: 0.84, cheeks: 0.94 };
  pub const BURLY: Build = Build { limbs: 1.12, chest: 1.1, waist: 1.04, cheeks: 1.02 };
}

const FOLD: Vec3 = Vec3::new(34.0, 5.0, 34.0);

fn cloth(mesh: bevy::mesh::Mesh, color: Srgba, seed: u32) -> Piece {
  Piece::new(mesh, color).creased(0.006, FOLD, seed)
}

fn hide(mesh: bevy::mesh::Mesh, color: Srgba, seed: u32) -> Piece {
  Piece::new(mesh, color).creased(0.003, Vec3::new(18.0, 14.0, 18.0), seed)
}

fn shirt(kit: &mut Kit, build: &Build, color: Srgba, cuffs: Option<Srgba>, seed: u32) {
  let hoops = chest_hoops(build);
  kit
    .add(Joint::Chest, Stuff::Cloth, cloth(loft(&hoops, TRUNK_SIDES), color, seed))
    .add(
      Joint::Chest,
      Stuff::Cloth,
      cloth(sheath(&hoops, 0.43, 0.49, 1.12), color * 0.92, seed + 1)
    )
    .both(
      Joint::ArmR,
      Stuff::Cloth,
      cloth(loft(&limb_hoops(&UPPER_ARM, build), LIMB_SIDES), color, seed + 2)
        .sized(Vec3::new(1.04, 1.0, 1.04))
    )
    .both(
      Joint::ElbowR,
      Stuff::Cloth,
      cloth(
        sheath(&limb_hoops(&FOREARM, build), 0.03, -0.17, 1.18),
        color * 0.96,
        seed + 3
      )
    )
    .both(
      Joint::ElbowR,
      Stuff::Skin,
      Piece::new(loft(&limb_hoops(&FOREARM, build), LIMB_SIDES), srgb(0.55, 0.41, 0.32))
    );
  if let Some(trim) = cuffs {
    for y in [-0.06, -0.1, -0.14] {
      kit.both(
        Joint::ElbowR,
        Stuff::Cloth,
        Piece::new(
          sheath(&limb_hoops(&FOREARM, build), y - 0.006, y + 0.006, 1.22),
          trim
        )
        .rolled(0.25 * (y * 50.0).sin())
      );
    }
  }
}

fn hands(kit: &mut Kit, build: &Build, (flesh, skin): (Stuff, Srgba)) {
  let thumb = curve(
    Vec3::new(-0.018, -0.262, -0.026),
    Vec3::new(-0.036, -0.285, -0.046),
    Vec3::new(-0.024, -0.305, -0.052),
    8
  );
  kit
    .both(
      Joint::ElbowR,
      flesh,
      Piece::new(loft(&limb_hoops(&FIST, build), LIMB_SIDES), skin)
    )
    .both(
      Joint::ElbowR,
      flesh,
      Piece::new(tube(&thumb, &taper(8, 0.012, 0.009), 8), skin)
    );
}

fn hem(
  kit: &mut Kit,
  build: &Build,
  stuff: Stuff,
  color: Srgba,
  drop: f32,
  flare: f32,
  seed: u32
) {
  let waist = hip_hoops(build)[3].scaled(1.08);
  let hoops: Vec<Hoop> = (0..=6)
    .map(|step| {
      let t = step as f32 / 6.0;
      let wide = 1.0 + flare * t.powf(1.3);
      Hoop { at: Vec3::new(0.0, 0.02 - drop * t, 0.0), ..waist.scaled(wide) }
    })
    .rev()
    .collect();
  kit.add(
    Joint::Pelvis,
    stuff,
    cloth(loft(&hoops, TRUNK_SIDES), color, seed)
      .shaded(move |at, _| {
        let grime = ((-0.02 - at.y) / drop).clamp(0.0, 1.0) * 0.3;
        (LinearRgba::from(color) * (1.0 - grime)).with_alpha(1.0)
      })
      .creased(0.012, Vec3::new(40.0, 3.0, 40.0), seed + 5)
  );
}

fn girdle(kit: &mut Kit, build: &Build, color: Srgba, buckle: Option<Srgba>) {
  let hoops = chest_hoops(build);
  kit.add(
    Joint::Chest,
    Stuff::Leather,
    Piece::new(sheath(&hoops, 0.0, 0.045, 1.12), color)
  );
  if let Some(metal) = buckle {
    kit.add(
      Joint::Chest,
      Stuff::Iron,
      Piece::new(block(0.05, 0.04, 0.015), metal).at_xyz(
        0.0,
        0.022,
        -hoop_at(&hoops, 0.02).front * 1.12 - 0.004
      )
    );
  }
}

fn trousers(
  kit: &mut Kit,
  frame: &Frame,
  build: &Build,
  color: Srgba,
  wraps: Srgba,
  shoe: Srgba,
  seed: u32
) {
  let shin = limb_hoops(&SHIN, build);
  kit
    .both(
      Joint::LegR,
      Stuff::Cloth,
      cloth(loft(&limb_hoops(&THIGH, build), LIMB_SIDES), color, seed)
    )
    .both(Joint::KneeR, Stuff::Cloth, cloth(loft(&shin, LIMB_SIDES), color, seed + 1))
    .both(
      Joint::KneeR,
      Stuff::Leather,
      hide(loft(&FOOT, LIMB_SIDES), shoe, seed + 2).pitched(-FRAC_PI_2).at_xyz(
        0.0,
        -frame.shin + 0.035,
        0.0
      )
    )
    .both(
      Joint::KneeR,
      Stuff::Leather,
      Piece::new(sheath(&shin, -0.33, -0.405, 1.12), shoe)
    );
  for band in 0..6 {
    let y = -0.06 - band as f32 * 0.045;
    kit.both(
      Joint::KneeR,
      Stuff::Leather,
      Piece::new(sheath(&shin, y - 0.007, y + 0.007, 1.1), wraps)
        .rolled((band % 2) as f32 * 0.5 - 0.25)
    );
  }
}

fn boots(
  kit: &mut Kit,
  frame: &Frame,
  build: &Build,
  color: Srgba,
  top: f32,
  fold: Option<Srgba>,
  seed: u32
) {
  let shin = limb_hoops(&SHIN, build);
  kit
    .both(
      Joint::KneeR,
      Stuff::Leather,
      hide(sheath(&shin, top, -0.405, 1.16), color, seed)
    )
    .both(
      Joint::KneeR,
      Stuff::Leather,
      hide(loft(&FOOT, LIMB_SIDES), color * 0.9, seed + 1).pitched(-FRAC_PI_2).at_xyz(
        0.0,
        -frame.shin + 0.035,
        0.0
      )
    );
  if let Some(cuff) = fold {
    kit.both(
      Joint::KneeR,
      Stuff::Fur,
      Piece::new(model::lump(seed + 7, 0.2, 2), cuff)
        .sized(Vec3::new(0.07, 0.05, 0.08) * build.limbs)
        .at_xyz(0.0, top, 0.005)
    );
  }
}

fn hood(kit: &mut Kit, build: &Build, color: Srgba, seed: u32) {
  let hoops: Vec<Hoop> = head_hoops(build)
    .into_iter()
    .filter(|hoop| hoop.at.y > 0.02)
    .map(|hoop| Hoop {
      front: hoop.front * 0.35,
      ..hoop.scaled(1.22).shifted(0.0, 0.012)
    })
    .collect();
  let cowl = loft(
    &[
      Hoop::new(-0.06, 0.2, 0.12, 0.13),
      Hoop::new(0.0, 0.15, 0.1, 0.11),
      Hoop::new(0.05, 0.12, 0.04, 0.11)
    ],
    TRUNK_SIDES
  );
  kit.add(Joint::Head, Stuff::Cloth, cloth(loft(&hoops, TRUNK_SIDES), color, seed)).add(
    Joint::Head,
    Stuff::Cloth,
    cloth(cowl, color * 0.9, seed + 1)
  );
}

fn cook_cap(kit: &mut Kit, color: Srgba, seed: u32) {
  kit
    .add(
      Joint::Head,
      Stuff::Cloth,
      cloth(model::lump(seed, 0.18, 2), color, seed)
        .sized(Vec3::new(0.12, 0.07, 0.13))
        .at_xyz(0.0, 0.27, 0.01)
    )
    .add(
      Joint::Head,
      Stuff::Cloth,
      Piece::new(rod(0.1, 0.04), color * 0.9)
        .sized(Vec3::new(1.0, 1.0, 1.1))
        .at_xyz(0.0, 0.235, 0.005)
    );
}

fn apron(kit: &mut Kit, build: &Build, color: Srgba, seed: u32) {
  let front = hip_hoops(build)[3].front * 1.1;
  kit.add(
    Joint::Pelvis,
    Stuff::Cloth,
    cloth(model::block(0.34, 0.62, 0.012), color, seed)
      .shaded(move |at, _| {
        let smear = crate::noise::value3(at * 14.0, seed) * 0.35;
        (LinearRgba::from(color) * (1.0 - smear)).with_alpha(1.0)
      })
      .creased(0.01, Vec3::new(30.0, 4.0, 30.0), seed)
      .pitched(0.08)
      .at_xyz(0.0, -0.26, -front - 0.012)
  );
}

fn harness(kit: &mut Kit, build: &Build, leather: Srgba, metal: Srgba) {
  let hoops = chest_hoops(build);
  let front = hoop_at(&hoops, 0.26).front * 1.05;
  let back = hoop_at(&hoops, 0.26).back * 1.05;
  for side in [-1.0, 1.0] {
    kit
      .add(
        Joint::Chest,
        Stuff::Leather,
        Piece::new(block(0.055, 0.52, 0.012), leather)
          .rolled(side * 0.62)
          .at_xyz(0.0, 0.25, -front)
      )
      .add(
        Joint::Chest,
        Stuff::Leather,
        Piece::new(block(0.055, 0.52, 0.012), leather)
          .rolled(side * 0.62)
          .at_xyz(0.0, 0.25, back)
      );
  }
  kit.add(
    Joint::Chest,
    Stuff::Iron,
    Piece::new(rod(0.05, 0.012), metal).pitched(FRAC_PI_2).at_xyz(
      0.0,
      0.25,
      -front - 0.01
    )
  );
}

fn bare_arms(kit: &mut Kit, build: &Build, (flesh, skin): (Stuff, Srgba), bracer: Srgba) {
  kit
    .both(
      Joint::ArmR,
      flesh,
      Piece::new(loft(&limb_hoops(&UPPER_ARM, build), LIMB_SIDES), skin)
    )
    .both(
      Joint::ElbowR,
      flesh,
      Piece::new(loft(&limb_hoops(&FOREARM, build), LIMB_SIDES), skin)
    )
    .both(
      Joint::ElbowR,
      Stuff::Leather,
      hide(sheath(&limb_hoops(&FOREARM, build), -0.1, -0.22, 1.14), bracer, 3)
    );
  hands(kit, build, (flesh, skin));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Calling {
  Cook,
  Huntress,
  Priest,
  Farmer,
  Barbarian
}

impl Calling {
  pub fn frame(self) -> Frame {
    match self {
      Calling::Huntress => WOMAN,
      _ => MAN
    }
  }

  pub fn title(self) -> &'static str {
    match self {
      Calling::Cook => "Innkeeper",
      Calling::Huntress => "Huntress",
      Calling::Priest => "Priest",
      Calling::Farmer => "Farmer",
      Calling::Barbarian => "Sellsword"
    }
  }
}

pub fn villager(calling: Calling, person: &Person) -> Kit {
  let mut kit = Kit::new();
  let frame = calling.frame();
  let seed = person.seed;
  let skin = person.skin;
  let flesh = person.flesh();
  let pick = |colors: &[Srgba]| colors[(seed / 7) as usize % colors.len()];
  match calling {
    Calling::Cook => {
      let build = Build::HALE;
      face::head(&mut kit, &Person { style: Style::Cropped, ..*person }, &build);
      cook_cap(&mut kit, srgb(0.7, 0.68, 0.64), seed);
      let linen = pick(&[srgb(0.56, 0.54, 0.5), srgb(0.6, 0.55, 0.46)]);
      shirt(&mut kit, &build, linen, Some(srgb(0.45, 0.22, 0.14)), seed);
      hands(&mut kit, &build, (flesh, skin));
      hem(&mut kit, &build, Stuff::Cloth, linen * 0.95, 0.22, 0.25, seed + 10);
      girdle(&mut kit, &build, srgb(0.4, 0.3, 0.26), None);
      apron(&mut kit, &build, srgb(0.6, 0.56, 0.48), seed + 11);
      trousers(
        &mut kit,
        &frame,
        &build,
        srgb(0.5, 0.48, 0.44),
        srgb(0.38, 0.24, 0.15),
        srgb(0.22, 0.15, 0.1),
        seed + 12
      );
    }
    Calling::Huntress => {
      let build = Build::FAIR;
      face::head(&mut kit, person, &build);
      let leather = srgb(0.34, 0.24, 0.16);
      let iron = srgb(0.3, 0.32, 0.33);
      let hoops = chest_hoops(&build);
      kit
        .add(
          Joint::Chest,
          Stuff::Cloth,
          cloth(loft(&hoops, TRUNK_SIDES), srgb(0.3, 0.36, 0.26), seed)
        )
        .add(
          Joint::Chest,
          Stuff::Leather,
          hide(sheath(&hoops, -0.04, 0.4, 1.1), leather, seed + 1)
        )
        .add(
          Joint::Chest,
          Stuff::Fur,
          Piece::new(sheath(&hoops, 0.36, 0.46, 1.16), srgb(0.36, 0.28, 0.2)).creased(
            0.012,
            Vec3::new(50.0, 20.0, 50.0),
            seed
          )
        );
      for y in [0.3, 0.23, 0.16] {
        kit.add(
          Joint::Chest,
          Stuff::Iron,
          Piece::new(block(0.1, 0.022, 0.012), iron).at_xyz(
            0.0,
            y,
            -hoop_at(&hoops, y).front * 1.1 - 0.006
          )
        );
      }
      arms(
        &mut kit,
        &build,
        (Stuff::Cloth, srgb(0.36, 0.44, 0.3)),
        leather * 0.8,
        (flesh, skin),
        Some(iron)
      );
      girdle(&mut kit, &build, srgb(0.2, 0.15, 0.1), Some(iron));
      kit.add(
        Joint::Pelvis,
        Stuff::Iron,
        Piece::new(loft(&hip_hoops(&build), TRUNK_SIDES), srgb(0.22, 0.22, 0.23))
          .creased(0.004, Vec3::splat(90.0), seed)
      );
      hem(&mut kit, &build, Stuff::Leather, leather * 0.9, 0.34, 0.35, seed + 2);
      kit
        .both(
          Joint::LegR,
          flesh,
          Piece::new(loft(&limb_hoops(&THIGH, &build), LIMB_SIDES), skin)
        )
        .both(
          Joint::KneeR,
          flesh,
          Piece::new(loft(&limb_hoops(&SHIN, &build), LIMB_SIDES), skin)
        );
      boots(
        &mut kit,
        &frame,
        &build,
        srgb(0.3, 0.22, 0.15),
        -0.02,
        Some(srgb(0.3, 0.24, 0.18)),
        seed + 3
      );
    }
    Calling::Priest => {
      let build = Build { limbs: 0.92, ..Build::HALE };
      let robe =
        pick(&[srgb(0.1, 0.1, 0.12), srgb(0.16, 0.12, 0.2), srgb(0.36, 0.3, 0.22)]);
      face::head(&mut kit, &Person { style: Style::Cropped, ..*person }, &build);
      hood(&mut kit, &build, robe, seed);
      shirt(&mut kit, &build, robe, None, seed + 1);
      hands(&mut kit, &build, (flesh, skin));
      kit.add(
        Joint::Chest,
        Stuff::Cloth,
        Piece::new(block(0.03, 0.5, 0.012), srgb(0.5, 0.42, 0.3))
          .rolled(-0.35)
          .at_xyz(0.03, 0.22, -0.125)
      );
      hem(&mut kit, &build, Stuff::Cloth, robe, 0.9, 0.55, seed + 2);
      kit.add(
        Joint::Chest,
        Stuff::Leather,
        Piece::new(
          sheath(&chest_hoops(&build), 0.0, 0.025, 1.12),
          srgb(0.46, 0.38, 0.26)
        )
        .creased(0.004, Vec3::splat(60.0), seed)
      );
      kit.add(
        Joint::Pelvis,
        Stuff::Leather,
        Piece::new(
          tube(
            &[
              Vec3::new(0.05, 0.0, -0.13),
              Vec3::new(0.06, -0.18, -0.14),
              Vec3::new(0.07, -0.34, -0.15)
            ],
            &[0.008],
            6
          ),
          srgb(0.46, 0.38, 0.26)
        )
      );
      kit.both(
        Joint::KneeR,
        Stuff::Leather,
        Piece::new(loft(&FOOT, LIMB_SIDES), srgb(0.2, 0.15, 0.1))
          .pitched(-FRAC_PI_2)
          .at_xyz(0.0, -frame.shin + 0.035, 0.0)
      );
    }
    Calling::Farmer => {
      let build = Build::HALE;
      face::head(&mut kit, person, &build);
      (person.race != Race::Argonian).then(|| {
        kit.add(
          Joint::Head,
          Stuff::Cloth,
          Piece::new(sheath(&head_hoops(&build), 0.2, 0.225, 1.06), srgb(0.4, 0.3, 0.2))
        )
      });
      let linen =
        pick(&[srgb(0.7, 0.64, 0.46), srgb(0.66, 0.62, 0.54), srgb(0.55, 0.5, 0.4)]);
      shirt(&mut kit, &build, linen, None, seed);
      hands(&mut kit, &build, (flesh, skin));
      let vest = srgb(0.42, 0.28, 0.17);
      kit.add(
        Joint::Chest,
        Stuff::Leather,
        hide(sheath(&chest_hoops(&build), -0.03, 0.44, 1.08), vest, seed + 1)
      );
      hem(&mut kit, &build, Stuff::Leather, vest, 0.26, 0.3, seed + 2);
      girdle(&mut kit, &build, srgb(0.24, 0.17, 0.11), Some(srgb(0.5, 0.5, 0.52)));
      trousers(
        &mut kit,
        &frame,
        &build,
        pick(&[srgb(0.34, 0.4, 0.24), srgb(0.42, 0.36, 0.26)]),
        srgb(0.3, 0.2, 0.12),
        srgb(0.3, 0.2, 0.13),
        seed + 3
      );
    }
    Calling::Barbarian => {
      let build = Build::BURLY;
      face::head(&mut kit, person, &build);
      kit.add(
        Joint::Chest,
        flesh,
        Piece::new(loft(&chest_hoops(&build), TRUNK_SIDES), skin).creased(
          0.004,
          Vec3::new(12.0, 16.0, 12.0),
          seed
        )
      );
      harness(&mut kit, &build, srgb(0.3, 0.22, 0.14), srgb(0.62, 0.62, 0.6));
      bare_arms(&mut kit, &build, (flesh, skin), srgb(0.3, 0.22, 0.14));
      kit.add(
        Joint::Pelvis,
        Stuff::Fur,
        Piece::new(sheath(&hip_hoops(&build), 0.07, -0.02, 1.2), srgb(0.46, 0.4, 0.3))
          .creased(0.014, Vec3::new(50.0, 20.0, 50.0), seed)
      );
      skirt(&mut kit, &build, srgb(0.32, 0.24, 0.16), srgb(0.14, 0.12, 0.1));
      kit
        .both(
          Joint::LegR,
          Stuff::Cloth,
          cloth(
            loft(&limb_hoops(&THIGH, &build), LIMB_SIDES),
            srgb(0.14, 0.12, 0.1),
            seed
          )
        )
        .both(
          Joint::KneeR,
          Stuff::Cloth,
          cloth(
            loft(&limb_hoops(&SHIN, &build), LIMB_SIDES),
            srgb(0.14, 0.12, 0.1),
            seed + 1
          )
        );
      boots(
        &mut kit,
        &frame,
        &build,
        srgb(0.34, 0.26, 0.18),
        -0.06,
        Some(srgb(0.75, 0.7, 0.58)),
        seed + 4
      );
    }
  }
  kit
}

pub fn plugin(app: &mut App) {
  app.add_systems(PostUpdate, animate.before(TransformSystems::Propagate));
}
