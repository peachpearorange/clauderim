use {crate::{humanoid::Motion,
             player::{self, Player, View},
             signal::{Cue, Sound},
             walker::{Walker, Walking}},
     bevy::prelude::*};

const POWER_HOLD: f32 = 0.32;
const STRIKE_AT: f32 = 0.5;

#[derive(Component, Clone)]
pub struct Vitals {
  pub health: f32,
  pub health_max: f32,
  pub stamina: f32,
  pub stamina_max: f32,
  pub magicka: f32,
  pub magicka_max: f32
}

impl Vitals {
  pub fn new(health: f32, stamina: f32) -> Self {
    Self {
      health,
      health_max: health,
      stamina,
      stamina_max: stamina,
      magicka: 100.0,
      magicka_max: 100.0
    }
  }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
  Hero,
  Wild
}

#[derive(Component, Clone)]
pub struct Fighter {
  pub reach: f32,
  pub damage: f32,
  pub swing_time: f32,
  pub cone: f32,
  pub girth: f32
}

#[derive(Component)]
pub struct Dead;

#[derive(Message, Clone)]
pub struct Struck {
  pub target: Entity,
  pub attacker: Entity,
  pub damage: f32,
  pub power: bool,
  pub blocked: bool,
  pub at: Vec3
}

#[derive(Resource, Default)]
pub struct Shake(pub f32);

fn player_attacks(
  mouse: Res<ButtonInput<MouseButton>>,
  time: Res<Time>,
  mut view: ResMut<View>,
  mut held: Local<Option<f32>>,
  player: Single<(&mut Motion, &mut Vitals), (With<Player>, Without<Dead>)>
) {
  let (mut motion, mut vitals) = player.into_inner();
  let guarding = mouse.pressed(MouseButton::Right) && view.captured;
  motion.guard =
    motion.guard.lerp(guarding as u8 as f32, 1.0 - (-14.0 * time.delta_secs()).exp());
  if guarding {
    view.combat = 5.0;
  }
  if !view.captured || motion.swing.is_some() {
    *held = None;
  } else if mouse.just_pressed(MouseButton::Left) {
    *held = Some(0.0);
    view.combat = 5.0;
  } else if let Some(charge) = held.as_mut() {
    *charge += time.delta_secs();
    let power = *charge > POWER_HOLD && vitals.stamina > 5.0;
    if mouse.just_released(MouseButton::Left) || power {
      motion.swing = Some(0.0);
      motion.power = power;
      vitals.stamina -= power.then_some(25.0).unwrap_or(0.0);
      *held = None;
    }
  }
}

fn swing(
  time: Res<Time>,
  mut bodies: Query<
    (Entity, &Transform, &Fighter, &Side, &mut Motion),
    (With<Vitals>, Without<Dead>)
  >,
  mut struck: MessageWriter<Struck>
) {
  let strikes: Vec<_> = bodies
    .iter_mut()
    .filter_map(|(entity, transform, fighter, side, mut motion)| {
      let before = motion.swing?;
      let pace = motion.power.then_some(1.45).unwrap_or(1.0) * fighter.swing_time;
      let after = before + time.delta_secs() / pace;
      motion.swing = (after < 1.0).then_some(after);
      (before < STRIKE_AT && after >= STRIKE_AT).then_some((
        entity,
        *transform,
        fighter.clone(),
        *side,
        motion.power
      ))
    })
    .collect();
  let hits: Vec<Struck> = strikes
    .into_iter()
    .flat_map(|(attacker, transform, fighter, side, power)| {
      let forward = transform.forward().as_vec3().with_y(0.0).normalize_or_zero();
      bodies
        .iter()
        .filter(move |(target, _, _, their_side, ..)| {
          *target != attacker && **their_side != side
        })
        .filter_map(move |(target, their, their_fighter, _, their_motion)| {
          let gap = their.translation - transform.translation;
          let flat = gap.with_y(0.0);
          let within =
            flat.length() < fighter.reach + their_fighter.girth && gap.y.abs() < 2.2;
          let aimed = forward.dot(flat.normalize_or_zero()) > fighter.cone.cos();
          (within && aimed).then(|| {
            let facing_us =
              their.forward().as_vec3().dot(-flat.normalize_or_zero()) > 0.3;
            let blocked = their_motion.guard > 0.6 && facing_us;
            let damage = fighter.damage
              * power.then_some(2.2).unwrap_or(1.0)
              * blocked.then_some(0.15).unwrap_or(1.0);
            Struck {
              target,
              attacker,
              damage,
              power,
              blocked,
              at: transform.translation.lerp(their.translation, 0.7) + Vec3::Y * 0.4
            }
          })
        })
    })
    .collect();
  hits.into_iter().for_each(|hit| {
    struck.write(hit);
  });
}

