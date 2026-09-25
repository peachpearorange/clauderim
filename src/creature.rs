use {crate::{combat::{Dead, Fighter, Side, Struck, Vitals},
             humanoid::{self, Grip, MAN, Motion},
             inventory::{Inventory, Item, Loot},
             noise::Roll,
             player::{Player, View},
             signal::{Cue, FoeKind, FoeSpawn, Notice, Prompt, Prompting, Sound},
             stuff::Stuffs,
             terrain::Ground,
             walker::{Walker, Walking},
             wolf},
     avian3d::prelude::*,
     bevy::prelude::*};

struct Breed {
  name: &'static str,
  health: f32,
  damage: f32,
  reach: f32,
  swing_time: f32,
  run: f32,
  walk: f32,
  aggro: f32,
  scale: f32,
  radius: f32,
  height: f32
}

const UNWATCHED: f32 = 200.0;

const fn breed(kind: FoeKind) -> Breed {
  match kind {
    FoeKind::Wolf => Breed {
      name: "Wolf",
      health: 38.0,
      damage: 7.0,
      reach: 1.35,
      swing_time: 0.7,
      run: 7.4,
      walk: 1.6,
      aggro: 30.0,
      scale: 1.0,
      radius: 0.4,
      height: 0.8
    },
    FoeKind::Draugr => Breed {
      name: "Restless Draugr",
      health: 60.0,
      damage: 10.0,
      reach: 1.9,
      swing_time: 1.0,
      run: 4.1,
      walk: 1.3,
      aggro: 16.0,
      scale: 1.0,
      radius: 0.34,
      height: 1.84
    },
    FoeKind::DraugrOverlord => Breed {
      name: "Draugr Overlord",
      health: 150.0,
      damage: 17.0,
      reach: 2.1,
      swing_time: 1.15,
      run: 4.0,
      walk: 1.3,
      aggro: 18.0,
      scale: 1.12,
      radius: 0.38,
      height: 2.06
    },
    FoeKind::Bandit => Breed {
      name: "Bandit",
      health: 55.0,
      damage: 9.0,
      reach: 1.9,
      swing_time: 0.85,
      run: 5.0,
      walk: 1.5,
      aggro: 26.0,
      scale: 1.0,
      radius: 0.34,
      height: 1.84
    },
    FoeKind::BanditChief => Breed {
      name: "Bandit Chief",
      health: 110.0,
      damage: 14.0,
      reach: 2.0,
      swing_time: 1.0,
      run: 4.8,
      walk: 1.4,
      aggro: 26.0,
      scale: 1.08,
      radius: 0.37,
      height: 1.98
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mind {
  Dormant,
  Idle(f32),
  Roam(Vec3),
  Hunt,
  Return
}

#[derive(Component)]
pub struct Foe {
  pub kind: FoeKind,
  home: Vec3,
  mind: Mind,
  cooldown: f32,
  circling: f32,
  roll: Roll,
  looted: bool
}

impl Foe {
  pub fn hunting(&self) -> bool { self.mind == Mind::Hunt }
}

fn loot(kind: FoeKind, roll: &mut Roll) -> Vec<Loot> {
  let gold =
    |low: f32, high: f32, roll: &mut Roll| Loot::Gold(roll.range(low, high) as u32);
  match kind {
    FoeKind::Wolf => vec![Loot::one(Item::WolfPelt)],
    FoeKind::Draugr => vec![gold(3.0, 18.0, roll), Loot::one(Item::AncientNordWarAxe)],
    FoeKind::DraugrOverlord => vec![
      gold(40.0, 90.0, roll),
      Loot::one(Item::AncientNordHelmet),
      Loot::one(Item::OverlordsKey),
    ],
    FoeKind::Bandit => vec![
      gold(5.0, 30.0, roll),
      Loot::one(Item::FurArmor),
      Loot::one(Item::PotionOfMinorHealing),
    ],
    FoeKind::BanditChief => vec![
      gold(60.0, 120.0, roll),
      Loot::one(Item::SteelWarAxe),
      Loot::one(Item::RotfenPlans),
    ]
  }
}

fn raise(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  spawns: Query<(Entity, &FoeSpawn, &Transform), Added<FoeSpawn>>
) {
  spawns.iter().for_each(|(entity, spawn, transform)| {
    let stats = breed(spawn.kind);
    let seed = entity.index().index();
    let center = transform.translation + Vec3::Y * (stats.height / 2.0 + 0.05);
    let facing = transform.rotation.normalize();
    commands.entity(entity).remove::<FoeSpawn>().insert((
      Name::new(stats.name),
      Foe {
        kind: spawn.kind,
        home: center,
        mind: spawn.dormant.then_some(Mind::Dormant).unwrap_or(Mind::Idle(2.0)),
        cooldown: 0.0,
        circling: 0.0,
        roll: Roll::new(seed),
        looted: false
      },
      Walker::default(),
      Motion::default(),
      Vitals::new(stats.health, 100.0),
      Side::Wild,
      Fighter {
        reach: stats.reach,
        damage: stats.damage,
        swing_time: stats.swing_time,
        cone: 0.8,
        girth: stats.radius
      },
      Transform::from_translation(center).with_rotation(facing),
      match spawn.kind {
        FoeKind::Wolf => {
          Collider::capsule_endpoints(stats.radius, Vec3::Z * -0.3, Vec3::Z * 0.3)
        }
        _ => Collider::capsule(stats.radius, stats.height - 2.0 * stats.radius)
      }
    ));
    let body = commands
      .spawn((
        Transform::from_xyz(0.0, -stats.height / 2.0 - 0.02, 0.0)
          .with_scale(Vec3::splat(stats.scale)),
        Visibility::Inherited,
        ChildOf(entity)
      ))
      .id();
    match spawn.kind {
      FoeKind::Wolf => {
        let beast = wolf::spawn_wolf(&mut commands, &mut meshes, &stuffs, body, seed);
        commands.entity(entity).insert(beast);
      }
      kind => {
        let (kit, grip, hunch) = match kind {
          FoeKind::Draugr => (humanoid::draugr(seed), Grip::Axe, 0.22),
          FoeKind::DraugrOverlord => (humanoid::draugr(seed * 2), Grip::Axe, 0.12),
          FoeKind::BanditChief => (humanoid::bandit(seed * 2 + 1), Grip::Axe, 0.0),
          _ => (humanoid::bandit(seed), Grip::Blade, 0.0)
        };
        let rig = humanoid::spawn_body(
          &mut commands,
          &mut meshes,
          &stuffs,
          body,
          MAN,
          grip,
          hunch,
          kit
        );
        commands.entity(entity).insert(rig);
      }
    }
  });
}

fn alarm(kind: FoeKind) -> Cue {
  match kind {
    FoeKind::Wolf => Cue::WolfGrowl,
    FoeKind::Draugr | FoeKind::DraugrOverlord => Cue::DraugrGroan,
    FoeKind::Bandit | FoeKind::BanditChief => Cue::BanditShout
  }
}

fn think(
  time: Res<Time>,
  mut struck: MessageReader<Struck>,
  mut sounds: MessageWriter<Sound>,
  player: Single<(Entity, &Transform, Has<Dead>), With<Player>>,
  mut foes: Query<
    (Entity, &mut Foe, &Transform, &mut Walker, &mut Motion, &Fighter),
    (Without<Dead>, Without<Specimen>)
  >
) {
  let (hero, hero_at, hero_dead) = *player;
  let target = hero_at.translation;
  let delta = time.delta_secs();
  let provoked: Vec<Entity> =
    struck.read().filter(|hit| hit.attacker == hero).map(|hit| hit.target).collect();
  foes.iter_mut().for_each(
    |(entity, mut foe, transform, mut walker, mut motion, fighter)| {
      let stats = breed(foe.kind);
      let at = transform.translation;
      let gap = (target - at).with_y(0.0);
      let distance = gap.length();
      let noticed = !hero_dead && (distance < stats.aggro || provoked.contains(&entity));
      foe.cooldown -= delta;
      let mind = match foe.mind {
        Mind::Dormant if distance < 6.5 || provoked.contains(&entity) => {
          sounds.write(Sound::here(Cue::DraugrWake, at));
          Mind::Hunt
        }
        Mind::Dormant => Mind::Dormant,
        Mind::Idle(_) | Mind::Roam(_) | Mind::Return
          if noticed && (foe.home - at).length() < 70.0 =>
        {
          sounds.write(Sound::here(alarm(foe.kind), at));
          foe.cooldown = 0.6;
          Mind::Hunt
        }
        Mind::Idle(wait) if wait <= 0.0 && distance < UNWATCHED => {
          let wander =
            foe.home + Vec3::new(foe.roll.spread(14.0), 0.0, foe.roll.spread(14.0));
          Mind::Roam(wander)
        }
        Mind::Idle(wait) => Mind::Idle(wait - delta),
        Mind::Roam(spot) if (spot - at).with_y(0.0).length() < 1.2 => {
          Mind::Idle(foe.roll.range(3.0, 8.0))
        }
        Mind::Roam(spot) => Mind::Roam(spot),
        Mind::Hunt
          if hero_dead
            || distance > stats.aggro * 2.8
            || (foe.home - at).length() > 90.0 =>
        {
          Mind::Return
        }
        Mind::Hunt => Mind::Hunt,
        Mind::Return if (foe.home - at).with_y(0.0).length() < 2.0 => Mind::Idle(4.0),
        Mind::Return => Mind::Return
      };
      foe.mind = mind;
      let toward = gap.normalize_or_zero();
      let strike_range = fighter.reach + 0.2;
      let busy = motion.swing.is_some() || motion.flinch > 0.4;
      let (wish, facing) = match mind {
        Mind::Dormant | Mind::Idle(_) => (Vec3::ZERO, None),
        Mind::Roam(spot) => {
          ((spot - at).with_y(0.0).normalize_or_zero() * stats.walk, None)
        }
        Mind::Return => {
          ((foe.home - at).with_y(0.0).normalize_or_zero() * stats.run * 0.6, None)
        }
        Mind::Hunt => {
          foe.circling += delta;
          let side = Vec3::new(-toward.z, 0.0, toward.x) * (foe.circling * 0.7).sin();
          let closing = if distance > strike_range * 0.85 {
            toward * stats.run
          } else if foe.kind == FoeKind::Wolf && foe.cooldown > 0.4 {
            (side * 2.6 - toward * 0.8).normalize_or_zero() * stats.run * 0.45
          } else {
            Vec3::ZERO
          };
          (busy.then_some(closing * 0.2).unwrap_or(closing), Some(toward))
        }
      };
      walker.wish = wish;
      walker.facing = facing;
      if mind == Mind::Hunt
        && distance < strike_range
        && foe.cooldown <= 0.0
        && !busy
        && !hero_dead
      {
        motion.swing = Some(0.0);
        motion.power = foe.roll.chance(0.2);
        foe.cooldown = stats.swing_time * motion.power.then_some(1.45).unwrap_or(1.0)
          + foe.roll.range(0.5, 1.6);
        let cue =
          (foe.kind == FoeKind::Wolf).then_some(Cue::WolfBite).unwrap_or(Cue::Swing);
        sounds.write(Sound::here(cue, at));
      }
    }
  );
}

fn perish(
  fallen: Query<(&Foe, &Transform), Added<Dead>>,
  mut sounds: MessageWriter<Sound>,
  mut commands: Commands,
  bodies: Query<Entity, (With<Foe>, Added<Dead>)>
) {
  fallen.iter().for_each(|(foe, transform)| {
    let cue = match foe.kind {
      FoeKind::Wolf => Cue::WolfDie,
      FoeKind::Draugr | FoeKind::DraugrOverlord => Cue::DraugrDie,
      FoeKind::Bandit | FoeKind::BanditChief => Cue::ManDie
    };
    sounds.write(Sound::here(cue, transform.translation));
  });
  bodies.iter().for_each(|entity| {
    commands.entity(entity).insert(CollisionLayers::NONE);
  });
}

fn search(
  keys: Res<ButtonInput<KeyCode>>,
  view: Res<View>,
  mut prompt: ResMut<Prompt>,
  mut notices: MessageWriter<Notice>,
  mut sounds: MessageWriter<Sound>,
  player: Single<(&Transform, &mut Inventory), (With<Player>, Without<Dead>)>,
  mut corpses: Query<(&mut Foe, &Transform, &Name), With<Dead>>
) {
  let (hero, mut inventory) = player.into_inner();
  let at = hero.translation;
  let forward = view.flat_forward();
  let nearest = corpses
    .iter_mut()
    .filter(|(foe, transform, _)| {
      let gap = (transform.translation - at).with_y(0.0);
      !foe.looted && gap.length() < 2.6 && forward.dot(gap.normalize_or_zero()) > 0.3
    })
    .min_by(|a, b| a.1.translation.distance(at).total_cmp(&b.1.translation.distance(at)));
  if let Some((mut foe, transform, name)) = nearest {
    prompt.0 = Some(Prompting { verb: "Search".into(), noun: name.as_str().into() });
    if keys.just_pressed(KeyCode::KeyE) {
      foe.looted = true;
      let kind = foe.kind;
      loot(kind, &mut foe.roll).into_iter().for_each(|loot| {
        notices.write(Notice(format!("{loot} added")));
        inventory.take(loot);
      });
      sounds.write(Sound::here(Cue::Coins, transform.translation));
    }
  }
}

#[derive(Component)]
struct Specimen;

fn specimen(
  mut commands: Commands,
  ground: Res<Ground>,
  player: Single<&Transform, Added<Player>>
) {
  let kind = match crate::opts::opts().foe.as_deref() {
    Some("wolf") => Some(FoeKind::Wolf),
    Some("draugr") => Some(FoeKind::Draugr),
    Some("overlord") => Some(FoeKind::DraugrOverlord),
    Some("bandit") => Some(FoeKind::Bandit),
    Some("chief") => Some(FoeKind::BanditChief),
    _ => None
  };
  if let Some(kind) = kind {
    let ahead = player.translation + player.forward().as_vec3() * 3.2;
    commands.spawn((
      FoeSpawn { kind, dormant: true },
      Specimen,
      Transform::from_translation(ground.surface(ahead.xz()))
        .looking_to(player.right().as_vec3().with_y(0.0), Vec3::Y)
    ));
  }
}

fn encounters(mut commands: Commands, ground: Res<Ground>) {
  let packs = [
    (Vec2::new(-35.0, 175.0), FoeKind::Wolf, 2),
    (Vec2::new(-95.0, -35.0), FoeKind::Wolf, 3),
    (Vec2::new(250.0, -60.0), FoeKind::Wolf, 2),
    (Vec2::new(172.0, 76.0), FoeKind::Bandit, 2),
    (Vec2::new(-150.0, 20.0), FoeKind::Bandit, 3),
    (Vec2::new(-156.0, 14.0), FoeKind::BanditChief, 1)
  ];
  packs.into_iter().enumerate().for_each(|(index, (spot, kind, count))| {
    (0..count).for_each(|member| {
      let angle = member as f32 * 2.3 + index as f32;
      let at = spot + Vec2::from_angle(angle) * (member as f32 * 2.5);
      commands.spawn((
        FoeSpawn { kind, dormant: false },
        Transform::from_translation(ground.surface(at))
          .with_rotation(Quat::from_rotation_y(angle))
      ));
    });
  });
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Thinking;

pub fn plugin(app: &mut App) {
  app
    .add_systems(Startup, encounters)
    .add_systems(PostStartup, specimen)
    .add_systems(Update, (raise, think).chain().in_set(Thinking).before(Walking))
    .add_systems(Update, (perish, search).after(Walking));
}
