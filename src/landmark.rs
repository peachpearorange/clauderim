use {crate::{model::{self, Piece, ball, block, lathe, lump, rod},
             noise::Roll,
             place::Place,
             player::{Player, View},
             signal::{Cue, Notice, Prompt, Prompting, Sound},
             stuff::{Stuff, Stuffs},
             terrain::Ground},
     avian3d::prelude::*,
     bevy::{color::Mix, light::NotShadowCaster, prelude::*},
     std::f32::consts::{FRAC_PI_2, PI, TAU}};

const TOWER_RADIUS: f32 = 4.6;
const TOWER_COURSE: f32 = 0.55;
const TOWER_COURSES: usize = 24;
const TOWER_WALL: f32 = 0.9;
const TOWER_DOOR: f32 = 0.0;

fn srgb(red: f32, green: f32, blue: f32) -> Srgba { Srgba::new(red, green, blue, 1.0) }

fn weathered(piece: Piece, base: Srgba, roll: &mut Roll) -> Piece {
  let tone = roll.range(0.78, 1.1);
  let moss = roll.chance(0.25);
  let (base, lichen) =
    (LinearRgba::from(base * tone), LinearRgba::from(srgb(0.34, 0.38, 0.2)));
  piece.shaded(move |position, normal| {
    let grime = crate::noise::value3(position * 1.7, 5) * 0.25;
    let top = (normal.y * 1.5 - 0.5).clamp(0.0, 1.0) * moss as u8 as f32;
    (base * (0.85 + grime)).mix(&lichen, top * 0.7).with_alpha(1.0)
  })
}

fn spawn_static(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  name: &str,
  origin: Vec3,
  turn: f32,
  parts: Vec<(Stuff, Vec<Piece>)>,
  solid: bool
) {
  let root = commands
    .spawn((
      Name::new(name.to_string()),
      Transform::from_translation(origin).with_rotation(Quat::from_rotation_y(turn)),
      Visibility::Inherited
    ))
    .id();
  parts.into_iter().filter(|(_, pieces)| !pieces.is_empty()).for_each(
    |(stuff, pieces)| {
      let mesh = model::merge(pieces);
      let mut part = commands.spawn((
        Mesh3d(meshes.add(mesh.clone())),
        MeshMaterial3d(stuffs.of(stuff)),
        ChildOf(root)
      ));
      if solid && !matches!(stuff, Stuff::Ember | Stuff::Cloth | Stuff::Fur) {
        if let Some(collider) = Collider::trimesh_from_mesh(&mesh) {
          part.insert((RigidBody::Static, collider));
        }
      }
      if matches!(stuff, Stuff::Ember) {
        part.insert(NotShadowCaster);
      }
    }
  );
}

