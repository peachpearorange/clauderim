use {crate::{inventory::{Inventory, Item, Loot},
             noise::hash,
             place,
             player::{Player, View},
             signal::{Cue, Notice, Prompt, Prompting, Sound},
             terrain::Ground},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            image::ImageSampler,
            mesh::{Indices, PrimitiveTopology},
            prelude::*,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat}}};

const PX: f32 = 1.0 / 20.0;
const ATLAS: u32 = 256;

#[derive(Clone, Copy, PartialEq)]
enum Skin {
  Plate,
  Trim,
  Iron,
  Hazard,
  Rubber,
  Core,
  Monitor,
  Horn,
  Bell,
  Lamp,
  Bulb,
  Grille
}

#[derive(Clone, Copy, PartialEq)]
enum Face {
  Top,
  Bottom,
  Left,
  Front,
  Right,
  Back
}

impl Face {
  const ALL: [Face; 6] =
    [Face::Top, Face::Bottom, Face::Left, Face::Front, Face::Right, Face::Back];

  fn axes(self) -> (Vec3, Vec3, Vec3) {
    match self {
      Face::Top => (Vec3::Y, Vec3::X, Vec3::NEG_Z),
      Face::Bottom => (Vec3::NEG_Y, Vec3::X, Vec3::Z),
      Face::Left => (Vec3::NEG_X, Vec3::Z, Vec3::Y),
      Face::Front => (Vec3::Z, Vec3::X, Vec3::Y),
      Face::Right => (Vec3::X, Vec3::NEG_Z, Vec3::Y),
      Face::Back => (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y)
    }
  }

  fn rect(self, [w, h, d]: [u32; 3]) -> URect {
    let (at, size) = match self {
      Face::Top => (UVec2::new(d, 0), UVec2::new(w, d)),
      Face::Bottom => (UVec2::new(d + w, 0), UVec2::new(w, d)),
      Face::Left => (UVec2::new(0, d), UVec2::new(d, h)),
      Face::Front => (UVec2::new(d, d), UVec2::new(w, h)),
      Face::Right => (UVec2::new(d + w, d), UVec2::new(d, h)),
      Face::Back => (UVec2::new(2 * d + w, d), UVec2::new(w, h))
    };
    URect::from_corners(at, at + size)
  }
}

#[derive(Clone, Copy)]
struct Cube {
  center: Vec3,
  size: Vec3,
  turn: Quat,
  skin: Skin
}

impl Cube {
  fn boxed(from: Vec3, to: Vec3, skin: Skin) -> Self {
    Self {
      center: (from + to) / 2.0,
      size: (to - from).abs(),
      turn: Quat::IDENTITY,
      skin
    }
  }

  fn span(from: Vec3, to: Vec3, thick: f32, skin: Skin) -> Self {
    let reach = to - from;
    Self {
      center: (from + to) / 2.0,
      size: Vec3::new(thick, reach.length(), thick),
      turn: Quat::from_rotation_arc(Vec3::Y, reach.normalize()),
      skin
    }
  }

  fn tilted(self, turn: Quat) -> Self { Self { turn: turn * self.turn, ..self } }

  fn placed(self, at: Vec3, turn: Quat) -> Self {
    Self { center: at + turn * self.center, turn: turn * self.turn, ..self }
  }

