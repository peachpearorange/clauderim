use {crate::{blocky::{self, Bone, Facet, Pixel, Shape, Texel},
             noise::Roll,
             place,
             player::Player,
             terrain::{self, height_at}},
     bevy::prelude::*,
     std::{f32::consts::{FRAC_PI_2, PI, TAU},
           sync::LazyLock}};

const PX: f32 = 0.45;
const FEMUR: f32 = 45.0;
const TIBIA: f32 = 55.0;
const RIDE: f32 = 23.0 * PX;
const SPEED: f32 = 3.3 * PX;
const TURN: f32 = 0.12;
const STEP: f32 = 1.7;
const STRIDE: f32 = 12.0 * PX;
const LIFT: f32 = 10.0 * PX;
const ROAM: f32 = 280.0;
const SPIDERS: usize = 7;
const HIPS: [Vec3; 4] = [
  Vec3::new(7.0, -1.0, 6.0),
  Vec3::new(8.0, -1.0, 2.0),
  Vec3::new(8.0, -1.0, -2.0),
  Vec3::new(7.0, -1.0, -6.0)
];
const REACH: [(f32, f32); 4] = [(40.0, 73.0), (75.0, 67.0), (110.0, 67.0), (145.0, 73.0)];

#[derive(Clone, Copy, PartialEq)]
enum Skin {
  Shell,
  Abdomen,
  Leg,
  Joint,
  Fang,
  Eye
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
  Body,
  Femur,
  Tibia
}

struct Coat {
  hide: [u32; 4],
  band: u32,
  mark: u32,
  eye: u32
}

const COATS: [Coat; 2] = [
  Coat {
    hide: [0x0c0a1a, 0x221c44, 0x4a3e86, 0x161230],
    band: 0xe8902a,
    mark: 0x2fc8b8,
    eye: 0xff3a30
  },
  Coat {
    hide: [0x1a0a08, 0x4a1c14, 0x8a3a24, 0x2e120c],
    band: 0xf0d050,
    mark: 0x58e05a,
    eye: 0x40d0ff
  }
];

fn paint(coat: &Coat, skin: Skin, pixel: Pixel) -> Texel {
  let Pixel { facet, at: UVec2 { x, y }, size: UVec2 { x: w, y: h }, .. } = pixel;
  let [dark, base, light, _] = coat.hide;
  let fuzz = pixel.speck();
  let hairy = if fuzz > 0.86 {
    light
  } else if fuzz < 0.12 {
    dark
  } else {
    base
  };
  match skin {
    Skin::Eye => Texel::lit(if pixel.size.x > 1 && pixel.at == UVec2::ZERO {
      0xffe0d0
    } else {
      coat.eye
    }),
    Skin::Fang => Texel::flat(pixel.shade([0x0a0806, 0x2a2018, 0x6a5a48, 0x1a140e])),
    Skin::Joint => Texel::flat(if fuzz < 0.3 { dark } else { coat.band }),
    Skin::Leg if facet == Facet::Rim && (y < 3 || y + 4 > h || y % 14 == 7) => {
      Texel::flat(coat.band)
    }
    Skin::Leg => Texel::flat(hairy),
    Skin::Abdomen if facet == Facet::Rim => {
      let side = x * 8 / w;
      let ridge = (x as i32 - (6 * w / 8) as i32).abs();
      let diamond = ridge + ((y % 12) as i32 - 6).abs();
      match () {
        _ if (5..=6).contains(&side) && diamond < 4 => Texel::lit(coat.mark),
        _ if (5..=6).contains(&side) && diamond == 4 => Texel::flat(dark),
        _ if (3..=7).contains(&side) && y % 12 < 2 => Texel::flat(coat.band),
        _ => Texel::flat(hairy)
      }
    }
    Skin::Abdomen => Texel::flat(hairy),
    Skin::Shell if facet == Facet::Top && pixel.middle().x < 1.0 && y > 1 => {
      Texel::flat(coat.band)
    }
    Skin::Shell if facet == Facet::Top && (x + y) % 5 == 0 => Texel::flat(light),
    Skin::Shell => Texel::flat(pixel.shade(coat.hide))
  }
}

fn along_z(shape: Shape<Skin>) -> Shape<Skin> {
  shape.turned(Quat::from_rotation_x(-FRAC_PI_2))
}

