use {crate::{blocky::{self, Bone, Facet, Pixel, Shape, Texel},
             combat::{Dead, Shake},
             opts::opts,
             place,
             player::{MainCamera, Player, Seated, View, capsule_offset},
             river,
             signal::{Cue, Notice, Prompt, Prompting, Sound},
             terrain::{self, BOUND, smooth},
             walker::{Layer, Walker, Walking}},
     avian3d::{math::AdjustPrecision, prelude::*},
     bevy::{input::mouse::AccumulatedMouseMotion, prelude::*},
     std::{f32::consts::{FRAC_PI_2, PI, TAU},
           sync::LazyLock}};

const PX: f32 = 1.0 / 8.0;
const PAD_HALF: f32 = 8.0;
const PAD_LIFT: f32 = 0.35;
const GRAVITY: f32 = 9.81;
const SPOOL_UP: f32 = 0.25;
const SPOOL_DOWN: f32 = 0.12;
const LEVER_RATE: f32 = 2.5;
const LEVER_RANGE: f32 = 1.8;
const RESERVE: f32 = 0.55;
const MAX_TILT: f32 = 0.45;
const MAX_BANK: f32 = 0.42;
const COORDINATION: f32 = 0.05;
const ATTITUDE_SPRING: f32 = 7.0;
const ATTITUDE_DAMP: f32 = 5.0;
const YAW_SPRING: f32 = 3.5;
const YAW_DAMP: f32 = 3.0;
const YAW_LIMIT: f32 = 2.5;
const VANE: f32 = 0.004;
const TORQUE_KICK: f32 = 0.6;
const FORE_DRAG: f32 = 0.002;
const ROTOR_DRAG: f32 = 0.06;
const SIDE_DRAG: f32 = 0.02;
const CLIMB_DRAG: f32 = 0.7;
const CLIMB_DRAG_QUAD: f32 = 0.01;
const CUSHION: f32 = 0.12;
const ROTOR_SPAN: f32 = 12.0;
const SCALE_HEIGHT: f32 = 9000.0;
const ROTOR_RATE: f32 = 32.0;
const STEP: f32 = 0.01;
const BOARD_REACH: f32 = 5.0;
const PARKED: f32 = 2.0;
const HARD_LANDING: f32 = 4.0;
const CHASE: f32 = 5.0;
const LOOK_SPEED: f32 = 0.0025;
const PITCH_LIMIT: f32 = 1.35;
const SEAT: Vec3 = Vec3::new(4.5 * PX, 10.0 * PX, -10.0 * PX);
const SEAT_EYE: Vec3 = Vec3::new(4.5 * PX, 18.0 * PX, -11.0 * PX);
const DOOR: Vec3 = Vec3::new(-2.6, 0.0, -0.8);
const ROTOR_HUB: Vec3 = Vec3::new(0.0, 31.5, 0.0);
const TAIL_HUB: Vec3 = Vec3::new(2.2, 24.5, 61.0);

fn wrap(angle: f32) -> f32 { (angle + PI).rem_euclid(TAU) - PI }

pub struct Pad {
  pub at: Vec2,
  pub top: f32,
  base: f32,
  yaw: f32
}

impl Pad {
  pub fn covers(&self, at: Vec2, margin: f32) -> bool {
    let local = Vec2::from_angle(self.yaw).rotate(at - self.at);
    local.abs().max_element() < PAD_HALF + margin
  }
}

pub static PAD: LazyLock<Pad> = LazyLock::new(|| {
  let footprint = |center: Vec2| {
    (-2..=2)
      .flat_map(move |x| (-2..=2).map(move |z| Vec2::new(x as f32, z as f32)))
      .map(move |step| terrain::height_at(center + step * (PAD_HALF + 1.0) / 2.0))
      .fold((f32::MAX, f32::MIN), |(low, high), height| {
        (low.min(height), high.max(height))
      })
  };
  let clear = |at: Vec2| {
    place::road_distance(at) > PAD_HALF + 10.0
      && river::course_distance(at) > PAD_HALF + 20.0
      && river::water_level(at).is_none()
      && place::around(at).iter().all(|place| {
        at.distance(place.spot()) > place.flat() * place.clearance() + PAD_HALF + 6.0
      })
  };
  let at = (0..14)
    .flat_map(|ring| {
      (0..36).map(move |step| {
        place::START
          + Vec2::from_angle(step as f32 / 36.0 * TAU + ring as f32 * 0.37)
            * (60.0 + ring as f32 * 8.0)
      })
    })
    .filter(|&at| clear(at))
    .map(|at| {
      let (low, high) = footprint(at);
      (at, high - low + at.distance(place::START) * 0.01)
    })
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map_or(place::START + Vec2::X * 60.0, |(at, _)| at);
  let (low, high) = footprint(at);
  let toward = place::START - at;
  Pad { at, top: high + PAD_LIFT, base: low - 0.8, yaw: f32::atan2(-toward.x, -toward.y) }
});

