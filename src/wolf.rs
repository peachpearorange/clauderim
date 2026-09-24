use {crate::{humanoid::Motion,
             model::{self, Piece, ball, curve, limb, tube},
             stuff::{Stuff, Stuffs}},
     bevy::prelude::*};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bone {
  Body,
  Head,
  Jaw,
  Tail,
  FrontL,
  FrontR,
  BackL,
  BackR,
  ShinFL,
  ShinFR,
  ShinBL,
  ShinBR
}

const BONES: usize = 12;

impl Bone {
  const ALL: [Bone; BONES] = [
    Bone::Body,
    Bone::Head,
    Bone::Jaw,
    Bone::Tail,
    Bone::FrontL,
    Bone::FrontR,
    Bone::BackL,
    Bone::BackR,
    Bone::ShinFL,
    Bone::ShinFR,
    Bone::ShinBL,
    Bone::ShinBR
  ];

  const fn parent(self) -> Option<Bone> {
    match self {
      Bone::Body => None,
      Bone::Head
      | Bone::Tail
      | Bone::FrontL
      | Bone::FrontR
      | Bone::BackL
      | Bone::BackR => Some(Bone::Body),
      Bone::Jaw => Some(Bone::Head),
      Bone::ShinFL => Some(Bone::FrontL),
      Bone::ShinFR => Some(Bone::FrontR),
      Bone::ShinBL => Some(Bone::BackL),
      Bone::ShinBR => Some(Bone::BackR)
    }
  }

  const fn rest(self) -> Vec3 {
    match self {
      Bone::Body => Vec3::new(0.0, WITHERS, 0.0),
      Bone::Head => Vec3::new(0.0, 0.21, -0.57),
      Bone::Jaw => Vec3::new(0.0, -0.035, -0.05),
      Bone::Tail => Vec3::new(0.0, 0.08, 0.46),
      Bone::FrontL => Vec3::new(-0.09, -0.06, -0.3),
      Bone::FrontR => Vec3::new(0.09, -0.06, -0.3),
      Bone::BackL => Vec3::new(-0.09, -0.02, 0.34),
      Bone::BackR => Vec3::new(0.09, -0.02, 0.34),
      Bone::ShinFL | Bone::ShinFR => Vec3::new(0.0, -UPPER, 0.0),
      Bone::ShinBL | Bone::ShinBR => Vec3::new(0.0, -UPPER, 0.0)
    }
  }
}

const WITHERS: f32 = 0.72;
const UPPER: f32 = 0.34;
const LOWER: f32 = 0.36;

#[derive(Component)]
pub struct Beast {
  bones: [Entity; BONES]
}

fn fur(red: f32, green: f32, blue: f32) -> Srgba { Srgba::new(red, green, blue, 1.0) }

fn shaded_fur(piece: Piece, coat: Srgba, belly: Srgba) -> Piece {
  let (coat, belly) = (LinearRgba::from(coat), LinearRgba::from(belly));
  piece.grained(3.0).shaded(move |position, normal| {
    let under = ((-normal.y + 0.1) * 1.6).clamp(0.0, 1.0);
    let saddle = ((normal.y - 0.5) * 2.0).clamp(0.0, 1.0) * 0.35;
    let streak = crate::noise::value3(position * 9.0, 7) * 0.2;
    let base = coat * (1.0 - saddle + streak - 0.1);
    let blended = base * (1.0 - under) + belly * under;
    blended.with_alpha(1.0)
  })
}

