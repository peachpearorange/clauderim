mod combat;
mod humanoid;
mod model;
mod noise;
mod opts;
mod place;
mod signal;
mod player;
mod sky;
mod stuff;
mod terrain;
mod texture;
mod walker;

use {avian3d::prelude::*,
     bevy::{prelude::*,
            render::view::screenshot::{Screenshot, save_to_disk}}};

fn snapshot(time: Res<Time>, mut taken: Local<Option<f32>>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
  if let Some(at) = opts::opts().shot
    && time.elapsed_secs() > at
  {
    match *taken {
      None => {
        let path = format!("screenshots/shot-{}.png", std::env::var("SHOT_NAME").unwrap_or("latest".into()));
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

fn main() {
  App::new()
    .add_plugins((
      DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
          title: "Skyrim II".into(),
          resolution: (1600, 900).into(),
          ..default()
        }),
        ..default()
      }),
      PhysicsPlugins::default()
    ))
    .add_plugins((
      signal::plugin,
      terrain::plugin,
      sky::plugin,
      stuff::plugin,
      humanoid::plugin,
      walker::plugin,
      player::plugin,
      combat::plugin
    ))
    .add_systems(Update, snapshot)
    .run();
}