fn watchtower(roll: &mut Roll) -> Vec<(Stuff, Vec<Piece>)> {
  let granite = srgb(0.56, 0.55, 0.52);
  let ruin_height = |angle: f32| {
    let broken = (angle * 2.0 + 0.7).sin() * 0.5 + 0.5;
    TOWER_COURSES as f32 - broken * 7.0
  };
  let door_half = 0.32;
  let stones: Vec<Piece> = (0..TOWER_COURSES)
    .flat_map(|course| {
      let blocks = 22;
      let offset = (course % 2) as f32 * 0.5;
      (0..blocks)
        .map(move |block| (course, (block as f32 + offset) / blocks as f32 * TAU))
    })
    .filter(|&(course, angle)| {
      let doorway = course < 5
        && (angle - TOWER_DOOR).sin().abs() < door_half
        && (angle - TOWER_DOOR).cos() > 0.0;
      (course as f32) < ruin_height(angle) && !doorway
    })
    .map(|(course, angle)| {
      let size = Vec3::new(
        TOWER_WALL * roll.range(0.9, 1.05),
        TOWER_COURSE * 0.96,
        TAU * TOWER_RADIUS / 22.0 * roll.range(0.92, 1.0)
      );
      let jitter = roll.spread(0.05);
      let piece =
        Piece::new(block(size.x, size.y, size.z), granite).yawed(-angle).at(Vec3::new(
          angle.cos() * (TOWER_RADIUS + jitter),
          course as f32 * TOWER_COURSE + TOWER_COURSE / 2.0,
          angle.sin() * (TOWER_RADIUS + jitter)
        ));
      weathered(piece, granite, roll)
    })
    .collect();
  let lintel = weathered(
    Piece::new(block(TOWER_WALL * 1.1, 0.5, 2.3), granite * 0.9).at_xyz(
      TOWER_RADIUS,
      5.0 * TOWER_COURSE + 0.25,
      0.0
    ),
    granite,
    roll
  );
  let rubble: Vec<Piece> = (0..26)
    .map(|index| {
      let angle = roll.range(0.0, TAU);
      let reach = TOWER_RADIUS + roll.range(0.8, 4.5);
      let size = roll.range(0.25, 0.7);
      let chunk = Piece::new(lump(index + 40, 0.25, 1), granite)
        .sized(Vec3::new(size * 1.3, size * 0.7, size))
        .yawed(roll.range(0.0, TAU))
        .at_xyz(angle.cos() * reach, size * 0.25, angle.sin() * reach);
      weathered(chunk, granite, roll)
    })
    .collect();
  let floor = Piece::new(model::rod(TOWER_RADIUS - 0.3, 0.2), srgb(0.4, 0.37, 0.33))
    .at_xyz(0.0, 0.1, 0.0);
  let timber = srgb(0.36, 0.26, 0.17);
  let beams: Vec<Piece> = (0..5)
    .map(|index| {
      let across = -2.8 + index as f32 * 1.4;
      Piece::new(
        block(
          0.22,
          0.22,
          2.0 * (TOWER_RADIUS * TOWER_RADIUS - across * across).sqrt() - 0.6
        ),
        timber
      )
      .at_xyz(across, 6.2, 0.0)
    })
    .collect();
  let planks: Vec<Piece> = (0..9)
    .filter(|index| index % 4 != 2)
    .map(|index| {
      let along = -3.4 + index as f32 * 0.8;
      Piece::new(
        block(
          2.0 * (TOWER_RADIUS * TOWER_RADIUS - along * along).max(0.5).sqrt() - 0.8,
          0.07,
          0.72
        ),
        timber * roll.range(0.8, 1.1)
      )
      .yawed(roll.spread(0.03))
      .at_xyz(0.0, 6.35, along)
    })
    .collect();
  let steps: Vec<Piece> = (0..14)
    .map(|step| {
      let angle = PI * 0.6 + step as f32 * 0.19;
      let reach = TOWER_RADIUS - 1.1;
      Piece::new(block(1.5, 0.12, 0.45), timber).yawed(-angle).at_xyz(
        angle.cos() * reach,
        0.35 + step as f32 * 0.43,
        angle.sin() * reach
      )
    })
    .collect();
  let banner = Piece::new(block(0.9, 2.4, 0.03), srgb(0.45, 0.1, 0.08))
    .at_xyz(-0.2, 9.4, -TOWER_RADIUS - 0.5)
    .yawed(PI);
  let pole = Piece::new(rod(0.05, 3.2), timber).rolled(FRAC_PI_2).at_xyz(
    0.0,
    10.6,
    -TOWER_RADIUS - 0.5
  );
  vec![
    (Stuff::Stone, stones.into_iter().chain([lintel]).chain(rubble).collect()),
    (Stuff::Stone, vec![floor]),
    (Stuff::Wood, beams.into_iter().chain(planks).chain(steps).chain([pole]).collect()),
    (Stuff::Cloth, vec![banner]),
  ]
}

fn tent(roll: &mut Roll) -> Vec<(Stuff, Piece)> {
  let hide = srgb(0.62, 0.52, 0.38) * roll.range(0.85, 1.05);
  let pole = srgb(0.34, 0.25, 0.16);
  let (length, half_width, height): (f32, f32, f32) = (3.6, 1.5, 1.9);
  let slope = (half_width * half_width + height * height).sqrt();
  let lean = f32::atan2(half_width, height);
  let sheet = |side: f32| {
    Piece::new(block(0.04, slope, length), hide).rolled(side * lean).at_xyz(
      side * half_width / 2.0,
      height / 2.0,
      0.0
    )
  };
  let frame = |z: f32| {
    [
      Piece::new(rod(0.05, slope + 0.3), pole).rolled(lean).at_xyz(
        half_width / 2.0,
        height / 2.0,
        z
      ),
      Piece::new(rod(0.05, slope + 0.3), pole).rolled(-lean).at_xyz(
        -half_width / 2.0,
        height / 2.0,
        z
      )
    ]
  };
  let ridge =
    Piece::new(rod(0.05, length + 0.6), pole).pitched(FRAC_PI_2).at_xyz(0.0, height, 0.0);
  let bedroll = Piece::new(rod(0.22, 1.9), srgb(0.5, 0.2, 0.15))
    .pitched(FRAC_PI_2)
    .sized(Vec3::new(1.0, 0.45, 1.0))
    .at_xyz(0.3, 0.1, 0.0);
  let pelt = Piece::new(lump(roll.below(50) as u32, 0.1, 2), srgb(0.5, 0.42, 0.34))
    .sized(Vec3::new(0.8, 0.05, 1.1))
    .at_xyz(-0.5, 0.04, 0.2);
  [
    (Stuff::Leather, sheet(1.0)),
    (Stuff::Leather, sheet(-1.0)),
    (Stuff::Wood, ridge),
    (Stuff::Cloth, bedroll),
    (Stuff::Fur, pelt)
  ]
  .into_iter()
  .chain(frame(length / 2.0).map(|piece| (Stuff::Wood, piece)))
  .chain(frame(-length / 2.0).map(|piece| (Stuff::Wood, piece)))
  .collect()
}