fn floor(at: Vec2) -> f32 {
  [
    Some(terrain::height_at(at)),
    river::deck(at),
    river::water_level(at),
    PAD.covers(at, 0.0).then_some(PAD.top)
  ]
  .into_iter()
  .flatten()
  .fold(f32::MIN, f32::max)
}

#[derive(Clone, Copy, Default, Debug)]
struct Controls {
  crewed: bool,
  ahead: f32,
  aside: f32,
  climb: f32,
  heading: f32
}

#[derive(Clone, Copy, Debug)]
struct Contact {
  above: f32,
  altitude: f32,
  ground: Option<Vec3>,
  gust: Vec3
}

#[derive(Clone, Copy, Default, Debug)]
struct Airframe {
  velocity: Vec3,
  yaw: f32,
  pitch: f32,
  roll: f32,
  spin: Vec3,
  lever: f32,
  rotor: f32
}

impl Airframe {
  fn rotation(&self) -> Quat {
    Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, self.roll)
  }

  fn step(
    self,
    Controls { crewed, ahead, aside, climb, heading }: Controls,
    Contact { above, altitude, ground, gust }: Contact,
    dt: f32
  ) -> Self {
    let up = self.rotation() * Vec3::Y;
    let forward = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
    let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
    let rotor = self.rotor
      + ((crewed as u8 as f32) - self.rotor).clamp(-SPOOL_DOWN * dt, SPOOL_UP * dt);
    let hover = 1.0 / up.y.max(0.6);
    let wanted = match ground {
      Some(_) if climb <= 0.0 => 0.0,
      _ => (hover + climb * RESERVE).max(0.0)
    };
    let lever = (self.lever + (wanted - self.lever) * (1.0 - (-LEVER_RATE * dt).exp()))
      .clamp(0.0, LEVER_RANGE);
    let (cruise, slip) = (self.velocity.dot(forward), self.velocity.dot(right));
    let airspeed = self.velocity.length();
    let cushion = CUSHION * (1.0 - smooth(0.0, ROTOR_SPAN, above));
    let translational =
      0.05 * smooth(5.0, 15.0, airspeed) * (1.0 - smooth(25.0, 45.0, airspeed));
    let lift = GRAVITY
      * lever
      * rotor
      * rotor
      * (-altitude / SCALE_HEIGHT).exp()
      * (1.0 + cushion + translational);
    let rise = self.velocity.y;
    let drag = -forward * FORE_DRAG * cruise.abs() * cruise
      - right * SIDE_DRAG * slip.abs() * slip
      - self.velocity.with_y(0.0) * ROTOR_DRAG
      - Vec3::Y * (CLIMB_DRAG * rise + CLIMB_DRAG_QUAD * rise.abs() * rise);
    let load = ground.map_or(0.0, |_| (1.0 - lift / GRAVITY).clamp(0.0, 1.0));
    let accelerated = self.velocity + (up * lift - Vec3::Y * GRAVITY + drag) * dt;
    let velocity =
      accelerated.with_y(0.0) * (-6.0 * load * dt).exp() + Vec3::Y * accelerated.y;
    let (settle_pitch, settle_roll) = ground.map_or((0.0, 0.0), |normal| {
      (-normal.dot(forward).asin(), -normal.dot(right).asin())
    });
    let coordinated = (self.spin.y * cruise.max(0.0) * COORDINATION).clamp(-0.5, 0.5);
    let aim_pitch = (-ahead * MAX_TILT).lerp(settle_pitch, load);
    let aim_roll = (-aside * MAX_BANK + coordinated).lerp(settle_roll, load);
    let authority = rotor.max(load);
    let airborne = 1.0 - load;
    let cyclic = |aim: f32, angle: f32, rate: f32| {
      (ATTITUDE_SPRING * (aim - angle) - ATTITUDE_DAMP * rate) * authority - 0.3 * rate
    };
    let pedal = (YAW_SPRING * wrap(heading - self.yaw) - YAW_DAMP * self.spin.y)
      .clamp(-YAW_LIMIT, YAW_LIMIT)
      * rotor
      * (crewed as u8 as f32);
    let vane = -VANE * airspeed * slip;
    let reaction = -TORQUE_KICK * LEVER_RATE * (wanted - self.lever);
    let turning =
      (pedal + vane + reaction + gust.y) * airborne - (6.0 * load + 0.3) * self.spin.y;
    let spin = self.spin
      + Vec3::new(
        cyclic(aim_pitch, self.pitch, self.spin.x) + gust.x * airborne,
        turning,
        cyclic(aim_roll, self.roll, self.spin.z) + gust.z * airborne
      ) * dt;
    Self {
      velocity,
      yaw: wrap(self.yaw + spin.y * dt),
      pitch: (self.pitch + spin.x * dt).clamp(-1.2, 1.2),
      roll: (self.roll + spin.z * dt).clamp(-1.2, 1.2),
      spin,
      lever,
      rotor
    }
  }

  fn fly(self, controls: Controls, contact: Contact, dt: f32) -> Self {
    let steps = (dt / STEP).ceil().max(1.0);
    (0..steps as usize)
      .fold(self, |airframe, _| airframe.step(controls, contact, dt / steps))
  }
}

