mod ambience;
mod music;
mod output;
mod sfx;
mod synth;

use {crate::{humanoid::Motion,
             player::{MainCamera, Player},
             signal::{Cue, Engaged, Sound, WordWall},
             sky::Daylight,
             walker::Walker},
     bevy::{platform::collections::HashMap, prelude::*},
     output::{Mix, Output, Take, Voice},
     std::{f32::consts::{PI, TAU},
           sync::{Arc, Mutex,
                  mpsc::{Receiver, channel}}},
     synth::{Rng, Wave},
     web_time::Instant};

const CUES: [Cue; 28] = [
  Cue::Swing,
  Cue::PowerSwing,
  Cue::Hit,
  Cue::Block,
  Cue::Footstep,
  Cue::WolfGrowl,
  Cue::WolfBite,
  Cue::WolfDie,
  Cue::DraugrWake,
  Cue::DraugrGroan,
  Cue::DraugrDie,
  Cue::BanditShout,
  Cue::ManDie,
  Cue::PlayerHurt,
  Cue::ChestOpen,
  Cue::Discover,
  Cue::WordWall,
  Cue::WordLearned,
  Cue::Shout,
  Cue::DragonRoar,
  Cue::FireBreath,
  Cue::Wingbeat,
  Cue::LevelUp,
  Cue::Coins,
  Cue::Gunshot,
  Cue::Explosion,
  Cue::IceGrind,
  Cue::IceShatter
];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Clip {
  Cue(Cue),
  Bird,
  Owl,
  Wind,
  Night,
  Cave,
  Chant,
  Explore,
  Combat
}

impl Clip {
  fn all() -> Vec<Clip> {
    [Clip::Explore, Clip::Combat]
      .into_iter()
      .chain(CUES.map(Clip::Cue))
      .chain([Clip::Wind, Clip::Cave, Clip::Night, Clip::Chant, Clip::Bird, Clip::Owl])
      .collect()
  }

  fn render(self) -> Vec<Wave> {
    match self {
      Clip::Cue(cue) => (0..sfx::variants(cue))
        .map(|variant| sfx::render(cue, variant * 7919 + cue as u64))
        .collect(),
      Clip::Bird => (0..6).map(ambience::bird).collect(),
      Clip::Owl => (0..2).map(ambience::owl).collect(),
      Clip::Wind => vec![ambience::wind()],
      Clip::Night => vec![ambience::night()],
      Clip::Cave => vec![ambience::cave()],
      Clip::Chant => vec![ambience::chant(16.0, true, 5)],
      Clip::Explore => vec![music::explore()],
      Clip::Combat => vec![music::combat()]
    }
  }
}

fn loudness(cue: Cue) -> (f32, f32) {
  match cue {
    Cue::Swing => (0.7, 5.0),
    Cue::PowerSwing => (0.7, 6.0),
    Cue::Hit => (0.6, 8.0),
    Cue::Block => (0.6, 10.0),
    Cue::Footstep => (0.08, 4.0),
    Cue::WolfGrowl => (0.3, 10.0),
    Cue::WolfBite => (0.8, 8.0),
    Cue::WolfDie => (0.45, 10.0),
    Cue::DraugrWake => (0.4, 10.0),
    Cue::DraugrGroan => (0.35, 10.0),
    Cue::DraugrDie => (0.55, 10.0),
    Cue::BanditShout => (0.45, 14.0),
    Cue::ManDie => (0.4, 12.0),
    Cue::PlayerHurt => (0.45, 6.0),
    Cue::ChestOpen => (0.8, 6.0),
    Cue::Discover => (0.5, 10.0),
    Cue::WordWall => (0.5, 10.0),
    Cue::WordLearned => (1.0, 10.0),
    Cue::Shout => (0.9, 30.0),
    Cue::DragonRoar => (1.0, 240.0),
    Cue::FireBreath => (0.75, 40.0),
    Cue::Wingbeat => (0.7, 40.0),
    Cue::LevelUp => (0.4, 10.0),
    Cue::Coins => (0.4, 5.0),
    Cue::Gunshot => (0.5, 30.0),
    Cue::Explosion => (1.0, 120.0),
    Cue::IceGrind => (0.6, 18.0),
    Cue::IceShatter => (0.8, 20.0)
  }
}