fn body() -> Vec<Shape<Skin>> {
  let eye = |at: Vec3, size: f32| Shape::block(Vec3::splat(size), Skin::Eye).at(at);
  let eyes = [
    (2.2, 6.4, 11.2, 2.4),
    (4.4, 5.4, 10.0, 1.4),
    (1.2, 7.6, 9.6, 1.2),
    (5.4, 3.8, 10.4, 1.2)
  ]
  .into_iter()
  .flat_map(|(x, y, z, size)| {
    [eye(Vec3::new(x, y, z), size), eye(Vec3::new(-x, y, z), size)]
  });
  let jaw = |side: f32| {
    let (root, tip) =
      (Vec3::new(side * 2.5, 1.0, 11.5), Vec3::new(side * 2.0, -6.0, 15.0));
    [
      Shape::frustum(Vec2::splat(3.6), Vec2::splat(2.2), root.distance(tip), Skin::Shell)
        .along(root, tip),
      Shape::prism(4, 1.0, 0.0, 4.0, Skin::Fang)
        .along(tip, tip + Vec3::new(-side, -3.0, -1.5)),
      Shape::rod(
        Vec3::new(side * 4.0, -1.0, 10.0),
        Vec3::new(side * 6.5, -5.0, 15.0),
        1.4,
        Skin::Leg
      )
    ]
  };
  let tilt = Quat::from_rotation_x(-0.25);
  let pedicel = Vec3::new(0.0, 0.0, -10.0);
  let abdomen = [
    along_z(Shape::prism(8, 7.0, 13.0, 8.0, Skin::Abdomen)).at(Vec3::new(0.0, 4.0, -6.0)),
    along_z(Shape::prism(8, 13.0, 12.0, 14.0, Skin::Abdomen))
      .at(Vec3::new(0.0, 4.0, -17.0)),
    along_z(Shape::prism(8, 12.0, 4.5, 10.0, Skin::Abdomen))
      .at(Vec3::new(0.0, 4.0, -29.0)),
    along_z(Shape::prism(6, 4.5, 1.0, 4.0, Skin::Fang)).at(Vec3::new(0.0, 4.0, -36.0))
  ]
  .map(|shape| shape.placed(pedicel, tilt));
  let coxae = HIPS
    .into_iter()
    .flat_map(|hip| [hip, hip * Vec3::new(-1.0, 1.0, 1.0)])
    .map(|hip| Shape::block(Vec3::splat(4.0), Skin::Joint).at(hip));
  [
    Shape::frustum(Vec2::new(16.0, 21.0), Vec2::new(11.0, 15.0), 8.0, Skin::Shell),
    Shape::frustum(Vec2::new(10.0, 14.0), Vec2::new(16.0, 21.0), 3.0, Skin::Shell)
      .at(Vec3::Y * -5.5),
    Shape::frustum(Vec2::new(10.0, 8.0), Vec2::new(7.0, 5.0), 4.0, Skin::Shell)
      .at(Vec3::new(0.0, 5.0, 8.5)),
    Shape::block(Vec3::new(6.0, 5.0, 4.0), Skin::Leg).at(pedicel)
  ]
  .into_iter()
  .chain(eyes)
  .chain(jaw(-1.0))
  .chain(jaw(1.0))
  .chain(abdomen)
  .chain(coxae)
  .collect()
}

fn bones() -> Vec<Bone<Part, Skin>> {
  let bone = |part, shapes| Bone { part, parent: None, pivot: Vec3::ZERO, shapes };
  vec![
    bone(Part::Body, body()),
    bone(Part::Femur, vec![
      along_z(Shape::prism(6, 2.4, 1.8, FEMUR, Skin::Leg)).at(Vec3::Z * -FEMUR / 2.0),
      Shape::block(Vec3::splat(3.6), Skin::Joint).at(Vec3::Z * -FEMUR),
    ]),
    bone(Part::Tibia, vec![
      along_z(Shape::prism(6, 1.8, 0.8, TIBIA - 7.0, Skin::Leg))
        .at(Vec3::Z * -(TIBIA - 7.0) / 2.0),
      along_z(Shape::prism(6, 0.8, 0.0, 7.0, Skin::Fang)).at(Vec3::Z * -(TIBIA - 3.5)),
    ]),
  ]
}

fn sure_footing(at: Vec2, below: f32) -> bool {
  let height = height_at(at);
  let slope = Vec2::new(
    height_at(at + Vec2::X * 15.0) - height,
    height_at(at + Vec2::Y * 15.0) - height
  ) / 15.0;
  height > below && slope.length() < 0.9
}

static HOMES: LazyLock<Vec<Vec2>> = LazyLock::new(|| {
  let floor = height_at(place::START) + 140.0;
  let reach = terrain::WORLD - 200.0;
  let mut spots: Vec<Vec2> = (-44..=44)
    .flat_map(|row| {
      (-44..=44)
        .map(move |column| place::START + Vec2::new(column as f32, row as f32) * 60.0)
    })
    .filter(|spot| {
      spot.abs().max_element() < reach
        && (500.0..2600.0).contains(&spot.distance(place::START))
    })
    .filter(|&spot| sure_footing(spot, floor))
    .collect();
  spots.sort_by(|a, b| a.distance(place::START).total_cmp(&b.distance(place::START)));
  spots.into_iter().fold(Vec::new(), |mut homes, spot| {
    if homes.len() < SPIDERS
      && homes.iter().all(|home: &Vec2| home.distance(spot) > 650.0)
    {
      homes.push(spot)
    }
    homes
  })
});

