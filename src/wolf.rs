use {crate::{humanoid::Motion,
             model::{self, Piece, ball, sculpt},
             noise,
             stuff::Stuff},
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
  pub bones: [Entity; BONES]
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

pub fn skeleton(commands: &mut Commands, owner: Entity) -> Beast {
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
  Beast { bones }
}

pub fn hide(seed: u32) -> Vec<(usize, Stuff, Mesh)> {
  let coat =
    [fur(0.44, 0.42, 0.4), fur(0.36, 0.33, 0.3), fur(0.52, 0.5, 0.48)][seed as usize % 3];
  let belly = fur(0.72, 0.68, 0.62);
  let pelt = |keys: &[(Vec3, Vec3)], shag: f32, seed: u32| {
    let mesh =
      model::ruffled(sculpt(keys, 6, 18), shag, Vec3::new(38.0, 22.0, 9.0), seed);
    shaded_fur(Piece::new(mesh, coat), coat, belly)
  };
  let key = |x: f32, y: f32, z: f32, wide: f32, high: f32, deep: f32| {
    (Vec3::new(x, y, z), Vec3::new(wide, high, deep))
  };
  let body = [
    pelt(
      &[
        key(0.0, 0.09, -0.49, 0.004, 0.004, 0.004),
        key(0.0, 0.07, -0.45, 0.075, 0.085, 0.11),
        key(0.0, 0.02, -0.34, 0.118, 0.1, 0.19),
        key(0.0, -0.03, -0.18, 0.124, 0.095, 0.225),
        key(0.0, -0.03, -0.02, 0.112, 0.085, 0.18),
        key(0.0, 0.0, 0.14, 0.088, 0.078, 0.11),
        key(0.0, 0.01, 0.28, 0.098, 0.082, 0.12),
        key(0.0, -0.005, 0.4, 0.096, 0.08, 0.125),
        key(0.0, -0.02, 0.49, 0.055, 0.055, 0.07),
        key(0.0, -0.025, 0.52, 0.004, 0.004, 0.004)
      ],
      0.016,
      1
    ),
    pelt(
      &[
        key(0.0, -0.02, -0.24, 0.004, 0.004, 0.004),
        key(0.0, 0.0, -0.3, 0.1, 0.1, 0.16),
        key(0.0, 0.08, -0.42, 0.095, 0.1, 0.15),
        key(0.0, 0.16, -0.52, 0.08, 0.085, 0.11),
        key(0.0, 0.2, -0.57, 0.062, 0.068, 0.08),
        key(0.0, 0.21, -0.6, 0.004, 0.004, 0.004)
      ],
      0.03,
      2
    )
  ];
  let head = [
    pelt(
      &[
        key(0.0, 0.035, 0.075, 0.004, 0.004, 0.004),
        key(0.0, 0.04, 0.05, 0.055, 0.05, 0.05),
        key(0.0, 0.042, 0.0, 0.072, 0.062, 0.058),
        key(0.0, 0.034, -0.06, 0.066, 0.05, 0.052),
        key(0.0, 0.014, -0.11, 0.044, 0.036, 0.044),
        key(0.0, 0.004, -0.17, 0.034, 0.03, 0.034),
        key(0.0, -0.001, -0.23, 0.028, 0.025, 0.028),
        key(0.0, -0.004, -0.268, 0.021, 0.019, 0.021),
        key(0.0, -0.005, -0.282, 0.004, 0.004, 0.004)
      ],
      0.004,
      3
    ),
    pelt(
      &[
        key(0.0, -0.01, 0.04, 0.004, 0.004, 0.004),
        key(0.0, -0.01, 0.02, 0.078, 0.045, 0.06),
        key(0.0, -0.005, -0.04, 0.07, 0.035, 0.04),
        key(0.0, 0.0, -0.08, 0.004, 0.004, 0.004)
      ],
      0.02,
      4
    )
  ];
  let ears = [1.0, -1.0].map(|side| {
    Piece::new(model::cone(0.044, 0.12), coat * 0.7)
      .sized(Vec3::new(1.0, 1.0, 0.42))
      .pitched(0.25)
      .rolled(-side * 0.28)
      .at_xyz(side * 0.042, 0.11, 0.02)
  });
  let jaw = pelt(
    &[
      key(0.0, 0.0, 0.04, 0.004, 0.004, 0.004),
      key(0.0, 0.0, 0.02, 0.036, 0.018, 0.022),
      key(0.0, 0.0, -0.05, 0.034, 0.014, 0.024),
      key(0.0, 0.001, -0.14, 0.025, 0.012, 0.018),
      key(0.0, 0.003, -0.2, 0.018, 0.01, 0.013),
      key(0.0, 0.004, -0.214, 0.004, 0.004, 0.004)
    ],
    0.003,
    5
  );
  let fangs = |down: f32, height: f32, reach: f32| {
    [1.0, -1.0].map(|side| {
      Piece::new(model::cone(0.006, 0.026), fur(0.9, 0.88, 0.8)).pitched(down).at_xyz(
        side * 0.017,
        height,
        reach
      )
    })
  };
  let (upper_fangs, lower_fangs) =
    (fangs(std::f32::consts::PI, -0.03, -0.215), fangs(0.0, 0.018, -0.17));
  let nose = Piece::new(ball(0.02), fur(0.05, 0.05, 0.05))
    .sized(Vec3::new(1.0, 0.8, 1.0))
    .at_xyz(0.0, 0.002, -0.278);
  let eyes = [0.048, -0.048]
    .map(|side| Piece::new(ball(0.012), fur(0.9, 0.7, 0.2)).at_xyz(side, 0.04, -0.088));
  let tail_path = model::spline(
    &[
      Vec3::ZERO,
      Vec3::new(0.0, -0.03, 0.12),
      Vec3::new(0.0, -0.15, 0.25),
      Vec3::new(0.0, -0.31, 0.31),
      Vec3::new(0.0, -0.45, 0.32)
    ],
    5
  );
  let bushy = model::taper(tail_path.len() - 1, 0.0, 1.0)
    .into_iter()
    .map(|t| {
      Vec3::splat(
        0.03 * (1.0 - t)
          + 0.075 * (std::f32::consts::PI * t.powf(0.7)).sin().max(0.0)
          + 0.003
      )
    })
    .collect::<Vec<_>>();
  let tip = LinearRgba::from(coat) * 0.35;
  let tail = Piece::new(
    model::ruffled(
      model::sweep(&tail_path, &bushy, 14),
      0.03,
      Vec3::new(30.0, 12.0, 30.0),
      6
    ),
    coat
  )
  .grained(3.0)
  .shaded(move |position, normal| {
    let brush = ((-position.y - 0.34) * 14.0).clamp(0.0, 1.0);
    let under = ((-normal.y + 0.1) * 1.6).clamp(0.0, 1.0);
    let shade = LinearRgba::from(coat)
      * (1.0 - 0.15 * under + noise::value3(position * 9.0, 7) * 0.2);
    (shade * (1.0 - brush) + tip * brush).with_alpha(1.0)
  });
  let upper = |bone: Bone, front: bool| {
    let inward = -bone.rest().x.signum();
    front
      .then(|| {
        pelt(
          &[
            key(inward * 0.045, 0.08, 0.0, 0.004, 0.004, 0.004),
            key(inward * 0.04, 0.06, 0.0, 0.03, 0.05, 0.05),
            key(inward * 0.015, -0.06, 0.0, 0.034, 0.055, 0.052),
            key(0.0, -0.16, -0.005, 0.03, 0.042, 0.04),
            key(0.0, -0.26, 0.0, 0.024, 0.03, 0.028),
            key(0.0, -UPPER, 0.005, 0.021, 0.025, 0.024),
            key(0.0, -UPPER - 0.03, 0.005, 0.004, 0.004, 0.004)
          ],
          0.006,
          7
        )
      })
      .unwrap_or_else(|| {
        pelt(
          &[
            key(inward * 0.045, 0.08, 0.02, 0.004, 0.004, 0.004),
            key(inward * 0.04, 0.06, 0.02, 0.035, 0.055, 0.055),
            key(inward * 0.018, -0.05, 0.01, 0.042, 0.08, 0.072),
            key(0.0, -0.16, -0.015, 0.036, 0.06, 0.05),
            key(0.0, -0.27, -0.01, 0.028, 0.038, 0.032),
            key(0.0, -UPPER, 0.0, 0.024, 0.028, 0.026),
            key(0.0, -UPPER - 0.03, 0.0, 0.004, 0.004, 0.004)
          ],
          0.008,
          8
        )
      })
  };
  let lower = |front: bool| {
    front
      .then(|| {
        [
          key(0.0, 0.02, 0.0, 0.004, 0.004, 0.004),
          key(0.0, 0.0, 0.0, 0.021, 0.024, 0.024),
          key(0.0, -0.14, 0.0, 0.019, 0.022, 0.02),
          key(0.0, -0.26, 0.004, 0.017, 0.022, 0.019),
          key(0.0, -0.3, -0.004, 0.016, 0.018, 0.018),
          key(0.0, -LOWER + 0.01, -0.02, 0.018, 0.018, 0.018)
        ]
      })
      .unwrap_or([
        key(0.0, 0.02, 0.0, 0.004, 0.004, 0.004),
        key(0.0, 0.0, 0.0, 0.028, 0.032, 0.034),
        key(0.0, -0.09, 0.03, 0.024, 0.03, 0.026),
        key(0.0, -0.16, 0.065, 0.016, 0.026, 0.016),
        key(0.0, -0.26, 0.04, 0.015, 0.017, 0.016),
        key(0.0, -LOWER + 0.01, 0.0, 0.018, 0.018, 0.018)
      ])
  };
  let paw = |front: bool| {
    let heel = front.then_some(-0.02).unwrap_or(0.0);
    [Piece::new(ball(1.0), coat * 0.8).sized(Vec3::new(0.028, 0.02, 0.04)).at_xyz(
      0.0,
      -LOWER,
      heel - 0.012
    )]
    .into_iter()
    .chain([-0.018, -0.006, 0.006, 0.018].map(|x| {
      Piece::new(ball(0.011), coat * 0.55).at_xyz(
        x,
        -LOWER - 0.008,
        heel - 0.045 + (x * 40.0).abs() * 0.004
      )
    }))
    .collect::<Vec<_>>()
  };

  let mut parts = Vec::new();
  let mut spawn = |bone: Bone, stuff: Stuff, pieces: Vec<Piece>| {
    parts.push((bone as usize, stuff, model::merge(pieces)))
  };
  spawn(Bone::Body, Stuff::Fur, body.into());
  spawn(Bone::Head, Stuff::Fur, head.into_iter().chain(ears).collect());
  spawn(Bone::Head, Stuff::Gloss, [nose].into_iter().chain(eyes).collect());
  spawn(Bone::Head, Stuff::Bone, upper_fangs.into());
  spawn(Bone::Jaw, Stuff::Fur, vec![jaw]);
  spawn(Bone::Jaw, Stuff::Bone, lower_fangs.into());
  spawn(Bone::Tail, Stuff::Fur, vec![tail]);
  [
    (Bone::FrontL, true),
    (Bone::FrontR, true),
    (Bone::BackL, false),
    (Bone::BackR, false)
  ]
  .into_iter()
  .for_each(|(bone, front)| spawn(bone, Stuff::Fur, vec![upper(bone, front)]));
  [
    (Bone::ShinFL, true),
    (Bone::ShinFR, true),
    (Bone::ShinBL, false),
    (Bone::ShinBR, false)
  ]
  .into_iter()
  .for_each(|(bone, front)| {
    spawn(
      bone,
      Stuff::Fur,
      [pelt(&lower(front), 0.004, 9)].into_iter().chain(paw(front)).collect()
    )
  });
  parts
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