#[derive(Clone, Copy, PartialEq)]
enum Skin {
  Hull,
  Glass,
  Iron,
  Barrel,
  Grille,
  Blade,
  Red,
  Green,
  White,
  Deck,
  Plinth,
  Lamp,
  Sock
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
  Pad,
  Body,
  Rotor,
  Tail
}

fn paint(skin: Skin, pixel: Pixel) -> Texel {
  let Pixel { facet, at: UVec2 { x, y }, size: UVec2 { x: w, y: h }, .. } = pixel;
  let concrete = [0x55524d, 0x8d8982, 0x9d9992, 0x7b766f];
  let worn = |hex: u32| if pixel.speck() < 0.12 { pixel.shade(concrete) } else { hex };
  match skin {
    Skin::Hull if pixel.side() && h >= 8 && (y == h / 3 || y == h / 3 + 1) => {
      Texel::flat(pixel.shade([0x6a6048, 0xe8dcb8, 0xfff4d8, 0xcfc3a0]))
    }
    Skin::Hull => Texel::flat(pixel.shade([0x3a1012, 0x8e2226, 0xc44a46, 0x7a1c20])),
    Skin::Glass if pixel.edge() => Texel::flat(0x1a1a1e),
    Skin::Glass if (x + y) % 11 < 2 => Texel::flat(0x5a7890),
    Skin::Glass => Texel::flat(0x1b2a38),
    Skin::Iron => Texel::flat(pixel.shade([0x1a1c22, 0x4a4e57, 0x80868f, 0x34373e])),
    Skin::Barrel if facet == Facet::Cap => Texel::flat(0x08080a),
    Skin::Barrel => Texel::flat(pixel.shade([0x101014, 0x2a2c32, 0x5a5e66, 0x202228])),
    Skin::Grille if pixel.side() && !pixel.edge() && x % 2 == 1 => Texel::flat(0x0c0c0e),
    Skin::Grille => Texel::flat(pixel.shade([0x1a1c22, 0x4a4e57, 0x80868f, 0x34373e])),
    Skin::Blade => {
      let (along, long) = if w > h { (x, w) } else { (y, h) };
      Texel::flat(if along < 6 || along + 6 >= long {
        0xe0c020
      } else {
        pixel.shade([0x101012, 0x2a2b30, 0x45474e, 0x202226])
      })
    }
    Skin::Red => Texel::lit(0xff2a20),
    Skin::Green => Texel::lit(0x30ff60),
    Skin::White => Texel::lit(0xfff8f0),
    Skin::Deck if facet == Facet::Top => {
      let offset = pixel.at.as_vec2() + 0.5 - pixel.size.as_vec2() / 2.0;
      let (across, reach) = (offset.abs(), offset.length());
      let rim = across.max_element() > w as f32 / 2.0 - 4.0;
      let ring = (40.0..44.5).contains(&reach);
      let letter = (across.x > 8.0 && across.x < 13.0 && across.y < 17.0)
        || (across.x <= 8.0 && across.y < 2.5);
      Texel::flat(match () {
        _ if rim && (x + y) / 3 % 2 == 0 => worn(0xe0b020),
        _ if rim => worn(0x1e1e1e),
        _ if ring => worn(0xe8c040),
        _ if letter => worn(0xeeeeea),
        _ if x % 32 == 0 || y % 32 == 0 => 0x5e5a55,
        _ => pixel.shade(concrete)
      })
    }
    Skin::Deck | Skin::Plinth if pixel.column(5) < 0.2 => Texel::flat(0x5d5a55),
    Skin::Deck | Skin::Plinth => Texel::flat(pixel.shade(concrete)),
    Skin::Lamp => Texel::lit(0x50ff70),
    Skin::Sock if facet == Facet::Rim => {
      Texel::flat(if y * 5 / h.max(1) % 2 == 0 { 0xff6a10 } else { 0xf0f0f0 })
    }
    Skin::Sock => Texel::flat(0x303030)
  }
}