  fn net(&self) -> [u32; 3] {
    self.size.to_array().map(|side| (side.round() as u32).max(1))
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Part {
  Hips,
  Body,
  Core,
  Head,
  Rotor,
  Beacon,
  Horn,
  Claw
}

struct Bone {
  part: Part,
  parent: Option<Part>,
  pivot: Vec3,
  cubes: Vec<Cube>
}

fn leg(side: f32) -> Vec<Cube> {
  let (hip, knee, ankle) = (
    Vec3::new(side * 7.0, 29.0, 0.0),
    Vec3::new(side * 8.0, 18.0, 6.0),
    Vec3::new(side * 7.0, 5.0, -3.0)
  );
  let toe = |yaw: f32, reach: f32| {
    let tip = ankle.with_y(1.0) + Quat::from_rotation_y(yaw) * Vec3::Z * reach;
    Cube::span(ankle.with_y(1.5), tip, 2.0, Skin::Rubber)
  };
  vec![
    Cube::span(hip, knee, 4.0, Skin::Plate),
    Cube::boxed(knee - 2.5, knee + 2.5, Skin::Hazard),
    Cube::span(knee, ankle, 3.0, Skin::Trim),
    Cube::boxed(ankle - 2.0, ankle + 2.0, Skin::Iron),
    Cube::boxed(
      ankle - Vec3::new(3.0, 5.0, 3.0),
      ankle + Vec3::new(3.0, -3.0, 3.0),
      Skin::Plate
    ),
    toe(-0.55, 8.0),
    toe(0.0, 9.0),
    toe(0.55, 8.0),
    toe(std::f32::consts::PI, 5.0),
  ]
}

fn horn() -> Vec<Cube> {
  let shoulder = Vec3::new(-11.0, 43.0, 0.0);
  let elbow = Vec3::new(-17.0, 36.0, 3.0);
  let wrist = Vec3::new(-17.0, 47.0, 7.0);
  let bore = Quat::from_rotation_arc(Vec3::Z, Vec3::new(-0.45, 0.75, 0.5).normalize());
  let flare = (0..5).map(|step| {
    let side = 2.5 * 1.32_f32.powi(step);
    let along = step as f32 * 2.2;
    Cube::boxed(
      Vec3::new(-side / 2.0, -side / 2.0, along),
      Vec3::new(side / 2.0, side / 2.0, along + 2.2),
      Skin::Horn
    )
  });
  let lip = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].map(|(x, y)| {
    let offset = Vec3::new(x, y, 0.0) * 4.4;
    let half =
      Vec3::new(if x == 0.0 { 5.0 } else { 0.6 }, if y == 0.0 { 5.0 } else { 0.6 }, 0.8);
    Cube::boxed(offset - half, offset + half, Skin::Trim)
      .placed(Vec3::Z * 11.8, Quat::IDENTITY)
  });
  let bell =
    Cube::boxed(Vec3::new(-4.2, -4.2, 11.0), Vec3::new(4.2, 4.2, 11.3), Skin::Bell);
  [
    Cube::span(shoulder, elbow, 3.0, Skin::Plate),
    Cube::boxed(elbow - 2.0, elbow + 2.0, Skin::Hazard),
    Cube::span(elbow, wrist, 3.0, Skin::Trim),
    Cube::boxed(wrist - 2.0, wrist + 2.0, Skin::Iron)
  ]
  .into_iter()
  .chain(flare.chain(lip).chain([bell]).map(|cube| cube.placed(wrist, bore)))
  .collect()
}

fn claw() -> Vec<Cube> {
  let shoulder = Vec3::new(11.0, 43.0, 0.0);
  let elbow = Vec3::new(17.0, 31.0, -1.0);
  let wrist = Vec3::new(17.0, 22.0, 7.0);
  let finger = |yaw: f32| {
    let out = Quat::from_rotation_y(yaw) * Vec3::new(0.0, -0.4, 1.0).normalize();
    let knuckle = wrist + out * 4.0;
    [
      Cube::span(wrist, knuckle, 1.5, Skin::Iron),
      Cube::span(
        knuckle,
        knuckle + Vec3::new(0.0, -3.5, 0.0) - out * 1.5,
        1.5,
        Skin::Iron
      )
    ]
  };
  let lantern = wrist + Vec3::new(0.0, -9.0, 3.5);
  [
    Cube::span(shoulder, elbow, 3.0, Skin::Plate),
    Cube::boxed(elbow - 2.0, elbow + 2.0, Skin::Hazard),
    Cube::span(elbow, wrist, 2.5, Skin::Trim),
    Cube::boxed(wrist - 2.0, wrist + 2.0, Skin::Iron),
    Cube::span(lantern + Vec3::Y * 3.0, lantern + Vec3::Y * 7.5, 0.6, Skin::Iron),
    Cube::boxed(
      lantern - Vec3::new(2.5, 3.0, 2.5),
      lantern + Vec3::new(2.5, 3.0, 2.5),
      Skin::Lamp
    ),
    Cube::boxed(
      lantern + Vec3::new(-3.0, 3.0, -3.0),
      lantern + Vec3::new(3.0, 4.0, 3.0),
      Skin::Trim
    ),
    Cube::boxed(
      lantern + Vec3::new(-3.0, -4.0, -3.0),
      lantern + Vec3::new(3.0, -3.0, 3.0),
      Skin::Trim
    )
  ]
  .into_iter()
  .chain([0.0, 2.1, -2.1].into_iter().flat_map(finger))
  .collect()
}

fn body() -> Vec<Cube> {
  let post = |x: f32, z: f32| {
    Cube::boxed(
      Vec3::new(x - 1.0, 34.0, z - 1.0),
      Vec3::new(x + 1.0, 44.0, z + 1.0),
      Skin::Trim
    )
  };
  let bars = [-5.0, -1.5, 2.0, 5.5].into_iter().flat_map(|along| {
    [
      Cube::boxed(
        Vec3::new(along, 34.0, 6.5),
        Vec3::new(along + 0.8, 44.0, 7.3),
        Skin::Iron
      ),
      Cube::boxed(
        Vec3::new(along, 34.0, -7.3),
        Vec3::new(along + 0.8, 44.0, -6.5),
        Skin::Iron
      ),
      Cube::boxed(
        Vec3::new(8.5, 34.0, along),
        Vec3::new(9.3, 44.0, along + 0.8),
        Skin::Iron
      ),
      Cube::boxed(
        Vec3::new(-9.3, 34.0, along),
        Vec3::new(-8.5, 44.0, along + 0.8),
        Skin::Iron
      )
    ]
  });
  let shoulder = |side: f32| {
    Cube::boxed(
      Vec3::new(side * 11.0 - 2.5, 40.5, -2.5),
      Vec3::new(side * 11.0 + 2.5, 45.5, 2.5),
      Skin::Hazard
    )
  };
  [
    Cube::boxed(Vec3::new(-11.0, 32.0, -9.0), Vec3::new(11.0, 34.0, 9.0), Skin::Plate),
    Cube::boxed(Vec3::new(-11.0, 44.0, -9.0), Vec3::new(11.0, 47.0, 9.0), Skin::Plate),
    post(-9.0, -7.0),
    post(9.0, -7.0),
    post(-9.0, 7.0),
    post(9.0, 7.0),
    shoulder(-1.0),
    shoulder(1.0),
    Cube::boxed(Vec3::new(-7.0, 33.0, -15.0), Vec3::new(7.0, 46.0, -9.0), Skin::Grille),
    Cube::boxed(Vec3::new(2.0, 46.0, -14.0), Vec3::new(5.0, 57.0, -11.0), Skin::Trim),
    Cube::boxed(Vec3::new(1.0, 57.0, -15.0), Vec3::new(6.0, 59.0, -10.0), Skin::Iron),
    Cube::boxed(Vec3::new(-2.0, 47.0, -2.0), Vec3::new(2.0, 50.0, 2.0), Skin::Iron)
  ]
  .into_iter()
  .chain(bars)
  .collect()
}

fn head() -> Vec<Cube> {
  let antenna = |side: f32| {
    Cube::span(
      Vec3::new(side * 7.0, 63.0, -3.0),
      Vec3::new(side * 12.0, 67.0, -4.0),
      0.8,
      Skin::Iron
    )
  };
  vec![
    Cube::boxed(Vec3::new(-9.0, 50.0, -6.0), Vec3::new(9.0, 63.0, 7.0), Skin::Monitor),
    Cube::boxed(Vec3::new(-10.0, 62.0, -1.0), Vec3::new(10.0, 63.5, 9.0), Skin::Plate),
    Cube::boxed(Vec3::new(-6.0, 51.0, -9.0), Vec3::new(6.0, 61.0, -6.0), Skin::Plate),
    Cube::boxed(Vec3::new(-1.0, 63.0, -1.0), Vec3::new(1.0, 72.0, 1.0), Skin::Iron),
    antenna(-1.0),
    antenna(1.0),
  ]
}

fn robot() -> Vec<Bone> {
  let bulb = |side: f32| {
    let at = Vec3::new(side * 12.0, 67.5, -4.0);
    Cube::boxed(at - 1.2, at + 1.2, Skin::Bulb)
  };
  let blade = |angle: f32| {
    Cube::boxed(Vec3::new(-13.0, 72.0, -1.5), Vec3::new(13.0, 73.2, 1.5), Skin::Plate)
      .tilted(Quat::from_rotation_y(angle) * Quat::from_rotation_x(0.25))
  };
  let core = Vec3::new(0.0, 39.0, 0.0);
  let gem = |turn: Quat, side: f32| {
    Cube::boxed(core - side, core + side, Skin::Core).tilted(turn)
  };
  vec![
    Bone {
      part: Part::Hips,
      parent: None,
      pivot: Vec3::new(0.0, 30.0, 0.0),
      cubes: leg(-1.0)
        .into_iter()
        .chain(leg(1.0))
        .chain([Cube::boxed(
          Vec3::new(-9.0, 27.0, -4.0),
          Vec3::new(9.0, 32.0, 4.0),
          Skin::Trim
        )])
        .collect()
    },
    Bone {
      part: Part::Body,
      parent: Some(Part::Hips),
      pivot: Vec3::new(0.0, 32.0, 0.0),
      cubes: body()
    },
    Bone {
      part: Part::Core,
      parent: Some(Part::Body),
      pivot: core,
      cubes: vec![
        gem(Quat::from_euler(EulerRot::XYZ, 0.62, 0.0, 0.78), 3.5),
        gem(Quat::from_euler(EulerRot::XYZ, 0.0, 0.78, 0.62), 2.5),
      ]
    },
    Bone {
      part: Part::Head,
      parent: Some(Part::Body),
      pivot: Vec3::new(0.0, 50.0, 0.0),
      cubes: head()
    },
    Bone {
      part: Part::Rotor,
      parent: Some(Part::Head),
      pivot: Vec3::new(0.0, 72.0, 0.0),
      cubes: vec![
        blade(0.0),
        blade(std::f32::consts::FRAC_PI_2),
        Cube::boxed(Vec3::new(-2.0, 71.5, -2.0), Vec3::new(2.0, 73.5, 2.0), Skin::Trim),
      ]
    },
    Bone {
      part: Part::Beacon,
      parent: Some(Part::Head),
      pivot: Vec3::new(0.0, 67.5, -4.0),
      cubes: vec![bulb(-1.0), bulb(1.0)]
    },
    Bone {
      part: Part::Horn,
      parent: Some(Part::Body),
      pivot: Vec3::new(-11.0, 43.0, 0.0),
      cubes: horn()
    },
    Bone {
      part: Part::Claw,
      parent: Some(Part::Body),
      pivot: Vec3::new(11.0, 43.0, 0.0),
      cubes: claw()
    },
  ]
}

fn srgb(hex: u32) -> Srgba {
  Srgba::rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const SMILE: [&str; 9] = [
  "..............",
  "..+##+..+##+..",
  "..#*#+..#*#+..",
  "..###+..###+..",
  "..+##+..+##+..",
  "..............",
  "....#....#....",
  ".....####.....",
  ".............."
];

struct Texel {
  tone: Srgba,
  glow: Srgba
}

impl Texel {
  fn flat(tone: Srgba) -> Self { Self { tone, glow: Srgba::BLACK } }

  fn lit(tone: Srgba) -> Self { Self { tone, glow: tone } }
}

fn paint(skin: Skin, face: Face, at: UVec2, size: UVec2, seed: UVec2) -> Texel {
  let (UVec2 { x: i, y: j }, UVec2 { x: w, y: h }) = (at, size);
  let edge = i == 0 || j == 0 || i + 1 == w || j + 1 == h;
  let speck = hash((seed.x + i) as i32, (seed.y + j) as i32, 11);
  let side = !matches!(face, Face::Top | Face::Bottom);
  let shades = |[dark, base, light, fleck]: [u32; 4]| {
    srgb(match () {
      _ if edge => dark,
      _ if speck < 0.07 => fleck,
      _ if speck > 0.93 || (j == 1 && side) => light,
      _ => base
    })
  };
  let teal = [0x1f4a4a, 0x3f8f86, 0x6cc0ae, 0x2f6f69];
  let brass = [0x6e4f1e, 0xc9a24a, 0xf0d888, 0xa47d34];
  let iron = [0x1c1e24, 0x4a4e57, 0x7a808c, 0x3a3d45];
  let cream = [0x6b624c, 0xe3d6b8, 0xfff4dc, 0xc7b896];
  let moss = face == Face::Top
    && hash(((seed.x + i) / 2) as i32, ((seed.y + j) / 2) as i32, 5) < 0.3;
  let rust =
    side && !edge && j > h / 2 && hash((seed.x + i) as i32, seed.y as i32, 9) < 0.12;
  let rivet = w >= 6 && h >= 6 && (i == 1 || i + 2 == w) && (j == 1 || j + 2 == h);
  let middle = (Vec2::new(i as f32 + 0.5, j as f32 + 0.5) - size.as_vec2() / 2.0).abs();
  match skin {
    Skin::Plate if rivet => Texel::flat(srgb(0xb8e8d8)),
    Skin::Plate if moss && !edge => {
      Texel::flat(srgb(if speck < 0.5 { 0x5f8a32 } else { 0x7aa63e }))
    }
    Skin::Plate if rust => {
      Texel::flat(srgb(if speck < 0.5 { 0x8a4e2c } else { 0xa8643a }))
    }
    Skin::Plate => Texel::flat(shades(teal)),
    Skin::Grille
      if side
        && !edge
        && i >= 2
        && j >= 2
        && i + 2 < w
        && j + 2 < h
        && i % 2 == 0
        && j % 2 == 0 =>
    {
      Texel::lit(srgb(if speck < 0.4 { 0x3a1c08 } else { 0x7a2e0a }))
    }
    Skin::Grille => Texel::flat(shades(teal)),
    Skin::Trim => Texel::flat(shades(brass)),
    Skin::Iron => Texel::flat(shades(iron)),
    Skin::Rubber => Texel::flat(srgb(if edge {
      0x101012
    } else if (i + j) % 3 == 0 {
      0x1e1e22
    } else {
      0x2c2c32
    })),
    Skin::Hazard if edge => Texel::flat(srgb(0x15151a)),
    Skin::Hazard => {
      Texel::flat(srgb(if (i + j) / 2 % 2 == 0 { 0xf2c230 } else { 0x26262c }))
    }
    Skin::Core => {
      let ring = middle.max_element() as u32;
      Texel::lit(srgb(
        [0xfff4b0, 0xffd060, 0xffa030, 0xff6a20, 0xd84010][ring.min(4) as usize]
      ))
    }
    Skin::Bulb => Texel::lit(srgb(if i == 0 && j == 0 { 0xffd0d0 } else { 0xff3030 })),
    Skin::Lamp if edge || !side => Texel::flat(shades(iron)),
    Skin::Lamp => {
      let flame = middle.x < 1.0 && j + 2 >= h / 2 && j + 2 < h;
      Texel::lit(srgb(if flame {
        0xfff0a0
      } else if speck < 0.5 {
        0xffa040
      } else {
        0xf08a30
      }))
    }
    Skin::Horn if side && j % 2 == 0 && !edge => Texel::flat(srgb(0xa47d34)),
    Skin::Horn => Texel::flat(shades(brass)),
    Skin::Bell => {
      let depth = middle.max_element() / (size.max_element() as f32 / 2.0);
      Texel::flat(srgb(match depth {
        depth if depth > 0.8 => 0x6e4f1e,
        depth if depth > 0.5 => 0x3a2810,
        _ => 0x120c06
      }))
    }
    Skin::Monitor
      if face == Face::Front && i >= 2 && j >= 2 && i + 2 < w && j + 2 < h =>
    {
      let (x, y) = ((i - 2) as usize, (j - 2) as usize);
      let (sw, sh) = ((w - 4) as usize, (h - 4) as usize);
      let pixel = SMILE[(y * SMILE.len() / sh).min(SMILE.len() - 1)].as_bytes()
        [(x * SMILE[0].len() / sw).min(SMILE[0].len() - 1)];
      let corner = (x == 0 || x + 1 == sw) && (y == 0 || y + 1 == sh);
      match pixel {
        b'*' => Texel::lit(srgb(0xf0fff0)),
        b'#' => Texel::lit(srgb(0x8dffa8)),
        b'+' => Texel::lit(srgb(0x2fd070)),
        _ if corner => Texel::flat(srgb(0x050c08)),
        _ => Texel::lit(srgb(if y % 2 == 0 { 0x0c2a1a } else { 0x081a10 }))
      }
    }
    Skin::Monitor
      if matches!(face, Face::Left | Face::Right)
        && !edge
        && j >= 3
        && j + 3 < h
        && j % 2 == 1
        && i >= 2
        && i + 2 < w =>
    {
      Texel::flat(srgb(0x3c3628))
    }
    Skin::Monitor if face == Face::Front && (i == 1 || j == 1) && !edge => {
      Texel::flat(srgb(0xfff4dc))
    }
    Skin::Monitor => Texel::flat(shades(cream))
  }
}

struct Atlas {
  tone: Vec<[u8; 4]>,
  glow: Vec<[u8; 4]>,
  height: u32
}

fn pack(nets: &[[u32; 3]]) -> (Vec<UVec2>, u32) {
  let (spots, cursor, row) = nets.iter().fold(
    (Vec::new(), UVec2::ZERO, 0),
    |(mut spots, cursor, row), &[w, h, d]| {
      let net = UVec2::new(2 * (w + d), d + h);
      let wrapped = cursor.x + net.x > ATLAS;
      let at = if wrapped { UVec2::new(0, cursor.y + row) } else { cursor };
      spots.push(at);
      (spots, at + UVec2::X * net.x, if wrapped { net.y } else { row.max(net.y) })
    }
  );
  (spots, (cursor.y + row).next_power_of_two())
}

fn painted(cubes: &[Cube], spots: &[UVec2], height: u32) -> Atlas {
  let blank = vec![[0, 0, 0, 255]; (ATLAS * height) as usize];
  let texels = cubes.iter().zip(spots).flat_map(|(cube, &spot)| {
    Face::ALL.into_iter().flat_map(move |face| {
      let rect = face.rect(cube.net());
      let size = rect.size();
      (0..size.y).flat_map(move |j| {
        (0..size.x).map(move |i| {
          let at = UVec2::new(i, j);
          (spot + rect.min + at, paint(cube.skin, face, at, size, spot + rect.min))
        })
      })
    })
  });
  let bytes = |color: Srgba| color.to_u8_array();
  let (tone, glow) =
    texels.fold((blank.clone(), blank), |(mut tone, mut glow), (at, texel)| {
      let index = (at.y * ATLAS + at.x) as usize;
      tone[index] = bytes(texel.tone);
      glow[index] = bytes(texel.glow);
      (tone, glow)
    });
  Atlas { tone, glow, height }
}

fn image(texels: &[[u8; 4]], height: u32) -> Image {
  let mut image = Image::new(
    Extent3d { width: ATLAS, height, depth_or_array_layers: 1 },
    TextureDimension::D2,
    texels.concat(),
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::RENDER_WORLD
  );
  image.sampler = ImageSampler::nearest();
  image
}

fn mesh(cubes: &[(Cube, UVec2)], pivot: Vec3, height: u32) -> Mesh {
  let scale = Vec2::new(ATLAS as f32, height as f32);
  let quads: Vec<_> = cubes
    .iter()
    .flat_map(|&(cube, spot)| {
      Face::ALL.into_iter().map(move |face| {
        let (normal, right, up) = face.axes();
        let rect = face.rect(cube.net());
        let half = cube.size / 2.0;
        let corners =
          [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(s, t)| {
            let local = (normal + right * s + up * t) * half;
            let uv = (spot + rect.min).as_vec2()
              + rect.size().as_vec2() * Vec2::new((s + 1.0) / 2.0, (1.0 - t) / 2.0);
            ((cube.turn * local + cube.center - pivot) * PX, uv / scale)
          });
        (corners, cube.turn * normal)
      })
    })
    .collect();
  let positions: Vec<Vec3> =
    quads.iter().flat_map(|(corners, _)| corners.map(|(at, _)| at)).collect();
  let uvs: Vec<Vec2> =
    quads.iter().flat_map(|(corners, _)| corners.map(|(_, uv)| uv)).collect();
  let normals: Vec<Vec3> = quads.iter().flat_map(|&(_, normal)| [normal; 4]).collect();
  let indices = (0..quads.len() as u32)
    .flat_map(|quad| [0, 1, 2, 0, 2, 3].map(|corner| quad * 4 + corner))
    .collect();
  Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[derive(Component)]
struct Idle {
  part: Part,
  rest: Vec3
}

fn spawn_robot(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  images: &mut Assets<Image>,
  materials: &mut Assets<StandardMaterial>,
  placed: Transform
) -> Entity {
  let bones = robot();
  let cubes: Vec<Cube> = bones.iter().flat_map(|bone| bone.cubes.clone()).collect();
  let (spots, height) = pack(&cubes.iter().map(Cube::net).collect::<Vec<_>>());
  let Atlas { tone, glow, height } = painted(&cubes, &spots, height);
  let material = materials.add(StandardMaterial {
    base_color_texture: Some(images.add(image(&tone, height))),
    emissive_texture: Some(images.add(image(&glow, height))),
    emissive: LinearRgba::rgb(6.0, 6.0, 6.0),
    perceptual_roughness: 0.75,
    ..default()
  });
  let root = commands.spawn((placed, Visibility::default())).id();
  let pivot = |part: Option<Part>| {
    part
      .and_then(|part| bones.iter().find(|bone| bone.part == part))
      .map_or(Vec3::ZERO, |bone| bone.pivot)
  };
  bones.iter().fold((0, Vec::<(Part, Entity)>::new()), |(first, mut entities), bone| {
    let count = bone.cubes.len();
    let placed: Vec<(Cube, UVec2)> = bone
      .cubes
      .iter()
      .copied()
      .zip(spots[first..first + count].iter().copied())
      .collect();
    let rest = (bone.pivot - pivot(bone.parent)) * PX;
    let parent = bone
      .parent
      .and_then(|parent| entities.iter().find(|(part, _)| *part == parent))
      .map_or(root, |&(_, entity)| entity);
    let entity = commands
      .spawn((
        Mesh3d(meshes.add(mesh(&placed, bone.pivot, height))),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(rest),
        Idle { part: bone.part, rest },
        ChildOf(parent)
      ))
      .id();
    entities.push((bone.part, entity));
    (first + count, entities)
  });
  root
}

fn idle(time: Res<Time>, mut bones: Query<(&Idle, &mut Transform, &mut Visibility)>) {
  let now = time.elapsed_secs();
  for (&Idle { part, rest }, mut transform, mut visibility) in bones.iter_mut() {
    let (lift, turn) = match part {
      Part::Body => (
        Vec3::Y * (now * 2.2).sin() * 0.6,
        Quat::from_rotation_z((now * 1.1).sin() * 0.03)
      ),
      Part::Core => (
        Vec3::Y * (now * 1.7).sin() * 1.2,
        Quat::from_rotation_y(now * 1.3) * Quat::from_rotation_x(now * 0.7)
      ),
      Part::Head => (
        Vec3::ZERO,
        Quat::from_rotation_z((now * 0.6).sin() * 0.18)
          * Quat::from_rotation_y((now * 0.37).sin() * 0.3)
      ),
      Part::Rotor => (Vec3::ZERO, Quat::from_rotation_y(now * 2.5)),
      Part::Horn => (Vec3::ZERO, Quat::from_rotation_x((now * 0.9).sin() * 0.12)),
      Part::Claw => (Vec3::ZERO, Quat::from_rotation_x((now * 0.9 + 1.5).sin() * 0.1)),
      Part::Hips | Part::Beacon => (Vec3::ZERO, Quat::IDENTITY)
    };
    *transform = Transform::from_translation(rest + lift * PX).with_rotation(turn);
    *visibility = match part {
      Part::Beacon if now.fract() > 0.6 => Visibility::Hidden,
      _ => Visibility::Inherited
    };
  }
}

const NAME: &str = "Brasswick";
const PELTS: u32 = 3;

#[derive(Component)]
struct Questgiver;

#[derive(Resource, Default, Clone, Copy, PartialEq)]
enum Errand {
  #[default]
  Unasked,
  Asked {
    carried: u32
  },
  Done
}

fn stand(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  ground: Res<Ground>
) {
  let facing = place::START_FACING.normalize();
  let spot = place::START + facing * 5.0 + facing.perp() * 1.5;
  let toward = (place::START - spot).normalize().extend(0.0).xzy();
  let at = ground.surface(spot);
  let robot = spawn_robot(
    &mut commands,
    &mut meshes,
    &mut images,
    &mut materials,
    Transform::from_translation(at)
      .with_rotation(Quat::from_rotation_arc(Vec3::Z, toward))
  );
  commands.entity(robot).insert((Name::new(NAME), Questgiver));
  commands.spawn((
    RigidBody::Static,
    Collider::cylinder(0.75, 3.6),
    Transform::from_translation(at + Vec3::Y * 1.8)
  ));
}

fn heed(
  time: Res<Time>,
  player: Single<&Transform, (With<Player>, Without<Questgiver>)>,
  mut robots: Query<&mut Transform, With<Questgiver>>
) {
  for mut robot in robots.iter_mut() {
    let gap = (player.translation - robot.translation).with_y(0.0);
    if gap.length() < 12.0
      && let Ok(toward) = Dir3::new(gap)
    {
      let goal = Quat::from_rotation_arc(Vec3::Z, toward.as_vec3());
      robot.rotation = robot.rotation.slerp(goal, (time.delta_secs() * 2.0).min(1.0))
    }
  }
}

fn tally(
  mut errand: ResMut<Errand>,
  mut notices: MessageWriter<Notice>,
  inventory: Single<&Inventory, With<Player>>
) {
  let held = inventory.holding(Item::WolfPelt).min(PELTS);
  if let Errand::Asked { carried } = *errand
    && held != carried
  {
    *errand = Errand::Asked { carried: held };
    let progress = match held {
      PELTS => format!("Return the wolf pelts to {NAME}"),
      _ => format!("Wolf pelts gathered: {held}/{PELTS}")
    };
    (held > carried).then(|| notices.write(Notice(progress)));
  }
}

fn converse(
  keys: Res<ButtonInput<KeyCode>>,
  view: Res<View>,
  mut prompt: ResMut<Prompt>,
  mut errand: ResMut<Errand>,
  mut notices: MessageWriter<Notice>,
  mut sounds: MessageWriter<Sound>,
  mut player: Single<(&Transform, &mut Inventory), With<Player>>,
  robots: Query<&Transform, With<Questgiver>>
) {
  let at = player.0.translation;
  let near = robots.iter().find(|robot| {
    let gap = (robot.translation - at).with_y(0.0);
    gap.length() < 5.5 && view.flat_forward().dot(gap.normalize_or_zero()) > 0.4
  });
  if prompt.0.is_none()
    && let Some(robot) = near
  {
    prompt.0 = Some(Prompting { verb: "Talk".into(), noun: NAME.into() });
    if keys.just_pressed(KeyCode::KeyE) {
      let say = |line: &str| Notice(format!("{NAME}: \"{line}\""));
      let held = player.1.holding(Item::WolfPelt);
      let (lines, next): (Vec<Notice>, Errand) = match *errand {
        Errand::Unasked => (
          vec![
            say("BZZT. Greetings, flesh-traveller! I am Brasswick, keeper of the lamps."),
            say(
              "The northern wind chills my ember core. Bring me three wolf pelts to line my cage."
            ),
            Notice("Quest started: Fur for the Furnace".into()),
          ],
          Errand::Asked { carried: held.min(PELTS) }
        ),
        Errand::Asked { .. } if held >= PELTS => {
          (0..PELTS).for_each(|_| {
            player.1.spend(Item::WolfPelt);
          });
          [Loot::Gold(150), Loot::one(Item::Amethyst)]
            .into_iter()
            .for_each(|loot| player.1.take(loot));
          sounds.write(Sound::here(Cue::Coins, robot.translation));
          (
            vec![
              say("Warmth! Glorious warmth! My core burns bright once more."),
              say("Take this, with the gratitude of a grateful machine."),
              Notice("Quest completed: Fur for the Furnace".into()),
              Notice("Gold (150) added".into()),
              Notice("Amethyst added".into()),
            ],
            Errand::Done
          )
        }
        Errand::Asked { carried } => (
          vec![say(&format!(
            "Wolf pelts: {held}/{PELTS}. The wolves prowl the woods around this valley. Whirr."
          ))],
          Errand::Asked { carried }
        ),
        Errand::Done => (
          vec![say(
            "My core is warm and my lamps are lit. Safe travels, flesh-traveller."
          )],
          Errand::Done
        )
      };
      *errand = next;
      notices.write_batch(lines);
    }
  }
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Errand>()
    .add_systems(PostStartup, stand)
    .add_systems(Update, (idle, heed, tally, converse));
}