struct Rendered {
  clip: Clip,
  waves: Vec<Wave>,
  took: f32
}

#[derive(Resource)]
struct Rendering {
  results: Mutex<Receiver<Rendered>>,
  pending: usize,
  started: Instant
}

#[derive(Resource, Default)]
struct Library(HashMap<Clip, Vec<(Take, f32)>>);

enum Anchor {
  Flat,
  At(Vec3),
  On(Entity)
}

struct Sounding {
  voice: Voice,
  anchor: Anchor,
  volume: f32,
  reach: f32,
  ends: f32
}

#[derive(Resource, Default)]
struct Voices(Vec<Sounding>);

#[derive(Component)]
struct Bed {
  clip: Clip,
  voice: Voice,
  level: f32
}

#[derive(Component)]
struct Chanting;

fn render(world: &mut World) {
  let clips = Clip::all();
  let pending = clips.len();
  let queue = Arc::new(Mutex::new(clips.into_iter().rev().collect::<Vec<_>>()));
  let (send, results) = channel();
  let workers = std::thread::available_parallelism()
    .map(|count| count.get() / 2)
    .unwrap_or(2)
    .clamp(1, 3);
  for _ in 0..workers {
    let (queue, send) = (queue.clone(), send.clone());
    crate::par::spawn(move || {
      for clip in std::iter::from_fn(|| queue.lock().ok()?.pop()) {
        let start = Instant::now();
        let waves = clip.render();
        send.send(Rendered { clip, waves, took: start.elapsed().as_secs_f32() }).ok();
      }
    });
  }
  world.insert_resource(Rendering {
    results: Mutex::new(results),
    pending,
    started: Instant::now()
  });
  output::Output::open()
    .map(|output| world.insert_non_send(output))
    .unwrap_or_else(|| warn!("no audio output"))
}

fn gather(
  mut rendering: ResMut<Rendering>,
  mut library: ResMut<Library>,
  output: Option<NonSendMut<Output>>
) {
  if let Some(mut output) = output {
    let arrived: Vec<Rendered> = rendering
      .results
      .lock()
      .map(|results| results.try_iter().collect())
      .unwrap_or_default();
    for Rendered { clip, waves, took } in arrived {
      debug!("synthesised {clip:?} in {took:.2}s");
      library.0.insert(
        clip,
        waves
          .into_iter()
          .map(|wave| {
            let secs = wave.0[0].len() as f32 / synth::RATE;
            (output.load(wave), secs)
          })
          .collect()
      );
      rendering.pending -= 1;
      if rendering.pending == 0 {
        info!("audio synthesised in {:.2}s", rendering.started.elapsed().as_secs_f32())
      }
    }
  }
}

fn heard(from: &GlobalTransform, at: Vec3, volume: f32, reach: f32) -> Mix {
  let gap = at - from.translation();
  let distance = gap.length();
  Mix {
    gain: volume * (reach / distance.max(reach)).powi(2),
    pan: gap.normalize_or_zero().dot(from.right().into())
  }
}

fn play(
  mut sounds: MessageReader<Sound>,
  library: Res<Library>,
  time: Res<Time<Real>>,
  camera: Single<&GlobalTransform, With<MainCamera>>,
  output: Option<NonSendMut<Output>>,
  mut voices: ResMut<Voices>,
  mut rng: Local<Rng>
) {
  if let Some(mut output) = output {
    for &Sound { cue, at } in sounds.read() {
      if let Some(takes) = library.0.get(&Clip::Cue(cue))
        && !takes.is_empty()
        && let (volume, reach) = loudness(cue)
        && let (take, secs) = takes[rng.below(takes.len())]
        && let speed = at.map_or(1.0, |_| rng.range(0.96, 1.04))
        && let mix = at.map_or(Mix::flat(volume), |at| heard(&camera, at, volume, reach))
        && (mix.gain > 0.002 || at.is_none())
      {
        voices.0.push(Sounding {
          voice: output.start(take, mix, speed, false),
          anchor: at.map_or(Anchor::Flat, Anchor::At),
          volume,
          reach,
          ends: time.elapsed_secs() + secs / speed + 0.2
        })
      }
    }
  }
}

