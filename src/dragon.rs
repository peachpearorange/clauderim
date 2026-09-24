use {crate::{combat::{Dead, Fighter, Shake, Side, Struck, Vitals},
             fx::Effects,
             humanoid::Motion,
             model::{self, Piece, ball, cone, curve, taper, tube},
             opts::opts,
             player::{Player, View},
             shout::{self, Staggered, WispLook},
             signal::{Cue, Notice, Shouts, Sound},
             sky::Daylight,
             stuff::{Stuff, Stuffs},
             terrain::{BOUND, Ground}},
     avian3d::prelude::*,
     bevy::{color::Mix, prelude::*},
     bevy_hanabi::EffectSpawner,
     std::f32::consts::{FRAC_PI_2, TAU}};

const ARRIVAL: f32 = 75.0;
const CRUISE: f32 = 24.0;
const ORBIT: f32 = 75.0;
const ALTITUDE: f32 = 48.0;
const STANCE: f32 = 2.5;
const BREATH_REACH: f32 = 24.0;
const BREATH_DPS: f32 = 16.0;
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flight {
  Waiting,
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
  bones: [Entity; BONES],
  velocity: Vec3,
  flap: f32,
  flap_rate: f32,
  breath: f32,
  breath_cooldown: f32,
  bite_cooldown: f32,
  passes: u32,
  scorch: f32,
  roared: bool,
  spent: bool
}

#[derive(Component)]
struct Breath;

#[derive(Component)]
struct DragonPart;

#[derive(Component)]
struct FoldsAway;

fn srgb(red: f32, green: f32, blue: f32) -> Srgba { Srgba::new(red, green, blue, 1.0) }

fn scaled(piece: Piece, back: Srgba, belly: Srgba) -> Piece {
  let (back, belly) = (LinearRgba::from(back), LinearRgba::from(belly));
  piece.shaded(move |position, normal| {
    let under = ((-normal.y - 0.15) * 1.8).clamp(0.0, 1.0);
    let mottle = crate::noise::value3(position * 2.3, 13) * 0.3 + 0.85;
    (back * mottle).mix(&belly, under).with_alpha(1.0)
  })
}

