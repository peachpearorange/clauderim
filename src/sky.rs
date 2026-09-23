use {crate::opts::opts,
     bevy::{anti_alias::fxaa::Fxaa,
            camera::Exposure,
            core_pipeline::tonemapping::Tonemapping,
            light::{AtmosphereEnvironmentMapLight, CascadeShadowConfigBuilder, SunDisk,
                    atmosphere::ScatteringMedium, light_consts::lux},
            pbr::{AtmosphereSettings, DistanceFog, FogFalloff},
            post_process::bloom::Bloom,
            prelude::*},
     std::f32::consts::PI};

const DAY_EXPOSURE: f32 = 13.2;
const NIGHT_EXPOSURE: f32 = 7.4;
const MOONLIGHT: f32 = 90.0;

#[derive(Resource)]
pub struct Clock {
  pub hour: f32,
  pub hours_per_second: f32
}

impl Default for Clock {
  fn default() -> Self { Self { hour: opts().hour, hours_per_second: 24.0 / opts().day } }
}

#[derive(Resource, Default)]
pub struct Daylight {
  pub level: f32,
  pub shelter: f32
}

#[derive(Component)]
struct Sun;

pub fn lens() -> impl Bundle {
  (
    AtmosphereSettings { aerial_view_lut_max_distance: 1.2e4, ..default() },
    AtmosphereEnvironmentMapLight::default(),
    Exposure { ev100: DAY_EXPOSURE },
    Tonemapping::AcesFitted,
    Bloom { intensity: 0.12, ..Bloom::NATURAL },
    DistanceFog {
      color: Color::srgba(0.62, 0.68, 0.76, 1.0),
      directional_light_color: Color::srgba(1.0, 0.92, 0.78, 0.4),
      directional_light_exponent: 24.0,
      falloff: FogFalloff::from_visibility_colors(
        3200.0,
        Color::srgb(0.42, 0.48, 0.56),
        Color::srgb(0.78, 0.82, 0.88)
      )
    },
    Msaa::Off,
    Fxaa::default()
  )
}

fn spawn_sky(mut commands: Commands, mut media: ResMut<Assets<ScatteringMedium>>) {
  commands.spawn(bevy::light::Atmosphere::earth(media.add(ScatteringMedium::earth(256, 256))));
  commands.spawn((
    Name::new("Sun"),
    Sun,
    DirectionalLight {
      illuminance: lux::RAW_SUNLIGHT,
      shadow_maps_enabled: true,
      ..default()
    },
    SunDisk::EARTH,
    CascadeShadowConfigBuilder {
      num_cascades: 4,
      first_cascade_far_bound: 14.0,
      maximum_distance: 320.0,
      ..default()
    }
    .build(),
    Transform::default()
  ));
}

fn toward_sun(hour: f32) -> Vec3 {
  let arc = (hour - 6.0) / 12.0 * PI;
  Vec3::new(arc.cos(), arc.sin() * 0.62, arc.sin() * 0.5 + 0.12).normalize()
}

fn cycle_day(
  time: Res<Time>,
  mut clock: ResMut<Clock>,
  mut daylight: ResMut<Daylight>,
  sun: Single<(&mut Transform, &mut DirectionalLight, &mut SunDisk), With<Sun>>,
  mut lenses: Query<(&mut Exposure, &mut AtmosphereEnvironmentMapLight, &mut DistanceFog)>
) {
  clock.hour = (clock.hour + clock.hours_per_second * time.delta_secs()).rem_euclid(24.0);
  let sun_ray = toward_sun(clock.hour);
  let day = ((sun_ray.y + 0.06) / 0.2).clamp(0.0, 1.0);
  daylight.level = day;
  let (mut transform, mut light, mut disk) = sun.into_inner();
  let moon_ray = Vec3::new(-sun_ray.x, -sun_ray.y, sun_ray.z * 0.6 + 0.3).normalize();
  let ray = (day > 0.0).then_some(sun_ray).unwrap_or(moon_ray);
  *transform = Transform::default().looking_to(-ray, Vec3::Y);
  light.illuminance = (day > 0.0).then_some(lux::RAW_SUNLIGHT).unwrap_or(MOONLIGHT);
  light.color = (day > 0.0)
    .then_some(Color::WHITE)
    .unwrap_or(Color::srgb(0.62, 0.72, 1.0));
  *disk = (day > 0.0)
    .then_some(SunDisk::EARTH)
    .unwrap_or(SunDisk { angular_size: 0.03, intensity: 60.0 });
  let outdoors = 1.0 - daylight.shelter;
  lenses.iter_mut().for_each(|(mut exposure, mut ambient, mut fog)| {
    exposure.ev100 =
      NIGHT_EXPOSURE.lerp(DAY_EXPOSURE, day.powf(0.5)) - 4.2 * daylight.shelter;
    ambient.intensity = 0.04 + 0.96 * outdoors;
    fog.color = Color::srgba(0.62, 0.68, 0.76, outdoors * (0.25 + 0.75 * day));
  });
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Clock>()
    .init_resource::<Daylight>()
    .add_systems(Startup, spawn_sky)
    .add_systems(Update, cycle_day);
}
