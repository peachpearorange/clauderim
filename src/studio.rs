use {crate::{atronach,
             cage::Cage,
             creature, dragon,
             face::{Person, Race},
             flora,
             fx::Effects,
             humanoid::{self, Calling, Frame, Grip, Kit, MAN, Motion},
             model::Piece,
             noise,
             opts::opts,
             player::MainCamera,
             robot, sdf,
             settlement::{self, Clutter, House, Roof, Walls, Work},
             signal::FoeKind,
             stuff::{Stuff, Stuffs},
             wolf},
     bevy::{camera::primitives::Aabb,
            light::{CascadeShadowConfig, CascadeShadowConfigBuilder},
            mesh::skinning::SkinnedMeshInverseBindposes,
            prelude::*,
            transform::TransformSystems,
            window::PrimaryWindow},
     std::f32::consts::{FRAC_PI_2, PI}};

const FOV: f32 = 20.0;

enum Subject {
  Hero,
  Foe(FoeKind),
  Villager(Calling),
  Wolf,
  Dragon { aloft: bool },
  Robot,
  Work(Work),
  Growth(String),
  Cages,
  Atronach
}

impl Subject {
  fn named(name: &str, seed: u32) -> Subject {
    let (at, facing) = (Vec2::ZERO, 0.0);
    let house = |length: f32, depth: f32, tall: f32| House {
      at,
      facing,
      length,
      depth,
      tall,
      roof: [Roof::Thatch, Roof::Shingle, Roof::Gilded][seed as usize % 3],
      walls: [Walls::Timber, Walls::Logs, Walls::Stone, Walls::Jettied]
        [seed as usize / 3 % 4]
    };
    let clutter = |kind: Clutter| Subject::Work(Work::Clutter { at, facing, kind });
    match name {
      "dragonborn" => Subject::Hero,
      "wolf" => Subject::Wolf,
      "draugr" => Subject::Foe(FoeKind::Draugr),
      "overlord" => Subject::Foe(FoeKind::DraugrOverlord),
      "bandit" => Subject::Foe(FoeKind::Bandit),
      "chief" => Subject::Foe(FoeKind::BanditChief),
      "cook" => Subject::Villager(Calling::Cook),
      "huntress" => Subject::Villager(Calling::Huntress),
      "priest" => Subject::Villager(Calling::Priest),
      "farmer" => Subject::Villager(Calling::Farmer),
      "sellsword" => Subject::Villager(Calling::Barbarian),
      "dragon" => Subject::Dragon { aloft: false },
      "dragonaloft" => Subject::Dragon { aloft: true },
      "robot" => Subject::Robot,
      "cages" => Subject::Cages,
      "atronach" => Subject::Atronach,
      "house" => Subject::Work(Work::House(house(9.0, 6.5, 3.0))),
      "inn" => Subject::Work(Work::Inn(house(15.0, 9.0, 6.1))),
      "longhall" => Subject::Work(Work::Longhall(house(22.0, 11.0, 4.4))),
      "forge" => Subject::Work(Work::Forge { at, facing }),
      "temple" => Subject::Work(Work::Temple { at, facing }),
      "keep" => Subject::Work(Work::Keep { at, facing }),
      "windmill" => Subject::Work(Work::Windmill { at, facing }),
      "mill" => Subject::Work(Work::Mill { at, facing }),
      "stable" => Subject::Work(Work::Stable { at, facing }),
      "horse" => Subject::Work(Work::Horse { at, facing }),
      "tent" => Subject::Work(Work::Tent { at, facing }),
      "shrine" => Subject::Work(Work::Shrine { at, facing }),
      "portal" => Subject::Work(Work::Portal { at, facing }),
      "terrace" => Subject::Work(Work::Terrace { at, facing }),
      "den" => Subject::Work(Work::Den { at, facing }),
      "well" => Subject::Work(Work::Well { at }),
      "fire" => Subject::Work(Work::Fire { at }),
      "menhirs" => Subject::Work(Work::Menhirs { at }),
      "rubble" => Subject::Work(Work::Rubble { at }),
      "pillar" => Subject::Work(Work::Pillar { at, tall: 4.0 }),
      "tower" => Subject::Work(Work::Tower { at, radius: 3.6, tall: 11.0 }),
      "gate" => Subject::Work(Work::Gate { at, facing, tall: 8.0 }),
      "spire" => Subject::Work(Work::Spire { at, facing, tall: 5.0, lit: true }),
      "tiers" => Subject::Work(Work::Tiers { at, facing, radius: 9.0 }),
      "pit" => Subject::Work(Work::Pit { at, facing, inner: 6.0, outer: 10.0 }),
      "rampart" => Subject::Work(Work::Rampart {
        from: Vec2::new(-12.0, 0.0),
        to: Vec2::new(12.0, 0.0),
        tall: 8.0
      }),
      "barrels" => clutter(Clutter::Barrels),
      "crates" => clutter(Clutter::Crates),
      "hay" => clutter(Clutter::Hay),
      "woodpile" => clutter(Clutter::Woodpile),
      "cart" => clutter(Clutter::Cart),
      "stall" => clutter(Clutter::Stall),
      growth => Subject::Growth(growth.into())
    }
  }