struct Foot {
  planted: Vec3,
  from: Vec3,
  to: Vec3,
  lift: Option<f32>
}

impl Foot {
  fn now(&self) -> Vec3 {
    self.lift.map_or(self.planted, |lift| {
      let eased = lift * lift * (3.0 - 2.0 * lift);
      self.from.lerp(self.to, eased) + Vec3::Y * (lift * PI).sin() * LIFT
    })
  }
}

struct Leg {
  side: f32,
  pair: usize,
  femur: Entity,
  tibia: Entity,
  foot: Foot
}

impl Leg {
  fn hip(&self) -> Vec3 { HIPS[self.pair] * Vec3::new(self.side, 1.0, 1.0) * PX }

  fn rest(&self) -> Vec2 {
    let (angle, distance) = REACH[self.pair];
    Vec2::new(angle.to_radians().sin() * self.side, angle.to_radians().cos())
      * distance
      * PX
  }

  fn group(&self) -> usize { (self.pair + (self.side > 0.0) as usize) % 2 }
}

#[derive(Component)]
struct Spider {
  home: Vec2,
  floor: f32,
  at: Vec2,
  height: f32,
  yaw: f32,
  tilt: Quat,
  goal: Vec2,
  pause: f32,
  roll: Roll,
  body: Entity,
  legs: Vec<Leg>
}

fn rest_spot(at: Vec2, yaw: f32, offset: Vec2) -> Vec3 {
  let spot = at + Vec2::from_angle(-yaw).rotate(offset);
  spot.extend(height_at(spot)).xzy()
}

fn spawn_spider(commands: &mut Commands, kit: &blocky::Kit<Part>, home: Vec2, seed: u32) {
  let piece = |commands: &mut Commands, part: Part, parent: Entity| {
    let model = kit.parts.iter().find(|model| model.part == part).expect("spider part");
    commands
      .spawn((
        Mesh3d(model.mesh.clone()),
        MeshMaterial3d(kit.material.clone()),
        Transform::default(),
        ChildOf(parent)
      ))
      .id()
  };
  let mut roll = Roll::new(seed);
  let yaw = roll.range(0.0, TAU);
  let root = commands.spawn((Transform::default(), Visibility::default())).id();
  let body = piece(commands, Part::Body, root);
  let legs = (0..8)
    .map(|index| {
      let mut leg = Leg {
        side: if index < 4 { -1.0 } else { 1.0 },
        pair: index % 4,
        femur: piece(commands, Part::Femur, root),
        tibia: piece(commands, Part::Tibia, root),
        foot: Foot { planted: Vec3::ZERO, from: Vec3::ZERO, to: Vec3::ZERO, lift: None }
      };
      leg.foot.planted = rest_spot(home, yaw, leg.rest());
      leg
    })
    .collect();
  commands.entity(root).insert(Spider {
    home,
    floor: height_at(home) - 80.0,
    at: home,
    height: height_at(home) + RIDE,
    yaw,
    tilt: Quat::from_rotation_y(yaw),
    goal: home,
    pause: roll.range(0.0, 8.0),
    roll,
    body,
    legs
  });
}

fn hatch(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  player: Single<&Transform, Added<Player>>
) {
  let kits = blocky::kits(
    &bones(),
    PX,
    COATS.iter().map(|coat| move |skin: Skin, pixel: Pixel| paint(coat, skin, pixel)),
    4.0,
    &mut meshes,
    &mut images,
    &mut materials
  );
  let specimen = (crate::opts::opts().foe.as_deref() == Some("spider")).then(|| {
    player.translation.xz()
      + player.forward().as_vec3().xz() * crate::opts::opts().gap.unwrap_or(45.0)
  });
  for (index, &home) in HOMES.iter().chain(specimen.iter()).enumerate() {
    spawn_spider(&mut commands, &kits[index % kits.len()], home, index as u32 * 7 + 3);
  }
}