fn build(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  owner: Entity
) -> [Entity; BONES] {
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
  let hide = srgb(0.3, 0.27, 0.24);
  let belly = srgb(0.62, 0.55, 0.42);
  let horn = srgb(0.5, 0.45, 0.38);
  let lump = |seed: u32, size: Vec3, at: Vec3| {
    scaled(
      Piece::new(model::blob(seed, 0.06), hide).grained(3.0).sized(size).at(at),
      hide,
      belly
    )
  };
  let spikes = |from: f32, to: f32, count: usize, top: f32, height: f32| {
    (0..count)
      .map(|index| {
        let z = from + (to - from) * index as f32 / (count.max(2) - 1) as f32;
        Piece::new(cone(height * 0.35, height), horn * 0.8)
          .pitched(0.5)
          .at_xyz(0.0, top, z)
      })
      .collect::<Vec<_>>()
  };
  let mut parts: Vec<(Bone, Stuff, Vec<Piece>)> = vec![
    (Bone::Body, Stuff::Leather, vec![
      lump(1, Vec3::new(1.15, 1.0, 1.4), Vec3::new(0.0, 0.0, -0.4)),
      lump(2, Vec3::new(1.0, 0.92, 1.6), Vec3::new(0.0, -0.08, 1.0)),
      lump(3, Vec3::new(0.9, 0.85, 1.0), Vec3::new(0.0, 0.05, 2.3)),
      lump(4, Vec3::new(0.55, 0.5, 0.7), Vec3::new(0.75, 0.45, -0.7)),
      lump(5, Vec3::new(0.55, 0.5, 0.7), Vec3::new(-0.75, 0.45, -0.7)),
    ]),
    (Bone::Body, Stuff::Bone, spikes(-1.3, 2.8, 8, 0.95, 0.55)),
  ];
  let segment = |length: f32, girth: f32, next: f32| {
    let path = curve(Vec3::ZERO, Vec3::Z * length * 0.5, Vec3::Z * length, 6);
    [
      Piece::new(tube(&path, &taper(6, girth, next), 14), hide),
      Piece::new(ball(girth), hide)
    ]
    .map(|piece| scaled(piece.sized(Vec3::new(1.0, 0.88, 1.0)), hide, belly))
    .into()
  };
  [(Bone::Neck1, 0.62, 0.5), (Bone::Neck2, 0.5, 0.42), (Bone::Neck3, 0.42, 0.36)]
    .into_iter()
    .for_each(|(bone, girth, next)| {
      parts.push((bone, Stuff::Leather, segment(-1.05, girth, next)));
      parts.push((bone, Stuff::Bone, spikes(-0.2, -0.8, 2, girth * 0.85, 0.4)));
    });
  let snout = model::lathe(
    &[
      Vec2::new(0.0, 0.0),
      Vec2::new(0.3, 0.02),
      Vec2::new(0.27, 0.4),
      Vec2::new(0.2, 0.8),
      Vec2::new(0.12, 1.0),
      Vec2::new(0.0, 1.05)
    ],
    12
  );
  let horn_path = |side: f32, lift: f32, sweep: f32| {
    curve(
      Vec3::new(side * 0.22, 0.22 + lift, -0.15),
      Vec3::new(side * 0.42, 0.4 + lift, 0.5),
      Vec3::new(side * 0.35, 0.32 + lift * 2.0, 1.1 * sweep),
      10
    )
  };
  let horns: Vec<Piece> =
    [(1.0, 0.0, 1.0), (-1.0, 0.0, 1.0), (1.0, -0.15, 0.75), (-1.0, -0.15, 0.75)]
      .into_iter()
      .map(|(side, lift, sweep)| {
        Piece::new(
          tube(&horn_path(side, lift, sweep), &taper(10, 0.11 * sweep, 0.01), 8),
          horn
        )
      })
      .collect();
  let fangs = |row: f32, down: f32| {
    (0..6)
      .flat_map(|index| {
        let z = -0.55 - index as f32 * 0.12;
        let spread = 0.22 - index as f32 * 0.02;
        [1.0, -1.0].map(|side| {
          Piece::new(cone(0.035, 0.14), srgb(0.9, 0.86, 0.75)).pitched(down).at_xyz(
            side * spread,
            row,
            z
          )
        })
      })
      .collect::<Vec<_>>()
  };
  parts.extend([
    (Bone::Head, Stuff::Leather, vec![
      lump(20, Vec3::new(0.42, 0.36, 0.55), Vec3::new(0.0, 0.05, -0.3)),
      scaled(
        Piece::new(snout, hide)
          .sized(Vec3::new(1.0, 1.0, 0.6))
          .pitched(-FRAC_PI_2)
          .at_xyz(0.0, 0.02, -0.5),
        hide,
        belly
      ),
      lump(21, Vec3::new(0.13, 0.08, 0.3), Vec3::new(0.2, 0.25, -0.5)),
      lump(22, Vec3::new(0.13, 0.08, 0.3), Vec3::new(-0.2, 0.25, -0.5)),
    ]),
    (
      Bone::Head,
      Stuff::Bone,
      horns.into_iter().chain(fangs(-0.08, std::f32::consts::PI)).collect()
    ),
    (Bone::Head, Stuff::Ember, vec![
      Piece::new(ball(0.06), srgb(1.0, 0.7, 0.2)).at_xyz(0.24, 0.15, -0.55),
      Piece::new(ball(0.06), srgb(1.0, 0.7, 0.2)).at_xyz(-0.24, 0.15, -0.55),
    ]),
    (Bone::Jaw, Stuff::Leather, vec![lump(
      23,
      Vec3::new(0.26, 0.1, 0.62),
      Vec3::new(0.0, -0.02, -0.55)
    )]),
    (Bone::Jaw, Stuff::Bone, fangs(0.08, 0.0))
  ]);
  [
    (Bone::Tail1, 0.66, 0.48),
    (Bone::Tail2, 0.48, 0.34),
    (Bone::Tail3, 0.34, 0.22),
    (Bone::Tail4, 0.22, 0.06)
  ]
  .into_iter()
  .enumerate()
  .for_each(|(index, (bone, girth, next))| {
    parts.push((bone, Stuff::Leather, segment(1.55, girth, next)));
    parts.push((
      bone,
      Stuff::Bone,
      spikes(0.3, 1.3, 2, girth * 0.8, 0.35 - index as f32 * 0.05)
    ));
  });
  parts.push((Bone::Tail4, Stuff::Bone, vec![
    Piece::new(
      model::fan(
        &[
          Vec2::new(0.0, 0.0),
          Vec2::new(0.35, -0.5),
          Vec2::new(0.0, -1.1),
          Vec2::new(-0.35, -0.5)
        ],
        0.06
      ),
      horn
    )
    .pitched(-FRAC_PI_2)
    .at_xyz(0.0, 0.0, 1.5),
  ]));
  let wing_arm = tube(
    &curve(Vec3::ZERO, Vec3::new(2.0, 0.35, 0.3), Vec3::new(4.0, 0.2, 0.3), 10),
    &taper(10, 0.24, 0.13),
    8
  );
  let finger_tips = [
    Vec3::new(1.3, 0.0, -0.6),
    Vec3::new(0.6, 0.0, 2.6),
    Vec3::new(-0.8, 0.0, 3.9),
    Vec3::new(-2.6, 0.0, 4.3)
  ];
  let fingers: Vec<Piece> = finger_tips
    .iter()
    .map(|&tip| {
      Piece::new(
        tube(
          &curve(Vec3::ZERO, tip * 0.5 + Vec3::Y * 0.15, tip, 6),
          &taper(6, 0.09, 0.02),
          6
        ),
        hide * 0.8
      )
    })
    .collect();
  let skin = LinearRgba::from(srgb(0.36, 0.25, 0.2));
  let sheet = |corners: &[Vec3]| {
    Piece::new(
      model::fan(
        &corners.iter().map(|corner| Vec2::new(corner.x, -corner.z)).collect::<Vec<_>>(),
        0.02
      ),
      srgb(0.36, 0.26, 0.22)
    )
    .pitched(-FRAC_PI_2)
    .shaded(move |position, _| {
      let reach = position.length();
      let veins = (f32::atan2(position.z, position.x) * 7.0).sin().abs().powf(12.0);
      let blotch = crate::noise::value3(position * 1.7, 29) * 0.35 + 0.8;
      (skin * blotch * (1.15 - reach * 0.08).max(0.55) * (1.0 - veins * 0.35))
        .with_alpha(1.0)
    })
  };
  let membranes: Vec<Piece> =
    finger_tips.windows(2).map(|pair| sheet(&[Vec3::ZERO, pair[0], pair[1]])).collect();
  let inner = sheet(&[
    Vec3::ZERO,
    Vec3::new(4.0, 0.0, 0.3),
    Vec3::new(4.0 - 2.6, 0.0, 4.6),
    Vec3::new(0.3, 0.0, 3.6)
  ]);
  let thigh = lump(40, Vec3::new(0.42, 0.85, 0.6), Vec3::new(0.0, -0.55, 0.05));
  let shin = Piece::new(
    tube(
      &curve(Vec3::ZERO, Vec3::new(0.0, -0.5, 0.35), Vec3::new(0.0, -1.0, 0.1), 6),
      &taper(6, 0.2, 0.13),
      8
    ),
    hide
  );
  let foot = Piece::new(model::lump(41, 0.08, 1), hide)
    .sized(Vec3::new(0.3, 0.12, 0.45))
    .at_xyz(0.0, -1.05, -0.2);
  let claws: Vec<Piece> = [-0.15, 0.0, 0.15]
    .into_iter()
    .map(|x| {
      Piece::new(cone(0.05, 0.22), srgb(0.2, 0.18, 0.16))
        .pitched(-1.3)
        .at_xyz(x, -1.1, -0.6)
    })
    .collect();
  let right_side: Vec<(Bone, Stuff, Vec<Piece>)> = vec![
    (Bone::WingR, Stuff::Leather, vec![scaled(Piece::new(wing_arm, hide), hide, belly)]),
    (Bone::WingR, Stuff::Membrane, vec![inner]),
    (Bone::TipR, Stuff::Leather, fingers),
    (Bone::TipR, Stuff::Membrane, membranes),
    (Bone::LegR, Stuff::Leather, vec![thigh]),
    (Bone::ShinR, Stuff::Leather, vec![shin, foot]),
    (Bone::ShinR, Stuff::Bone, claws),
  ];
  right_side.into_iter().for_each(|(bone, stuff, pieces)| {
    let mirrored: Vec<Piece> = pieces.iter().map(Piece::mirrored).collect();
    parts.push((bone.right_twin().unwrap_or(bone), stuff, mirrored));
    parts.push((bone, stuff, pieces));
  });
  parts.into_iter().for_each(|(bone, stuff, pieces)| {
    let part = commands
      .spawn((
        DragonPart,
        Mesh3d(meshes.add(model::merge(pieces))),
        MeshMaterial3d(stuffs.of(stuff)),
        ChildOf(bones[bone as usize])
      ))
      .id();
    if matches!(bone, Bone::WingL | Bone::WingR) && stuff == Stuff::Membrane {
      commands.entity(part).insert(FoldsAway);
    }
  });
  bones
}

