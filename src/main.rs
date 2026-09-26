mod audio;
mod cave;
mod cloud;
mod combat;
mod creature;
mod dragon;
mod flora;
mod fx;
mod hud;
mod humanoid;
mod inventory;
mod landmark;
mod model;
mod noise;
mod opts;
mod par;
mod paving;
mod place;
mod player;
mod sdf;
mod settlement;
mod shout;
mod signal;
mod sky;
mod stuff;
mod terrain;
mod texture;
mod walker;
mod wolf;

use {avian3d::prelude::*,
     bevy::{prelude::*,
            render::{RenderPlugin,
                     settings::{InstanceFlags, RenderCreation, WgpuSettings},
                     view::screenshot::{Screenshot, save_to_disk}}}};

fn snapshot(
  time: Res<Time>,
  mut taken: Local<Option<f32>>,
  mut commands: Commands,
  mut exit: MessageWriter<AppExit>
) {
  if let Some(at) = opts::opts().shot
    && time.elapsed_secs() > at
  {
    match *taken {
      None => {
        let path = format!(
          "screenshots/shot-{}.png",
          std::env::var("SHOT_NAME").unwrap_or("latest".into())
        );
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *taken = Some(time.elapsed_secs());
      }
      Some(when) if time.elapsed_secs() > when + 1.5 => {
        exit.write(AppExit::Success);
      }
      _ => {}
    }
  }
}

fn press(
  time: Res<Time>,
  mut keys: ResMut<ButtonInput<KeyCode>>,
  mut buttons: ResMut<ButtonInput<MouseButton>>
) {
  let (now, before) = (time.elapsed_secs(), time.elapsed_secs() - time.delta_secs());
  opts::opts().press.iter().for_each(|&(at, ref name)| {
    let (start, stop) =
      (before < at && at <= now, before < at + 0.15 && at + 0.15 <= now);
    let key = match name.as_str() {
      "LMB" => Err(MouseButton::Left),
      "RMB" => Err(MouseButton::Right),
      "Space" => Ok(KeyCode::Space),
      "Shift" => Ok(KeyCode::ShiftLeft),
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
  });
}

fn main() {
  App::new()
    .add_plugins((
      DefaultPlugins
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
        }),
      PhysicsPlugins::default()
    ))
    .add_plugins((
      signal::plugin,
      terrain::plugin,
      flora::plugin,
      sky::plugin,
      cloud::plugin,
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
    .add_plugins((cave::plugin, paving::plugin, settlement::plugin))
    .add_plugins(audio::plugin)
    .add_systems(Update, snapshot)
    .add_systems(
      PreUpdate,
      press.after(bevy::input::InputSystems).before(inventory::browse)
    )
    .run();
}
