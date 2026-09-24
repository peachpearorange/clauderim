use {crate::{model::{self, Piece, ball, block, curve, lathe, limb, rod, taper, tube},
             stuff::{Stuff, Stuffs}},
     bevy::{light::NotShadowCaster, prelude::*},
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
    .with(Joint::ArmL, Vec3::new(1.2, 0.5, 0.2))
    .with(Joint::ElbowL, Vec3::new(1.3, -0.3, 0.0))
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
  let moved = walked.blend(&airborne, motion.airborne);
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
    if joint == Joint::Head {
      part.insert(Hidden1st);
    }
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

fn horned_helmet(kit: &mut Kit, iron: Srgba, horn: Srgba, horn_size: f32) {
  let dome = shell(
    &[
      (0.132, 0.06),
      (0.138, 0.13),
      (0.132, 0.2),
      (0.11, 0.255),
      (0.07, 0.29),
      (0.0, 0.302)
    ],
    18
  );
  kit
    .add(
      Joint::Head,
      Stuff::Iron,
      Piece::new(dome, iron).sized(Vec3::new(1.0, 1.0, 1.08))
    )
    .add(
      Joint::Head,
      Stuff::Iron,
      Piece::new(model::rod(0.143, 0.03), iron * 0.8)
        .at_xyz(0.0, 0.1, 0.0)
        .sized(Vec3::new(1.0, 1.0, 1.08))
    )
    .add(
      Joint::Head,
      Stuff::Iron,
      Piece::new(block(0.022, 0.12, 0.02), iron).at_xyz(0.0, 0.1, -0.15)
    )
    .add(
      Joint::Head,
      Stuff::Iron,
      Piece::new(block(0.02, 0.05, 0.3), iron * 0.9).at_xyz(0.0, 0.27, 0.0).pitched(0.0)
    )
    .both(
      Joint::Head,
      Stuff::Iron,
      Piece::new(block(0.02, 0.13, 0.09), iron * 0.85)
        .rolled(0.12)
        .at_xyz(0.125, 0.07, -0.06)
    );
  let horn_path = curve(
    Vec3::new(0.11, 0.21, 0.0),
    Vec3::new(0.3, 0.25, 0.03),
    Vec3::new(0.32, 0.43, -0.1),
    10
  )
  .into_iter()
  .map(|point| {
    Vec3::new(0.11, 0.21, 0.0) + (point - Vec3::new(0.11, 0.21, 0.0)) * horn_size
  })
  .collect::<Vec<_>>();
  let horn_mesh = tube(&horn_path, &taper(10, 0.042 * horn_size, 0.004), 10);
  kit.both(Joint::Head, Stuff::Bone, Piece::new(horn_mesh, horn));
}

fn face(kit: &mut Kit, skin: Srgba, beard: Srgba, eyes: Stuff, eye_color: Srgba) {
  kit
    .add(
      Joint::Head,
      Stuff::Skin,
      Piece::new(ball(1.0), skin)
        .sized(Vec3::new(0.1, 0.125, 0.112))
        .at_xyz(0.0, 0.14, -0.005)
    )
    .add(
      Joint::Head,
      Stuff::Skin,
      Piece::new(block(0.03, 0.05, 0.04), skin).pitched(0.3).at_xyz(0.0, 0.13, -0.11)
    )
    .add(
      Joint::Head,
      Stuff::Skin,
      Piece::new(rod(0.05, 0.12), skin).at_xyz(0.0, 0.0, 0.0)
    )
    .add(
      Joint::Head,
      Stuff::Fur,
      Piece::new(ball(1.0), beard)
        .sized(Vec3::new(0.085, 0.09, 0.06))
        .at_xyz(0.0, 0.05, -0.075)
    )
    .add(
      Joint::Head,
      Stuff::Fur,
      Piece::new(block(0.07, 0.022, 0.02), beard).at_xyz(0.0, 0.105, -0.112)
    )
    .both(
      Joint::Head,
      eyes,
      Piece::new(ball(0.013), eye_color).at_xyz(0.038, 0.155, -0.1)
    );
}

pub fn iron_sword(kit: &mut Kit, frame: &Frame) {
  let hand = frame.hand();
  let steel = srgb(0.78, 0.8, 0.83);
  let grip_forward = |piece: Piece| piece.pitched(-FRAC_PI_2).at(hand);
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
  under: Srgba,
  plate: Option<Srgba>,
  fur: Srgba,
  belt: Srgba,
  frame: &Frame
) {
  let top = frame.shoulder.y + 0.04;
  let body = shell(
    &[
      (0.0, -0.02),
      (0.15, 0.0),
      (0.16, 0.12),
      (0.19, 0.28),
      (0.2, top - 0.08),
      (0.14, top),
      (0.06, top + 0.03),
      (0.0, top + 0.04)
    ],
    16
  );
  kit.add(
    Joint::Chest,
    Stuff::Cloth,
    Piece::new(body, under).sized(Vec3::new(1.0, 1.0, 0.68))
  );
  if let Some(plate) = plate {
    let cuirass = shell(
      &[(0.165, 0.1), (0.2, 0.2), (0.215, 0.32), (0.2, top - 0.04), (0.12, top + 0.02)],
      16
    );
    kit.add(
      Joint::Chest,
      Stuff::Iron,
      Piece::new(cuirass, plate).sized(Vec3::new(1.0, 1.0, 0.74))
    );
    (0..3).for_each(|band| {
      let height = 0.14 + band as f32 * 0.07;
      kit.add(
        Joint::Chest,
        Stuff::Iron,
        Piece::new(rod(0.175 + band as f32 * 0.014, 0.018), plate * 0.75)
          .sized(Vec3::new(1.0, 1.0, 0.8))
          .at_xyz(0.0, height, 0.0)
      );
    });
  }
  kit
    .add(
      Joint::Chest,
      Stuff::Fur,
      Piece::new(model::lump(3, 0.12, 2), fur).sized(Vec3::new(0.24, 0.08, 0.17)).at_xyz(
        0.0,
        top - 0.03,
        0.0
      )
    )
    .add(
      Joint::Chest,
      Stuff::Fur,
      Piece::new(model::lump(4, 0.1, 2), fur * 0.9)
        .sized(Vec3::new(0.2, 0.2, 0.05))
        .at_xyz(0.0, top - 0.18, 0.13)
        .pitched(0.0)
    )
    .add(
      Joint::Chest,
      Stuff::Leather,
      Piece::new(rod(0.158, 0.05), belt)
        .sized(Vec3::new(1.0, 1.0, 0.72))
        .at_xyz(0.0, 0.03, 0.0)
    )
    .add(
      Joint::Chest,
      Stuff::Gold,
      Piece::new(block(0.06, 0.045, 0.02), srgb(0.75, 0.6, 0.3))
        .at_xyz(0.0, 0.03, -0.118)
    )
    .add(
      Joint::Chest,
      Stuff::Leather,
      Piece::new(block(0.05, 0.5, 0.012), belt).rolled(0.7).at_xyz(0.0, 0.26, -0.15)
    );
}

fn skirt(kit: &mut Kit, leather: Srgba, cloth: Srgba) {
  kit.add(
    Joint::Pelvis,
    Stuff::Cloth,
    Piece::new(
      shell(&[(0.0, -0.12), (0.13, -0.1), (0.16, 0.0), (0.155, 0.12)], 14),
      cloth
    )
    .sized(Vec3::new(1.0, 1.0, 0.7))
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
        .at(outward * Vec3::new(0.12, 0.0, 0.11) + Vec3::Y * -0.14)
    );
  });
}