pub fn spawn_wolf(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  owner: Entity,
  seed: u32
) -> Beast {
  let coat =
    [fur(0.44, 0.42, 0.4), fur(0.36, 0.33, 0.3), fur(0.52, 0.5, 0.48)][seed as usize % 3];
  let belly = fur(0.72, 0.68, 0.62);
  let mut bones = [Entity::PLACEHOLDER; BONES];
  Bone::ALL.into_iter().for_each(|bone| {
    let parent = bone.parent().map_or(owner, |parent| bones[parent as usize]);
    bones[bone as usize] = commands
      .spawn((
        Transform::from_translation(bone.rest()),
        Visibility::Inherited,
        ChildOf(parent)
      ))
      .id();
  });
  let lump = |seed: u32, size: Vec3, at: Vec3| {
    Piece::new(model::blob(seed, 0.05), coat).sized(size).at(at)
  };
  let body = [
    lump(1, Vec3::new(0.15, 0.2, 0.3), Vec3::new(0.0, -0.03, -0.2)),
    lump(2, Vec3::new(0.12, 0.14, 0.3), Vec3::new(0.0, 0.02, 0.12)),
    lump(3, Vec3::new(0.14, 0.17, 0.17), Vec3::new(0.0, 0.02, 0.33)),
    Piece::new(model::blob(4, 0.05), coat)
      .sized(Vec3::new(0.1, 0.11, 0.2))
      .pitched(0.7)
      .at_xyz(0.0, 0.1, -0.44),
    lump(5, Vec3::new(0.15, 0.11, 0.2), Vec3::new(0.0, 0.13, -0.32))
  ]
  .map(|piece| shaded_fur(piece, coat, belly));
  let muzzle = model::lathe(
    &[
      Vec2::new(0.0, -0.02),
      Vec2::new(0.078, 0.0),
      Vec2::new(0.068, 0.08),
      Vec2::new(0.046, 0.17),
      Vec2::new(0.03, 0.2),
      Vec2::new(0.0, 0.21)
    ],
    10
  );
  let head = [
    lump(6, Vec3::new(0.1, 0.095, 0.115), Vec3::new(0.0, 0.03, 0.0)),
    Piece::new(muzzle, coat)
      .sized(Vec3::new(1.0, 1.0, 0.82))
      .pitched(-1.62)
      .at_xyz(0.0, 0.0, -0.07),
    Piece::new(model::cone(0.042, 0.12), coat * 0.75)
      .sized(Vec3::new(1.0, 1.0, 0.45))
      .rolled(-0.22)
      .at_xyz(0.055, 0.12, 0.03),
    Piece::new(model::cone(0.042, 0.12), coat * 0.75)
      .sized(Vec3::new(1.0, 1.0, 0.45))
      .rolled(0.22)
      .at_xyz(-0.055, 0.12, 0.03)
  ]
  .map(|piece| shaded_fur(piece, coat, belly));
  let jaw = Piece::new(model::blob(8, 0.05), coat)
    .sized(Vec3::new(0.045, 0.025, 0.11))
    .at_xyz(0.0, 0.0, -0.1);
  let teeth = Piece::new(model::block(0.04, 0.012, 0.1), fur(0.9, 0.88, 0.8))
    .at_xyz(0.0, 0.012, -0.1);
  let nose = Piece::new(ball(0.022), fur(0.05, 0.05, 0.05)).at_xyz(0.0, 0.012, -0.28);
  let eyes = [0.048, -0.048]
    .map(|side| Piece::new(ball(0.014), fur(0.9, 0.7, 0.2)).at_xyz(side, 0.045, -0.075));
  let tail_path =
    curve(Vec3::ZERO, Vec3::new(0.0, -0.05, 0.22), Vec3::new(0.0, -0.4, 0.34), 8);
  let tail = shaded_fur(
    Piece::new(
      tube(&tail_path, &[0.035, 0.055, 0.07, 0.075, 0.075, 0.07, 0.06, 0.04, 0.008], 8),
      coat
    ),
    coat,
    belly
  );
  let upper = |front: bool| {
    let thick = front.then_some(0.052).unwrap_or(0.072);
    shaded_fur(Piece::new(limb(thick, 0.034, UPPER), coat), coat, belly)
  };
  let lower = Piece::new(limb(0.03, 0.024, LOWER), coat * 0.9);
  let paw = Piece::new(model::lump(9, 0.06, 1), coat * 0.7)
    .sized(Vec3::new(0.045, 0.03, 0.07))
    .at_xyz(0.0, -LOWER, -0.03);

  let mut spawn = |bone: Bone, stuff: Stuff, pieces: Vec<Piece>| {
    commands.spawn((
      Mesh3d(meshes.add(model::merge(pieces))),
      MeshMaterial3d(stuffs.of(stuff)),
      ChildOf(bones[bone as usize])
    ));
  };
  spawn(Bone::Body, Stuff::Fur, body.into());
  spawn(Bone::Head, Stuff::Fur, head.into());
  spawn(Bone::Head, Stuff::Gloss, [nose].into_iter().chain(eyes).collect());
  spawn(Bone::Jaw, Stuff::Fur, vec![jaw]);
  spawn(Bone::Jaw, Stuff::Bone, vec![teeth]);
  spawn(Bone::Tail, Stuff::Fur, vec![tail]);
  [Bone::FrontL, Bone::FrontR]
    .into_iter()
    .for_each(|bone| spawn(bone, Stuff::Fur, vec![upper(true)]));
  [Bone::BackL, Bone::BackR]
    .into_iter()
    .for_each(|bone| spawn(bone, Stuff::Fur, vec![upper(false)]));
  [Bone::ShinFL, Bone::ShinFR, Bone::ShinBL, Bone::ShinBR].into_iter().for_each(|bone| {
    spawn(bone, Stuff::Fur, vec![Piece(lower.0.clone()), Piece(paw.0.clone())])
  });
  Beast { bones }
}

