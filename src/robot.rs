use {crate::{blocky::{self, Facet, Pixel, Shape, Texel},
             inventory::{Inventory, Item, Loot},
             place,
             player::{Player, View},
             signal::{Cue, Notice, Prompt, Prompting, Sound},
             terrain::Ground},
     avian3d::prelude::*,
     bevy::prelude::*};

const PX: f32 = 1.0 / 20.0;

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

type Bone = blocky::Bone<Part, Skin>;
type Cube = Shape<Skin>;

fn leg(side: f32) -> Vec<Cube> {
  let (hip, knee, ankle) = (
    Vec3::new(side * 7.0, 29.0, 0.0),
    Vec3::new(side * 8.0, 18.0, 6.0),
    Vec3::new(side * 7.0, 5.0, -3.0)
  );
  let toe = |yaw: f32, reach: f32| {
    let tip = ankle.with_y(1.0) + Quat::from_rotation_y(yaw) * Vec3::Z * reach;
    Cube::rod(ankle.with_y(1.5), tip, 2.0, Skin::Rubber)
  };
  vec![
    Cube::rod(hip, knee, 4.0, Skin::Plate),
    Cube::boxed(knee - 2.5, knee + 2.5, Skin::Hazard),
    Cube::rod(knee, ankle, 3.0, Skin::Trim),
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
    Cube::rod(shoulder, elbow, 3.0, Skin::Plate),
    Cube::boxed(elbow - 2.0, elbow + 2.0, Skin::Hazard),
    Cube::rod(elbow, wrist, 3.0, Skin::Trim),
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
      Cube::rod(wrist, knuckle, 1.5, Skin::Iron),
      Cube::rod(
        knuckle,
        knuckle + Vec3::new(0.0, -3.5, 0.0) - out * 1.5,
        1.5,
        Skin::Iron
      )
    ]
  };
  let lantern = wrist + Vec3::new(0.0, -9.0, 3.5);
  [
    Cube::rod(shoulder, elbow, 3.0, Skin::Plate),
    Cube::boxed(elbow - 2.0, elbow + 2.0, Skin::Hazard),
    Cube::rod(elbow, wrist, 2.5, Skin::Trim),
    Cube::boxed(wrist - 2.0, wrist + 2.0, Skin::Iron),
    Cube::rod(lantern + Vec3::Y * 3.0, lantern + Vec3::Y * 7.5, 0.6, Skin::Iron),
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
    Cube::rod(
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
      .turned(Quat::from_rotation_y(angle) * Quat::from_rotation_x(0.25))
  };
  let core = Vec3::new(0.0, 39.0, 0.0);
  let gem = |turn: Quat, side: f32| {
    Cube::boxed(core - side, core + side, Skin::Core).turned(turn)
  };
  vec![
    Bone {
      part: Part::Hips,
      parent: None,
      pivot: Vec3::new(0.0, 30.0, 0.0),
      shapes: leg(-1.0)
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
      shapes: body()
    },
    Bone {
      part: Part::Core,
      parent: Some(Part::Body),
      pivot: core,
      shapes: vec![
        gem(Quat::from_euler(EulerRot::XYZ, 0.62, 0.0, 0.78), 3.5),
        gem(Quat::from_euler(EulerRot::XYZ, 0.0, 0.78, 0.62), 2.5),
      ]
    },
    Bone {
      part: Part::Head,
      parent: Some(Part::Body),
      pivot: Vec3::new(0.0, 50.0, 0.0),
      shapes: head()
    },
    Bone {
      part: Part::Rotor,
      parent: Some(Part::Head),
      pivot: Vec3::new(0.0, 72.0, 0.0),
      shapes: vec![
        blade(0.0),
        blade(std::f32::consts::FRAC_PI_2),
        Cube::boxed(Vec3::new(-2.0, 71.5, -2.0), Vec3::new(2.0, 73.5, 2.0), Skin::Trim),
      ]
    },
    Bone {
      part: Part::Beacon,
      parent: Some(Part::Head),
      pivot: Vec3::new(0.0, 67.5, -4.0),
      shapes: vec![bulb(-1.0), bulb(1.0)]
    },
    Bone {
      part: Part::Horn,
      parent: Some(Part::Body),
      pivot: Vec3::new(-11.0, 43.0, 0.0),
      shapes: horn()
    },
    Bone {
      part: Part::Claw,
      parent: Some(Part::Body),
      pivot: Vec3::new(11.0, 43.0, 0.0),
      shapes: claw()
    },
  ]
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

fn paint(skin: Skin, pixel: Pixel) -> Texel {
  let Pixel { facet, at: UVec2 { x: i, y: j }, size, .. } = pixel;
  let UVec2 { x: w, y: h } = size;
  let (edge, speck, side, middle) =
    (pixel.edge(), pixel.speck(), pixel.side(), pixel.middle());
  let shades = |palette: [u32; 4]| pixel.shade(palette);
  let teal = [0x1f4a4a, 0x3f8f86, 0x6cc0ae, 0x2f6f69];
  let brass = [0x6e4f1e, 0xc9a24a, 0xf0d888, 0xa47d34];
  let iron = [0x1c1e24, 0x4a4e57, 0x7a808c, 0x3a3d45];
  let cream = [0x6b624c, 0xe3d6b8, 0xfff4dc, 0xc7b896];
  let moss = facet == Facet::Top && pixel.grain(2, 5) < 0.3;
  let rust = side && !edge && j > h / 2 && pixel.column(9) < 0.12;
  let rivet = w >= 6 && h >= 6 && (i == 1 || i + 2 == w) && (j == 1 || j + 2 == h);
  match skin {
    Skin::Plate if rivet => Texel::flat(0xb8e8d8),
    Skin::Plate if moss && !edge => {
      Texel::flat(if speck < 0.5 { 0x5f8a32 } else { 0x7aa63e })
    }
    Skin::Plate if rust => Texel::flat(if speck < 0.5 { 0x8a4e2c } else { 0xa8643a }),
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
      Texel::lit(if speck < 0.4 { 0x3a1c08 } else { 0x7a2e0a })
    }
    Skin::Grille => Texel::flat(shades(teal)),
    Skin::Trim => Texel::flat(shades(brass)),
    Skin::Iron => Texel::flat(shades(iron)),
    Skin::Rubber => Texel::flat(if edge {
      0x101012
    } else if (i + j) % 3 == 0 {
      0x1e1e22
    } else {
      0x2c2c32
    }),
    Skin::Hazard if edge => Texel::flat(0x15151a),
    Skin::Hazard => Texel::flat(if (i + j) / 2 % 2 == 0 { 0xf2c230 } else { 0x26262c }),
    Skin::Core => {
      let ring = middle.max_element() as u32;
      Texel::lit([0xfff4b0, 0xffd060, 0xffa030, 0xff6a20, 0xd84010][ring.min(4) as usize])
    }
    Skin::Bulb => Texel::lit(if i == 0 && j == 0 { 0xffd0d0 } else { 0xff3030 }),
    Skin::Lamp if edge || !side => Texel::flat(shades(iron)),
    Skin::Lamp => {
      let flame = middle.x < 1.0 && j + 2 >= h / 2 && j + 2 < h;
      Texel::lit(if flame {
        0xfff0a0
      } else if speck < 0.5 {
        0xffa040
      } else {
        0xf08a30
      })
    }
    Skin::Horn if side && j % 2 == 0 && !edge => Texel::flat(0xa47d34),
    Skin::Horn => Texel::flat(shades(brass)),
    Skin::Bell => {
      let depth = middle.max_element() / (size.max_element() as f32 / 2.0);
      Texel::flat(match depth {
        depth if depth > 0.8 => 0x6e4f1e,
        depth if depth > 0.5 => 0x3a2810,
        _ => 0x120c06
      })
    }
    Skin::Monitor
      if facet == Facet::Front && i >= 2 && j >= 2 && i + 2 < w && j + 2 < h =>
    {
      let (x, y) = ((i - 2) as usize, (j - 2) as usize);
      let (sw, sh) = ((w - 4) as usize, (h - 4) as usize);
      let pixel = SMILE[(y * SMILE.len() / sh).min(SMILE.len() - 1)].as_bytes()
        [(x * SMILE[0].len() / sw).min(SMILE[0].len() - 1)];
      let corner = (x == 0 || x + 1 == sw) && (y == 0 || y + 1 == sh);
      match pixel {
        b'*' => Texel::lit(0xf0fff0),
        b'#' => Texel::lit(0x8dffa8),
        b'+' => Texel::lit(0x2fd070),
        _ if corner => Texel::flat(0x050c08),
        _ => Texel::lit(if y % 2 == 0 { 0x0c2a1a } else { 0x081a10 })
      }
    }
    Skin::Monitor
      if matches!(facet, Facet::Left | Facet::Right)
        && !edge
        && j >= 3
        && j + 3 < h
        && j % 2 == 1
        && i >= 2
        && i + 2 < w =>
    {
      Texel::flat(0x3c3628)
    }
    Skin::Monitor if facet == Facet::Front && (i == 1 || j == 1) && !edge => {
      Texel::flat(0xfff4dc)
    }
    Skin::Monitor => Texel::flat(shades(cream))
  }
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
  let kits = blocky::kits(&robot(), PX, [paint], 6.0, meshes, images, materials);
  let (root, parts) = kits[0].spawn(commands, placed);
  for (part, entity, rest) in parts {
    commands.entity(entity).insert(Idle { part, rest });
  }
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