fn wound(
  mut struck: MessageReader<Struck>,
  mut shake: ResMut<Shake>,
  mut commands: Commands,
  mut victims: Query<
    (&mut Vitals, &mut Motion, Option<&mut Walker>, &Transform, Has<Player>),
    Without<Dead>
  >,
  places: Query<&Transform>,
  heroes: Query<(), With<Player>>
) {
  struck.read().for_each(|hit| {
    if let Ok((mut vitals, mut motion, walker, transform, is_player)) =
      victims.get_mut(hit.target)
    {
      vitals.health -= hit.damage;
      vitals.stamina -= hit.blocked.then_some(12.0).unwrap_or(0.0);
      motion.flinch = hit.blocked.then_some(0.35).unwrap_or(1.0);
      let away = places
        .get(hit.attacker)
        .map(|from| {
          (transform.translation - from.translation).with_y(0.0).normalize_or_zero()
        })
        .unwrap_or_default();
      if let Some(mut walker) = walker {
        walker.shove += away * hit.power.then_some(5.5).unwrap_or(2.2);
      }
      if is_player || heroes.contains(hit.attacker) {
        shake.0 = shake.0.max(hit.power.then_some(0.5).unwrap_or(0.25));
      }
      if vitals.health <= 0.0 {
        commands.entity(hit.target).insert(Dead);
      }
    }
  });
}

fn recover(
  time: Res<Time>,
  mut living: Query<
    (&mut Vitals, &mut Motion, Option<&Walker>, Has<Player>),
    Without<Dead>
  >,
  mut fallen: Query<(&mut Motion, Option<&mut Walker>), With<Dead>>
) {
  let delta = time.delta_secs();
  living.iter_mut().for_each(|(mut vitals, mut motion, walker, is_player)| {
    let sprint = is_player
      && walker
        .is_some_and(|walker| player::sprinting(walker) && walker.wish.length() > 0.1);
    let stamina_flow = sprint.then_some(-16.0).unwrap_or(9.0);
    vitals.stamina =
      (vitals.stamina + stamina_flow * delta).clamp(-10.0, vitals.stamina_max);
    vitals.health = (vitals.health + 0.5 * delta).min(vitals.health_max);
    vitals.magicka = (vitals.magicka + 4.0 * delta).min(vitals.magicka_max);
    motion.flinch = (motion.flinch - 3.5 * delta).max(0.0);
  });
  fallen.iter_mut().for_each(|(mut motion, walker)| {
    motion.fallen = (motion.fallen + 2.2 * delta).min(1.0);
    motion.swing = None;
    motion.guard = 0.0;
    motion.flinch = 0.0;
    if let Some(mut walker) = walker {
      walker.wish = Vec3::ZERO;
      walker.facing = None;
    }
  });
}

fn resound(
  mut struck: MessageReader<Struck>,
  mut sounds: MessageWriter<Sound>,
  heroes: Query<(), With<Player>>,
  swingers: Query<(&Motion, &Transform), (With<Player>, Changed<Motion>)>,
  mut swinging: Local<bool>
) {
  struck.read().for_each(|hit| {
    let cue = if hit.blocked {
      Cue::Block
    } else if heroes.contains(hit.target) {
      Cue::PlayerHurt
    } else {
      Cue::Hit
    };
    sounds.write(Sound::here(cue, hit.at));
  });
  swingers.iter().for_each(|(motion, transform)| {
    if motion.swing.is_some() && !*swinging {
      let cue = motion.power.then_some(Cue::PowerSwing).unwrap_or(Cue::Swing);
      sounds.write(Sound::here(cue, transform.translation));
    }
    *swinging = motion.swing.is_some();
  });
}

pub fn plugin(app: &mut App) {
  app.add_message::<Struck>().init_resource::<Shake>().add_systems(
    Update,
    (player_attacks, swing, wound, recover, resound).chain().after(Walking)
  );
}