fn aft(shape: Shape<Skin>) -> Shape<Skin> {
  shape.turned(Quat::from_rotation_x(FRAC_PI_2))
}

fn sideways(shape: Shape<Skin>) -> Shape<Skin> {
  shape.turned(Quat::from_rotation_z(-FRAC_PI_2))
}

fn airframe() -> Vec<Shape<Skin>> {
  let skid = |side: f32| {
    let rail = |z: f32| Vec3::new(side * 11.0, 0.6, z);
    [
      Shape::rod(rail(-16.0), rail(18.0), 1.2, Skin::Iron),
      Shape::rod(rail(-16.0), Vec3::new(side * 11.0, 3.2, -20.0), 1.2, Skin::Iron),
      Shape::rod(rail(-9.0), Vec3::new(side * 8.0, 7.5, -9.0), 1.2, Skin::Iron),
      Shape::rod(rail(9.0), Vec3::new(side * 8.0, 7.5, 9.0), 1.2, Skin::Iron)
    ]
  };
  let exhaust = |side: f32| {
    aft(Shape::prism(6, 1.5, 1.2, 4.0, Skin::Barrel)).at(Vec3::new(
      side * 3.0,
      23.0,
      13.5
    ))
  };
  let lamp = |skin, at: Vec3, size: f32| Shape::block(Vec3::splat(size), skin).at(at);
  [
    Shape::frustum(Vec2::new(20.0, 34.0), Vec2::new(17.0, 28.0), 14.0, Skin::Hull)
      .at(Vec3::Y * 14.0),
    Shape::frustum(Vec2::new(18.0, 10.0), Vec2::new(17.0, 9.0), 6.0, Skin::Hull)
      .at(Vec3::new(0.0, 10.0, -21.0)),
    Shape::frustum(Vec2::new(17.0, 9.0), Vec2::new(11.0, 3.0), 8.0, Skin::Glass)
      .at(Vec3::new(0.0, 17.0, -20.0)),
    Shape::block(Vec3::new(18.8, 6.0, 9.0), Skin::Glass).at(Vec3::new(0.0, 17.0, -9.0)),
    Shape::block(Vec3::new(18.6, 5.0, 6.0), Skin::Glass).at(Vec3::new(0.0, 17.0, 3.0)),
    Shape::block(Vec3::new(12.0, 5.0, 18.0), Skin::Hull).at(Vec3::new(0.0, 23.5, 3.0)),
    Shape::block(Vec3::new(12.6, 3.0, 4.0), Skin::Grille).at(Vec3::new(0.0, 23.5, -3.0)),
    Shape::prism(6, 1.3, 1.3, 5.0, Skin::Iron).at(Vec3::new(0.0, 28.5, 0.0)),
    aft(Shape::frustum(Vec2::splat(7.0), Vec2::splat(3.0), 44.0, Skin::Hull))
      .at(Vec3::new(0.0, 19.0, 39.0)),
    Shape::frustum(Vec2::new(1.4, 9.0), Vec2::new(1.0, 5.0), 12.0, Skin::Hull)
      .at(Vec3::new(0.0, 26.0, 59.5)),
    Shape::block(Vec3::new(16.0, 1.0, 4.0), Skin::Hull).at(Vec3::new(0.0, 19.0, 50.0)),
    Shape::rod(Vec3::new(0.0, 17.5, 57.0), Vec3::new(0.0, 14.5, 60.0), 0.8, Skin::Iron),
    Shape::block(Vec3::new(15.0, 3.0, 3.0), Skin::Iron).at(Vec3::new(0.0, 13.0, -16.0)),
    Shape::block(Vec3::new(5.0, 6.0, 5.0), Skin::Iron).at(Vec3::new(4.5, 10.0, -9.0)),
    Shape::block(Vec3::new(5.0, 6.0, 5.0), Skin::Iron).at(Vec3::new(-4.5, 10.0, -9.0)),
    lamp(Skin::Red, Vec3::new(-10.6, 12.0, -7.0), 1.0),
    lamp(Skin::Green, Vec3::new(10.6, 12.0, -7.0), 1.0),
    lamp(Skin::White, Vec3::new(0.0, 20.0, 62.0), 1.0),
    lamp(Skin::Red, Vec3::new(0.0, 26.8, 10.0), 1.5),
    exhaust(-1.0),
    exhaust(1.0)
  ]
  .into_iter()
  .chain(skid(-1.0))
  .chain(skid(1.0))
  .collect()
}