  fn front(&self) -> f32 {
    match self {
      Subject::Hero
      | Subject::Foe(_)
      | Subject::Villager(_)
      | Subject::Wolf
      | Subject::Dragon { .. }
      | Subject::Atronach => PI,
      Subject::Robot | Subject::Work(_) | Subject::Growth(_) | Subject::Cages => 0.0
    }
  }
}

#[derive(Component)]
struct Turntable(usize);

#[derive(Component)]
struct Floor;

fn posed() -> Motion {
  let pose = opts().pose.as_deref().unwrap_or_default();
  Motion {
    swing: (pose == "swing").then_some(0.4),
    guard: (pose == "guard") as u8 as f32,
    fallen: (pose == "dead") as u8 as f32,
    speed: (pose == "run").then_some(5.0).unwrap_or(0.0),
    crouch: (pose == "sneak") as u8 as f32,
    stride: 1.0,
    ..default()
  }
}

fn figure(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  poses: &mut Assets<SkinnedMeshInverseBindposes>,
  stuffs: &Stuffs,
  holder: Entity,
  (frame, grip, hunch, scale): (Frame, Grip, f32, f32),
  kit: Kit
) {
  let body = commands
    .spawn((
      Transform::from_scale(Vec3::splat(scale)),
      Visibility::Inherited,
      ChildOf(holder)
    ))
    .id();
  let (rig, _) =
    humanoid::spawn_body(commands, meshes, poses, stuffs, body, frame, grip, hunch, kit);
  commands.entity(holder).insert((rig, posed()));
}

