#![recursion_limit = "256"]

mod atronach;
mod audio;
mod blocky;
mod blur;
mod cage;
mod cave;
mod cloud;
mod combat;
mod creature;
mod depths;
mod dragon;
mod face;
mod flora;
mod fx;
mod heli;
mod horse;
mod hud;
mod humanoid;
mod inventory;
mod landmark;
mod mist;
mod model;
mod noise;
mod npc;
mod opts;
mod par;
mod patrol;
mod paving;
mod place;
mod player;
mod ragdoll;
mod river;
mod robot;
mod sdf;
mod settlement;
mod shout;
mod signal;
mod site;
mod sky;
mod spider;
mod studio;
mod stuff;
mod terrain;
mod texture;
mod trail;
mod trail_cells;
mod treegiant;
mod uv;
mod walker;
mod wolf;
mod work;

use {avian3d::prelude::*,
     bevy::{camera::{Viewport, visibility::RenderLayers},
            prelude::*,
            render::{Render, RenderApp, RenderPlugin, RenderSystems,
                     render_resource::PipelineCache,
                     settings::{InstanceFlags, RenderCreation, WgpuSettings},
                     view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk}}}};

const WARM_UP_FRAMES: u32 = 3;
const WARM_UP_LIMIT: u32 = 40;

#[derive(Resource, Clone, Default)]
struct Compiling(std::sync::Arc<std::sync::atomic::AtomicUsize>);

fn count_compiling(compiling: Res<Compiling>, cache: Res<PipelineCache>) {
  compiling
    .0
    .store(cache.waiting_pipelines().count(), std::sync::atomic::Ordering::Relaxed);
}

fn watch_compiling(app: &mut App) {
  let compiling = Compiling::default();
  app.insert_resource(compiling.clone());
  app
    .sub_app_mut(RenderApp)
    .insert_resource(compiling)
    .add_systems(Render, count_compiling.in_set(RenderSystems::Cleanup));
}
const UNSEEN: usize = 7;

fn snapshot(
  time: Res<Time>,
  pending: Res<signal::Pending>,
  mut warmed: Local<Option<u32>>,
  mut taken: Local<bool>,
  mut cameras: Query<(Entity, &mut Camera)>,
  mut suns: Query<&mut DirectionalLight>,
  compiling: Res<Compiling>,
  mut commands: Commands
) {
  if let Some(at) = opts::opts().shot {
    let settled = time.elapsed_secs() > at && pending.idle();
    *warmed = warmed.map(|frames| frames + 1).or(settled.then_some(0));
    let drawing = warmed.is_some();
    let glimpse = Viewport { physical_size: UVec2::ONE, ..default() };
    for (entity, mut camera) in
      cameras.iter_mut().filter(|(_, camera)| camera.viewport.is_none() != drawing)
    {
      camera.viewport = (!drawing).then(|| glimpse.clone());
      match drawing {
        true => commands.entity(entity).remove::<RenderLayers>(),
        false => commands.entity(entity).insert(RenderLayers::layer(UNSEEN))
      };
    }
    for mut sun in suns.iter_mut().filter(|sun| sun.shadow_maps_enabled != drawing) {
      sun.shadow_maps_enabled = drawing
    }
    let compiled = compiling.0.load(std::sync::atomic::Ordering::Relaxed) == 0;
    let ready = !*taken
      && warmed.is_some_and(|frames| {
        (frames >= WARM_UP_FRAMES && compiled) || frames >= WARM_UP_LIMIT
      });
    if ready {
      *taken = true;
      let path = format!(
        "screenshots/shot-{}.png",
        std::env::var("SHOT_NAME").unwrap_or("latest".into())
      );
      let mut save = save_to_disk(path);
      commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>| {
          save(captured);
          std::process::exit(0)
        }
      );
    }
  }
}