fn heli_bones() -> Vec<Bone<Part, Skin>> {
  vec![
    Bone { part: Part::Body, parent: None, pivot: Vec3::ZERO, shapes: airframe() },
    Bone {
      part: Part::Rotor,
      parent: Some(Part::Body),
      pivot: ROTOR_HUB,
      shapes: vec![
        Shape::prism(8, 2.2, 1.8, 2.0, Skin::Iron).at(ROTOR_HUB),
        Shape::block(Vec3::new(96.0, 0.7, 4.5), Skin::Blade)
          .at(ROTOR_HUB + Vec3::Y * 0.6),
        Shape::block(Vec3::new(4.5, 0.6, 96.0), Skin::Blade)
          .at(ROTOR_HUB + Vec3::Y * 0.75),
      ]
    },
    Bone {
      part: Part::Tail,
      parent: Some(Part::Body),
      pivot: TAIL_HUB,
      shapes: vec![
        sideways(Shape::prism(6, 1.2, 1.2, 1.6, Skin::Iron)).at(TAIL_HUB),
        Shape::block(Vec3::new(0.5, 16.0, 2.0), Skin::Blade).at(TAIL_HUB + Vec3::X * 0.6),
        Shape::block(Vec3::new(0.5, 2.0, 16.0), Skin::Blade)
          .at(TAIL_HUB + Vec3::X * 0.65),
      ]
    },
  ]
}

fn pad_bones(depth: f32) -> Vec<Bone<Part, Skin>> {
  let side = PAD_HALF * 2.0 / PX;
  let edge = side / 2.0 - 2.0;
  let lamps = [-1.0, 0.0, 1.0]
    .into_iter()
    .flat_map(|x| [-1.0, 0.0, 1.0].map(|z| Vec2::new(x, z)))
    .filter(|spot| *spot != Vec2::ZERO)
    .map(|spot| {
      Shape::block(Vec3::new(2.0, 1.5, 2.0), Skin::Lamp)
        .at((spot * edge).extend(0.75).xzy())
    });
  let pole = Vec3::new(edge - 2.0, 0.0, -(edge - 2.0));
  vec![Bone {
    part: Part::Pad,
    parent: None,
    pivot: Vec3::ZERO,
    shapes: [
      Shape::block(Vec3::new(side, 3.0, side), Skin::Deck).at(Vec3::Y * -1.5),
      Shape::block(Vec3::new(side - 4.0, depth, side - 4.0), Skin::Plinth)
        .at(Vec3::Y * (-3.0 - depth / 2.0)),
      Shape::rod(pole, pole + Vec3::Y * 40.0, 1.0, Skin::Iron),
      sideways(Shape::prism(8, 3.0, 1.8, 18.0, Skin::Sock))
        .at(pole + Vec3::new(9.5, 37.0, 0.0))
    ]
    .into_iter()
    .chain(lamps)
    .collect()
  }]
}

fn hull() -> Collider {
  Collider::compound(vec![
    (Vec3::new(0.0, 1.75, -0.56), Quat::IDENTITY, Collider::cuboid(2.5, 1.75, 5.4)),
    (Vec3::new(0.0, 2.94, 0.38), Quat::IDENTITY, Collider::cuboid(1.5, 0.65, 2.25)),
    (Vec3::new(0.0, 2.4, 4.9), Quat::IDENTITY, Collider::cuboid(0.7, 0.9, 5.5)),
    (Vec3::new(0.0, 0.125, 0.1), Quat::IDENTITY, Collider::cuboid(2.9, 0.25, 4.4)),
  ])
}

#[derive(Component)]
struct Helicopter {
  airframe: Airframe,
  crewed: bool,
  landed: bool,
  blades: Vec2,
  rotor: (Entity, Vec3),
  tail: (Entity, Vec3)
}

