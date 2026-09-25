mod ambience;
mod music;
mod sfx;
mod synth;

use {crate::{humanoid::Motion,
             player::{MainCamera, Player},
             signal::{Cue, Engaged, Sound, WordWall},
             sky::Daylight,
             walker::Walker},
     bevy::{audio::{SpatialScale, Volume},
            platform::collections::HashMap,
            prelude::*},
     std::{f32::consts::{PI, TAU},
           sync::{Arc, Mutex,
                  mpsc::{Receiver, channel}},
           time::Instant},
     synth::{Rng, Wave}};

const CUES: [Cue; 23] = [
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
  Cue::Wingbeat,
  Cue::LevelUp,
  Cue::Coins
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
    Cue::DragonRoar => (1.0, 80.0),
    Cue::Wingbeat => (0.7, 40.0),
    Cue::LevelUp => (0.4, 10.0),
    Cue::Coins => (0.4, 5.0)
  }
}

struct Rendered {
  clip: Clip,
  wavs: Vec<Vec<u8>>,
  took: f32
}

#[derive(Resource)]
struct Rendering {
  results: Mutex<Receiver<Rendered>>,
  pending: usize,
  started: Instant
}

#[derive(Resource, Default)]
struct Library(HashMap<Clip, Vec<Handle<AudioSource>>>);

#[derive(Component)]
struct Bed {
  clip: Clip,
  level: f32
}

#[derive(Component)]
struct Chanting;

fn render(mut commands: Commands) {
  let clips = Clip::all();
  let pending = clips.len();
  let queue = Arc::new(Mutex::new(clips.into_iter().rev().collect::<Vec<_>>()));
  let (send, results) = channel();
  let workers = std::thread::available_parallelism()
    .map(|count| count.get() / 2)
    .unwrap_or(2)
    .clamp(2, 6);
  (0..workers).for_each(|_| {
    let (queue, send) = (queue.clone(), send.clone());
    std::thread::spawn(move || {
      std::iter::from_fn(|| queue.lock().ok()?.pop()).for_each(|clip| {
        let start = Instant::now();
        let wavs = clip.render().iter().map(Wave::wav).collect();
        send.send(Rendered { clip, wavs, took: start.elapsed().as_secs_f32() }).ok();
      })
    });
  });
  commands.insert_resource(Rendering {
    results: Mutex::new(results),
    pending,
    started: Instant::now()
  })
}

fn gather(
  mut rendering: ResMut<Rendering>,
  mut library: ResMut<Library>,
  mut sources: ResMut<Assets<AudioSource>>
) {
  let arrived: Vec<Rendered> = rendering
    .results
    .lock()
    .map(|results| results.try_iter().collect())
    .unwrap_or_default();
  arrived.into_iter().for_each(|Rendered { clip, wavs, took }| {
    debug!("synthesised {clip:?} in {took:.2}s");
    library.0.insert(
      clip,
      wavs
        .into_iter()
        .map(|bytes| sources.add(AudioSource { bytes: bytes.into() }))
        .collect()
    );
    rendering.pending -= 1;
    if rendering.pending == 0 {
      info!("audio synthesised in {:.2}s", rendering.started.elapsed().as_secs_f32())
    }
  })
}

fn listen(
  cameras: Query<Entity, (With<MainCamera>, Without<SpatialListener>)>,
  mut commands: Commands
) {
  cameras.iter().for_each(|camera| {
    commands.entity(camera).insert(SpatialListener::new(0.4));
  })
}

fn play(
  mut sounds: MessageReader<Sound>,
  library: Res<Library>,
  mut rng: Local<Rng>,
  mut commands: Commands
) {
  sounds.read().for_each(|&Sound { cue, at }| {
    if let Some(takes) = library.0.get(&Clip::Cue(cue))
      && !takes.is_empty()
    {
      let (volume, reach) = loudness(cue);
      let take = takes[rng.below(takes.len())].clone();
      let settings = PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume));
      match at {
        Some(at) => commands.spawn((
          AudioPlayer(take),
          settings
            .with_spatial(true)
            .with_spatial_scale(SpatialScale::new(1.0 / reach))
            .with_speed(rng.range(0.96, 1.04)),
          Transform::from_translation(at)
        )),
        None => commands.spawn((AudioPlayer(take), settings))
      };
    }
  })
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

