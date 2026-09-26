use {crate::{creature::Dressing,
             humanoid::{self, Calling, Grip, Motion, Rig},
             noise::Roll,
             place::{self, Marker},
             player::{Player, View},
             settlement,
             signal::{Notice, Prompt, Prompting},
             terrain::Ground,
             walker::{Walker, Walking}},
     avian3d::prelude::*,
     bevy::{platform::collections::HashMap, prelude::*},
     std::sync::LazyLock};

const GATHER: f32 = 320.0;
const DISPERSE: f32 = 460.0;
const STROLL: f32 = 1.3;
const HEIGHT: f32 = 1.8;
const RADIUS: f32 = 0.3;
const TRAVELLERS: usize = 3;
const MEET: (f32, f32) = (110.0, 240.0);
const PART: f32 = 420.0;
const TRAMP: f32 = 1.45;

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

fn kindle(commands: &mut Commands, effects: &crate::fx::Effects, rig: &Rig) {
  let flame = rig.frame.hand() + Vec3::NEG_Z * 0.5;
  let hand = rig.bones[humanoid::Joint::ElbowL as usize];
  commands.spawn((
    PointLight {
      color: Color::srgb(1.0, 0.6, 0.28),
      intensity: 160_000.0,
      range: 16.0,
      ..default()
    },
    Transform::from_translation(flame + Vec3::NEG_Z * 0.15),
    ChildOf(hand)
  ));
  commands.spawn((
    effects.emit(&effects.torch),
    Transform::from_translation(flame),
    ChildOf(hand)
  ));
}

fn spawn_villager(
  commands: &mut Commands,
  calling: Calling,
  seed: u32,
  at: Vec3,
  facing: f32,
  torch: Option<&crate::fx::Effects>
) -> Entity {
  let lit = torch.is_some();
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
  let rig = Rig {
    bones,
    frame,
    grip: lit.then_some(Grip::Torch).unwrap_or(Grip::Bare),
    hunch: 0.0
  };
  if let Some(effects) = torch {
    kindle(commands, effects, &rig)
  }
  commands.entity(entity).insert((rig, Dressing {
    bones: bones.to_vec(),
    tailoring: crate::work::task(move || {
      let mut kit = humanoid::villager(calling, seed);
      if lit {
        humanoid::torch(&mut kit, &frame)
      }
      humanoid::tailor(kit)
    })
  }));
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
    for (index, home) in HOMES.iter().enumerate() {
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
                roll.range(0.0, std::f32::consts::TAU),
                None
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
          for entity in present.remove(&index).into_iter().flatten() {
            commands.entity(entity).despawn()
          }
        }
        _ => {}
      }
    }
  }
}

fn wander(
  time: Res<Time>,
  player: Single<&Transform, With<Player>>,
  mut folk: Query<(&mut Townsfolk, &mut Walker, &Transform), Without<Player>>
) {
  let delta = time.delta_secs();
  let hero = player.translation;
  for (mut person, mut walker, transform) in folk.iter_mut() {
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
  }
}

#[derive(Component)]
pub struct Travelling {
  road: usize,
  next: usize,
  onward: bool,
  stuck: f32,
  last: Vec2
}

fn wayside(road: &place::Road, index: usize) -> bool {
  let at = road.path[index];
  place::around(at).iter().all(|place| at.distance(place.spot()) > place.flat() * 1.8)
}

