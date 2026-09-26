use {crate::{humanoid::{self, Grip, Hidden1st, MAN, Motion},
             opts::opts,
             place, sky,
             stuff::Stuffs,
             terrain::Ground,
             walker::Walker},
     avian3d::prelude::*,
     bevy::{camera::visibility::RenderLayers,
            input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll,
                           MouseScrollUnit},
            prelude::*,
            window::{CursorGrabMode, CursorOptions}}};

pub const RUN_SPEED: f32 = 5.2;
pub const SPRINT_SPEED: f32 = 8.4;
pub const WALK_SPEED: f32 = 2.2;
const CAPSULE_RADIUS: f32 = 0.34;
const CAPSULE_HEIGHT: f32 = 1.84;
const EYE: f32 = 1.66;
const FOCUS: Vec3 = Vec3::new(0.42, 1.62, 0.0);
const LOOK_SPEED: f32 = 0.0025;
const PITCH_LIMIT: f32 = 1.35;
const NEAREST: f32 = 1.4;
const FARTHEST: f32 = 9.0;
const ZOOM_STEP: f32 = 1.3;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource)]
pub struct View {
  pub yaw: f32,
  pub pitch: f32,
  pub distance: f32,
  pub first_person: bool,
  pub captured: bool,
  pub combat: f32
}

impl View {
  pub fn forward(&self) -> Vec3 {
    Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0) * Vec3::NEG_Z
  }

  pub fn flat_forward(&self) -> Vec3 { Quat::from_rotation_y(self.yaw) * Vec3::NEG_Z }
}

pub fn capsule_offset() -> f32 { CAPSULE_HEIGHT / 2.0 }

fn start_spot() -> (Vec2, Vec2) {
  let named = opts().at.as_deref().and_then(|name| {
    place::all().find(|place| place.name().to_lowercase().contains(&name.to_lowercase()))
  });
  named
    .map(|place| {
      let spot = place.spot() + Vec2::new(0.0, place.flat() * 1.6 + 6.0);
      (spot, (place.spot() - spot).normalize())
    })
    .unwrap_or((place::START, place::START_FACING.normalize()))
}

fn spawn_player(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  ground: Res<Ground>
) {
  let (spot, facing) = start_spot();
  let yaw = f32::atan2(-facing.x, -facing.y) + opts().yaw.unwrap_or(0.0).to_radians();
  let player = commands
    .spawn((
      Name::new("Dragonborn"),
      Player,
      Walker::default(),
      Motion::default(),
      crate::combat::Vitals::new(120.0, 110.0),
      crate::combat::Side::Hero,
      crate::combat::Fighter {
        reach: 2.1,
        damage: 18.0,
        swing_time: 0.62,
        cone: 1.0,
        girth: 0.35
      },
      Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS),
      Transform::from_translation(
        opts().inside.map_or(ground.surface(spot), Vec3::from_array)
          + Vec3::Y * (capsule_offset() + 0.3)
      )
      .with_rotation(Quat::from_rotation_y(yaw))
    ))
    .id();
  let body = commands
    .spawn((
      Transform::from_xyz(0.0, -capsule_offset(), 0.0),
      Visibility::Inherited,
      ChildOf(player)
    ))
    .id();
  let rig = humanoid::spawn_body(
    &mut commands,
    &mut meshes,
    &stuffs,
    body,
    MAN,
    Grip::Blade,
    0.0,
    humanoid::dragonborn()
  );
  commands.entity(rig.bones[humanoid::Joint::Head as usize]).queue(
    |head: EntityWorldMut| {
      let parts =
        head.get::<Children>().map(|children| children.to_vec()).unwrap_or_default();
      head.into_world_mut().insert_batch(
        parts.into_iter().map(|part| (part, (Hidden1st, RenderLayers::default())))
      );
    }
  );
  commands.entity(player).insert(rig);

  commands.insert_resource(View {
    yaw: yaw + opts().turn.to_radians(),
    pitch: opts().pitch.unwrap_or(-8.0).to_radians(),
    distance: opts().zoom.unwrap_or(3.2),
    first_person: opts().first,
    captured: !cfg!(target_arch = "wasm32"),
    combat: 0.0
  });
  commands.spawn((
    Name::new("Camera"),
    MainCamera,
    Camera3d::default(),
    Projection::Perspective(PerspectiveProjection {
      fov: 72f32.to_radians(),
      near: 0.08,
      far: 30000.0,
      ..default()
    }),
    sky::lens(),
    Transform::default()
  ));
}