fn build(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  player: Single<Entity, Added<Player>>
) {
  let &Pad { at, top, base, yaw } = &*PAD;
  let turn = Quat::from_rotation_y(yaw);
  let mut kit = |bones: &[Bone<Part, Skin>]| {
    blocky::kits(bones, PX, [paint], 3.0, &mut meshes, &mut images, &mut materials)
      .remove(0)
  };
  let (pad_kit, heli_kit) =
    (kit(&pad_bones((top - base) / PX - 3.0)), kit(&heli_bones()));
  let deck = at.extend(top).xzy();
  pad_kit.spawn(&mut commands, Transform::from_translation(deck).with_rotation(turn));
  commands.spawn((
    RigidBody::Static,
    Collider::cuboid(PAD_HALF * 2.0, top - base, PAD_HALF * 2.0),
    Transform::from_translation(deck - Vec3::Y * (top - base) / 2.0).with_rotation(turn)
  ));
  let (root, parts) =
    heli_kit.spawn(&mut commands, Transform::from_translation(deck).with_rotation(turn));
  let joint = |wanted: Part| {
    parts
      .iter()
      .find(|(part, ..)| *part == wanted)
      .map(|&(_, entity, rest)| (entity, rest))
      .expect("helicopter part")
  };
  let crewed = opts().pilot;
  commands.entity(root).insert((
    Name::new("Helicopter"),
    RigidBody::Kinematic,
    hull(),
    Helicopter {
      airframe: Airframe { yaw, rotor: crewed as u8 as f32, ..default() },
      crewed,
      landed: true,
      blades: Vec2::ZERO,
      rotor: joint(Part::Rotor),
      tail: joint(Part::Tail)
    }
  ));
  if crewed {
    commands.entity(*player).insert((Seated, Visibility::Hidden, ColliderDisabled));
  }
}

fn board(
  keys: Res<ButtonInput<KeyCode>>,
  view: Res<View>,
  mut prompt: ResMut<Prompt>,
  mut notices: MessageWriter<Notice>,
  mut commands: Commands,
  player: Single<(Entity, &mut Transform, Has<Seated>, Has<Dead>), With<Player>>,
  mut helis: Query<(&Transform, &mut Helicopter), Without<Player>>
) {
  let (entity, mut body, seated, dead) = player.into_inner();
  for (craft, mut heli) in helis.iter_mut() {
    let gap = (craft.translation - body.translation).with_y(0.0);
    let near = !seated
      && !dead
      && gap.length() < BOARD_REACH
      && view.flat_forward().dot(gap.normalize_or_zero()) > 0.3;
    let parked = heli.crewed && heli.landed && heli.airframe.velocity.length() < PARKED;
    if prompt.0.is_none() && (near || parked) {
      prompt.0 = Some(Prompting {
        verb: if seated { "Leave" } else { "Fly" }.into(),
        noun: "Helicopter".into()
      });
      if keys.just_pressed(KeyCode::KeyE) {
        heli.crewed = !seated;
        if seated {
          let door = craft.transform_point(DOOR).xz();
          body.translation = door.extend(floor(door) + capsule_offset() + 0.3).xzy();
          commands
            .entity(entity)
            .remove::<(Seated, ColliderDisabled)>()
            .insert(Visibility::Inherited);
        } else {
          commands.entity(entity).insert((Seated, Visibility::Hidden, ColliderDisabled));
          notices.write(Notice(
            "W/S pitch, A/D bank, mouse steers, Space climbs, Ctrl descends".into()
          ));
        }
      }
    }
  }
}

