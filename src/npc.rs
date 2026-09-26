use {crate::{creature::Dressing,
             humanoid::{self, Calling, Grip, Motion, Rig},
             noise::Roll,
             place::Marker,
             player::{Player, View},
             settlement,
             signal::{Notice, Prompt, Prompting},
             terrain::Ground,
             walker::{Walker, Walking}},
     avian3d::prelude::*,
     bevy::{platform::collections::HashMap, prelude::*, tasks::AsyncComputeTaskPool},
     std::sync::LazyLock};

const GATHER: f32 = 320.0;
const DISPERSE: f32 = 460.0;
const STROLL: f32 = 1.3;
const HEIGHT: f32 = 1.8;
const RADIUS: f32 = 0.3;

struct Home {
  center: Vec2,
  paths: Vec<Vec2>,
  folk: Vec<(Calling, u32)>
}

static HOMES: LazyLock<Vec<Home>> = LazyLock::new(|| {
  settlement::layouts()
    .filter_map(|layout| {
      let (count, callings): (usize, &[Calling]) = match layout.place.marker() {
        Marker::City => (12, &[
          Calling::Cook,
          Calling::Huntress,
          Calling::Priest,
          Calling::Farmer,
          Calling::Barbarian
        ]),
        Marker::Town => {
          (7, &[Calling::Cook, Calling::Huntress, Calling::Farmer, Calling::Priest])
        }
        Marker::Farm => (3, &[Calling::Farmer, Calling::Farmer, Calling::Cook]),
        _ => (0, &[])
      };
      let paths: Vec<Vec2> = layout
        .streets
        .iter()
        .flat_map(|street| street.path.iter().copied().step_by(2))
        .chain(layout.worn.iter().map(|&(at, _)| at))
        .collect();
      let seed = layout.place.spot().x.to_bits() ^ layout.place.spot().y.to_bits();
      let mut roll = Roll::new(seed);
      (count > 0 && !paths.is_empty()).then(|| Home {
        center: layout.place.spot(),
        folk: (0..count)
          .map(|_| (callings[roll.below(callings.len())], roll.below(100_000) as u32))
          .collect(),
        paths
      })
    })
    .collect()
});

#[derive(Component)]
pub struct Townsfolk {
  home: usize,
  goal: Vec2,
  idle: f32,
  stuck: f32,
  last: Vec2,
  roll: Roll
}

fn spawn_villager(
  commands: &mut Commands,
  calling: Calling,
  seed: u32,
  at: Vec3,
  facing: f32
) -> Entity {
  let frame = calling.frame();
  let center = at + Vec3::Y * (HEIGHT / 2.0 + 0.05);
  let entity = commands
    .spawn((
      Name::new(calling.title()),
      Walker::default(),
      Motion::default(),
      Collider::capsule(RADIUS, HEIGHT - 2.0 * RADIUS),
      Transform::from_translation(center).with_rotation(Quat::from_rotation_y(facing))
    ))
    .id();
  let body = commands
    .spawn((
      Transform::from_xyz(0.0, -HEIGHT / 2.0 - 0.02, 0.0),
      Visibility::Inherited,
      ChildOf(entity)
    ))
    .id();
  let bones = humanoid::skeleton(commands, body, frame);
  commands.entity(entity).insert((
    Rig { bones, frame, grip: Grip::Bare, hunch: 0.0 },
    Dressing {
      bones: bones.to_vec(),
      tailoring: AsyncComputeTaskPool::get()
        .spawn(async move { humanoid::tailor(humanoid::villager(calling, seed)) })
    }
  ));
  entity
}

fn gather(
  mut commands: Commands,
  ground: Res<Ground>,
  mut present: Local<HashMap<usize, Vec<Entity>>>,
  players: Query<&Transform, With<Player>>
) {
  if let Ok(player) = players.single() {
    let here = player.translation.xz();
    HOMES.iter().enumerate().for_each(|(index, home)| {
      let distance = home.center.distance(here);
      match (present.contains_key(&index), distance < GATHER, distance > DISPERSE) {
        (false, true, _) => {
          let mut roll = Roll::new(index as u32 * 31 + 7);
          let folk = home
            .folk
            .iter()
            .map(|&(calling, seed)| {
              let at = home.paths[roll.below(home.paths.len())]
                + Vec2::new(roll.spread(2.0), roll.spread(2.0));
              let entity = spawn_villager(
                &mut commands,
                calling,
                seed,
                ground.surface(at),
                roll.range(0.0, std::f32::consts::TAU)
              );
              commands.entity(entity).insert(Townsfolk {
                home: index,
                goal: at,
                idle: roll.range(0.0, 6.0),
                stuck: 0.0,
                last: at,
                roll: Roll::new(seed)
              });
              entity
            })
            .collect();
          present.insert(index, folk);
        }
        (true, _, true) => {
          present
            .remove(&index)
            .into_iter()
            .flatten()
            .for_each(|entity| commands.entity(entity).despawn());
        }
        _ => {}
      }
    });
  }
}