fn arms(
  kit: &mut Kit,
  frame: &Frame,
  sleeve: Stuff,
  sleeve_color: Srgba,
  bracer: Srgba,
  hand: Srgba,
  pauldron: Option<Srgba>
) {
  kit
    .both(
      Joint::ArmR,
      sleeve,
      Piece::new(limb(0.074, 0.058, frame.upper_arm), sleeve_color)
    )
    .both(
      Joint::ElbowR,
      Stuff::Leather,
      Piece::new(limb(0.064, 0.05, frame.forearm), bracer)
    )
    .both(
      Joint::ElbowR,
      Stuff::Leather,
      Piece::new(block(0.07, 0.09, 0.085), hand).at(frame.hand() + Vec3::Y * 0.02)
    );
  if let Some(plate) = pauldron {
    let cap =
      shell(&[(0.105, -0.07), (0.1, -0.02), (0.08, 0.03), (0.04, 0.06), (0.0, 0.07)], 12);
    kit
      .both(
        Joint::ArmR,
        Stuff::Iron,
        Piece::new(cap.clone(), plate).rolled(-0.5).at_xyz(0.03, -0.01, 0.0)
      )
      .both(
        Joint::ArmR,
        Stuff::Iron,
        Piece::new(cap, plate * 0.85)
          .sized(Vec3::splat(0.85))
          .rolled(-0.7)
          .at_xyz(0.06, -0.1, 0.0)
      )
      .both(
        Joint::ElbowR,
        Stuff::Iron,
        Piece::new(rod(0.056, 0.05), plate).at_xyz(0.0, -0.08, 0.0)
      );
  }
}

