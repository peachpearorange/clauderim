use {crate::sky::{self, Clock},
     bevy::{asset::embedded_asset,
            light::{NotShadowCaster, NotShadowReceiver, light_consts::lux},
            pbr::{Material, MaterialPlugin},
            prelude::*,
            render::render_resource::AsBindGroup,
            shader::ShaderRef},
     std::f32::consts::PI};

const ALTITUDE: f32 = 1900.0;
const SPAN: f32 = 60000.0;
const WIND: Vec2 = Vec2::new(6.5, 2.2);
const COVERAGE: f32 = 0.22;
const ZENITH_DEPTH: Vec3 = Vec3::new(0.06, 0.12, 0.27);

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
struct Cloud {
  #[uniform(0)]
  toward_light: Vec4,
  #[uniform(1)]
  light: Vec4,
  #[uniform(2)]
  ambient: Vec4,
  #[uniform(3)]
  drift: Vec4,
  #[uniform(4)]
  night: Vec4
}

impl Material for Cloud {
  fn fragment_shader() -> ShaderRef { "embedded://skyrim2/cloud.wgsl".into() }

  fn alpha_mode(&self) -> AlphaMode { AlphaMode::Premultiplied }

  fn enable_prepass() -> bool { false }

  fn enable_shadows() -> bool { false }
}

fn aurora_strength(day: u32) -> f32 {
  let roll = (day.wrapping_mul(2_654_435_761) >> 16) as f32 / 65536.0;
  0.35 + 0.65 * roll
}

#[derive(Resource)]
struct Canopy(Handle<Cloud>);

fn spawn_clouds(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut clouds: ResMut<Assets<Cloud>>
) {
  let cloud = clouds.add(Cloud::default());
  commands.insert_resource(Canopy(cloud.clone()));
  commands.spawn((
    Name::new("Clouds"),
    Mesh3d(meshes.add(Plane3d::default().mesh().size(SPAN, SPAN))),
    MeshMaterial3d(cloud),
    Transform::from_xyz(0.0, ALTITUDE, 0.0).with_rotation(Quat::from_rotation_x(PI)),
    NotShadowCaster,
    NotShadowReceiver
  ));
}

pub fn lighting(hour: f32) -> (Vec3, Vec3, Vec3) {
  let sun = sky::toward_sun(hour);
  let moon = sky::toward_moon(sun);
  let sunset = sun.y.clamp(-0.1, 0.0) / 0.1 + 1.0;
  let day = (sun.y / 0.35).clamp(0.0, 1.0);
  let twilight = ((sun.y + 0.2) / 0.2).clamp(0.0, 1.0);
  let (toward_light, light) = (sun.y > -0.08)
    .then(|| {
      (sun, sky::reddened(sun, ZENITH_DEPTH) * lux::RAW_SUNLIGHT * sunset * sunset)
    })
    .unwrap_or((
      moon,
      Vec3::new(0.62, 0.72, 1.0) * sky::MOONLIGHT * 0.3 * (moon.y / 0.15).clamp(0.0, 1.0)
    ));
  let ambient = Vec3::new(0.55, 0.64, 0.82) * 9000.0 * day.powf(1.3)
    + Vec3::new(0.62, 0.5, 0.55) * 900.0 * twilight * twilight * (1.0 - day)
    + Vec3::new(0.5, 0.6, 0.9) * sky::MOONLIGHT * 0.035;
  (toward_light, light, ambient)
}

fn light_clouds(
  time: Res<Time>,
  clock: Res<Clock>,
  canopy: Res<Canopy>,
  mut clouds: ResMut<Assets<Cloud>>
) {
  let sun = sky::toward_sun(clock.hour);
  let (toward_light, light, ambient) = lighting(clock.hour);
  let dark = (-(sun.y + 0.04) / 0.16).clamp(0.0, 1.0);
  if let Some(mut cloud) = clouds.get_mut(&canopy.0) {
    *cloud = Cloud {
      toward_light: toward_light.extend(0.0),
      light: light.extend(0.0),
      ambient: ambient.extend(0.0),
      drift: (WIND * time.elapsed_secs()).extend(COVERAGE).extend(0.0),
      night: Vec4::new(
        dark * dark,
        dark * dark * aurora_strength(clock.night()),
        time.elapsed_secs(),
        0.0
      )
    };
  }
}

pub fn plugin(app: &mut App) {
  embedded_asset!(app, "cloud.wgsl");
  app
    .add_plugins(MaterialPlugin::<Cloud>::default())
    .add_systems(Startup, spawn_clouds)
    .add_systems(Update, light_clouds);
}