fn set_out(
  mut commands: Commands,
  time: Res<Time>,
  ground: Res<Ground>,
  daylight: Res<crate::sky::Daylight>,
  effects: Res<crate::fx::Effects>,
  mut roll: Local<Option<Roll>>,
  mut wait: Local<f32>,
  player: Single<&Transform, With<Player>>,
  travellers: Query<(Entity, &Transform), With<Travelling>>
) {
  let roll = roll.get_or_insert_with(|| Roll::new(9091));
  let here = player.translation.xz();
  for (entity, _) in travellers
    .iter()
    .filter(|(_, transform)| transform.translation.xz().distance(here) > PART)
  {
    commands.entity(entity).despawn()
  }
  *wait -= time.delta_secs();
  if *wait <= 0.0 && travellers.iter().count() < TRAVELLERS && daylight.shelter < 0.5 {
    *wait = roll.range(4.0, 10.0);
    let meeting: Vec<(usize, usize)> = place::ROADS
      .iter()
      .enumerate()
      .flat_map(|(road, way)| (0..way.path.len()).map(move |index| (road, index)))
      .filter(|&(road, index)| {
        let way = &place::ROADS[road];
        (MEET.0..MEET.1).contains(&way.path[index].distance(here)) && wayside(way, index)
      })
      .collect();
    if !meeting.is_empty()
      && let (road, index) = meeting[roll.below(meeting.len())]
      && let path = &place::ROADS[road].path
    {
      let calling = [
        Calling::Farmer,
        Calling::Huntress,
        Calling::Barbarian,
        Calling::Priest,
        Calling::Cook
      ][roll.below(5)];
      let lit = roll.chance(if daylight.level < 0.35 { 0.8 } else { 0.2 });
      let onward = roll.chance(0.5);
      let next =
        if onward { (index + 1).min(path.len() - 1) } else { index.saturating_sub(1) };
      let at = path[index];
      let heading = path[next] - at;
      let entity = spawn_villager(
        &mut commands,
        calling,
        roll.below(100_000) as u32,
        ground.surface(at),
        f32::atan2(-heading.x, -heading.y),
        lit.then_some(&*effects)
      );
      commands.entity(entity).insert(Travelling {
        road,
        next,
        onward,
        stuck: 0.0,
        last: at
      });
    }
  }
}

fn tramp(
  time: Res<Time>,
  player: Single<&Transform, With<Player>>,
  mut travellers: Query<(&mut Travelling, &mut Walker, &Transform), Without<Player>>
) {
  let delta = time.delta_secs();
  let hero = player.translation;
  for (mut travel, mut walker, transform) in travellers.iter_mut() {
    let path = &place::ROADS[travel.road].path;
    let at = transform.translation.xz();
    let toward_hero = (hero - transform.translation).with_y(0.0);
    let attentive = toward_hero.length() < 3.5;
    travel.stuck = (at.distance(travel.last) < TRAMP * delta * 0.3 && !attentive)
      .then_some(travel.stuck + delta)
      .unwrap_or(0.0);
    travel.last = at;
    let end =
      if travel.onward { travel.next + 1 >= path.len() } else { travel.next == 0 };
    if at.distance(path[travel.next]) < 2.5 || travel.stuck > 4.0 {
      travel.onward = travel.onward ^ end;
      travel.stuck = 0.0;
      travel.next = if travel.onward {
        (travel.next + 1).min(path.len() - 1)
      } else {
        travel.next.saturating_sub(1)
      };
    }
    let side = (path[travel.next] - at).normalize_or_zero().perp() * 1.2;
    let heading = (path[travel.next] + side - at).normalize_or_zero();
    walker.wish =
      (!attentive).then(|| heading.extend(0.0).xzy() * TRAMP).unwrap_or(Vec3::ZERO);
    walker.facing = attentive.then_some(toward_hero);
  }
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
  mut roll: Local<Option<Roll>>,
  folk: Query<
    (&Transform, &Name),
    (Without<Player>, Or<(With<Townsfolk>, With<Travelling>)>)
  >
) {
  let at = player.translation;
  let forward = view.flat_forward();
  let nearest = folk
    .iter()
    .filter(|(transform, _)| {
      let gap = (transform.translation - at).with_y(0.0);
      gap.length() < 3.0 && forward.dot(gap.normalize_or_zero()) > 0.5
    })
    .min_by(|a, b| a.0.translation.distance(at).total_cmp(&b.0.translation.distance(at)));
  if prompt.0.is_none()
    && let Some((_, name)) = nearest
  {
    prompt.0 = Some(Prompting { verb: "Talk".into(), noun: name.as_str().into() });
    if keys.just_pressed(KeyCode::KeyE) {
      let roll = roll.get_or_insert_with(|| Roll::new(77));
      let line = GREETINGS[roll.below(GREETINGS.len())];
      notices.write(Notice(format!("{name}: \"{line}\"")));
    }
  }
}

fn specimen(
  mut commands: Commands,
  ground: Res<Ground>,
  effects: Res<crate::fx::Effects>,
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
      f32::atan2(-toward.x, -toward.z),
      crate::opts::opts().torch.then_some(&*effects)
    );
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_systems(PostStartup, specimen)
    .add_systems(Update, (gather, wander, set_out, tramp).chain().before(Walking))
    .add_systems(Update, converse.after(Walking));
}