fn roam(
  time: Res<Time>,
  mut spiders: Query<&mut Spider>,
  mut parts: Query<&mut Transform, Without<Spider>>
) {
  let (dt, now) = (time.delta_secs().min(0.25), time.elapsed_secs());
  for mut spider in spiders.iter_mut() {
    let spider = &mut *spider;
    let toward = spider.goal - spider.at;
    let arrived = toward.length() < 40.0 * PX;
    spider.pause -= if arrived { dt } else { 0.0 };
    if arrived && spider.pause <= 0.0 {
      let (home, floor) = (spider.home, spider.floor);
      let roll = &mut spider.roll;
      spider.goal = (0..10)
        .map(|_| {
          home + Vec2::from_angle(roll.range(0.0, TAU)) * roll.range(0.3, 1.0) * ROAM
        })
        .find(|&goal| sure_footing(goal, floor))
        .unwrap_or(home);
      spider.pause = roll.range(6.0, 20.0);
    }
    let wanted = toward.x.atan2(toward.y);
    let turn = (wanted - spider.yaw + PI).rem_euclid(TAU) - PI;
    let moving = !arrived;
    spider.yaw += if moving { turn.clamp(-TURN * dt, TURN * dt) } else { 0.0 };
    let forward = Vec2::new(spider.yaw.sin(), spider.yaw.cos());
    let speed = if moving { SPEED * turn.cos().max(0.0).powi(2) } else { 0.0 };
    spider.at += forward * speed * dt;
    let (at, yaw) = (spider.at, spider.yaw);
    let stepping = |legs: &[Leg], group: usize| {
      legs.iter().any(|leg| leg.group() == group && leg.foot.lift.is_some())
    };
    let busy = [stepping(&spider.legs, 0), stepping(&spider.legs, 1)];
    let due: Vec<(usize, Vec3)> = spider
      .legs
      .iter()
      .enumerate()
      .filter(|(_, leg)| leg.foot.lift.is_none() && !busy[1 - leg.group()])
      .filter_map(|(index, leg)| {
        let target = rest_spot(at + forward * speed * STEP * 0.9, yaw, leg.rest());
        let slack = if speed > 0.05 { STRIDE } else { 4.0 * PX };
        (leg.foot.planted.xz().distance(target.xz()) > slack).then_some((index, target))
      })
      .collect();
    let starting = due.first().map(|&(index, _)| spider.legs[index].group());
    for (index, target) in due {
      let leg = &mut spider.legs[index];
      if Some(leg.group()) == starting {
        leg.foot = Foot {
          planted: leg.foot.planted,
          from: leg.foot.planted,
          to: target,
          lift: Some(0.0)
        };
      }
    }
    for leg in spider.legs.iter_mut() {
      let lift = leg.foot.lift.map(|lift| lift + dt / STEP);
      leg.foot.planted =
        if lift.is_some_and(|lift| lift >= 1.0) { leg.foot.to } else { leg.foot.planted };
      leg.foot.lift = lift.filter(|&lift| lift < 1.0);
    }
    let feet: Vec<Vec3> = spider.legs.iter().map(|leg| leg.foot.planted).collect();
    let mean = |pick: &dyn Fn(&Leg) -> bool| {
      let chosen: Vec<f32> = spider
        .legs
        .iter()
        .zip(&feet)
        .filter(|(leg, _)| pick(leg))
        .map(|(_, foot)| foot.y)
        .collect();
      chosen.iter().sum::<f32>() / chosen.len() as f32
    };
    let (front, back) = (mean(&|leg| leg.pair == 0), mean(&|leg| leg.pair == 3));
    let (left, right) = (mean(&|leg| leg.side < 0.0), mean(&|leg| leg.side > 0.0));
    let level = mean(&|_| true) + RIDE;
    let lowest = height_at(at) + 12.0 * PX;
    spider.height = spider.height.lerp(level.max(lowest), (dt * 1.2).min(1.0));
    let rise = ((front - back) / (113.0 * PX)).atan() * 0.8;
    let lean = ((right - left) / (100.0 * PX)).atan() * 0.8;
    let goal = Quat::from_rotation_y(yaw)
      * Quat::from_rotation_x(-rise)
      * Quat::from_rotation_z(lean);
    spider.tilt = spider.tilt.slerp(goal, (dt * 1.5).min(1.0));
    let breath = (now * 0.8 + spider.home.x).sin() * 0.25;
    let body = Transform::from_translation(at.extend(spider.height + breath).xzy())
      .with_rotation(spider.tilt);
    if let Ok(mut transform) = parts.get_mut(spider.body) {
      *transform = body;
    }
    let up = spider.tilt * Vec3::Y;
    for leg in spider.legs.iter() {
      let hip = body.transform_point(leg.hip());
      let foot = leg.foot.now();
      let outward =
        (foot - hip).with_y(0.0).normalize_or(spider.tilt * Vec3::X * leg.side);
      let knee = blocky::knee(hip, foot, FEMUR * PX, TIBIA * PX, up * 2.0 + outward);
      let across = outward.cross(Vec3::Y);
      if let Ok(mut transform) = parts.get_mut(leg.femur) {
        *transform = blocky::aim(hip, knee, across);
      }
      if let Ok(mut transform) = parts.get_mut(leg.tibia) {
        *transform = blocky::aim(knee, foot, across);
      }
    }
  }
}

pub fn plugin(app: &mut App) {
  app.add_systems(PostStartup, hatch).add_systems(Update, roam);
}