fn spawn_dragon(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  effects: Res<Effects>
) {
  let dragon = commands
    .spawn((
      Name::new("Dragon"),
      Side::Wild,
      Vitals::new(HEALTH, 200.0),
      Motion::default(),
      Fighter { reach: 7.5, damage: 26.0, swing_time: 1.1, cone: 0.6, girth: 3.0 },
      RigidBody::Kinematic,
      Collider::sphere(2.0),
      Transform::from_xyz(0.0, -500.0, 0.0),
      Visibility::Hidden
    ))
    .id();
  let bones = build(&mut commands, &mut meshes, &stuffs, dragon);
  commands.spawn((
    Breath,
    effects.emit(&effects.breath),
    EffectSpawner::new(&bevy_hanabi::SpawnerSettings::rate(420.0.into()))
      .with_active(false),
    Transform::from_xyz(0.0, -0.05, -1.3),
    ChildOf(bones[Bone::Head as usize])
  ));
  commands.entity(dragon).insert(Dragon {
    flight: Flight::Waiting,
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
    spent: false
  });
}

fn steer_toward(
  velocity: Vec3,
  goal: Vec3,
  at: Vec3,
  speed: f32,
  agility: f32,
  delta: f32
) -> Vec3 {
  let wanted = (goal - at).normalize_or_zero() * speed;
  velocity.lerp(wanted, 1.0 - (-agility * delta).exp())
}