pub fn campfire() -> Vec<(Stuff, Piece)> {
  let stone = srgb(0.45, 0.44, 0.42);
  let ring = (0..9).map(|index| {
    let angle = index as f32 / 9.0 * TAU;
    (
      Stuff::Stone,
      Piece::new(lump(index + 60, 0.2, 1), stone)
        .sized(Vec3::new(0.2, 0.14, 0.18))
        .at_xyz(angle.cos() * 0.62, 0.06, angle.sin() * 0.62)
    )
  });
  let logs = (0..4).map(|index| {
    let angle = index as f32 / 4.0 * TAU + 0.4;
    let foot = Vec3::new(angle.cos() * 0.45, 0.05, angle.sin() * 0.45);
    (
      Stuff::Bark,
      Piece::new(rod(0.07, 1.0), srgb(0.2, 0.15, 0.1))
        .span(foot, Vec3::new(0.0, 0.42, 0.0))
    )
  });
  let embers = (
    Stuff::Ember,
    Piece::new(lump(77, 0.3, 1), srgb(1.0, 0.5, 0.2))
      .sized(Vec3::new(0.35, 0.08, 0.35))
      .at_xyz(0.0, 0.05, 0.0)
  );
  ring.chain(logs).chain([embers]).collect()
}

fn camp(roll: &mut Roll) -> Vec<(Stuff, Vec<Piece>)> {
  let placed = |pieces: Vec<(Stuff, Piece)>, at: Vec3, turn: f32| {
    pieces
      .into_iter()
      .map(move |(stuff, piece)| (stuff, piece.yawed(turn).at(at)))
      .collect::<Vec<_>>()
  };
  let timber = srgb(0.38, 0.28, 0.18);
  let stakes: Vec<(Stuff, Piece)> = (0..30)
    .filter(|index| !(12..16).contains(index))
    .map(|index| {
      let angle = index as f32 / 30.0 * TAU * 0.75 + 0.8;
      let reach = 10.5 + roll.spread(0.2);
      let height = roll.range(2.2, 2.9);
      let stake =
        Piece::new(model::cone(0.14, 0.5), timber).at_xyz(0.0, height / 2.0 + 0.25, 0.0);
      let post = Piece::new(rod(0.14, height), timber * roll.range(0.8, 1.1));
      (
        Stuff::Wood,
        model::Piece(model::merge([post, stake])).rolled(roll.spread(0.08)).at_xyz(
          angle.cos() * reach,
          height / 2.0 - 0.2,
          angle.sin() * reach
        )
      )
    })
    .collect();
  let crates: Vec<(Stuff, Piece)> = (0..5)
    .map(|index| {
      let at = Vec3::new(-6.0 + index as f32 * 0.9, 0.35, 5.5 + roll.spread(0.4));
      (
        Stuff::Wood,
        Piece::new(block(0.7, 0.7, 0.7), timber * 1.2)
          .yawed(roll.spread(0.4))
          .at(at + Vec3::Y * (index == 2) as u8 as f32 * 0.7)
      )
    })
    .collect();
  let barrels: Vec<(Stuff, Piece)> = (0..3)
    .flat_map(|index| {
      let at = Vec3::new(5.5 + index as f32 * 0.85, 0.0, 4.5);
      let staves = lathe(
        &[
          Vec2::new(0.0, 0.0),
          Vec2::new(0.32, 0.0),
          Vec2::new(0.38, 0.45),
          Vec2::new(0.32, 0.9),
          Vec2::new(0.0, 0.9)
        ],
        14
      );
      [
        (Stuff::Wood, Piece::new(staves, timber * 1.1).at(at)),
        (
          Stuff::Iron,
          Piece::new(rod(0.385, 0.06), srgb(0.3, 0.3, 0.3)).at(at + Vec3::Y * 0.25)
        ),
        (
          Stuff::Iron,
          Piece::new(rod(0.385, 0.06), srgb(0.3, 0.3, 0.3)).at(at + Vec3::Y * 0.65)
        )
      ]
    })
    .collect();
  let spit = [
    (Stuff::Wood, Piece::new(rod(0.035, 1.3), timber).at_xyz(0.9, 0.65, 0.0)),
    (Stuff::Wood, Piece::new(rod(0.035, 1.3), timber).at_xyz(-0.9, 0.65, 0.0)),
    (
      Stuff::Wood,
      Piece::new(rod(0.03, 2.0), timber).rolled(FRAC_PI_2).at_xyz(0.0, 1.25, 0.0)
    ),
    (
      Stuff::Skin,
      Piece::new(ball(1.0), srgb(0.45, 0.25, 0.15))
        .sized(Vec3::new(0.35, 0.16, 0.16))
        .at_xyz(0.0, 1.15, 0.0)
    )
  ];
  let all: Vec<(Stuff, Piece)> = placed(tent(roll), Vec3::new(-5.0, 0.0, -3.0), 0.5)
    .into_iter()
    .chain(placed(tent(roll), Vec3::new(1.0, 0.0, -6.0), -0.3))
    .chain(placed(tent(roll), Vec3::new(6.0, 0.0, -1.5), -1.2))
    .chain(campfire())
    .chain(spit)
    .chain(stakes)
    .chain(crates)
    .chain(barrels)
    .collect();
  group(all)
}