fn capture_cursor(
  keys: Res<ButtonInput<KeyCode>>,
  mouse: Res<ButtonInput<MouseButton>>,
  mut view: ResMut<View>,
  mut cursor: Single<&mut CursorOptions>
) {
  if keys.just_pressed(KeyCode::Escape) {
    view.captured = false;
  }
  let clicked = mouse.just_pressed(MouseButton::Left);
  view.captured |= clicked;
  let grab =
    view.captured.then_some(CursorGrabMode::Locked).unwrap_or(CursorGrabMode::None);
  if clicked || cursor.grab_mode != grab || cursor.visible == view.captured {
    cursor.visible = !view.captured;
    cursor.grab_mode = grab;
  }
}

fn steer(
  keys: Res<ButtonInput<KeyCode>>,
  motion: Res<AccumulatedMouseMotion>,
  scroll: Res<AccumulatedMouseScroll>,
  time: Res<Time>,
  mut view: ResMut<View>,
  player: Single<(&mut Walker, &mut Motion, &crate::combat::Vitals), With<Player>>
) {
  let (mut walker, mut body, vitals) = player.into_inner();
  if let Some(pose) = opts().pose.as_deref() {
    body.swing = (pose == "swing").then_some(0.4);
    body.guard = (pose == "guard") as u8 as f32;
    body.fallen = (pose == "dead") as u8 as f32;
    body.speed = (pose == "run").then_some(5.0).unwrap_or(0.0);
    body.stride = 1.0;
  }
  if view.captured {
    view.yaw -= motion.delta.x * LOOK_SPEED;
    view.pitch =
      (view.pitch - motion.delta.y * LOOK_SPEED).clamp(-PITCH_LIMIT, PITCH_LIMIT);
  }
  let notches = scroll.delta.y
    * match scroll.unit {
      MouseScrollUnit::Line => 1.0,
      MouseScrollUnit::Pixel => 0.02
    };
  (view.first_person, view.distance) = match (view.first_person, notches) {
    (true, out) if out < 0.0 => (false, NEAREST),
    (false, into) if into > 0.0 && view.distance <= NEAREST => (true, NEAREST),
    (first, _) => {
      (first, (view.distance * ZOOM_STEP.powf(-notches)).clamp(NEAREST, FARTHEST))
    }
  };
  if keys.just_pressed(KeyCode::KeyF) {
    view.first_person = !view.first_person;
  }
  view.combat = (view.combat - time.delta_secs()).max(0.0);

  let alive = body.fallen < 0.1;
  let forward = view.flat_forward();
  let right = Vec3::new(-forward.z, 0.0, forward.x);
  let heading = [
    (KeyCode::KeyW, forward),
    (KeyCode::KeyS, -forward),
    (KeyCode::KeyD, right),
    (KeyCode::KeyA, -right)
  ]
  .into_iter()
  .filter(|&(key, _)| keys.pressed(key))
  .fold(Vec3::ZERO, |sum, (_, direction)| sum + direction)
  .normalize_or_zero();
  let sprinting =
    keys.pressed(KeyCode::ShiftLeft) && vitals.stamina > 1.0 && body.guard < 0.3;
  let pace = if sprinting {
    SPRINT_SPEED
  } else if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::CapsLock) {
    WALK_SPEED
  } else {
    RUN_SPEED
  };
  let slowed = 1.0 - 0.55 * body.guard - 0.4 * body.swing.map_or(0.0, |_| 1.0);
  walker.wish = heading * pace * slowed * alive as u8 as f32;
  let fighting = view.combat > 0.0 || view.first_person;
  walker.facing = (alive && fighting).then_some(forward);
  if keys.just_pressed(KeyCode::Space) && (walker.grounded || walker.swimming) && alive {
    walker.leap = Some(1.1);
  }
}