fn legs(kit: &mut Kit, frame: &Frame, pants: Srgba, boot: Srgba, cuff: Option<Srgba>) {
  kit
    .both(Joint::LegR, Stuff::Cloth, Piece::new(limb(0.1, 0.074, frame.thigh), pants))
    .both(Joint::KneeR, Stuff::Leather, Piece::new(limb(0.074, 0.062, frame.shin), boot))
    .both(
      Joint::KneeR,
      Stuff::Leather,
      Piece::new(model::lump(9, 0.05, 2), boot * 0.9)
        .sized(Vec3::new(0.075, 0.06, 0.15))
        .at_xyz(0.0, -frame.shin + 0.01, -0.05)
    );
  if let Some(cuff) = cuff {
    kit.both(
      Joint::KneeR,
      Stuff::Fur,
      Piece::new(model::lump(5, 0.14, 2), cuff)
        .sized(Vec3::new(0.1, 0.08, 0.1))
        .at_xyz(0.0, -0.12, 0.0)
    );
  }
}

pub fn dragonborn() -> Kit {
  let frame = MAN;
  let mut kit = Kit::new();
  let iron = srgb(0.5, 0.5, 0.52);
  let fur = srgb(0.42, 0.3, 0.2);
  let leather = srgb(0.3, 0.2, 0.13);
  face(
    &mut kit,
    srgb(0.78, 0.6, 0.48),
    srgb(0.36, 0.22, 0.12),
    Stuff::Gloss,
    srgb(0.1, 0.12, 0.14)
  );
  horned_helmet(&mut kit, iron, srgb(0.82, 0.76, 0.64), 1.0);
  torso(&mut kit, srgb(0.3, 0.26, 0.2), Some(iron), fur, leather, &frame);
  skirt(&mut kit, leather, srgb(0.25, 0.21, 0.17));
  arms(&mut kit, &frame, Stuff::Fur, fur, leather, srgb(0.26, 0.18, 0.12), Some(iron));
  legs(&mut kit, &frame, srgb(0.24, 0.2, 0.16), srgb(0.3, 0.22, 0.15), Some(fur));
  iron_sword(&mut kit, &frame);
  round_shield(&mut kit, &frame);
  kit
}

pub fn draugr(seed: u32) -> Kit {
  let frame = MAN;
  let mut kit = Kit::new();
  let flesh = srgb(0.36, 0.33, 0.27);
  let rags = srgb(0.22, 0.2, 0.17);
  let ancient = srgb(0.3, 0.33, 0.3);
  face(&mut kit, flesh, srgb(0.7, 0.68, 0.62), Stuff::Frost, srgb(0.5, 0.8, 1.0));
  kit.add(
    Joint::Head,
    Stuff::Fur,
    Piece::new(model::lump(seed, 0.2, 2), srgb(0.62, 0.6, 0.55))
      .sized(Vec3::new(0.12, 0.2, 0.08))
      .at_xyz(0.0, 0.05, 0.08)
  );
  (seed % 2 == 0).then(|| horned_helmet(&mut kit, ancient, srgb(0.55, 0.5, 0.42), 0.6));
  torso(
    &mut kit,
    flesh * 0.85,
    (seed % 3 != 0).then_some(ancient),
    rags,
    srgb(0.18, 0.15, 0.12),
    &frame
  );
  skirt(&mut kit, rags, rags * 0.8);
  arms(
    &mut kit,
    &frame,
    Stuff::Skin,
    flesh,
    rags,
    flesh * 0.9,
    (seed % 3 == 1).then_some(ancient)
  );
  legs(&mut kit, &frame, rags, srgb(0.2, 0.17, 0.14), None);
  war_axe(&mut kit, &frame, srgb(0.28, 0.3, 0.28));
  kit
}

pub fn bandit(seed: u32) -> Kit {
  let frame = MAN;
  let mut kit = Kit::new();
  let fur = srgb(0.5, 0.4, 0.3);
  let leather = srgb(0.34, 0.24, 0.16);
  let hair = [srgb(0.3, 0.2, 0.1), srgb(0.6, 0.45, 0.25), srgb(0.15, 0.1, 0.08)]
    [seed as usize % 3];
  face(&mut kit, srgb(0.8, 0.62, 0.5), hair, Stuff::Gloss, srgb(0.1, 0.1, 0.1));
  kit.add(
    Joint::Head,
    Stuff::Fur,
    Piece::new(model::lump(seed + 20, 0.15, 2), hair)
      .sized(Vec3::new(0.115, 0.1, 0.12))
      .at_xyz(0.0, 0.2, 0.02)
  );
  torso(&mut kit, srgb(0.36, 0.3, 0.22), None, fur, leather, &frame);
  skirt(&mut kit, leather, srgb(0.3, 0.25, 0.2));
  arms(&mut kit, &frame, Stuff::Cloth, srgb(0.35, 0.3, 0.24), leather, leather, None);
  legs(&mut kit, &frame, srgb(0.3, 0.26, 0.2), leather, Some(fur));
  (seed % 2 == 0).then(|| iron_sword(&mut kit, &frame));
  (seed % 2 == 1).then(|| war_axe(&mut kit, &frame, srgb(0.45, 0.45, 0.47)));
  kit
}

pub fn plugin(app: &mut App) {
  app.add_systems(PostUpdate, animate.before(TransformSystems::Propagate));
}