fn fly(
  time: Res<Time>,
  keys: Res<ButtonInput<KeyCode>>,
  motion: Res<AccumulatedMouseMotion>,
  mut view: ResMut<View>,
  mut shake: ResMut<Shake>,
  mut sounds: MessageWriter<Sound>,
  move_and_slide: MoveAndSlide,
  player: Single<Entity, With<Player>>,
  mut helis: Query<(Entity, &Collider, &mut Transform, &mut Helicopter), Without<Player>>,
  mut parts: Query<&mut Transform, (Without<Helicopter>, Without<Player>)>
) {
  let (dt, now) = (time.delta_secs().min(0.05), time.elapsed_secs());
  for (entity, collider, mut transform, mut heli) in helis.iter_mut() {
    if heli.crewed {
      if view.captured {
        view.yaw -= motion.delta.x * LOOK_SPEED;
        view.pitch =
          (view.pitch - motion.delta.y * LOOK_SPEED).clamp(-PITCH_LIMIT, PITCH_LIMIT);
      }
      view.first_person ^= keys.just_pressed(KeyCode::KeyF);
    }
    let held = |key: KeyCode| keys.pressed(key) as u8 as f32;
    let controls = Controls {
      crewed: heli.crewed,
      ahead: held(KeyCode::KeyW) - held(KeyCode::KeyS),
      aside: held(KeyCode::KeyD) - held(KeyCode::KeyA),
      climb: held(KeyCode::Space).max(held(KeyCode::ShiftLeft))
        - held(KeyCode::ControlLeft),
      heading: view.yaw
    };
    let controls = if heli.crewed { controls } else { Controls::default() };
    let filter = SpatialQueryFilter::from_mask(Layer::World)
      .with_excluded_entities([entity, *player]);
    let &Transform { translation, rotation, .. } = &*transform;
    let below = floor(translation.xz());
    let settled = move_and_slide
      .spatial_query
      .cast_shape(
        collider,
        translation.adjust_precision(),
        rotation.adjust_precision(),
        Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(0.15),
        &filter
      )
      .filter(|hit| hit.normal1.y > 0.6)
      .map(|hit| hit.normal1)
      .or((translation.y - below < 0.1).then_some(Vec3::Y));
    let gust = Vec3::new(
      (now * 1.7).sin() + (now * 2.9 + 1.0).sin(),
      (now * 0.9 + 2.0).sin(),
      (now * 1.3 + 4.0).sin() + (now * 3.7).sin()
    ) * 0.12
      * heli.airframe.rotor;
    let contact = Contact {
      above: translation.y - below,
      altitude: translation.y,
      ground: settled,
      gust
    };
    let airframe = heli.airframe.fly(controls, contact, dt);
    let turned = airframe.rotation();
    let mut impact = 0.0f32;
    let MoveAndSlideOutput { position, projected_velocity } = move_and_slide
      .move_and_slide(
        collider,
        translation.adjust_precision(),
        turned.adjust_precision(),
        airframe.velocity,
        std::time::Duration::from_secs_f32(dt),
        &MoveAndSlideConfig::default(),
        &filter,
        |hit| {
          impact = impact.max(-hit.velocity.dot(hit.normal.as_vec3()));
          MoveAndSlideHitResponse::Accept
        }
      );
    let ground = floor(position.xz());
    let landed = settled.is_some() || position.y <= ground + 0.02;
    let velocity = if position.y < ground {
      projected_velocity.with_y(projected_velocity.y.max(0.0))
    } else {
      projected_velocity
    };
    if impact > HARD_LANDING {
      shake.0 = shake.0.max((impact * 0.08).min(1.0));
      sounds.write(Sound::here(Cue::Block, position));
    }
    transform.translation = position
      .with_y(position.y.max(ground))
      .clamp(Vec3::new(-BOUND, -500.0, -BOUND), Vec3::new(BOUND, 4000.0, BOUND));
    transform.rotation = turned;
    heli.airframe = Airframe { velocity, ..airframe };
    heli.landed = landed;
    heli.blades += Vec2::new(1.0, 5.0) * ROTOR_RATE * airframe.rotor * dt;
    let Vec2 { x: main, y: tail } = heli.blades;
    let spins = [
      (heli.rotor, Quat::from_rotation_y(main)),
      (heli.tail, Quat::from_rotation_x(tail))
    ];
    for ((part, rest), turn) in spins {
      if let Ok(mut pose) = parts.get_mut(part) {
        *pose = Transform::from_translation(rest).with_rotation(turn);
      }
    }
  }
}

fn ride(
  helis: Query<(&Transform, &Helicopter), Without<Player>>,
  player: Single<
    (&mut Transform, &mut Walker, &mut LinearVelocity),
    (With<Player>, With<Seated>)
  >
) {
  let (mut body, mut walker, mut velocity) = player.into_inner();
  if let Some((craft, _)) = helis.iter().find(|(_, heli)| heli.crewed) {
    body.translation = craft.transform_point(SEAT) + Vec3::Y * capsule_offset();
    body.rotation = Quat::from_rotation_y(craft.rotation.to_euler(EulerRot::YXZ).0);
    *walker = Walker { grounded: true, ..default() };
    velocity.0 = Vec3::ZERO;
  }
}