#[derive(Clone, Copy, Default)]
struct Stance {
  angles: [Vec3; BONES],
  lift: f32,
  reach: f32,
  roll: f32
}

impl Stance {
  fn with(mut self, bone: Bone, angles: Vec3) -> Self {
    self.angles[bone as usize] = angles;
    self
  }

  fn add(mut self, bone: Bone, angles: Vec3) -> Self {
    self.angles[bone as usize] += angles;
    self
  }

  fn blend(&self, other: &Stance, amount: f32) -> Stance {
    Stance {
      angles: std::array::from_fn(|index| {
        self.angles[index].lerp(other.angles[index], amount)
      }),
      lift: self.lift.lerp(other.lift, amount),
      reach: self.reach.lerp(other.reach, amount),
      roll: self.roll.lerp(other.roll, amount)
    }
  }
}

fn stance(motion: &Motion) -> Stance {
  let pace = (motion.speed / 7.0).min(1.3);
  let (wave, lift) = (motion.stride.sin(), motion.stride.cos());
  let bend = |phase: f32| (0.3 + 0.9 * pace) * (0.5 + 0.5 * phase).powi(2);
  let breathe = motion.breath.sin();
  let standing = Stance::default()
    .with(Bone::Head, Vec3::new(-0.1 + breathe * 0.03, 0.0, 0.0))
    .with(Bone::Tail, Vec3::new(0.2, breathe * 0.2, 0.0))
    .with(Bone::ShinFL, Vec3::new(0.08, 0.0, 0.0))
    .with(Bone::ShinFR, Vec3::new(0.08, 0.0, 0.0))
    .with(Bone::BackL, Vec3::new(0.15, 0.0, 0.0))
    .with(Bone::BackR, Vec3::new(0.15, 0.0, 0.0))
    .with(Bone::ShinBL, Vec3::new(-0.3, 0.0, 0.0))
    .with(Bone::ShinBR, Vec3::new(-0.3, 0.0, 0.0));
  let gait = standing
    .add(Bone::FrontL, Vec3::X * wave * 0.55 * pace)
    .add(Bone::BackR, Vec3::X * wave * 0.5 * pace)
    .add(Bone::FrontR, Vec3::X * -wave * 0.55 * pace)
    .add(Bone::BackL, Vec3::X * -wave * 0.5 * pace)
    .add(Bone::ShinFL, Vec3::X * -bend(lift) * 0.8)
    .add(Bone::ShinBR, Vec3::X * -bend(lift) * 0.6)
    .add(Bone::ShinFR, Vec3::X * -bend(-lift) * 0.8)
    .add(Bone::ShinBL, Vec3::X * -bend(-lift) * 0.6)
    .add(Bone::Body, Vec3::new(lift * 0.04 * pace, 0.0, 0.0))
    .add(Bone::Head, Vec3::new(-0.15 * pace + wave * 0.04, 0.0, 0.0))
    .add(Bone::Tail, Vec3::new(-0.5 * pace, 0.0, 0.0));
  let gait = Stance { lift: (2.0 * motion.stride).cos().abs() * 0.03 * pace, ..gait };
  let crouch = Stance { lift: -0.12, reach: 0.15, ..gait }
    .with(Bone::Head, Vec3::new(0.35, 0.0, 0.0))
    .with(Bone::Jaw, Vec3::new(-0.1, 0.0, 0.0))
    .add(Bone::FrontL, Vec3::X * 0.3)
    .add(Bone::FrontR, Vec3::X * 0.3)
    .add(Bone::ShinBL, Vec3::X * -0.5)
    .add(Bone::ShinBR, Vec3::X * -0.5);
  let lunge = Stance { lift: 0.12, reach: -0.6, ..gait }
    .with(Bone::Body, Vec3::new(0.2, 0.0, 0.0))
    .with(Bone::Head, Vec3::new(-0.3, 0.0, 0.0))
    .with(Bone::Jaw, Vec3::new(0.7, 0.0, 0.0))
    .with(Bone::FrontL, Vec3::new(1.0, 0.0, 0.0))
    .with(Bone::FrontR, Vec3::new(0.9, 0.0, 0.0))
    .with(Bone::BackL, Vec3::new(-0.6, 0.0, 0.0))
    .with(Bone::BackR, Vec3::new(-0.6, 0.0, 0.0));
  let bitten = motion
    .swing
    .map(|progress| match progress {
      early if early < 0.35 => gait.blend(&crouch, early / 0.35),
      mid if mid < 0.55 => crouch.blend(&lunge, (mid - 0.35) / 0.2),
      late => lunge.blend(&gait, (late - 0.55) / 0.45)
    })
    .unwrap_or(gait);
  let flinched = bitten
    .add(Bone::Head, Vec3::new(-0.5, 0.3, 0.0) * motion.flinch)
    .add(Bone::Body, Vec3::new(-0.2, 0.0, 0.1) * motion.flinch);
  let dead = Stance { lift: -WITHERS + 0.2, reach: 0.0, roll: 1.45, ..standing }
    .with(Bone::Head, Vec3::new(-0.3, 0.0, 0.3))
    .with(Bone::Jaw, Vec3::new(0.25, 0.0, 0.0))
    .with(Bone::FrontL, Vec3::new(0.6, 0.0, 0.0))
    .with(Bone::FrontR, Vec3::new(0.3, 0.0, 0.0))
    .with(Bone::BackL, Vec3::new(-0.4, 0.0, 0.0))
    .with(Bone::BackR, Vec3::new(-0.2, 0.0, 0.0))
    .with(Bone::Tail, Vec3::new(-0.8, 0.0, 0.0));
  flinched.blend(&dead, motion.fallen)
}

fn animate(
  time: Res<Time>,
  mut beasts: Query<(&Beast, &mut Motion)>,
  mut bones: Query<&mut Transform>
) {
  let blend = 1.0 - (-16.0 * time.delta_secs()).exp();
  beasts.iter_mut().for_each(|(beast, mut motion)| {
    motion.breath += time.delta_secs() * 2.4;
    let pose = stance(&motion);
    beast.bones.iter().zip(Bone::ALL).for_each(|(&entity, bone)| {
      if let Ok(mut transform) = bones.get_mut(entity) {
        let angles = pose.angles[bone as usize];
        let target = Quat::from_euler(EulerRot::YXZ, angles.y, angles.x, angles.z);
        let (target, place) = if bone == Bone::Body {
          (
            Quat::from_rotation_z(pose.roll) * target,
            bone.rest() + Vec3::new(0.0, pose.lift, pose.reach)
          )
        } else {
          (target, bone.rest())
        };
        transform.rotation = transform.rotation.slerp(target, blend);
        transform.translation = transform.translation.lerp(place, blend);
      }
    });
  });
}

pub fn plugin(app: &mut App) {
  app.add_systems(PostUpdate, animate.before(TransformSystems::Propagate));
}