fn group(pieces: Vec<(Stuff, Piece)>) -> Vec<(Stuff, Vec<Piece>)> {
  pieces.into_iter().fold(
    Vec::new(),
    |mut groups: Vec<(Stuff, Vec<Piece>)>, (stuff, piece)| {
      match groups.iter_mut().find(|(each, _)| *each == stuff) {
        Some((_, list)) => list.push(piece),
        None => groups.push((stuff, vec![piece]))
      }
      groups
    }
  )
}

fn standing_stones(roll: &mut Roll) -> Vec<(Stuff, Vec<Piece>)> {
  let granite = srgb(0.5, 0.5, 0.48);
  let monolith = |seed: u32, height: f32, roll: &mut Roll| {
    let slab = weathered(
      Piece::new(lump(seed, 0.08, 2), granite)
        .sized(Vec3::new(0.9, height / 2.0, 0.55))
        .at_xyz(0.0, height / 2.0 - 0.3, 0.0),
      granite,
      roll
    );
    let ring = Piece::new(
      model::tube(
        &(0..=24)
          .map(|step| {
            let angle = step as f32 / 24.0 * TAU;
            Vec3::new(angle.cos() * 0.5, angle.sin() * 0.5, 0.0)
          })
          .collect::<Vec<_>>(),
        &[0.09],
        6
      ),
      granite * 0.8
    )
    .at_xyz(0.0, height - 0.2, 0.0);
    let glyph = Piece::new(block(0.12, 0.8, 0.06), srgb(0.6, 0.85, 1.0)).at_xyz(
      0.0,
      height * 0.55,
      0.56
    );
    let bar = Piece::new(block(0.5, 0.1, 0.06), srgb(0.6, 0.85, 1.0)).at_xyz(
      0.0,
      height * 0.62,
      0.56
    );
    (slab, ring, [glyph, bar])
  };
  let dais = weathered(
    Piece::new(model::rod(4.2, 0.4), granite * 0.9).at_xyz(0.0, 0.1, 0.0),
    granite,
    roll
  );
  let (stones, glows): (Vec<Piece>, Vec<Piece>) =
    [(-2.6, 0.5, 3.6), (0.0, 0.0, 4.4), (2.6, -0.5, 3.6)].into_iter().enumerate().fold(
      (vec![dais], Vec::new()),
      |(mut stones, mut glows), (index, (x, turn, height))| {
        let (slab, ring, marks) = monolith(90 + index as u32, height, roll);
        let place =
          |piece: Piece| piece.yawed(turn).at_xyz(x, 0.0, -1.0 + x.abs() * 0.35);
        stones.extend([place(slab), place(ring)]);
        glows.extend(marks.map(place));
        (stones, glows)
      }
    );
  vec![(Stuff::Stone, stones), (Stuff::Frost, glows)]
}