fn chase(
  time: Res<Time>,
  view: Res<View>,
  shake: Res<Shake>,
  spatial: SpatialQuery,
  player: Single<Entity, With<Player>>,
  helis: Query<(Entity, &Transform, &Helicopter), Without<MainCamera>>,
  mut camera: Single<&mut Transform, With<MainCamera>>
) {
  if let Some((entity, craft, heli)) = helis.iter().find(|(_, _, heli)| heli.crewed)
    && opts().eye.is_none()
  {
    let look = Quat::from_euler(EulerRot::YXZ, view.yaw, view.pitch, 0.0);
    let now = time.elapsed_secs();
    let tremor = Vec3::new((now * 71.0).sin(), (now * 83.0).cos(), (now * 59.0).sin())
      * shake.0
      * shake.0
      * 0.25;
    **camera = if view.first_person {
      Transform::from_translation(craft.transform_point(SEAT_EYE) + tremor).with_rotation(
        craft.rotation
          * Quat::from_euler(
            EulerRot::YXZ,
            wrap(view.yaw - heli.airframe.yaw),
            view.pitch,
            0.0
          )
      )
    } else {
      let focus = craft.translation + Vec3::Y * 2.2;
      let back = look * Vec3::Z;
      let distance = CHASE * view.distance;
      let reach = spatial
        .cast_ray(
          focus,
          Dir3::new(back).unwrap_or(Dir3::Z),
          distance,
          true,
          &SpatialQueryFilter::from_mask(Layer::World)
            .with_excluded_entities([entity, *player])
        )
        .map_or(distance, |hit| (hit.distance - 0.3).max(2.0));
      let eye = focus + back * reach;
      Transform::from_translation(eye.with_y(eye.y.max(floor(eye.xz()) + 0.4)) + tremor)
        .with_rotation(look)
    };
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(PostStartup, build)
    .add_systems(Update, (board, fly, ride).chain().after(Walking))
    .add_systems(
      PostUpdate,
      chase.after(crate::player::follow).before(TransformSystems::Propagate)
    );
}

#[cfg(test)]
mod tests {
  use super::*;

  fn simulate(
    start: Airframe,
    altitude: f32,
    seconds: f32,
    controls: Controls
  ) -> (Airframe, f32) {
    let dt = 1.0 / 60.0;
    (0..(seconds / dt) as usize).fold((start, altitude), |(airframe, height), _| {
      let ground = (height < 0.05).then_some(Vec3::Y);
      let contact = Contact { above: height, altitude: height, ground, gust: Vec3::ZERO };
      let next = airframe.fly(controls, contact, dt);
      let height = height + next.velocity.y * dt;
      let velocity = if height < 0.0 {
        next.velocity.with_y(next.velocity.y.max(0.0))
      } else {
        next.velocity
      };
      (Airframe { velocity, ..next }, height.max(0.0))
    })
  }

  #[test]
  #[ignore]
  fn pad() {
    let Pad { at, top, base, yaw } = &*PAD;
    println!(
      "pad {at} top {top} base {base} yaw {yaw} from start {}",
      at.distance(place::START)
    )
  }

  #[test]
  fn flight() {
    let crewed = Controls { crewed: true, ..default() };
    let (idle, height) = simulate(Airframe::default(), 0.0, 5.0, crewed);
    assert!(
      height < 0.01 && idle.rotor > 0.99,
      "spools up on the ground: {height} {idle:?}"
    );
    let (climbed, height) =
      simulate(idle, height, 6.0, Controls { climb: 1.0, ..crewed });
    assert!(height > 15.0, "climbs: {height}");
    let (hovering, held) = simulate(climbed, height, 6.0, crewed);
    assert!(
      hovering.velocity.y.abs() < 0.5 && held > height,
      "holds height: {hovering:?}"
    );
    let (cruising, cruise_height) =
      simulate(hovering, held, 25.0, Controls { ahead: 1.0, ..crewed });
    let speed = cruising.velocity.with_y(0.0).length();
    assert!((30.0..45.0).contains(&speed), "cruises: {speed}");
    assert!(
      (cruise_height - held).abs() < 25.0,
      "keeps altitude: {held} {cruise_height}"
    );
    let (turning, turn_height) = simulate(cruising, cruise_height, 3.0, Controls {
      ahead: 1.0,
      heading: cruising.yaw + 1.5,
      ..crewed
    });
    assert!(wrap(turning.yaw - cruising.yaw) > 0.8, "turns: {}", turning.yaw);
    let level = Controls { heading: turning.yaw, ..crewed };
    let (flared, flare_height) =
      simulate(turning, turn_height, 5.0, Controls { ahead: -1.0, ..level });
    let (braked, braked_height) = simulate(flared, flare_height, 15.0, level);
    assert!(braked.velocity.length() < 3.0, "slows: {:?}", braked.velocity);
    let (down, height) = simulate(braked, braked_height, 40.0, Controls {
      climb: -1.0,
      heading: braked.yaw,
      ..crewed
    });
    assert!(height < 0.01 && down.velocity.length() < 0.5, "lands: {height} {down:?}");
    let (parked, _) = simulate(down, 0.0, 15.0, Controls::default());
    assert!(parked.rotor < 0.01 && parked.lever < 0.01, "spools down: {parked:?}");
  }
}