fn cages() -> Vec<(Stuff, Mesh)> {
  let chest = Cage::cuboid(Vec3::new(0.9, 0.55, 0.55))
    .beveled(0.03)
    .subdivided(3)
    .shaped(|point| point + Vec3::new(-2.2, 0.275, 0.0));
  let boulder = Cage::cuboid(Vec3::new(0.8, 0.6, 0.7))
    .creased(|a, b| a.y < 0.0 && b.y < 0.0, 3.0)
    .subdivided(2)
    .displaced(|point, _| noise::fbm3(point * 3.0, 3, 5) * 0.12)
    .subdivided(2)
    .shaped(|point| point + Vec3::new(-1.0, 0.3, 0.0));
  let dome = Cage::cuboid(Vec3::new(0.12, 0.2, 0.26))
    .shaped(|point| point + Vec3::new(0.06, 0.1, 0.0))
    .cut(Vec3::new(0.12, 0.1, 0.13), 0.45)
    .cut(Vec3::new(0.12, 0.2, 0.0), 0.5)
    .pulled(Vec3::new(0.0, 0.2, 0.0), 0.12, Vec3::Y * 0.05)
    .pulled(Vec3::new(0.06, 0.09, 0.13), 0.08, Vec3::Z * 0.02);
  let side = dome.faces_where(|centre, normal| {
    normal.x > 0.7 && centre.y > 0.12 && centre.z.abs() < 0.07
  })[0];
  let horned = (0..7).fold(dome.inset(&[side], 0.012), |cage, step| {
    let tip = cage.centre(side);
    let reach = Quat::from_rotation_z(0.28 * step as f32) * Vec3::X * 0.055;
    cage.extruded(&[side], |point| {
      tip + reach + Quat::from_rotation_z(0.28) * ((point - tip) * 0.84)
    })
  });
  let open = horned.facing(Vec3::NEG_Y);
  let helmet = horned
    .without(&open)
    .mirrored()
    .subdivided(3)
    .displaced(|point, _| noise::value3(point * 60.0, 3) * 0.002)
    .solidified(0.008)
    .shaped(|point| point * 1.6 + Vec3::new(0.3, 0.0, 0.0));
  let disc =
    Cage::prism(16, 0.42, 0.05).shaped(|point| Quat::from_rotation_x(FRAC_PI_2) * point);
  let front = disc.facing(Vec3::Z)[0];
  let shield = disc
    .inset(&[front], 0.04)
    .pushed(&[front], -0.01)
    .inset(&[front], 0.2)
    .inset(&[front], 0.03)
    .pushed(&[front], 0.07)
    .sharpened(30.0, 1.0)
    .subdivided(3)
    .shaped(|point| point + Vec3::new(1.6, 0.45, 0.0));
  let beast = Cage::from(sdf::surface(
    sdf::smooth_unions(
      [
        sdf::at(sdf::ellipsoid(Vec3::new(0.2, 0.22, 0.42)), Vec3::new(0.0, 0.6, 0.0)),
        sdf::limb(Vec3::new(0.0, 0.66, 0.3), 0.13, Vec3::new(0.0, 0.92, 0.55), 0.08),
        sdf::at(sdf::ellipsoid(Vec3::new(0.1, 0.1, 0.17)), Vec3::new(0.0, 0.95, 0.65)),
        sdf::limb(Vec3::new(0.0, 0.62, -0.38), 0.07, Vec3::new(0.0, 0.5, -0.8), 0.02)
      ]
      .into_iter()
      .chain([-1.0f32, 1.0].into_iter().flat_map(|side| {
        [0.25, -0.28].map(|z| {
          sdf::limb(
            Vec3::new(side * 0.12, 0.55, z),
            0.08,
            Vec3::new(side * 0.13, 0.04, z),
            0.04
          )
        })
      })),
      0.09
    ),
    &sdf::Bounds { center: Vec3::new(0.0, 0.5, 0.0), half_extent: 1.0, depth: 7 }
  ))
  .relaxed(3, 0.5)
  .shaped(|point| point + Vec3::new(3.0, 0.0, 0.0));
  let unwrapped = |mesh: Mesh, color: Srgba| Piece::new(mesh, color).unwrapped(0.25);
  [
    (Stuff::Fur, unwrapped(beast.mesh(60.0), Srgba::rgb(0.45, 0.4, 0.35))),
    (Stuff::Wood, Piece::new(chest.mesh(50.0), Srgba::rgb(0.55, 0.4, 0.27)).boxed(0.25)),
    (Stuff::Stone, unwrapped(boulder.mesh(50.0), Srgba::rgb(0.6, 0.6, 0.58))),
    (Stuff::Iron, unwrapped(helmet.mesh(50.0), Srgba::rgb(0.5, 0.5, 0.52))),
    (
      Stuff::Wood,
      Piece::new(shield.mesh(50.0), Srgba::rgb(0.5, 0.36, 0.24)).seamed(0.25, |edge| {
        let top = |point: Vec3| point.x.abs() < 1e-3 && point.y > 0.45;
        edge.bend > 0.5 || (top(edge.from) && top(edge.to))
      })
    )
  ]
  .into_iter()
  .map(|(stuff, Piece(mesh))| (stuff, stuff.fitted(mesh)))
  .collect()
}

fn hang(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  holders: &[Entity],
  parts: Vec<(Handle<StandardMaterial>, Mesh)>
) {
  let parts: Vec<_> =
    parts.into_iter().map(|(material, mesh)| (material, meshes.add(mesh))).collect();
  for &holder in holders {
    for (material, mesh) in &parts {
      commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        ChildOf(holder)
      ));
    }
  }
}

