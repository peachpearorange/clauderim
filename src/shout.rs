use {crate::{combat::{Dead, Shake, Side, Vitals},
             fx::{Effects, Fleeting},
             humanoid::Motion,
             model,
             player::{Player, View},
             signal::{Cue, Notice, Shouts, Sound, WordWall},
             stuff::{Stuff, Stuffs},
             walker::{Walker, Walking}},
     bevy::{light::NotShadowCaster, prelude::*}};

const WORDS: [&str; 3] = ["FUS", "RO", "DAH"];
const REACH: [f32; 3] = [7.0, 10.0, 15.0];
const FORCE: [f32; 3] = [9.0, 15.0, 26.0];
const RECHARGE: [f32; 3] = [6.0, 9.0, 12.0];
const WALL_REACH: f32 = 7.5;
const ABSORB_TIME: f32 = 4.5;

#[derive(Component)]
pub struct Staggered(pub f32);

#[derive(Component)]
pub struct Wisp {
  from: Vec3,
  bend: Vec3,
  target: Entity,
  progress: f32,
  pace: f32
}

#[derive(Resource)]
pub struct WispLook {
  mesh: Handle<Mesh>,
  frost: Handle<StandardMaterial>,
  ember: Handle<StandardMaterial>
}

#[derive(Resource, Default)]
struct Chanting {
  wall: Option<Entity>,
  elapsed: f32
}

fn prepare(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>
) {
  commands.insert_resource(WispLook {
    mesh: meshes.add(model::ball(0.07)),
    frost: stuffs.of(Stuff::Frost),
    ember: stuffs.of(Stuff::Ember)
  });
  commands.insert_resource(Shouts { learned: 1, cooldown: 0.0 });
}

pub fn stream(
  commands: &mut Commands,
  look: &WispLook,
  from: Vec3,
  target: Entity,
  seed: u32,
  fiery: bool
) {
  let mut roll = crate::noise::Roll::new(seed);
  let bend = from + Vec3::new(roll.spread(4.0), roll.range(1.0, 5.0), roll.spread(4.0));
  let material = fiery.then(|| look.ember.clone()).unwrap_or(look.frost.clone());
  commands.spawn((
    Wisp { from, bend, target, progress: 0.0, pace: roll.range(0.5, 0.9) },
    Mesh3d(look.mesh.clone()),
    MeshMaterial3d(material),
    NotShadowCaster,
    Transform::from_translation(from).with_scale(Vec3::splat(roll.range(0.6, 1.6)))
  ));
}

fn drift_wisps(
  time: Res<Time>,
  mut commands: Commands,
  targets: Query<&GlobalTransform, Without<Wisp>>,
  mut wisps: Query<(Entity, &mut Wisp, &mut Transform)>
) {
  wisps.iter_mut().for_each(|(entity, mut wisp, mut transform)| {
    wisp.progress += time.delta_secs() * wisp.pace;
    let goal = targets
      .get(wisp.target)
      .map_or(wisp.from, |target| target.translation() + Vec3::Y * 1.2);
    let t = wisp.progress.min(1.0);
    transform.translation =
      wisp.from * (1.0 - t) * (1.0 - t) + wisp.bend * 2.0 * t * (1.0 - t) + goal * t * t;
    if wisp.progress >= 1.0 {
      commands.entity(entity).despawn();
    }
  });
}