fn press(
  time: Res<Time>,
  mut keys: ResMut<ButtonInput<KeyCode>>,
  mut buttons: ResMut<ButtonInput<MouseButton>>
) {
  let (now, before) = (time.elapsed_secs(), time.elapsed_secs() - time.delta_secs());
  let taps = opts::opts().press.iter().map(|&(at, ref name)| (at, at + 0.15, name));
  let holds = opts::opts().hold.iter().map(|&(from, to, ref name)| (from, to, name));
  for (from, to, name) in taps.chain(holds) {
    let (start, stop) = (before < from && from <= now, before < to && to <= now);
    let key = match name.as_str() {
      "LMB" => Err(MouseButton::Left),
      "RMB" => Err(MouseButton::Right),
      "Space" => Ok(KeyCode::Space),
      "Shift" => Ok(KeyCode::ShiftLeft),
      "Ctrl" => Ok(KeyCode::ControlLeft),
      "W" => Ok(KeyCode::KeyW),
      "E" => Ok(KeyCode::KeyE),
      "F" => Ok(KeyCode::KeyF),
      "Z" => Ok(KeyCode::KeyZ),
      "A" => Ok(KeyCode::KeyA),
      "S" => Ok(KeyCode::KeyS),
      "D" => Ok(KeyCode::KeyD),
      "I" => Ok(KeyCode::KeyI),
      "Tab" => Ok(KeyCode::Tab),
      "Enter" => Ok(KeyCode::Enter),
      "Escape" => Ok(KeyCode::Escape),
      "Up" => Ok(KeyCode::ArrowUp),
      "Down" => Ok(KeyCode::ArrowDown),
      other => panic!("press: unknown key {other}")
    };
    match (key, start, stop) {
      (Ok(key), true, _) => keys.press(key),
      (Ok(key), _, true) => keys.release(key),
      (Err(button), true, _) => buttons.press(button),
      (Err(button), _, true) => buttons.release(button),
      _ => {}
    }
  }
}

#[cfg(target_arch = "wasm32")]
fn unveil() {
  web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.get_element_by_id("loading"))
    .map(|loading| loading.remove());
}

#[cfg(not(target_arch = "wasm32"))]
fn unveil() {}

fn main() {
  let mut app = App::new();
  app.add_plugins(
    DefaultPlugins
      .build()
      .set(RenderPlugin {
        render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
          instance_flags: InstanceFlags::empty().with_env(),
          ..default()
        })),
        ..default()
      })
      .set(WindowPlugin {
        primary_window: Some(Window {
          title: "The Vibe Scrolls V: Clauderim".into(),
          resolution: (1600, 900).into(),
          fit_canvas_to_parent: true,
          present_mode: opts::opts()
            .shot
            .map_or(bevy::window::PresentMode::AutoVsync, |_| {
              bevy::window::PresentMode::AutoNoVsync
            }),
          ..default()
        }),
        ..default()
      })
  );
  match opts::opts().studio {
    Some(_) => app.add_plugins((
      signal::plugin,
      sky::plugin,
      blur::plugin,
      stuff::plugin,
      humanoid::plugin,
      wolf::plugin,
      fx::plugin,
      studio::plugin
    )),
    None => app
      .add_plugins(PhysicsPlugins::default())
      .add_plugins((
        signal::plugin,
        terrain::plugin,
        flora::plugin,
        sky::plugin,
        blur::plugin,
        cloud::plugin,
        mist::plugin,
        stuff::plugin,
        humanoid::plugin,
        walker::plugin,
        player::plugin,
        combat::plugin,
        creature::plugin,
        wolf::plugin,
        landmark::plugin
      ))
      .add_plugins((
        fx::plugin,
        shout::plugin,
        dragon::plugin,
        hud::plugin,
        inventory::plugin
      ))
      .add_plugins((
        cave::plugin,
        depths::plugin,
        paving::plugin,
        settlement::plugin,
        river::plugin,
        npc::plugin,
        ragdoll::plugin,
        robot::plugin,
        patrol::plugin,
        spider::plugin,
        heli::plugin,
        atronach::plugin,
        treegiant::plugin
      ))
      .add_plugins(audio::plugin)
      .add_systems(
        Last,
        unveil.run_if(|frames: Res<bevy::diagnostic::FrameCount>| frames.0 == 3)
      )
      .add_systems(
        PreUpdate,
        press.after(bevy::input::InputSystems).before(inventory::browse)
      )
  };
  app.add_plugins((work::plugin, watch_compiling)).add_systems(Update, snapshot).run();
}
