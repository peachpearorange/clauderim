use {crate::{blocky::{self, Bone, Facet, Pixel, Shape, Texel, srgb},
             combat::{Dead, Shake, Struck, Vitals},
             fx::{Effects, Fleeting},
             humanoid::Motion,
             noise::hash,
             opts::opts,
             place,
             player::{MainCamera, Player, Seated, View, capsule_offset},
             ragdoll::Knocked,
             river,
             signal::{Cue, Notice, Prompt, Prompting, Sound},
             terrain::{self, BOUND, smooth},
             walker::{Layer, Walker, Walking}},
     avian3d::{math::AdjustPrecision, prelude::*},
     bevy::{input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll,
                           MouseScrollUnit},
            light::NotShadowCaster,
            prelude::*},
     std::{f32::consts::{FRAC_PI_2, PI, TAU},
           sync::LazyLock}};

const PX: f32 = 1.0 / 8.0;
const PAD_HALF: f32 = 8.0;
const PAD_LIFT: f32 = 0.35;
const GRAVITY: f32 = 9.81;
const SPOOL_UP: f32 = 0.25;
const SPOOL_DOWN: f32 = 0.12;
const LEVER_RATE: f32 = 4.0;
const LEVER_RANGE: f32 = 1.8;
const RESERVE: f32 = 0.6;
const DIVE: f32 = 0.3;
const CYCLIC: Vec2 = Vec2::new(0.14, 0.18);
const MOUSE_CYCLIC: f32 = 0.1;
const FLAP_TIME: f32 = 0.25;
const FLAPBACK: f32 = 0.0035;
const HUB_POWER: Vec3 = Vec3::new(2.4, 0.0, 3.2);
const TURN_LIMIT: f32 = 1.2;
const SPIN_DRAG: Vec3 = Vec3::new(0.1, 1.0, 0.1);
const VANE: f32 = 0.004;
const STABILISER: f32 = 0.002;
const TORQUE_KICK: f32 = 0.6;
const FORE_DRAG: f32 = 0.002;
const ROTOR_DRAG: f32 = 0.06;
const SIDE_DRAG: f32 = 0.02;
const CLIMB_DRAG: f32 = 0.4;
const CLIMB_DRAG_QUAD: f32 = 0.004;
const CUSHION: f32 = 0.12;
const ROTOR_SPAN: f32 = 12.0;
const SCALE_HEIGHT: f32 = 9000.0;
const ROTOR_RATE: f32 = 32.0;
const STEP: f32 = 0.01;
const BOARD_REACH: f32 = 5.0;
const PARKED: f32 = 2.0;
const HARD_LANDING: f32 = 4.0;
const CHASE_NEAREST: f32 = 8.0;
const CHASE_FARTHEST: f32 = 400.0;
const ZOOM_STEP: f32 = 1.25;
const TRAIL: f32 = 5.0;
const MOUSE_TURN: f32 = 0.002;
const SEAT: Vec3 = Vec3::new(4.5 * PX, 0.745, -10.0 * PX);
const SEAT_EYE: Vec3 = Vec3::new(4.5 * PX, 2.42, -10.6 * PX);
const CRASH: f32 = 9.0;
const TIPPED: f32 = 0.55;
const BLAST: f32 = 70.0;
const WRECKED_FOR: f32 = 30.0;
const GUN_RATE: f32 = 12.0;
const ROUND_DAMAGE: f32 = 14.0;
const ROUND_SPEED: f32 = 500.0;
const TRACER_LIFE: f32 = 0.08;
const GUN_RANGE: f32 = 900.0;
const GUN_SPREAD: f32 = 0.006;
const GUN_HUB: Vec3 = Vec3::new(12.5, 5.0, -14.0);
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
  turn: Vec2
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
  attitude: Quat,
  disc: Quat,
  spin: Vec3,
  lever: f32,
  rotor: f32
}

impl Airframe {
  fn facing(yaw: f32) -> Self {
    let attitude = Quat::from_rotation_y(yaw);
    Self { attitude, disc: attitude, ..default() }
  }

