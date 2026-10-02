use {crate::{blocky::{self, Bone, Facet, Kit, Pixel, Shape, Texel},
             noise::hash,
             place,
             player::Player,
             river, terrain},
     bevy::prelude::*,
     std::f32::consts::FRAC_PI_2};

const PX: f32 = 1.0 / 16.0;
const SPEED: f32 = 2.6;
const THIGH: f32 = 14.0;
const SHIN: f32 = 15.0;
const WHEEL: f32 = 5.0;
const HUB: f32 = 4.0;
const SHOWN: f32 = 700.0;
const KEEP_RIGHT: f32 = 1.6;
const HIPS: [Vec3; 4] = [
  Vec3::new(-9.0, 25.0, 8.0),
  Vec3::new(9.0, 25.0, 8.0),
  Vec3::new(-9.0, 25.0, -8.0),
  Vec3::new(9.0, 25.0, -8.0)
];
const STANCE: [Vec2; 4] = [
  Vec2::new(-15.0, 17.0),
  Vec2::new(15.0, 17.0),
  Vec2::new(-15.0, -17.0),
  Vec2::new(15.0, -17.0)
];

#[derive(Clone, Copy, PartialEq)]
enum Skin {
  Main,
  Accent,
  Iron,
  Barrel,
  Grille,
  Tread,
  Screen,
  Sign,
  Red,
  Blue
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
  Body,
  Red,
  Blue,
  Thigh,
  Shin,
  Wheel
}