fn wander(
  time: Res<Time>,
  player: Single<&Transform, With<Player>>,
  mut folk: Query<(&mut Townsfolk, &mut Walker, &Transform), Without<Player>>
) {
  let delta = time.delta_secs();
  let hero = player.translation;
  folk.iter_mut().for_each(|(mut person, mut walker, transform)| {
    let at = transform.translation.xz();
    let toward_hero = (hero - transform.translation).with_y(0.0);
    let attentive = toward_hero.length() < 3.5;
    let arrived = at.distance(person.goal) < 1.2;
    person.stuck = (at.distance(person.last) < STROLL * delta * 0.3
      && person.idle <= 0.0)
      .then_some(person.stuck + delta)
      .unwrap_or(0.0);
    person.last = at;
    if (arrived || person.stuck > 3.0) && person.idle <= 0.0 {
      let paths = &HOMES[person.home].paths;
      let next = paths[person.roll.below(paths.len())];
      person.goal = next + Vec2::new(person.roll.spread(2.0), person.roll.spread(2.0));
      person.idle = person.roll.range(2.0, 9.0);
      person.stuck = 0.0;
    }
    person.idle -= delta;
    let heading = (person.goal - at).normalize_or_zero();
    walker.wish = (!attentive && person.idle <= 0.0)
      .then(|| heading.extend(0.0).xzy() * STROLL)
      .unwrap_or(Vec3::ZERO);
    walker.facing = attentive.then_some(toward_hero);
  });
}

const GREETINGS: [&str; 8] = [
  "Watch the roads. Bandits have been bold of late.",
  "Cold enough for you? The wind off the peaks cuts to the bone.",
  "Keep your coin close and your blade closer.",
  "If you're looking for work, try the keep.",
  "I saw a shadow over the mountains last night. Big as a longhouse.",
  "The stables are just outside the gate if you need a horse.",
  "Mind the wolves past the old barrow.",
  "Stay out of trouble, traveller."
];

fn converse(
  keys: Res<ButtonInput<KeyCode>>,
  view: Res<View>,
  mut prompt: ResMut<Prompt>,
  mut notices: MessageWriter<Notice>,
  player: Single<&Transform, With<Player>>,
  mut folk: Query<(&mut Townsfolk, &Transform, &Name), Without<Player>>
) {
  let at = player.translation;
  let forward = view.flat_forward();
  let nearest = folk
    .iter_mut()
    .filter(|(_, transform, _)| {
      let gap = (transform.translation - at).with_y(0.0);
      gap.length() < 3.0 && forward.dot(gap.normalize_or_zero()) > 0.5
    })
    .min_by(|a, b| a.1.translation.distance(at).total_cmp(&b.1.translation.distance(at)));
  if prompt.0.is_none()
    && let Some((mut person, _, name)) = nearest
  {
    prompt.0 = Some(Prompting { verb: "Talk".into(), noun: name.as_str().into() });
    if keys.just_pressed(KeyCode::KeyE) {
      let line = GREETINGS[person.roll.below(GREETINGS.len())];
      notices.write(Notice(format!("{name}: \"{line}\"")));
    }
  }
}

fn specimen(
  mut commands: Commands,
  ground: Res<Ground>,
  player: Single<&Transform, Added<Player>>
) {
  let calling = match crate::opts::opts().foe.as_deref() {
    Some("cook") => Some(Calling::Cook),
    Some("huntress") => Some(Calling::Huntress),
    Some("priest") => Some(Calling::Priest),
    Some("farmer") => Some(Calling::Farmer),
    Some("sellsword") => Some(Calling::Barbarian),
    _ => None
  };
  if let Some(calling) = calling {
    let ahead = player.translation + player.forward().as_vec3() * 3.2;
    let toward = -player.forward().as_vec3();
    spawn_villager(
      &mut commands,
      calling,
      7,
      ground.surface(ahead.xz()),
      f32::atan2(-toward.x, -toward.z)
    );
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(PostStartup, specimen)
    .add_systems(Update, (gather, wander).chain().before(Walking))
    .add_systems(Update, converse.after(Walking));
}