fn place_voices(
  time: Res<Time<Real>>,
  camera: Single<&GlobalTransform, With<MainCamera>>,
  anchors: Query<&GlobalTransform>,
  output: Option<NonSendMut<Output>>,
  mut voices: ResMut<Voices>
) {
  if let Some(mut output) = output {
    let now = time.elapsed_secs();
    voices.0.retain(|&Sounding { voice, ref anchor, volume, reach, ends }| {
      let at = match *anchor {
        Anchor::Flat => None,
        Anchor::At(at) => Some(Some(at)),
        Anchor::On(entity) => {
          Some(anchors.get(entity).ok().map(GlobalTransform::translation))
        }
      };
      let alive = now < ends && at.is_none_or(|at| at.is_some());
      if !alive {
        output.stop(voice)
      } else if let Some(Some(at)) = at {
        output.set(voice, heard(&camera, at, volume, reach))
      }
      alive
    })
  }
}

fn pace(
  player: Single<(&Motion, &Walker, &GlobalTransform), With<Player>>,
  mut last: Local<f32>,
  mut sounds: MessageWriter<Sound>
) {
  let (&Motion { stride, speed, .. }, walker, transform) = player.into_inner();
  if (stride / PI).floor() != (*last / PI).floor() && walker.grounded && speed > 0.6 {
    sounds.write(Sound::here(Cue::Footstep, transform.translation() - Vec3::Y * 0.9));
  }
  *last = stride
}

fn lay_beds(
  library: Res<Library>,
  beds: Query<&Bed>,
  output: Option<NonSendMut<Output>>,
  mut commands: Commands
) {
  if library.is_changed()
    && let Some(mut output) = output
  {
    for (clip, take) in [Clip::Wind, Clip::Night, Clip::Cave, Clip::Explore, Clip::Combat]
      .into_iter()
      .filter(|&clip| !beds.iter().any(|bed| bed.clip == clip))
      .filter_map(|clip| {
        library
          .0
          .get(&clip)
          .and_then(|takes| takes.first())
          .map(|&(take, _)| (clip, take))
      })
    {
      commands.spawn(Bed {
        clip,
        voice: output.start(take, Mix::SILENT, 1.0, true),
        level: 0.0
      });
    }
  }
}

fn blend_beds(
  time: Res<Time>,
  daylight: Res<Daylight>,
  engaged: Res<Engaged>,
  output: Option<NonSendMut<Output>>,
  mut beds: Query<&mut Bed>
) {
  if let Some(mut output) = output {
    let &Daylight { level, shelter, .. } = daylight.into_inner();
    let outdoors = 1.0 - synth::smooth((shelter - 0.3) / 0.4);
    let fight = engaged.0.is_some() as u8 as f32;
    let delta = time.delta_secs();
    for mut bed in beds.iter_mut() {
      let (target, rate) = match bed.clip {
        Clip::Wind => (0.18 * outdoors, 0.6),
        Clip::Night => (0.2 * (1.0 - level) * outdoors, 0.4),
        Clip::Cave => (0.4 * (1.0 - outdoors), 0.6),
        Clip::Explore => (0.2 * (1.0 - fight), 0.3),
        Clip::Combat => (0.3 * fight, if fight > 0.0 { 1.2 } else { 0.35 }),
        _ => (0.0, 1.0)
      };
      let level = bed.level + (target - bed.level) * (1.0 - (-rate * delta).exp());
      let level = (level.abs() < 1e-4 && target == 0.0).then_some(0.0).unwrap_or(level);
      if (level - bed.level).abs() > 1e-5 || (level == 0.0 && bed.level != 0.0) {
        output.set(bed.voice, Mix::flat(level))
      }
      bed.level = level
    }
  }
}