  fn heading(&self) -> f32 {
    let nose = self.attitude * Vec3::NEG_Z;
    let top = self.attitude * Vec3::Y;
    let toward = if nose.xz().length() > 0.1 { nose } else { -top * nose.y.signum() };
    f32::atan2(-toward.x, -toward.z)
  }

  fn tilt(&self) -> Quat { self.attitude.inverse() * self.disc }

  fn step(
    self,
    Controls { crewed, ahead, aside, climb, turn }: Controls,
    Contact { above, altitude, ground, gust }: Contact,
    dt: f32
  ) -> Self {
    let heading = self.heading();
    let rotor = self.rotor
      + ((crewed as u8 as f32) - self.rotor).clamp(-SPOOL_DOWN * dt, SPOOL_UP * dt);
    let manned = crewed as u8 as f32;
    let cyclic = Vec2::new(-ahead * CYCLIC.x + turn.y * MOUSE_CYCLIC, -aside * CYCLIC.y)
      .clamp(-CYCLIC, CYCLIC)
      * manned;
    let along = self.attitude.inverse() * self.velocity;
    let flapped =
      cyclic + (Vec2::new(-along.z, along.x) * FLAPBACK).clamp_length_max(0.2);
    let swashed =
      self.attitude * Quat::from_euler(EulerRot::YXZ, 0.0, flapped.x, flapped.y);
    let disc = self.disc.slerp(swashed, 1.0 - (-dt / FLAP_TIME).exp()).normalize();
    let thrust = disc * Vec3::Y;
    let hover = smooth(-0.1, 0.5, thrust.y) / thrust.y.max(0.6);
    let wanted = match ground {
      Some(_) if climb <= 0.0 => 0.0,
      _ if climb < 0.0 => hover * (1.0 + climb) + DIVE * climb,
      _ => hover + climb * RESERVE
    };
    let lever = (self.lever + (wanted - self.lever) * (1.0 - (-LEVER_RATE * dt).exp()))
      .clamp(-DIVE, LEVER_RANGE);
    let airspeed = self.velocity.length();
    let cushion = CUSHION * (1.0 - smooth(0.0, ROTOR_SPAN, above)) * thrust.y.max(0.0);
    let translational =
      0.05 * smooth(5.0, 15.0, airspeed) * (1.0 - smooth(25.0, 45.0, airspeed));
    let lift = GRAVITY
      * lever
      * rotor
      * rotor
      * (-altitude / SCALE_HEIGHT).exp()
      * (1.0 + cushion + translational);
    let inflow = self.velocity.dot(thrust);
    let drag = self.attitude
      * Vec3::new(
        -SIDE_DRAG * along.x.abs() * along.x,
        0.0,
        -FORE_DRAG * along.z.abs() * along.z
      )
      - (self.velocity - thrust * inflow) * ROTOR_DRAG
      - thrust * (CLIMB_DRAG * inflow + CLIMB_DRAG_QUAD * inflow.abs() * inflow);
    let load = ground.map_or(0.0, |_| (1.0 - lift / GRAVITY).clamp(0.0, 1.0));
    let accelerated = self.velocity + (thrust * lift - Vec3::Y * GRAVITY + drag) * dt;
    let velocity =
      accelerated.with_y(0.0) * (-6.0 * load * dt).exp() + Vec3::Y * accelerated.y;
    let airborne = 1.0 - load;
    let hub = self.tilt().to_scaled_axis() * HUB_POWER * rotor * rotor;
    let vanes =
      Vec3::new(-STABILISER * along.z * along.y, -VANE * airspeed * along.x, 0.0);
    let pedal = Vec3::Y * turn.x * manned * rotor;
    let reaction = Vec3::Y * -TORQUE_KICK * LEVER_RATE * (wanted - self.lever);
    let spin = self.spin
      + ((hub + vanes + pedal + reaction + gust) * airborne
        - self.spin * (SPIN_DRAG + Vec3::splat(6.0 * load)))
        * dt;
    let turned = (self.attitude * Quat::from_scaled_axis(spin * dt)).normalize();
    let attitude = ground.map_or(turned, |normal| {
      let settled =
        Quat::from_rotation_arc(Vec3::Y, normal) * Quat::from_rotation_y(heading);
      turned.slerp(settled, 1.0 - (-8.0 * load * dt).exp())
    });
    Self { velocity, attitude, disc, spin, lever, rotor }
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
  Tail,
  Glass,
  Gun
}

fn paint(skin: Skin, pixel: Pixel) -> Texel {
  let Pixel { facet, at: UVec2 { x, y }, size: UVec2 { x: w, y: h }, .. } = pixel;
  let concrete = [0x55524d, 0x8d8982, 0x9d9992, 0x7b766f];
  let worn = |hex: u32| if pixel.speck() < 0.12 { pixel.shade(concrete) } else { hex };
  match skin {
    Skin::Hull if pixel.side() && h == 4 && (1..3).contains(&y) => {
      Texel::flat(pixel.shade([0x6a6048, 0xe8dcb8, 0xfff4d8, 0xcfc3a0]))
    }
    Skin::Hull => Texel::flat(pixel.shade([0x3a1012, 0x8e2226, 0xc44a46, 0x7a1c20])),
    Skin::Glass => {
      let (hex, alpha) = match () {
        _ if pixel.edge() => (0x1a1a1e, 0.95),
        _ if (x + y) % 11 < 2 => (0x9ab8d0, 0.4),
        _ => (0x2a4058, 0.22)
      };
      Texel { tone: srgb(hex).with_alpha(alpha), glow: Srgba::NONE }
    }
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
  let wall = |side: f32| {
    let panel = |size: Vec3, at: Vec3| {
      Shape::block(size, Skin::Hull).at(Vec3::new(side * at.x, at.y, at.z))
    };
    [
      panel(Vec3::new(1.0, 4.0, 34.0), Vec3::new(9.5, 12.0, 0.0)),
      panel(Vec3::new(1.0, 5.0, 8.0), Vec3::new(9.5, 16.5, 13.0)),
      panel(Vec3::new(1.0, 5.0, 1.5), Vec3::new(9.5, 16.5, -3.0)),
      panel(Vec3::new(1.0, 5.0, 1.5), Vec3::new(9.5, 16.5, -16.2))
    ]
  };
  [
    Shape::block(Vec3::new(20.0, 3.0, 34.0), Skin::Hull).at(Vec3::Y * 8.5),
    Shape::block(Vec3::new(20.0, 2.0, 36.0), Skin::Hull).at(Vec3::new(0.0, 20.0, -1.0)),
    Shape::block(Vec3::new(18.0, 9.0, 1.0), Skin::Hull).at(Vec3::new(0.0, 14.5, 16.5)),
    Shape::frustum(Vec2::new(18.0, 10.0), Vec2::new(17.0, 9.0), 6.0, Skin::Hull)
      .at(Vec3::new(0.0, 10.0, -21.0)),
    Shape::rod(Vec3::new(9.5, 7.0, -11.0), GUN_HUB + Vec3::Z * 3.0, 1.2, Skin::Iron),
    Shape::block(Vec3::new(3.2, 3.2, 6.0), Skin::Iron).at(GUN_HUB + Vec3::Z * 3.0),
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
  .chain(wall(-1.0))
  .chain(wall(1.0))
  .chain(skid(-1.0))
  .chain(skid(1.0))
  .collect()
}

fn glazing() -> Vec<Bone<Part, Skin>> {
  let pane = |side: f32, length: f32, z: f32| {
    Shape::block(Vec3::new(0.5, 5.0, length), Skin::Glass).at(Vec3::new(
      side * 9.6,
      16.5,
      z
    ))
  };
  vec![Bone {
    part: Part::Glass,
    parent: None,
    pivot: Vec3::ZERO,
    shapes: [-1.0, 1.0]
      .into_iter()
      .flat_map(|side| [pane(side, 11.7, -9.6), pane(side, 11.25, 3.4)])
      .chain([Shape::frustum(
        Vec2::new(17.0, 9.0),
        Vec2::new(11.0, 3.0),
        8.0,
        Skin::Glass
      )
      .at(Vec3::new(0.0, 17.0, -20.0))])
      .collect()
  }]
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
    Bone {
      part: Part::Gun,
      parent: Some(Part::Body),
      pivot: GUN_HUB,
      shapes: (0..3)
        .map(|barrel| {
          let angle = barrel as f32 * TAU / 3.0;
          aft(Shape::prism(6, 0.6, 0.6, 10.0, Skin::Barrel))
            .at(GUN_HUB + Vec3::new(angle.cos(), angle.sin(), -5.0))
        })
        .chain([
          aft(Shape::prism(8, 1.6, 1.6, 1.0, Skin::Iron)).at(GUN_HUB - Vec3::Z * 8.0)
        ])
        .collect()
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

struct Wreck {
  left: f32,
  fires: Entity
}

#[derive(Component)]
struct Helicopter {
  airframe: Airframe,
  crewed: bool,
  landed: bool,
  crashed: bool,
  wreck: Option<Wreck>,
  blades: Vec3,
  reload: f32,
  rounds: u32,
  rotor: (Entity, Vec3),
  tail: (Entity, Vec3),
  gun: (Entity, Vec3),
  paint: Handle<StandardMaterial>
}

#[derive(Resource)]
struct Rounds {
  mesh: Handle<Mesh>,
  material: Handle<StandardMaterial>
}

#[derive(Component)]
struct Tracer {
  velocity: Vec3,
  left: f32
}

#[derive(Component)]
struct Crosshair;

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
  let (pad_kit, heli_kit, glass_kit) =
    (kit(&pad_bones((top - base) / PX - 3.0)), kit(&heli_bones()), kit(&glazing()));
  if let Some(mut glass) = materials.get_mut(&glass_kit.material) {
    glass.alpha_mode = AlphaMode::Blend;
    glass.perceptual_roughness = 0.15;
    glass.reflectance = 0.6;
  }
  commands.insert_resource(Rounds {
    mesh: meshes.add(Cuboid::new(0.08, 0.08, 4.0)),
    material: materials.add(StandardMaterial {
      base_color: Color::srgb(1.0, 0.75, 0.35),
      emissive: LinearRgba::rgb(40.0, 18.0, 4.0),
      ..default()
    })
  });
  commands.spawn((
    Crosshair,
    Visibility::Hidden,
    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
    Node {
      position_type: PositionType::Absolute,
      left: percent(50.0),
      top: percent(50.0),
      width: px(4.0),
      height: px(4.0),
      margin: UiRect::all(px(-2.0)),
      ..default()
    }
  ));
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
  let (glazed, panes) = glass_kit.spawn(&mut commands, Transform::IDENTITY);
  commands.entity(glazed).insert(ChildOf(root));
  for &(_, pane, _) in &panes {
    commands.entity(pane).insert(NotShadowCaster);
  }
  let crewed = opts().pilot;
  commands.entity(root).insert((
    Name::new("Helicopter"),
    RigidBody::Kinematic,
    hull(),
    Helicopter {
      airframe: Airframe { rotor: crewed as u8 as f32, ..Airframe::facing(yaw) },
      crewed,
      landed: true,
      crashed: false,
      wreck: None,
      blades: Vec3::ZERO,
      reload: 0.0,
      rounds: 0,
      rotor: joint(Part::Rotor),
      tail: joint(Part::Tail),
      gun: joint(Part::Gun),
      paint: heli_kit.material.clone()
    }
  ));
  if crewed {
    commands.entity(*player).insert((Seated, ColliderDisabled));
  }
}

fn board(
  keys: Res<ButtonInput<KeyCode>>,
  view: Res<View>,
  mut prompt: ResMut<Prompt>,
  mut notices: MessageWriter<Notice>,
  mut commands: Commands,
  player: Single<
    (Entity, &mut Transform, &mut Motion, Has<Seated>, Has<Dead>),
    With<Player>
  >,
  mut helis: Query<(&Transform, &mut Helicopter), Without<Player>>
) {
  let (entity, mut body, mut motion, seated, dead) = player.into_inner();
  for (craft, mut heli) in helis.iter_mut() {
    let gap = (craft.translation - body.translation).with_y(0.0);
    let near = !seated
      && !dead
      && heli.wreck.is_none()
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
          body.rotation = Quat::from_rotation_y(heli.airframe.heading());
          motion.seated = 0.0;
          commands.entity(entity).remove::<(Seated, ColliderDisabled)>();
        } else {
          commands.entity(entity).insert((Seated, ColliderDisabled));
          notices.write_batch([
            Notice(
              "Mouse turns and pitches, W/S pitch, A/D roll, Space climbs, Shift dives"
                .into()
            ),
            Notice("Left mouse fires the gun".into())
          ]);
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
  let (dt, now) = (time.delta_secs().min(0.25), time.elapsed_secs());
  for (entity, collider, mut transform, mut heli) in helis.iter_mut() {
    if heli.crewed {
      view.first_person ^= keys.just_pressed(KeyCode::KeyF);
    }
    let held = |key: KeyCode| keys.pressed(key) as u8 as f32;
    let controls = Controls {
      crewed: heli.crewed,
      ahead: held(KeyCode::KeyW) - held(KeyCode::KeyS),
      aside: held(KeyCode::KeyD) - held(KeyCode::KeyA),
      climb: held(KeyCode::Space) - held(KeyCode::ShiftLeft),
      turn: (-motion.delta * MOUSE_TURN / dt.max(1e-3) * (view.captured as u8 as f32))
        .clamp_length_max(TURN_LIMIT)
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
    let turned = airframe.attitude;
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
    heli.crashed = heli.crewed
      && (impact > CRASH || (landed && (airframe.attitude * Vec3::Y).y < TIPPED));
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
    heli.blades += Vec3::new(1.0, 5.0, 0.0) * ROTOR_RATE * airframe.rotor * dt;
    let Vec3 { x: main, y: tail, z: gun } = heli.blades;
    let spins = [
      (heli.rotor, heli.airframe.tilt() * Quat::from_rotation_y(main)),
      (heli.tail, Quat::from_rotation_x(tail)),
      (heli.gun, Quat::from_rotation_z(gun))
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
    (&mut Transform, &mut Walker, &mut LinearVelocity, &mut Motion),
    (With<Player>, With<Seated>)
  >
) {
  let (mut body, mut walker, mut velocity, mut motion) = player.into_inner();
  if let Some((craft, _)) = helis.iter().find(|(_, heli)| heli.crewed) {
    body.translation = craft.transform_point(SEAT + Vec3::Y * capsule_offset());
    body.rotation = craft.rotation;
    *walker = Walker { grounded: true, ..default() };
    velocity.0 = Vec3::ZERO;
    motion.seated = 1.0;
  }
}

fn shoot(
  time: Res<Time>,
  mouse: Res<ButtonInput<MouseButton>>,
  view: Res<View>,
  rounds: Res<Rounds>,
  effects: Res<Effects>,
  spatial: SpatialQuery,
  mut commands: Commands,
  mut struck: MessageWriter<Struck>,
  mut sounds: MessageWriter<Sound>,
  player: Single<Entity, With<Player>>,
  mut crosshair: Single<&mut Visibility, With<Crosshair>>,
  mut helis: Query<(Entity, &Transform, &mut Helicopter)>,
  targets: Query<(), With<Vitals>>
) {
  let dt = time.delta_secs();
  let crewed = helis.iter().any(|(_, _, heli)| heli.crewed);
  crosshair.set_if_neq(if crewed { Visibility::Inherited } else { Visibility::Hidden });
  for (entity, craft, mut heli) in helis.iter_mut().filter(|(_, _, heli)| heli.crewed) {
    let firing = view.captured && mouse.pressed(MouseButton::Left);
    heli.reload = (heli.reload - dt).max(0.0);
    heli.blades.z += firing as u8 as f32 * 40.0 * dt;
    if firing && heli.reload <= 0.0 {
      heli.reload = 1.0 / GUN_RATE;
      heli.rounds += 1;
      let round = heli.rounds as i32;
      let scatter = Vec2::new(hash(round, 0, 61), hash(round, 1, 61)) * 2.0 - 1.0;
      let muzzle = craft.transform_point((GUN_HUB - Vec3::Z * 10.0) * PX);
      let aim = (craft.forward().as_vec3()
        + (craft.right().as_vec3() * scatter.x + craft.up().as_vec3() * scatter.y)
          * GUN_SPREAD)
        .normalize();
      let hit = spatial.cast_ray(
        muzzle,
        Dir3::new(aim).unwrap_or(Dir3::NEG_Z),
        GUN_RANGE,
        true,
        &SpatialQueryFilter::from_mask([Layer::World, Layer::Walker])
          .with_excluded_entities([entity, *player])
      );
      let target = muzzle + aim * hit.map_or(GUN_RANGE, |hit| hit.distance);
      let path = target - muzzle;
      let flight = (path.length() / ROUND_SPEED).max(TRACER_LIFE);
      commands.spawn((
        Tracer { velocity: path / flight, left: flight },
        Mesh3d(rounds.mesh.clone()),
        MeshMaterial3d(rounds.material.clone()),
        NotShadowCaster,
        Transform::from_translation(muzzle).looking_to(path, Vec3::Y)
      ));
      sounds.write(Sound::here(Cue::Gunshot, muzzle));
      if let Some(hit) = hit {
        if targets.contains(hit.entity) {
          struck.write(Struck {
            target: hit.entity,
            attacker: *player,
            damage: ROUND_DAMAGE,
            power: false,
            blocked: false,
            at: target
          });
        } else {
          commands.spawn((
            effects.emit(&effects.sparks),
            Fleeting(1.0),
            Transform::from_translation(target)
          ));
        }
      }
    }
  }
}

fn streak(
  time: Res<Time>,
  mut commands: Commands,
  mut tracers: Query<(Entity, &mut Transform, &mut Tracer)>
) {
  let dt = time.delta_secs();
  for (entity, mut transform, mut tracer) in tracers.iter_mut() {
    if tracer.left <= 0.0 {
      commands.entity(entity).despawn();
    } else {
      transform.translation += tracer.velocity * dt.min(tracer.left);
      tracer.left -= dt;
    }
  }
}

fn explode(
  mut commands: Commands,
  effects: Res<Effects>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut shake: ResMut<Shake>,
  mut sounds: MessageWriter<Sound>,
  mut notices: MessageWriter<Notice>,
  mut helis: Query<(Entity, &Transform, &mut Helicopter), Without<Player>>,
  player: Single<
    (Entity, &mut Transform, &mut Walker, &mut Vitals, &mut Motion),
    With<Player>
  >
) {
  let (hero, mut body, mut walker, mut vitals, mut motion) = player.into_inner();
  for (entity, craft, mut heli) in helis.iter_mut().filter(|(_, _, heli)| heli.crashed) {
    let at = craft.transform_point(Vec3::Y * 1.8);
    sounds.write(Sound::here(Cue::Explosion, at));
    shake.0 = 1.0;
    commands.spawn((
      effects.emit(&effects.blast),
      Fleeting(3.0),
      Transform::from_translation(at)
    ));
    commands.spawn((
      PointLight {
        color: Color::srgb(1.0, 0.55, 0.2),
        intensity: 4.0e7,
        range: 150.0,
        ..default()
      },
      Fleeting(0.4),
      Transform::from_translation(at)
    ));
    let fires = commands
      .spawn((
        effects.emit(&effects.campfire),
        Transform::from_translation(Vec3::Y * 2.0),
        ChildOf(entity)
      ))
      .with_child((effects.emit(&effects.smoke), Transform::from_translation(Vec3::Y)))
      .id();
    if let Some(mut paint) = materials.get_mut(&heli.paint) {
      paint.base_color = Color::srgb(0.16, 0.14, 0.13);
    }
    let door = craft.transform_point(DOOR).xz();
    let fling = heli.airframe.velocity * 0.5
      + Vec3::Y * 6.0
      + (craft.rotation * Vec3::NEG_X).with_y(0.0).normalize_or_zero() * 5.0;
    body.translation = door
      .extend((craft.translation.y + 1.5).max(floor(door) + 1.0) + capsule_offset())
      .xzy();
    body.rotation = Quat::from_rotation_y(heli.airframe.heading());
    motion.seated = 0.0;
    vitals.health -= BLAST;
    commands.entity(hero).remove::<(Seated, ColliderDisabled)>();
    if vitals.health <= 0.0 {
      walker.shove = fling;
      commands.entity(hero).insert(Dead);
    } else {
      commands.entity(hero).insert(Knocked { left: 3.0, fling });
    }
    notices.write(Notice("The helicopter is destroyed".into()));
    heli.crewed = false;
    heli.crashed = false;
    heli.wreck = Some(Wreck { left: WRECKED_FOR, fires });
    heli.airframe = Airframe { rotor: 0.0, lever: 0.0, ..heli.airframe };
  }
}

fn salvage(
  time: Res<Time>,
  mut commands: Commands,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut helis: Query<(&mut Transform, &mut Helicopter)>
) {
  for (mut transform, mut heli) in helis.iter_mut() {
    if let Some(Wreck { left, fires }) = heli.wreck.as_mut() {
      *left -= time.delta_secs();
      if *left <= 0.0 {
        commands.entity(*fires).despawn();
        if let Some(mut paint) = materials.get_mut(&heli.paint) {
          paint.base_color = Color::WHITE;
        }
        *transform = Transform::from_translation(PAD.at.extend(PAD.top).xzy())
          .with_rotation(Quat::from_rotation_y(PAD.yaw));
        heli.airframe = Airframe::facing(PAD.yaw);
        heli.wreck = None;
      }
    }
  }
}

struct Reach {
  set: f32,
  shown: f32
}

impl Default for Reach {
  fn default() -> Self { Self { set: 24.0, shown: 24.0 } }
}

fn chase(
  time: Res<Time>,
  scroll: Res<AccumulatedMouseScroll>,
  mut view: ResMut<View>,
  mut reach: Local<Reach>,
  shake: Res<Shake>,
  spatial: SpatialQuery,
  player: Single<Entity, With<Player>>,
  helis: Query<(Entity, &Transform, &Helicopter), Without<MainCamera>>,
  mut camera: Single<&mut Transform, With<MainCamera>>
) {
  if let Some((entity, craft, heli)) = helis.iter().find(|(_, _, heli)| heli.crewed)
    && opts().eye.is_none()
  {
    let dt = time.delta_secs().min(0.25);
    let notches = scroll.delta.y
      * match scroll.unit {
        MouseScrollUnit::Line => 1.0,
        MouseScrollUnit::Pixel => 0.02
      };
    (view.first_person, reach.set) = match (view.first_person, notches) {
      (true, out) if out < 0.0 => (false, CHASE_NEAREST),
      (false, into) if into > 0.0 && reach.set <= CHASE_NEAREST => (true, CHASE_NEAREST),
      (first, _) => (
        first,
        (reach.set * ZOOM_STEP.powf(-notches)).clamp(CHASE_NEAREST, CHASE_FARTHEST)
      )
    };
    reach.shown += (reach.set - reach.shown) * (1.0 - (-dt * 8.0).exp());
    let nose = craft.rotation * Vec3::NEG_Z;
    let trail = 1.0 - (-TRAIL * dt).exp();
    view.yaw = wrap(view.yaw + wrap(heli.airframe.heading() - view.yaw) * trail);
    view.pitch += (nose.y.clamp(-1.0, 1.0).asin() * 0.6 - 0.22 - view.pitch) * trail;
    let look = Quat::from_euler(EulerRot::YXZ, view.yaw, view.pitch, 0.0);
    let now = time.elapsed_secs();
    let tremor = Vec3::new((now * 71.0).sin(), (now * 83.0).cos(), (now * 59.0).sin())
      * shake.0
      * shake.0
      * 0.25;
    **camera = if view.first_person {
      Transform::from_translation(craft.transform_point(SEAT_EYE) + tremor)
        .with_rotation(craft.rotation * Quat::from_rotation_x(-0.12))
    } else {
      let focus = craft.translation + Vec3::Y * 2.2;
      let back = look * Vec3::Z;
      let distance = reach.shown;
      let clear = spatial
        .cast_ray(
          focus,
          Dir3::new(back).unwrap_or(Dir3::Z),
          distance,
          true,
          &SpatialQueryFilter::from_mask(Layer::World)
            .with_excluded_entities([entity, *player])
        )
        .map_or(distance, |hit| (hit.distance - 0.3).max(2.0));
      let eye = focus + back * clear;
      Transform::from_translation(eye.with_y(eye.y.max(floor(eye.xz()) + 0.4)) + tremor)
        .with_rotation(look)
    };
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(PostStartup, build)
    .add_systems(
      Update,
      (board, shoot, fly, explode, salvage, ride).chain().after(Walking)
    )
    .add_systems(Update, streak)
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
      simulate(idle, height, 10.0, Controls { climb: 1.0, ..crewed });
    assert!(height > 60.0, "climbs: {height}");
    let (hovering, held) = simulate(climbed, height, 8.0, crewed);
    assert!(
      hovering.velocity.y.abs() < 0.5 && held > height,
      "holds height: {hovering:?}"
    );
    let (diving, dived) =
      simulate(hovering, held, 3.0, Controls { climb: -1.0, ..crewed });
    assert!(diving.velocity.y < -18.0, "dives fast: {:?} {dived}", diving.velocity);
    let roll = Controls { aside: 1.0, ..crewed };
    let flips = (1..=16).any(|half| {
      let (rolled, _) = simulate(hovering, held, half as f32 * 0.5, roll);
      (rolled.attitude * Vec3::Y).y < 0.0
    });
    assert!(flips, "rolls over");
    let (rolled, _) = simulate(hovering, held, 2.5, roll);
    let (coasting, _) = simulate(rolled, held, 0.5, crewed);
    assert!(
      coasting.spin.z.abs() > 0.6 * rolled.spin.z.abs(),
      "keeps rolling when released: {:?} {:?}",
      rolled.spin,
      coasting.spin
    );
    let (banked, _) = simulate(hovering, held, 1.0, roll);
    let swaying = (1..=20).all(|second| {
      let (swung, _) = simulate(banked, held, second as f32, crewed);
      (swung.attitude * Vec3::Y).y > 0.8
    });
    assert!(swaying, "hands off it sways but does not tumble");
    let (pointed, _) =
      simulate(hovering, held, 3.0, Controls { turn: Vec2::new(1.0, 0.0), ..crewed });
    assert!(
      wrap(pointed.heading() - hovering.heading()) > 1.0,
      "turns: {}",
      pointed.heading()
    );
    let (cruising, _) = simulate(hovering, held, 15.0, Controls { ahead: 1.0, ..crewed });
    let speed = cruising.velocity.with_y(0.0).length();
    assert!(speed > 25.0, "flies forward: {speed}");
    assert!(
      (cruising.attitude * Vec3::Y).y > 0.7,
      "flapback and the tailplane hold the nose: {cruising:?}"
    );
    let (down, height) =
      simulate(hovering, held, 40.0, Controls { climb: -0.3, ..crewed });
    assert!(height < 0.01 && down.velocity.length() < 0.5, "lands: {height} {down:?}");
    let (parked, _) = simulate(down, 0.0, 15.0, Controls::default());
    assert!(parked.rotor < 0.01 && parked.lever < 0.01, "spools down: {parked:?}");
  }
}