struct Palette {
  main: [u32; 4],
  accent: [u32; 4],
  glow: [u32; 3],
  face: [&'static str; 9]
}

const PALETTES: [Palette; 4] = [
  Palette {
    main: [0x5a1a1c, 0xd8433c, 0xff8a6e, 0xb02e2c],
    accent: [0x6b5a2c, 0xf3e2b3, 0xfff8e0, 0xd8c690],
    glow: [0x2a1404, 0xc06a10, 0xffc04a],
    face: [
      "...........",
      "..##...##..",
      "..##...##..",
      "...........",
      "...........",
      ".#.......#.",
      "..#######..",
      "...#...#...",
      "..........."
    ]
  },
  Palette {
    main: [0x5e2c08, 0xf08a24, 0xffc070, 0xc86a14],
    accent: [0x0e3e3a, 0x2fa39a, 0x7ae0d0, 0x1f7a72],
    glow: [0x041e24, 0x14a0b8, 0x9af4ff],
    face: [
      "...........",
      ".##.....##.",
      "...#...#...",
      "..##...##..",
      "..##...##..",
      "...........",
      "...#####...",
      "..#.....#..",
      "..........."
    ]
  },
  Palette {
    main: [0x123a68, 0x3d8fe0, 0x9ccaff, 0x2a6cb4],
    accent: [0x6a5208, 0xf7d23e, 0xfff0a0, 0xd0a820],
    glow: [0x04200c, 0x20b050, 0xa0ffb8],
    face: [
      "...........",
      "..##.......",
      "..##...###.",
      "...........",
      "...........",
      "...#####...",
      "..#.....#..",
      "...........",
      "..........."
    ]
  },
  Palette {
    main: [0x2c1450, 0x8a4fc9, 0xc8a0f4, 0x6a3aa4],
    accent: [0x145a3e, 0x6fe3b0, 0xc4ffe4, 0x48b88a],
    glow: [0x2a0420, 0xd03890, 0xffa0dc],
    face: [
      "...........",
      ".###...###.",
      ".#.#...#.#.",
      ".###...###.",
      "...........",
      "....###....",
      "....#.#....",
      "....###....",
      "..........."
    ]
  }
];

const GLYPHS: [[&str; 5]; 4] = [
  ["###", "#..", "###", "..#", "###"],
  ["###", ".#.", ".#.", ".#.", ".#."],
  ["###", "#.#", "#.#", "#.#", "###"],
  ["###", "#.#", "###", "#..", "#.."]
];

fn paint(palette: &Palette, skin: Skin, pixel: Pixel) -> Texel {
  let Pixel { facet, at: UVec2 { x, y }, size: UVec2 { x: w, y: h }, .. } = pixel;
  let iron = [0x1a1c22, 0x4a4e57, 0x80868f, 0x34373e];
  let rivet = w >= 6 && h >= 6 && (x == 1 || x + 2 == w) && (y == 1 || y + 2 == h);
  let [dim, mid, bright] = palette.glow;
  match skin {
    Skin::Main if rivet => Texel::flat(palette.accent[2]),
    Skin::Main if pixel.side() && !pixel.edge() && (y == h / 4 || y == h / 4 + 1) => {
      Texel::flat(pixel.shade(palette.accent))
    }
    Skin::Main
      if pixel.side() && !pixel.edge() && y > h * 3 / 4 && (x + y) / 2 % 2 == 0 =>
    {
      Texel::flat(palette.main[3])
    }
    Skin::Main => Texel::flat(pixel.shade(palette.main)),
    Skin::Accent => Texel::flat(pixel.shade(palette.accent)),
    Skin::Iron => Texel::flat(pixel.shade(iron)),
    Skin::Barrel => Texel::flat(if facet == Facet::Cap {
      0x08080a
    } else {
      pixel.shade([0x101014, 0x2a2c32, 0x5a5e66, 0x202228])
    }),
    Skin::Grille if pixel.side() && !pixel.edge() && x % 2 == 1 && y % 2 == 1 => {
      Texel::lit(dim)
    }
    Skin::Grille => Texel::flat(pixel.shade(iron)),
    Skin::Tread if facet == Facet::Cap => {
      let reach = pixel.middle().length() / (w as f32 / 2.0);
      Texel::flat(match reach {
        reach if reach > 0.85 => 0x1a1a1e,
        reach if reach > 0.55 => palette.accent[1],
        reach if reach > 0.25 => 0x9aa0a8,
        _ => palette.main[1]
      })
    }
    Skin::Tread => Texel::flat(if (x + y / 2) % 3 == 0 { 0x121214 } else { 0x2a2a30 }),
    Skin::Screen if facet == Facet::Front && !pixel.edge() => {
      let (sx, sy) = ((x - 1) as usize, (y - 1) as usize);
      let (sw, sh) = ((w - 2) as usize, (h - 2) as usize);
      let glyph = palette.face[(sy * 9 / sh).min(8)].as_bytes()[(sx * 11 / sw).min(10)];
      match glyph {
        b'#' => Texel::lit(bright),
        _ if sy % 2 == 0 => Texel::lit(dim),
        _ => Texel::lit(if pixel.speck() < 0.1 { mid } else { dim })
      }
    }
    Skin::Screen => Texel::flat(0x101014),
    Skin::Sign if facet == Facet::Cap => {
      let offset = pixel.at.as_vec2() + 0.5 - pixel.size.as_vec2() / 2.0;
      let octagon = offset
        .abs()
        .max_element()
        .max((offset.x.abs() + offset.y.abs()) / std::f32::consts::SQRT_2);
      let (column, row) = (x as i32 - (w as i32 - 15) / 2, y as i32 - (h as i32 - 5) / 2);
      let letter = (0..15).contains(&column)
        && (0..5).contains(&row)
        && column % 4 != 3
        && GLYPHS[(column / 4) as usize][row as usize].as_bytes()[(column % 4) as usize]
          == b'#';
      Texel::flat(if letter || octagon > w as f32 / 2.0 * 0.924 - 1.2 {
        0xf4f0e8
      } else {
        0xc8202a
      })
    }
    Skin::Sign => Texel::flat(0xd8d8d8),
    Skin::Red => Texel::lit(if pixel.speck() < 0.3 { 0xff8080 } else { 0xff2020 }),
    Skin::Blue => Texel::lit(if pixel.speck() < 0.3 { 0x90b0ff } else { 0x2050ff })
  }
}

fn along_z(shape: Shape<Skin>) -> Shape<Skin> {
  shape.turned(Quat::from_rotation_x(-FRAC_PI_2))
}

fn along_x(shape: Shape<Skin>) -> Shape<Skin> {
  shape.turned(Quat::from_rotation_z(-FRAC_PI_2))
}

fn body() -> Vec<Shape<Skin>> {
  let arm = |side: f32| {
    let (shoulder, elbow) =
      (Vec3::new(side * 12.0, 46.0, 0.0), Vec3::new(side * 14.5, 38.0, 3.0));
    [
      along_x(Shape::prism(8, 3.5, 3.5, 4.0, Skin::Accent)).at(shoulder),
      Shape::rod(shoulder, elbow, 2.5, Skin::Iron),
      Shape::boxed(elbow - 1.8, elbow + 1.8, Skin::Main)
    ]
  };
  let gun = (0..3).map(|barrel| {
    let angle = barrel as f32 * std::f32::consts::TAU / 3.0;
    along_z(Shape::prism(6, 0.8, 0.8, 9.0, Skin::Barrel))
      .at(Vec3::new(-14.5, 38.0, 11.5) + Vec3::new(angle.cos(), angle.sin(), 0.0) * 1.3)
  });
  let sign = [
    Shape::rod(Vec3::new(14.5, 38.0, 3.0), Vec3::new(14.5, 46.0, 6.0), 1.0, Skin::Iron),
    Shape::prism(8, 10.0, 10.0, 0.8, Skin::Sign)
      .turned(Quat::from_rotation_x(FRAC_PI_2))
      .at(Vec3::new(14.5, 54.0, 6.5))
  ];
  let hips = HIPS.map(|hip| along_x(Shape::prism(8, 2.6, 2.6, 4.0, Skin::Iron)).at(hip));
  let antenna = Vec3::new(-5.0, 62.0, -4.0);
  [
    Shape::frustum(Vec2::new(20.0, 18.0), Vec2::new(18.0, 16.0), 4.0, Skin::Main)
      .at(Vec3::Y * 26.0),
    Shape::frustum(Vec2::new(16.0, 11.0), Vec2::new(15.0, 10.0), 22.0, Skin::Main)
      .at(Vec3::Y * 39.0),
    Shape::block(Vec3::new(13.0, 11.0, 1.0), Skin::Screen).at(Vec3::new(0.0, 41.0, 5.4)),
    Shape::block(Vec3::new(15.0, 1.5, 3.0), Skin::Accent).at(Vec3::new(0.0, 47.5, 5.6)),
    Shape::block(Vec3::new(12.0, 14.0, 4.0), Skin::Grille).at(Vec3::new(0.0, 38.0, -7.0)),
    Shape::prism(6, 1.0, 1.0, 9.0, Skin::Iron).at(Vec3::new(4.0, 49.0, -8.0)),
    Shape::prism(6, 1.0, 1.0, 9.0, Skin::Iron).at(Vec3::new(-4.0, 49.0, -8.0)),
    Shape::prism(8, 7.0, 4.0, 3.0, Skin::Accent).at(Vec3::Y * 51.5),
    Shape::rod(Vec3::new(-4.0, 53.0, -3.0), antenna, 0.6, Skin::Iron),
    Shape::boxed(antenna - 0.9, antenna + 0.9, Skin::Accent),
    along_z(Shape::prism(8, 2.6, 2.6, 4.0, Skin::Iron)).at(Vec3::new(-14.5, 38.0, 5.0))
  ]
  .into_iter()
  .chain(arm(-1.0))
  .chain(arm(1.0))
  .chain(gun)
  .chain(sign)
  .chain(hips)
  .collect()
}

fn bones() -> Vec<Bone<Part, Skin>> {
  let bone = |part, shapes| Bone { part, parent: None, pivot: Vec3::ZERO, shapes };
  let siren =
    |x: f32, skin| Shape::prism(6, 1.6, 1.3, 2.5, skin).at(Vec3::new(x, 54.2, 0.0));
  vec![
    bone(Part::Body, body()),
    bone(Part::Red, vec![siren(-2.6, Skin::Red)]),
    bone(Part::Blue, vec![siren(2.6, Skin::Blue)]),
    bone(Part::Thigh, vec![
      along_z(Shape::prism(6, 2.2, 1.7, THIGH, Skin::Main)).at(Vec3::Z * -THIGH / 2.0),
      along_x(Shape::prism(8, 2.4, 2.4, 3.0, Skin::Iron)).at(Vec3::Z * -THIGH),
    ]),
    bone(Part::Shin, vec![
      along_z(Shape::frustum(Vec2::splat(3.0), Vec2::splat(2.0), SHIN, Skin::Accent))
        .at(Vec3::Z * -SHIN / 2.0),
      Shape::block(Vec3::splat(HUB), Skin::Iron).at(Vec3::Z * -SHIN),
    ]),
    bone(Part::Wheel, vec![along_x(Shape::prism(10, WHEEL, WHEEL, 4.0, Skin::Tread))]),
  ]
}

#[derive(Clone)]
pub struct Route {
  points: Vec<Vec2>,
  marks: Vec<f32>
}

impl Route {
  fn new(points: Vec<Vec2>) -> Self {
    let marks = points
      .iter()
      .scan((0.0, points[0]), |(total, last), &point| {
        *total += last.distance(point);
        *last = point;
        Some(*total)
      })
      .collect();
    Self { points, marks }
  }