pub fn sprinting(walker: &Walker) -> bool { walker.wish.length() > RUN_SPEED + 0.5 }

fn follow(
  time: Res<Time>,
  mut shake: ResMut<crate::combat::Shake>,
  view: Res<View>,
  ground: Res<Ground>,
  spatial: SpatialQuery,
  player: Single<(Entity, &Transform), With<Player>>,
  mut camera: Single<&mut Transform, (With<MainCamera>, Without<Player>)>,
  mut shown: Local<Option<f32>>,
  mut hidden: Query<&mut RenderLayers, With<Hidden1st>>
) {
  let (entity, body) = *player;
  let feet = body.translation - Vec3::Y * capsule_offset();
  let rotation = Quat::from_euler(EulerRot::YXZ, view.yaw, view.pitch, 0.0);
  let distance = shown.map_or(view.distance, |shown| {
    shown + (view.distance - shown) * (1.0 - (-time.delta_secs() * 12.0).exp())
  });
  *shown = Some(distance);
  let (eye, wanted) = if view.first_person {
    let eye = feet + Vec3::Y * EYE + view.flat_forward() * 0.12;
    (eye, eye)
  } else {
    let shoulder = feet + Quat::from_rotation_y(view.yaw) * FOCUS;
    (shoulder, shoulder - rotation * Vec3::NEG_Z * distance)
  };
  let gap = wanted - eye;
  let reach = Dir3::new(gap)
    .ok()
    .and_then(|direction| {
      spatial.cast_ray(
        eye,
        direction,
        gap.length(),
        true,
        &SpatialQueryFilter::from_excluded_entities([entity])
      )
    })
    .map_or(gap.length(), |hit| (hit.distance - 0.25).max(0.1));
  let tremor = shake.0 * shake.0 * 0.25;
  let jitter = Vec3::new(
    (time.elapsed_secs() * 71.0).sin(),
    (time.elapsed_secs() * 83.0).cos(),
    (time.elapsed_secs() * 59.0).sin()
  ) * tremor;
  shake.0 = (shake.0 - time.delta_secs() * 1.6).max(0.0);
  let placed = eye + gap.normalize_or_zero() * reach + jitter;
  camera.translation = crate::river::water_level(placed.xz())
    .filter(|&level| {
      let floor = ground.height(placed.xz());
      floor < level && placed.y > floor - 4.0
    })
    .map_or(placed, |level| placed.with_y(placed.y.max(level + 0.25)));
  camera.rotation = rotation;
  if let Some(eye) = opts().eye {
    let lifted =
      |[x, above, z]: [f32; 3]| ground.surface(Vec2::new(x, z)) + Vec3::Y * above;
    **camera = Transform::from_translation(lifted(eye))
      .looking_at(opts().look.map_or(body.translation, lifted), Vec3::Y);
  }
  for mut layers in hidden.iter_mut() {
    layers
      .set_if_neq(view.first_person.then_some(humanoid::SHADOW_ONLY).unwrap_or_default());
  }
}

fn revive(
  time: Res<Time>,
  mut commands: Commands,
  ground: Res<Ground>,
  mut since: Local<f32>,
  player: Single<
    (
      Entity,
      &mut Transform,
      &mut crate::combat::Vitals,
      &mut Motion,
      Has<crate::combat::Dead>
    ),
    With<Player>
  >
) {
  let (entity, mut transform, mut vitals, mut motion, dead) = player.into_inner();
  *since = dead.then_some(*since + time.delta_secs()).unwrap_or(0.0);
  if *since > 6.0 {
    commands.entity(entity).remove::<crate::combat::Dead>();
    vitals.health = vitals.health_max;
    vitals.stamina = vitals.stamina_max;
    motion.fallen = 0.0;
    transform.translation =
      ground.surface(place::START) + Vec3::Y * (capsule_offset() + 0.3);
    *since = 0.0;
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(Startup, spawn_player)
    .add_systems(Update, (capture_cursor, steer).chain().before(crate::walker::Walking))
    .add_systems(Update, revive)
    .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}