fn stage(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut poses: ResMut<Assets<SkinnedMeshInverseBindposes>>,
  stuffs: Res<Stuffs>,
  effects: Res<Effects>
) {
  let opts = opts();
  let name = opts.studio.as_deref().unwrap_or_default();
  let seed = opts.seed.unwrap_or(7);
  let subject = Subject::named(name, seed);
  let holders: Vec<Entity> = opts
    .views
    .iter()
    .enumerate()
    .map(|(index, angle)| {
      commands
        .spawn((
          Turntable(index),
          Transform::from_rotation(Quat::from_rotation_y(
            subject.front() + angle.to_radians()
          )),
          Visibility::Inherited
        ))
        .id()
    })
    .collect();
  commands.spawn((
    Name::new("Camera"),
    MainCamera,
    Camera3d::default(),
    Projection::Perspective(PerspectiveProjection {
      fov: FOV.to_radians(),
      near: 0.05,
      far: 30000.0,
      ..default()
    }),
    crate::sky::lens(),
    Transform::default()
  ));
  commands.spawn((
    Floor,
    Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)))),
    MeshMaterial3d(materials.add(StandardMaterial {
      base_color: Color::srgb(0.32, 0.3, 0.27),
      perceptual_roughness: 0.95,
      ..default()
    })),
    Transform::default()
  ));
  match subject {
    Subject::Hero => {
      for &holder in &holders {
        figure(
          &mut commands,
          &mut meshes,
          &mut poses,
          &stuffs,
          holder,
          (MAN, Grip::Blade, 0.0, 1.0),
          humanoid::dragonborn()
        )
      }
    }
    Subject::Foe(kind) => {
      let (grip, hunch) = match kind {
        FoeKind::Draugr => (Grip::Axe, 0.22),
        FoeKind::DraugrOverlord => (Grip::Axe, 0.12),
        FoeKind::BanditChief => (Grip::Axe, 0.0),
        _ => (Grip::Blade, 0.0)
      };
      for &holder in &holders {
        let kit = match kind {
          FoeKind::Draugr => humanoid::draugr(seed),
          FoeKind::DraugrOverlord => humanoid::draugr(seed * 2),
          FoeKind::BanditChief => humanoid::bandit(seed * 2 + 1),
          _ => humanoid::bandit(seed)
        };
        figure(
          &mut commands,
          &mut meshes,
          &mut poses,
          &stuffs,
          holder,
          (MAN, grip, hunch, creature::stature(kind)),
          kit
        )
      }
    }
    Subject::Villager(calling) => {
      let frame = calling.frame();
      let race = opts.race.as_deref().and_then(Race::named).unwrap_or(Race::of(seed));
      for &holder in &holders {
        let mut kit =
          humanoid::villager(calling, &Person::roll(race, frame.hip < MAN.hip, seed));
        if opts.torch {
          humanoid::torch(&mut kit, &frame)
        }
        let grip = opts.torch.then_some(Grip::Torch).unwrap_or(Grip::Bare);
        figure(
          &mut commands,
          &mut meshes,
          &mut poses,
          &stuffs,
          holder,
          (frame, grip, 0.0, 1.0),
          kit
        )
      }
    }
    Subject::Wolf => {
      for &holder in &holders {
        let body = commands
          .spawn((
            Transform::from_scale(Vec3::splat(creature::stature(FoeKind::Wolf))),
            Visibility::Inherited,
            ChildOf(holder)
          ))
          .id();
        let beast = wolf::skeleton(&mut commands, body);
        humanoid::fasten(
          &mut commands,
          &mut meshes,
          &stuffs,
          &beast.bones,
          wolf::hide(seed)
        );
        commands.entity(holder).insert((beast, posed()));
      }
    }
    Subject::Dragon { aloft } => {
      dragon::specimen(&mut commands, &mut meshes, &stuffs, &effects, &holders, aloft)
    }
    Subject::Robot => {
      for &holder in &holders {
        let robot = robot::spawn_robot(
          &mut commands,
          &mut meshes,
          &mut images,
          &mut materials,
          Transform::IDENTITY
        );
        commands.entity(robot).insert(ChildOf(holder));
      }
    }
    Subject::Work(work) => hang(
      &mut commands,
      &mut meshes,
      &holders,
      settlement::specimen(&work, seed)
        .into_iter()
        .map(|(stuff, mesh)| (stuffs.of(stuff), mesh))
        .collect()
    ),
    Subject::Growth(name) => hang(
      &mut commands,
      &mut meshes,
      &holders,
      flora::specimen(&name, seed as usize, &mut images, &mut materials, &stuffs)
        .unwrap_or_else(|| panic!("studio: nothing called {name:?}"))
    ),
    Subject::Atronach => hang(
      &mut commands,
      &mut meshes,
      &holders,
      atronach::body()
        .into_iter()
        .map(|(stuff, mesh)| (stuffs.of(stuff), mesh))
        .collect()
    ),
    Subject::Cages => hang(
      &mut commands,
      &mut meshes,
      &holders,
      cages().into_iter().map(|(stuff, mesh)| (stuffs.of(stuff), mesh)).collect()
    )
  }
}