fn chant_walls(
  library: Res<Library>,
  walls: Query<Entity, (With<WordWall>, Without<Chanting>)>,
  output: Option<NonSendMut<Output>>,
  mut voices: ResMut<Voices>,
  mut commands: Commands
) {
  if let Some(&(take, _)) = library.0.get(&Clip::Chant).and_then(|takes| takes.first())
    && let Some(mut output) = output
  {
    for wall in walls.iter() {
      commands.entity(wall).try_insert(Chanting);
      voices.0.push(Sounding {
        voice: output.start(take, Mix::SILENT, 1.0, true),
        anchor: Anchor::On(wall),
        volume: 0.45,
        reach: 8.0,
        ends: f32::INFINITY
      })
    }
  }
}

fn critters(
  time: Res<Time>,
  real: Res<Time<Real>>,
  daylight: Res<Daylight>,
  library: Res<Library>,
  camera: Single<&GlobalTransform, With<MainCamera>>,
  output: Option<NonSendMut<Output>>,
  mut voices: ResMut<Voices>,
  mut rng: Local<Rng>
) {
  if let Some(mut output) = output {
    let &Daylight { level, shelter, .. } = daylight.into_inner();
    let outdoors = 1.0 - synth::smooth((shelter - 0.3) / 0.4);
    let delta = time.delta_secs();
    for (clip, rate, volume) in [
      (Clip::Bird, 0.25 * level * outdoors, 0.15),
      (Clip::Owl, 0.03 * (1.0 - level) * outdoors, 0.12)
    ]
    .into_iter()
    {
      if rng.unit() < rate * delta
        && let Some(takes) = library.0.get(&clip)
        && !takes.is_empty()
        && let (take, secs) = takes[rng.below(takes.len())]
        && let angle = rng.unit() * TAU
        && let distance = rng.range(12.0, 35.0)
        && let at = camera.translation()
          + Vec3::new(
            angle.cos() * distance,
            rng.range(4.0, 12.0),
            angle.sin() * distance
          )
        && let speed = rng.range(0.92, 1.08)
      {
        voices.0.push(Sounding {
          voice: output.start(take, heard(&camera, at, volume, 14.0), speed, false),
          anchor: Anchor::At(at),
          volume,
          reach: 14.0,
          ends: real.elapsed_secs() + secs / speed + 0.2
        })
      }
    }
  }
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Library>()
    .init_resource::<Voices>()
    .add_systems(Startup, render)
    .add_systems(
      Update,
      (gather, pace, play, lay_beds, blend_beds, chant_walls, critters, place_voices)
        .chain()
    );
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn render_to_files() {
    let folder = std::path::Path::new("screenshots/audio");
    std::fs::create_dir_all(folder).unwrap();
    let only = std::env::var("CLIP").ok();
    for clip in Clip::all().into_iter().filter(|clip| {
      only.as_ref().is_none_or(|name| format!("{clip:?}").contains(name.as_str()))
    }) {
      let start = Instant::now();
      let waves = clip.render();
      let took = start.elapsed().as_secs_f32();
      for (variant, wave) in waves.iter().enumerate() {
        let name = format!("{clip:?}").replace(['(', ')'], "").replace("Cue", "");
        std::fs::write(folder.join(format!("{name}-{variant}.wav")), wave.wav()).unwrap();
        let finite = wave.0.iter().flatten().all(|x| x.is_finite());
        println!(
          "{name:>14}-{variant} {:6.2}s peak {:.3} rms {:.3} finite {finite} took {took:.2}s",
          wave.0[0].len() as f32 / synth::RATE,
          wave.peak(),
          wave.rms()
        );
        assert!(finite);
      }
    }
  }
}