fn shout(
  keys: Res<ButtonInput<KeyCode>>,
  time: Res<Time>,
  view: Res<View>,
  effects: Res<Effects>,
  mut shouts: ResMut<Shouts>,
  mut shake: ResMut<Shake>,
  mut sounds: MessageWriter<Sound>,
  mut notices: MessageWriter<Notice>,
  mut commands: Commands,
  player: Single<(&Transform, &mut Motion), (With<Player>, Without<Dead>)>,
  mut foes: Query<
    (Entity, &Transform, &Side, &mut Walker, &mut Vitals),
    (Without<Player>, Without<Dead>)
  >
) {
  shouts.cooldown = (shouts.cooldown - time.delta_secs()).max(0.0);
  let (transform, mut motion) = player.into_inner();
  motion.shout = (motion.shout - time.delta_secs() * 1.4).max(0.0);
  if keys.just_pressed(KeyCode::KeyZ) && shouts.learned > 0 {
    if shouts.cooldown > 0.0 {
      notices.write(Notice("You cannot shout again yet.".into()));
    } else {
      let power = (shouts.learned as usize).min(3) - 1;
      shouts.cooldown = RECHARGE[power];
      motion.shout = 1.0;
      shake.0 = 0.4 + 0.3 * power as f32;
      sounds.write(Sound::flat(Cue::Shout));
      let facing = view.forward();
      let mouth = transform.translation + Vec3::Y * 0.6 + facing * 0.8;
      commands.spawn((
        effects.emit(&effects.gust),
        Fleeting(2.0),
        Transform::from_translation(mouth).looking_to(facing, Vec3::Y)
      ));
      foes.iter_mut().filter(|(_, _, side, ..)| **side == Side::Wild).for_each(
        |(entity, their, _, mut walker, mut vitals)| {
          let gap = their.translation - transform.translation;
          let flat = gap.with_y(0.0);
          if flat.length() < REACH[power]
            && view.flat_forward().dot(flat.normalize_or_zero()) > 0.55
          {
            let falloff = 1.0 - flat.length() / REACH[power] * 0.5;
            walker.shove += flat.normalize_or_zero() * FORCE[power] * falloff
              + Vec3::Y * FORCE[power] * 0.35 * falloff;
            vitals.health -= 4.0 * (power + 1) as f32;
            commands.entity(entity).insert(Staggered(1.2 + power as f32 * 0.6));
          }
        }
      );
    }
  }
}

fn stagger(
  time: Res<Time>,
  mut commands: Commands,
  mut staggered: Query<(Entity, &mut Staggered, &mut Motion, &mut Walker, Has<Dead>)>
) {
  staggered.iter_mut().for_each(|(entity, mut left, mut motion, mut walker, dead)| {
    left.0 -= time.delta_secs();
    motion.swing = None;
    walker.wish = Vec3::ZERO;
    walker.facing = None;
    let knocked = (left.0 * 2.5).clamp(0.0, 1.0);
    if !dead {
      motion.fallen = knocked;
    }
    if left.0 <= 0.0 {
      commands.entity(entity).remove::<Staggered>();
    }
  });
}

fn word_walls(
  time: Res<Time>,
  look: Res<WispLook>,
  mut chanting: ResMut<Chanting>,
  mut shouts: ResMut<Shouts>,
  mut sounds: MessageWriter<Sound>,
  mut notices: MessageWriter<Notice>,
  mut commands: Commands,
  player: Single<(Entity, &Transform), With<Player>>,
  walls: Query<(Entity, &GlobalTransform), With<WordWall>>,
  mut spawned: Local<u32>
) {
  let (hero, at) = *player;
  let near = walls
    .iter()
    .find(|(_, wall)| wall.translation().distance(at.translation) < WALL_REACH);
  match (near, chanting.wall) {
    (Some((wall, _)), None) if shouts.learned < 3 => {
      chanting.wall = Some(wall);
      chanting.elapsed = 0.0;
      *spawned = 0;
      sounds.write(Sound::flat(Cue::WordWall));
    }
    (None, Some(_)) => chanting.wall = None,
    _ => {}
  }
  if let Some(wall) = chanting.wall
    && let Ok((_, spot)) = walls.get(wall)
    && shouts.learned < 3
  {
    chanting.elapsed += time.delta_secs();
    let burst = (chanting.elapsed * 26.0) as u32;
    (*spawned..burst).for_each(|index| {
      let from = spot.translation()
        + spot.right() * ((index % 7) as f32 - 3.0) * 0.6
        + Vec3::Y * (1.0 + (index % 5) as f32 * 0.5);
      stream(&mut commands, &look, from, hero, index * 31 + 7, false);
    });
    *spawned = burst;
    if chanting.elapsed > ABSORB_TIME {
      shouts.learned = 3;
      shouts.cooldown = 0.0;
      *spawned = 0;
      sounds.write(Sound::flat(Cue::WordLearned));
      notices.write(Notice(format!(
        "Word of Power learned: {} {} {} — Unrelenting Force",
        WORDS[0], WORDS[1], WORDS[2]
      )));
      notices.write(Notice("Press Z to Shout.".into()));
    }
  }
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Chanting>()
    .add_systems(Startup, prepare)
    .add_systems(
      Update,
      (shout, stagger).chain().before(Walking).after(crate::creature::Thinking)
    )
    .add_systems(Update, (drift_wisps, word_walls));
}