fn bounds(aabb: &Aabb, global: &GlobalTransform) -> impl Iterator<Item = Vec3> {
  let (center, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
  (0..8).map(move |corner| {
    let sign = Vec3::new(
      [-1.0, 1.0][corner & 1],
      [-1.0, 1.0][corner >> 1 & 1],
      [-1.0, 1.0][corner >> 2 & 1]
    );
    global.transform_point(center + half * sign)
  })
}

fn frame(
  window: Single<&Window, With<PrimaryWindow>>,
  mut turntables: Query<(Entity, &Turntable, &mut Transform, &GlobalTransform)>,
  descendants: Query<&Children>,
  boxes: Query<(&Aabb, &GlobalTransform)>,
  camera: Single<
    (&mut Transform, &Projection),
    (With<MainCamera>, Without<Turntable>, Without<Floor>)
  >,
  floor: Single<&mut Transform, (With<Floor>, Without<Turntable>)>,
  sun: Single<&mut CascadeShadowConfig>
) {
  let mut tables: Vec<_> = turntables.iter_mut().collect();
  tables.sort_by_key(|&(_, &Turntable(index), ..)| index);
  let spans: Vec<(Vec3, Vec3)> = tables
    .iter()
    .map(|(holder, _, _, global)| {
      descendants
        .iter_descendants(*holder)
        .filter_map(|part| boxes.get(part).ok())
        .flat_map(|(aabb, global)| bounds(aabb, global))
        .fold((Vec3::MAX, Vec3::MIN), |(low, high), point| {
          (low.min(point - global.translation()), high.max(point - global.translation()))
        })
    })
    .collect();
  if let (mut camera, Projection::Perspective(lens)) = camera.into_inner()
    && spans.iter().all(|(low, high)| low.cmple(*high).all())
    && let widest =
      spans.iter().map(|(low, high)| (high - low).max_element()).fold(0.0, f32::max)
    && let gap = widest * 0.15
    && let row = spans.iter().map(|(low, high)| high.x - low.x).sum::<f32>()
      + gap * (spans.len() as f32 - 1.0)
  {
    let (placed, ..) = spans.iter().zip(tables.iter_mut()).fold(
      ((Vec3::MAX, Vec3::MIN), -row / 2.0),
      |((low, high), left), (&(near, far), (_, _, transform, _))| {
        let x = left - near.x;
        transform.translation = Vec3::X * x;
        (
          (low.min(near + Vec3::X * x), high.max(far + Vec3::X * x)),
          left + far.x - near.x + gap
        )
      }
    );
    let (low, high) = (placed.0.with_y(placed.0.y.max(0.0)), placed.1);
    let (size, center) =
      (high - low, opts().look.map_or((low + high) / 2.0, Vec3::from_array));
    let upright = (lens.fov / 2.0).tan();
    let across = upright * window.width() / window.height();
    let distance = (size.y / 2.0 / upright).max(size.x / 2.0 / across)
      * 1.15
      * opts().zoom.unwrap_or(1.0)
      + size.z / 2.0;
    let pitch = opts().pitch.unwrap_or(-8.0).to_radians();
    *camera = Transform::from_translation(
      center + Quat::from_rotation_x(pitch) * Vec3::Z * distance
    )
    .looking_at(center, Vec3::Y);
    *floor.into_inner() =
      Transform::from_xyz(center.x, 0.0, center.z).with_scale(Vec3::splat(20000.0));
    *sun.into_inner() = CascadeShadowConfigBuilder {
      num_cascades: 1,
      maximum_distance: distance + size.length(),
      ..default()
    }
    .build();
  }
}

fn checkered(
  mut commands: Commands,
  parts: Query<Entity, (Added<MeshMaterial3d<StandardMaterial>>, Without<Floor>)>,
  mut checker: Local<Option<Handle<StandardMaterial>>>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>
) {
  let checker = checker
    .get_or_insert_with(|| {
      materials.add(StandardMaterial {
        base_color_texture: Some(images.add(crate::texture::checker())),
        perceptual_roughness: 0.8,
        ..default()
      })
    })
    .clone();
  for part in &parts {
    commands.entity(part).insert(MeshMaterial3d(checker.clone()));
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_message::<crate::combat::Struck>()
    .add_systems(Startup, stage)
    .add_systems(Update, (robot::idle, frame, checkered.run_if(|| opts().checker)))
    .add_systems(PostUpdate, dragon::pose.before(TransformSystems::Propagate));
}