fn fly(
  time: Res<Time>,
  ground: Res<Ground>,
  daylight: Res<Daylight>,
  view: Res<View>,
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
  let arrival = opts().dragon.unwrap_or(ARRIVAL);
  dragons.iter_mut().for_each(
    |(
      entity,
      mut dragon,
      mut transform,
      mut visibility,
      mut motion,
      vitals,
      dead,
      staggered
    )| {
      let at = transform.translation;
      let floor = |spot: Vec3| ground.height(spot.xz());
      let flat_gap = (target - at).with_y(0.0);
      let distance = flat_gap.length();
      dragon.breath_cooldown -= delta;
      dragon.bite_cooldown -= delta;
      dragon.breath = (dragon.breath - delta).max(0.0);
      let aloft = at.y - floor(at) > STANCE + 1.5;
      let flight = match dragon.flight {
        Flight::Falling | Flight::Slain(_) => dragon.flight,
        _ if dead && aloft => Flight::Falling,
        _ if dead => Flight::Slain(0.0),
        other => other
      };
      let next = match flight {
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
          let ahead =
            target + view.flat_forward() * aloft.then_some(26.0).unwrap_or(13.0);
          transform.translation = ground.surface(ahead.xz())
            + Vec3::Y * aloft.then_some(14.0).unwrap_or(STANCE);
          transform.look_to(view.flat_forward().cross(Vec3::Y), Vec3::Y);
          *visibility = Visibility::Inherited;
          Flight::Posing { aloft }
        }
        Flight::Posing { .. } => flight,
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
          if circled > 10.0 && !hero_dead {
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
          if facing_player && distance < 60.0 && distance > 8.0 {
            dragon.breath = 0.3;
          }
          (progress > (to - from).with_y(0.0).length())
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
          if touched {
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
          let walking = distance > 9.5
            && dragon.breath <= 0.0
            && motion.swing.is_none()
            && !staggered;
          dragon.velocity = walking.then_some(facing * 3.2).unwrap_or(Vec3::ZERO);
          if !hero_dead
            && !staggered
            && distance < 8.5
            && dragon.bite_cooldown <= 0.0
            && motion.swing.is_none()
          {
            motion.swing = Some(0.0);
            dragon.bite_cooldown = 2.6;
          }
          if !hero_dead
            && !staggered
            && (8.5..BREATH_REACH).contains(&distance)
            && dragon.breath_cooldown <= 0.0
          {
            dragon.breath = 2.6;
            dragon.breath_cooldown = 7.0;
            sounds.write(Sound::here(Cue::DragonRoar, at));
          }
          let restless = time_down > 28.0
            || (hero_dead && time_down > 4.0)
            || (vitals.health < HEALTH * 0.45 && !dragon.spent);
          if restless {
            dragon.spent = dragon.spent || vitals.health < HEALTH * 0.45;
            Flight::Rising(0.0)
          } else {
            Flight::Grounded(time_down + delta)
          }
        }
        Flight::Rising(lifted) => {
          dragon.velocity = Vec3::Y * 9.0 + transform.forward().as_vec3() * lifted * 6.0;
          (lifted > 2.5)
            .then_some(Flight::Circling(0.0))
            .unwrap_or(Flight::Rising(lifted + delta))
        }
        Flight::Falling => {
          dragon.velocity += Vec3::NEG_Y * 12.0 * delta;
          dragon.velocity = dragon.velocity.with_y(dragon.velocity.y.max(-25.0));
          (at.y <= floor(at) + STANCE * 0.5)
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
        Flight::Grounded(_) | Flight::Slain(_) | Flight::Posing { aloft: false }
      );
      let moved = transform.translation + dragon.velocity * delta;
      let clamped = moved.clamp(
        Vec3::new(-BOUND * 1.4, -400.0, -BOUND * 1.4),
        Vec3::new(BOUND * 1.4, 3000.0, BOUND * 1.4)
      );
      transform.translation = if grounded {
        let settle = matches!(next, Flight::Slain(_)).then_some(1.2).unwrap_or(0.0);
        clamped.with_y(
          (floor(clamped) + STANCE - settle).lerp(clamped.y, (-6.0 * delta).exp())
        )
      } else {
        clamped.with_y(clamped.y.max(floor(clamped) + 2.0))
      };
      let heading = if grounded || matches!(next, Flight::Landing(_) | Flight::Rising(_))
      {
        flat_gap.normalize_or(transform.forward().as_vec3())
      } else {
        dragon.velocity.normalize_or(transform.forward().as_vec3())
      };
      let turn = transform
        .forward()
        .as_vec3()
        .with_y(0.0)
        .normalize_or_zero()
        .cross(heading.with_y(0.0).normalize_or_zero())
        .y;
      let bank = (!grounded).then_some(-turn * 2.5).unwrap_or(0.0).clamp(-0.8, 0.8);
      let pitch = (!grounded).then_some(heading.y.asin() * 0.8).unwrap_or(0.0);
      let yaw = f32::atan2(-heading.x, -heading.z);
      let aim = Quat::from_euler(EulerRot::YXZ, yaw, pitch, bank);
      if !matches!(next, Flight::Slain(_) | Flight::Waiting | Flight::Posing { .. }) {
        let agility = grounded.then_some(1.6).unwrap_or(3.0);
        transform.rotation =
          transform.rotation.slerp(aim, 1.0 - (-agility * delta).exp());
      }

      dragon.flap_rate = match next {
        Flight::Landing(_) | Flight::Rising(_) => 3.4,
        Flight::Grounded(_)
        | Flight::Slain(_)
        | Flight::Waiting
        | Flight::Posing { aloft: false } => 0.0,
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
      if breathing && !hero_dead {
        let mouth = at + transform.forward().as_vec3() * 5.5;
        let reach = target - mouth;
        let aimed = transform
          .forward()
          .as_vec3()
          .with_y(0.0)
          .normalize_or_zero()
          .dot(reach.with_y(0.0).normalize_or_zero())
          > 0.85;
        if aimed && reach.with_y(0.0).length() < BREATH_REACH && reach.y.abs() < 16.0 {
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
  );
}

fn kindle(dragons: Query<&Dragon>, mut breaths: Query<&mut EffectSpawner, With<Breath>>) {
  let breathing = dragons.iter().any(|dragon| {
    dragon.breath > 0.0 && !matches!(dragon.flight, Flight::Slain(_) | Flight::Falling)
  });
  breaths.iter_mut().for_each(|mut spawner| spawner.active = breathing);
}

fn pose(
  time: Res<Time>,
  dragons: Query<(&Dragon, &Motion)>,
  mut bones: Query<&mut Transform, (Without<Dragon>, Without<FoldsAway>)>,
  mut folding: Query<&mut Transform, (With<FoldsAway>, Without<Dragon>)>
) {
  let delta = time.delta_secs();
  let blend = 1.0 - (-8.0 * delta).exp();
  let clock = time.elapsed_secs();
  dragons.iter().for_each(|(dragon, motion)| {
    let beat = dragon.flap.sin();
    let lag = (dragon.flap - 0.9).sin();
    let flying = !matches!(
      dragon.flight,
      Flight::Grounded(_)
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
        Bone::Neck1 if breathing => Vec3::new(0.15, 0.0, 0.0),
        Bone::Neck1 if flying => Vec3::new(-0.05, 0.05 * sway, 0.0),
        Bone::Neck1 => Vec3::new(-0.55 + 0.4 * bite, 0.0, 0.0),
        Bone::Neck2 if slain => Vec3::new(0.2, 0.3, 0.0),
        Bone::Neck2 if flying => Vec3::new(0.05, 0.05 * sway, 0.0),
        Bone::Neck2 => Vec3::new(0.15 + 0.3 * bite, 0.0, 0.0),
        Bone::Neck3 if slain => Vec3::new(0.1, 0.4, 0.0),
        Bone::Neck3 => Vec3::new(0.25 + 0.2 * bite, 0.0, 0.0),
        Bone::Head if breathing => Vec3::new(0.25, 0.0, 0.0),
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
        Bone::WingR if slain => Vec3::new(0.0, 0.3, -0.2),
        Bone::WingL if slain => Vec3::new(0.0, -0.3, 0.2),
        Bone::WingR if flying => {
          Vec3::new(0.0, 0.0, beat * 0.75 * (dragon.flap_rate > 0.1) as u8 as f32 + 0.05)
        }
        Bone::WingL if flying => {
          Vec3::new(0.0, 0.0, -beat * 0.75 * (dragon.flap_rate > 0.1) as u8 as f32 - 0.05)
        }
        Bone::WingR => Vec3::new(0.2, 0.9, -1.1),
        Bone::WingL => Vec3::new(0.2, -0.9, 1.1),
        Bone::TipR if flying => Vec3::new(0.0, 0.0, lag * 0.5),
        Bone::TipL if flying => Vec3::new(0.0, 0.0, -lag * 0.5),
        Bone::TipR => Vec3::new(0.0, 2.2, 1.2),
        Bone::TipL => Vec3::new(0.0, -2.2, -1.2),
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
    let (spread, furl) = flying.then_some((1.0, 1.0)).unwrap_or((0.4, 0.25));
    dragon.bones.iter().zip(Bone::ALL).for_each(|(&entity, bone)| {
      if let Ok(mut transform) = bones.get_mut(entity) {
        let angles = angles(bone);
        let target = Quat::from_euler(EulerRot::YXZ, angles.y, angles.x, angles.z);
        transform.rotation = transform.rotation.slerp(target, blend);
        if matches!(bone, Bone::TipL | Bone::TipR) {
          transform.scale = transform.scale.lerp(Vec3::splat(spread), blend);
        }
      }
    });
    folding.iter_mut().for_each(|mut transform| {
      transform.scale = transform.scale.lerp(Vec3::new(1.0, 1.0, furl), blend)
    });
  });
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
  dragons: Query<(&Dragon, &Transform)>,
  mut parts: Query<&mut MeshMaterial3d<StandardMaterial>, With<DragonPart>>,
  mut streamed: Local<u32>,
  mut finished: Local<bool>
) {
  dragons.iter().for_each(|(dragon, transform)| {
    if let Flight::Slain(since) = dragon.flight
      && !*finished
    {
      let flowing = (since - 2.0).clamp(0.0, 5.0);
      let due = (flowing * 40.0) as u32;
      (*streamed..due).for_each(|index| {
        let mut roll = crate::noise::Roll::new(index * 97 + 5);
        let from = transform.transform_point(Vec3::new(
          roll.spread(1.5),
          roll.spread(1.0),
          roll.range(-3.0, 4.0)
        ));
        shout::stream(&mut commands, &look, from, *player, index + 999, true);
      });
      *streamed = due;
      if since > 2.0 && since - time.delta_secs() <= 2.0 {
        let embers = stuffs.of(Stuff::Cinder);
        parts.iter_mut().for_each(|mut material| material.0 = embers.clone());
      }
      if since > 7.5 {
        let bone = stuffs.of(Stuff::Bone);
        parts.iter_mut().for_each(|mut material| material.0 = bone.clone());
        *finished = true;
        shouts.cooldown = 0.0;
        sounds.write(Sound::flat(Cue::WordLearned));
        notices.write(Notice("Dragon Soul Absorbed".into()));
      }
    }
  });
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(Startup, spawn_dragon)
    .add_systems(Update, (fly, kindle, absorb).chain().after(crate::walker::Walking))
    .add_systems(PostUpdate, pose.before(TransformSystems::Propagate));
}