  fn length(&self) -> f32 { self.marks.last().copied().unwrap_or(0.0) }

  fn sample(&self, along: f32) -> (Vec2, Vec2) {
    let index =
      self.marks.partition_point(|&mark| mark < along).clamp(1, self.points.len() - 1);
    let (from, to) = (self.points[index - 1], self.points[index]);
    let span = (self.marks[index] - self.marks[index - 1]).max(1e-3);
    let fraction = ((along - self.marks[index - 1]) / span).clamp(0.0, 1.0);
    (from.lerp(to, fraction), (to - from).normalize_or(Vec2::Y))
  }
}

fn footing(at: Vec2) -> f32 {
  let land = terrain::height_at(at);
  river::deck(at).map_or(land, |deck| deck.max(land))
}

struct Leg {
  thigh: Entity,
  shin: Entity,
  wheel: Entity
}

#[derive(Component)]
struct Patrol {
  route: Route,
  along: f32,
  heading: f32,
  speed: f32,
  at: Vec2,
  yaw: f32,
  spin: f32,
  body: [Entity; 3],
  legs: Vec<Leg>
}

fn spawn_patrol(
  commands: &mut Commands,
  kit: &Kit<Part>,
  route: Route,
  along: f32,
  heading: f32,
  speed: f32
) {
  let piece = |commands: &mut Commands, part: Part, parent: Entity| {
    let model = kit.parts.iter().find(|model| model.part == part).expect("patrol part");
    commands
      .spawn((
        Mesh3d(model.mesh.clone()),
        MeshMaterial3d(kit.material.clone()),
        Transform::default(),
        ChildOf(parent)
      ))
      .id()
  };
  let root = commands.spawn((Transform::default(), Visibility::default())).id();
  let body = [Part::Body, Part::Red, Part::Blue].map(|part| piece(commands, part, root));
  let legs = (0..4)
    .map(|_| Leg {
      thigh: piece(commands, Part::Thigh, root),
      shin: piece(commands, Part::Shin, root),
      wheel: piece(commands, Part::Wheel, root)
    })
    .collect();
  let (at, tangent) = route.sample(along);
  let forward = tangent * heading;
  commands.entity(root).insert(Patrol {
    route,
    along,
    heading,
    speed,
    at,
    yaw: forward.x.atan2(forward.y),
    spin: 0.0,
    body,
    legs
  });
}

fn muster(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  player: Single<&Transform, Added<Player>>
) {
  let kits = blocky::kits(
    &bones(),
    PX,
    PALETTES
      .iter()
      .map(|palette| move |skin: Skin, pixel: Pixel| paint(palette, skin, pixel)),
    5.0,
    &mut meshes,
    &mut images,
    &mut materials
  );
  let routes: Vec<Route> = place::ROADS
    .iter()
    .filter(|road| road.path.len() > 1)
    .map(|road| Route::new(road.path.clone()))
    .filter(|route| route.length() > 300.0)
    .collect();
  let nearest = |route: &Route| {
    route
      .marks
      .iter()
      .zip(&route.points)
      .map(|(&mark, point)| (mark, point.distance(place::START)))
      .min_by(|a, b| a.1.total_cmp(&b.1))
      .unwrap_or((0.0, f32::MAX))
  };
  let first = routes
    .iter()
    .enumerate()
    .min_by(|a, b| nearest(a.1).1.total_cmp(&nearest(b.1).1))
    .map(|(index, route)| (index, nearest(route).0 + 60.0, -1.0));
  let spread = routes.iter().enumerate().flat_map(|(index, route)| {
    let count = (route.length() / 1200.0).ceil() as usize;
    (0..count).map(move |patrol| {
      let along = (patrol as f32 + hash(index as i32, patrol as i32, 41)) / count as f32
        * route.length();
      (index, along, if (index + patrol) % 2 == 0 { 1.0 } else { -1.0 })
    })
  });
  for (number, (index, along, heading)) in first.into_iter().chain(spread).enumerate() {
    spawn_patrol(
      &mut commands,
      &kits[number % kits.len()],
      routes[index].clone(),
      along,
      heading,
      SPEED
    );
  }
  if crate::opts::opts().foe.as_deref() == Some("patrol") {
    let opts = crate::opts::opts();
    let ahead =
      player.translation.xz() + player.forward().as_vec3().xz() * opts.gap.unwrap_or(5.0);
    let toward = Vec2::from_angle(0.7).rotate(-player.forward().as_vec3().xz());
    let route = Route::new(vec![ahead - toward, ahead + toward]);
    let kit = &kits[opts.seed.unwrap_or(0) as usize % kits.len()];
    spawn_patrol(&mut commands, kit, route, 1.0, 1.0, 0.0);
  }
}

fn patrol(
  time: Res<Time>,
  cameras: Query<&GlobalTransform, With<Camera3d>>,
  mut patrols: Query<(&mut Patrol, &mut Visibility)>,
  mut parts: Query<(&mut Transform, &mut Visibility), Without<Patrol>>
) {
  let (dt, now) = (time.delta_secs(), time.elapsed_secs());
  let eye = cameras.iter().next().map_or(Vec3::ZERO, |camera| camera.translation());
  for (mut patrol, mut visibility) in patrols.iter_mut() {
    let length = patrol.route.length();
    let along = patrol.along + patrol.heading * patrol.speed * dt;
    let turned = !(0.0..=length).contains(&along);
    patrol.heading *= if turned { -1.0 } else { 1.0 };
    patrol.along = along.clamp(0.0, length);
    let (point, tangent) = patrol.route.sample(patrol.along);
    let forward = tangent * patrol.heading;
    let goal = point + forward.perp() * KEEP_RIGHT;
    let before = patrol.at;
    patrol.at = before.lerp(goal, (dt * 3.0).min(1.0));
    let wanted = forward.x.atan2(forward.y);
    let turn = (wanted - patrol.yaw + std::f32::consts::PI)
      .rem_euclid(std::f32::consts::TAU)
      - std::f32::consts::PI;
    patrol.yaw += turn.clamp(-1.6 * dt, 1.6 * dt);
    let yaw = Quat::from_rotation_y(patrol.yaw);
    patrol.spin += (patrol.at - before).dot((yaw * Vec3::Z).xz()) / (WHEEL * PX);
    let shown = eye.xz().distance(patrol.at) < SHOWN;
    *visibility = if shown { Visibility::Inherited } else { Visibility::Hidden };
    if shown {
      let contacts = STANCE.map(|stance| {
        let spot = patrol.at + (yaw * stance.extend(0.0).xzy() * PX).xz();
        spot.extend(footing(spot)).xzy()
      });
      let height = |indices: [usize; 2]| {
        indices.map(|index| contacts[index].y).iter().sum::<f32>() / 2.0
      };
      let (front, back, left, right) =
        (height([0, 1]), height([2, 3]), height([0, 2]), height([1, 3]));
      let rise = ((front - back) / (STANCE[0].y * 2.0 * PX)).atan();
      let lean = ((right - left) / (STANCE[1].x * 2.0 * PX)).atan();
      let rotation = yaw * Quat::from_rotation_x(-rise) * Quat::from_rotation_z(lean);
      let center = patrol.at.extend((front + back) / 2.0).xzy()
        + Vec3::Y * ((now * 7.0).sin() * 0.015);
      let body = Transform::from_translation(center).with_rotation(rotation);
      let blink = (now * 2.0).fract() < 0.5;
      for (index, entity) in patrol.body.iter().enumerate() {
        if let Ok((mut transform, mut visibility)) = parts.get_mut(*entity) {
          *transform = body;
          *visibility = match index {
            1 if !blink => Visibility::Hidden,
            2 if blink => Visibility::Hidden,
            _ => Visibility::Inherited
          };
        }
      }
      let (outward, up, ahead) =
        (rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z);
      for (index, leg) in patrol.legs.iter().enumerate() {
        let side = HIPS[index].x.signum();
        let hip = body.transform_point(HIPS[index] * PX);
        let wheel = contacts[index] + Vec3::Y * WHEEL * PX;
        let foot = wheel - outward * side * (HUB / 2.0 + 2.0) * PX;
        let knee =
          blocky::knee(hip, foot, THIGH * PX, SHIN * PX, outward * side + up * 0.8);
        let poses = [
          (leg.thigh, blocky::aim(hip, knee, ahead)),
          (leg.shin, blocky::aim(knee, foot, ahead)),
          (
            leg.wheel,
            Transform::from_translation(wheel)
              .with_rotation(rotation * Quat::from_rotation_x(patrol.spin))
          )
        ];
        for (entity, pose) in poses {
          if let Ok((mut transform, _)) = parts.get_mut(entity) {
            *transform = pose;
          }
        }
      }
    }
  }
}

pub fn plugin(app: &mut App) {
  app.add_systems(PostStartup, muster).add_systems(Update, patrol);
}