fn lay_beds(library: Res<Library>, beds: Query<&Bed>, mut commands: Commands) {
  if library.is_changed() {
    [Clip::Wind, Clip::Night, Clip::Cave, Clip::Explore, Clip::Combat]
      .into_iter()
      .filter(|&clip| !beds.iter().any(|bed| bed.clip == clip))
      .filter_map(|clip| {
        library
          .0
          .get(&clip)
          .and_then(|takes| takes.first())
          .map(|take| (clip, take.clone()))
      })
      .for_each(|(clip, take)| {
        commands.spawn((
          AudioPlayer(take),
          PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
          Bed { clip, level: 0.0 }
        ));
      })
  }
}

fn blend_beds(
  time: Res<Time>,
  daylight: Res<Daylight>,
  engaged: Res<Engaged>,
  mut beds: Query<(&mut Bed, &mut AudioSink)>
) {
  let &Daylight { level, shelter } = daylight.into_inner();
  let outdoors = 1.0 - synth::smooth((shelter - 0.3) / 0.4);
  let fight = engaged.0.is_some() as u8 as f32;
  let delta = time.delta_secs();
  beds.iter_mut().for_each(|(mut bed, mut sink)| {
    let (target, rate) = match bed.clip {
      Clip::Wind => (0.18 * outdoors, 0.6),
      Clip::Night => (0.2 * (1.0 - level) * outdoors, 0.4),
      Clip::Cave => (0.4 * (1.0 - outdoors), 0.6),
      Clip::Explore => (0.2 * (1.0 - fight), 0.3),
      Clip::Combat => (0.3 * fight, if fight > 0.0 { 1.2 } else { 0.35 }),
      _ => (0.0, 1.0)
    };
    bed.level += (target - bed.level) * (1.0 - (-rate * delta).exp());
    sink.set_volume(Volume::Linear(bed.level))
  })
}

fn chant_walls(
  library: Res<Library>,
  walls: Query<Entity, (With<WordWall>, Without<Chanting>)>,
  mut commands: Commands
) {
  if let Some(take) = library.0.get(&Clip::Chant).and_then(|takes| takes.first()) {
    walls.iter().for_each(|wall| {
      commands.entity(wall).try_insert(Chanting);
      commands.spawn((
        AudioPlayer(take.clone()),
        PlaybackSettings::LOOP
          .with_volume(Volume::Linear(0.45))
          .with_spatial(true)
          .with_spatial_scale(SpatialScale::new(1.0 / 8.0)),
        Transform::from_xyz(0.0, 2.0, 0.0),
        ChildOf(wall)
      ));
    })
  }
}

fn critters(
  time: Res<Time>,
  daylight: Res<Daylight>,
  library: Res<Library>,
  camera: Single<&GlobalTransform, With<MainCamera>>,
  mut rng: Local<Rng>,
  mut commands: Commands
) {
  let &Daylight { level, shelter } = daylight.into_inner();
  let outdoors = 1.0 - synth::smooth((shelter - 0.3) / 0.4);
  let delta = time.delta_secs();
  [
    (Clip::Bird, 0.25 * level * outdoors, 0.15),
    (Clip::Owl, 0.03 * (1.0 - level) * outdoors, 0.12)
  ]
  .into_iter()
  .for_each(|(clip, rate, volume)| {
    if rng.unit() < rate * delta
      && let Some(takes) = library.0.get(&clip)
      && !takes.is_empty()
    {
      let angle = rng.unit() * TAU;
      let distance = rng.range(12.0, 35.0);
      let at = camera.translation()
        + Vec3::new(angle.cos() * distance, rng.range(4.0, 12.0), angle.sin() * distance);
      commands.spawn((
        AudioPlayer(takes[rng.below(takes.len())].clone()),
        PlaybackSettings::DESPAWN
          .with_volume(Volume::Linear(volume))
          .with_spatial(true)
          .with_spatial_scale(SpatialScale::new(1.0 / 14.0))
          .with_speed(rng.range(0.92, 1.08)),
        Transform::from_translation(at)
      ));
    }
  })
}

pub fn plugin(app: &mut App) {
  app.init_resource::<Library>().add_systems(Startup, render).add_systems(
    Update,
    (gather, listen, pace, play, lay_beds, blend_beds, chant_walls, critters).chain()
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
    Clip::all()
      .into_iter()
      .filter(|clip| only.as_ref().is_none_or(|name| format!("{clip:?}").contains(name.as_str())))
      .for_each(|clip| {
        let start = Instant::now();
        let waves = clip.render();
        let took = start.elapsed().as_secs_f32();
        waves.iter().enumerate().for_each(|(variant, wave)| {
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
        })
      })
  }
}