fn signpost() -> Vec<(Stuff, Vec<Piece>)> {
  let timber = srgb(0.4, 0.3, 0.2);
  let arrow = |turn: f32, height: f32| {
    Piece::new(block(1.1, 0.22, 0.05), timber * 1.15)
      .at_xyz(0.45, height, 0.0)
      .yawed(turn)
  };
  vec![(Stuff::Wood, vec![
    Piece::new(rod(0.08, 2.6), timber).at_xyz(0.0, 1.3, 0.0),
    arrow(0.4, 2.2),
    arrow(2.6, 1.9),
    arrow(-1.4, 1.6),
  ])]
}

#[derive(Component)]
pub struct Blessing {
  pub name: &'static str,
  pub told: bool
}

#[derive(Component)]
pub struct Flicker(f32);

fn raise_landmarks(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  ground: Res<Ground>,
  effects: Res<crate::fx::Effects>
) {
  let mut roll = Roll::new(4242);
  let floor = |place: Place| ground.surface(place.spot()) - Vec3::Y * 0.15;
  spawn_static(
    &mut commands,
    &mut meshes,
    &stuffs,
    "Greymoor Watch",
    floor(Place::Greymoor),
    0.9,
    watchtower(&mut roll),
    true
  );
  let camp_at = floor(Place::Rotfen);
  spawn_static(
    &mut commands,
    &mut meshes,
    &stuffs,
    "Rotfen Camp",
    camp_at,
    0.3,
    camp(&mut roll),
    true
  );
  commands.spawn((
    Flicker(0.0),
    PointLight {
      color: Color::srgb(1.0, 0.62, 0.3),
      intensity: 400_000.0,
      range: 22.0,
      shadow_maps_enabled: true,
      ..default()
    },
    crate::humanoid::shadowing(),
    Transform::from_translation(camp_at + Vec3::Y * 0.8)
  ));
  commands.spawn((
    effects.emit(&effects.campfire),
    Transform::from_translation(camp_at + Vec3::Y * 0.15)
  ));
  let stone_at = floor(Place::WarriorStone);
  spawn_static(
    &mut commands,
    &mut meshes,
    &stuffs,
    "The Warrior Stone",
    stone_at,
    0.2,
    standing_stones(&mut roll),
    true
  );
  commands.spawn((
    Blessing { name: "The Warrior Stone", told: false },
    Transform::from_translation(stone_at + Vec3::Y * 1.2)
  ));
  let junction = Vec2::new(-5.0, 85.0) + Vec2::new(4.0, 3.0);
  spawn_static(
    &mut commands,
    &mut meshes,
    &stuffs,
    "Signpost",
    ground.surface(junction),
    0.3,
    signpost(),
    true
  );
}

fn flicker(time: Res<Time>, mut lights: Query<(&mut Flicker, &mut PointLight)>) {
  lights.iter_mut().for_each(|(mut flicker, mut light)| {
    flicker.0 += time.delta_secs();
    let wobble = (flicker.0 * 13.0).sin() * 0.08
      + (flicker.0 * 23.7).sin() * 0.06
      + (flicker.0 * 5.1).sin() * 0.1;
    light.intensity = 400_000.0 * (1.0 + wobble);
  });
}

fn bless(
  keys: Res<ButtonInput<KeyCode>>,
  view: Res<View>,
  mut prompt: ResMut<Prompt>,
  mut notices: MessageWriter<Notice>,
  mut sounds: MessageWriter<Sound>,
  player: Single<&Transform, With<Player>>,
  mut stones: Query<(&mut Blessing, &Transform)>
) {
  stones.iter_mut().for_each(|(mut blessing, transform)| {
    let gap = (transform.translation - player.translation).with_y(0.0);
    if gap.length() < 4.5 && view.flat_forward().dot(gap.normalize_or_zero()) > 0.4 {
      prompt.0 = Some(Prompting { verb: "Activate".into(), noun: blessing.name.into() });
      if keys.just_pressed(KeyCode::KeyE) {
        let line = blessing
          .told
          .then_some("You already have this blessing.")
          .unwrap_or("The Warrior Stone: Combat skills improve 20% faster.");
        notices.write(Notice(line.into()));
        sounds.write(Sound::flat(Cue::WordLearned));
        blessing.told = true;
      }
    }
  });
}

pub fn plugin(app: &mut App) {
  app.add_systems(Startup, raise_landmarks).add_systems(Update, (flicker, bless));
}
