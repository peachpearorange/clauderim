use {crate::opts::opts,
     bevy::{anti_alias::taa::TemporalAntiAliasing,
            camera::Exposure,
            core_pipeline::tonemapping::Tonemapping,
            light::{AtmosphereEnvironmentMapLight, CascadeShadowConfigBuilder,
                    EnvironmentMapLight, GeneratedEnvironmentMapLight, SunDisk,
                    atmosphere::ScatteringMedium, light_consts::lux},
            pbr::{AtmosphereSettings, DistanceFog, FogFalloff},
            post_process::bloom::Bloom,
            prelude::*,
            render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection}},
     std::f32::consts::PI};

const DAY_EXPOSURE: f32 = 13.2;
const NIGHT_EXPOSURE: f32 = 6.2;
const EXPOSURE_BY_ELEVATION: [(f32, f32); 7] = [
  (-0.25, NIGHT_EXPOSURE),
  (-0.15, 6.6),
  (-0.065, 7.1),
  (0.0, 10.0),
  (0.06, 11.6),
  (0.2, 12.7),
  (0.45, DAY_EXPOSURE)
];
const UNDERGROUND_EXPOSURE: f32 = 7.6;
const BRIGHTENING: f32 = 1.4;
const DARKENING: f32 = 0.45;
const SUN_DISK: SunDisk = SunDisk { angular_size: 0.017, intensity: 1.0 };
const MOON_DISK: SunDisk = SunDisk { angular_size: 0.03, intensity: 60.0 };
const SUNRISE: f32 = 5.0;
const SUNSET: f32 = 20.0;
const DAYLIGHT_DEPTH: Vec3 = Vec3::new(0.014, 0.038, 0.1);
const SKY_FILL: f32 = 1.9;
const HAZE_VISIBILITY: f32 = 2000.0;
const DAY_HAZE: Vec3 = Vec3::new(0.70, 0.75, 0.82);
const DUSK_HAZE: Vec3 = Vec3::new(0.80, 0.66, 0.64);
const NIGHT_HAZE: Vec3 = Vec3::new(0.09, 0.14, 0.19);
const COOL: f32 = -0.025;
const WARM: f32 = 0.08;
const BLUSH: f32 = 0.02;
const MOON_TINT: Color = Color::srgb(0.62, 0.74, 1.0);
pub const MOONLIGHT: f32 = 90.0;

#[derive(Resource)]
pub struct Clock {
  pub hour: f32,
  pub hours_per_second: f32,
  days: u32
}

impl Clock {
  pub fn night(&self) -> u32 { self.days + u32::from(self.hour >= 12.0) }
}

impl Default for Clock {
  fn default() -> Self {
    Self { hour: opts().hour, hours_per_second: 24.0 / opts().day, days: 0 }
  }
}

#[derive(Resource, Default)]
pub struct Daylight {
  pub level: f32,
  pub shelter: f32,
  pub snap: u8
}

#[derive(Component)]
struct Sun;

fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
  let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

const BLUR_FROM: f32 = 250.0;
const BLUR_TO: f32 = 2400.0;
const BLUR_SPREAD: f32 = 2.2;

const MIRROR_SIZE: u32 = 256;
const MIRROR_REFRESH: f32 = 0.5;
const MIRROR_WARMUP: f32 = 3.0;

pub fn lens() -> impl Bundle {
  (
    AtmosphereSettings { aerial_view_lut_max_distance: 1.2e4, ..default() },
    AtmosphereEnvironmentMapLight { size: UVec2::splat(MIRROR_SIZE), ..default() },
    Exposure { ev100: DAY_EXPOSURE },
    Tonemapping::AcesFitted,
    ColorGrading {
      global: ColorGradingGlobal {
        temperature: COOL,
        post_saturation: 0.88,
        ..default()
      },
      shadows: ColorGradingSection {
        saturation: 0.82,
        lift: 0.01,
        gamma: 1.06,
        ..default()
      },
      midtones: ColorGradingSection { saturation: 0.88, ..default() },
      highlights: ColorGradingSection { saturation: 0.9, ..default() }
    },
    Bloom { intensity: 0.12, ..Bloom::NATURAL },
    DistanceFog {
      color: Color::srgb(DAY_HAZE.x, DAY_HAZE.y, DAY_HAZE.z),
      falloff: FogFalloff::from_visibility_colors(
        HAZE_VISIBILITY * crate::opts::opts().haze,
        Color::srgb(0.46, 0.50, 0.58),
        Color::srgb(0.76, 0.80, 0.86)
      ),
      ..default()
    },
    Msaa::Off,
    TemporalAntiAliasing::default(),
    crate::blur::DistanceBlur { from: BLUR_FROM, to: BLUR_TO, spread: BLUR_SPREAD }
  )
}

pub const SHADOW_DISTANCE: f32 = 320.0;

fn spawn_sky(mut commands: Commands, mut media: ResMut<Assets<ScatteringMedium>>) {
  commands
    .spawn(bevy::light::Atmosphere::earth(media.add(ScatteringMedium::earth(256, 256))));
  commands.spawn((
    Name::new("Sun"),
    Sun,
    DirectionalLight {
      illuminance: lux::RAW_SUNLIGHT,
      shadow_maps_enabled: true,
      ..default()
    },
    SUN_DISK,
    crate::humanoid::shadowing(),
    CascadeShadowConfigBuilder {
      num_cascades: 4,
      first_cascade_far_bound: 14.0,
      maximum_distance: SHADOW_DISTANCE,
      ..default()
    }
    .build(),
    Transform::default()
  ));
}

pub fn toward_sun(hour: f32) -> Vec3 {
  let (span, since_rise) = (SUNSET - SUNRISE, (hour - SUNRISE).rem_euclid(24.0));
  let arc = (since_rise < span)
    .then(|| since_rise / span * PI)
    .unwrap_or_else(|| PI + (since_rise - span) / (24.0 - span) * PI);
  Vec3::new(arc.cos(), arc.sin() * 0.62, arc.sin() * 0.5 + 0.12).normalize()
}

pub fn toward_moon(sun: Vec3) -> Vec3 {
  Vec3::new(-sun.x, -sun.y, sun.z * 0.6 + 0.3).normalize()
}

pub fn reddened(ray: Vec3, zenith_depth: Vec3) -> Vec3 {
  let elevation = ray.y.max(0.008).asin();
  let air_mass = |elevation: f32| {
    1.0 / (elevation.sin() + 0.50572 * (elevation.to_degrees() + 6.07995).powf(-1.6364))
  };
  (-(air_mass(elevation) - air_mass(PI / 2.0)) * zenith_depth).exp()
}

fn cycle_day(
  time: Res<Time>,
  mut adapted: Local<bool>,
  mut clock: ResMut<Clock>,
  mut daylight: ResMut<Daylight>,
  sun: Single<(&mut Transform, &mut DirectionalLight, &mut SunDisk), With<Sun>>,
  mut lenses: Query<(
    &mut Exposure,
    &mut DistanceFog,
    &mut ColorGrading,
    Option<&mut EnvironmentMapLight>,
    Option<&mut TemporalAntiAliasing>
  )>
) {
  let passed = clock.hour + clock.hours_per_second * time.delta_secs();
  clock.days += u32::from(passed >= 24.0);
  clock.hour = passed.rem_euclid(24.0);
  let sun_ray = toward_sun(clock.hour);
  let day = smooth(-0.3, 0.3, sun_ray.y);
  let sunlit = smooth(-0.045, 0.02, sun_ray.y);
  let moonlit = smooth(-0.045, -0.1, sun_ray.y);
  let golden = smooth(0.3, 0.04, sun_ray.y) * smooth(-0.16, -0.02, sun_ray.y);
  let dark = smooth(-0.02, -0.2, sun_ray.y);
  daylight.level = smooth(-0.06, 0.14, sun_ray.y);
  let (mut transform, mut light, mut disk) = sun.into_inner();
  let above = sunlit > 0.0;
  let warmth = reddened(sun_ray, DAYLIGHT_DEPTH);
  let ray = above.then_some(sun_ray).unwrap_or(toward_moon(sun_ray));
  *transform = Transform::default().looking_to(-ray, Vec3::Y);
  light.illuminance =
    above.then_some(lux::RAW_SUNLIGHT * sunlit).unwrap_or(MOONLIGHT * moonlit);
  light.color =
    above.then(|| Color::linear_rgb(warmth.x, warmth.y, warmth.z)).unwrap_or(MOON_TINT);
  *disk = above.then_some(SUN_DISK).unwrap_or(MOON_DISK);
  let outdoors = 1.0 - daylight.shelter;
  let settled = EXPOSURE_BY_ELEVATION
    .windows(2)
    .find(|pair| sun_ray.y <= pair[1].0)
    .map_or(DAY_EXPOSURE, |pair| {
      let ((low, dim), (high, bright)) = (pair[0], pair[1]);
      dim.lerp(bright, ((sun_ray.y - low) / (high - low)).clamp(0.0, 1.0))
    })
    .lerp(UNDERGROUND_EXPOSURE, daylight.shelter);
  let haze = DAY_HAZE.lerp(DUSK_HAZE, golden).lerp(NIGHT_HAZE, dark);
  let adapting = *adapted && daylight.snap == 0;
  *adapted = !lenses.is_empty();
  daylight.snap = daylight.snap.saturating_sub(1);
  for (mut exposure, mut fog, mut grading, ambient, history) in lenses.iter_mut() {
    if let Some(mut history) = history
      && !adapting
    {
      history.reset = true;
    }
    let rate = (settled > exposure.ev100).then_some(BRIGHTENING).unwrap_or(DARKENING);
    exposure.ev100 = adapting
      .then(|| exposure.ev100.lerp(settled, 1.0 - (-rate * time.delta_secs()).exp()))
      .unwrap_or(settled);
    if let Some(mut ambient) = ambient {
      ambient.intensity = 0.015
        + (SKY_FILL * (1.0 + 1.2 * golden + 1.5 * dark) - 0.015) * outdoors.powf(2.0);
    }
    fog.color = Color::srgba(haze.x, haze.y, haze.z, outdoors * (0.6 + 0.4 * day));
    grading.global.temperature = COOL.lerp(WARM, golden * outdoors);
    grading.global.tint = golden * outdoors * BLUSH;
  }
}

const SHADOW_REACH: f32 = 4.0;

#[derive(Component)]
pub struct CloseShadows;

fn shade_close_lights(
  camera: Single<&GlobalTransform, With<crate::player::MainCamera>>,
  mut lights: Query<(&mut PointLight, &GlobalTransform), With<CloseShadows>>
) {
  for (mut light, place) in lights.iter_mut() {
    let close =
      place.translation().distance(camera.translation()) < light.range * SHADOW_REACH;
    if light.shadow_maps_enabled != close {
      light.shadow_maps_enabled = close;
    }
  }
}

fn refresh_mirror(
  time: Res<Time>,
  mut commands: Commands,
  mut kept: Local<Option<GeneratedEnvironmentMapLight>>,
  mut age: Local<f32>,
  mut drawn: Local<u32>,
  camera: Single<
    (Entity, Option<&GeneratedEnvironmentMapLight>, Has<EnvironmentMapLight>),
    With<crate::player::MainCamera>
  >
) {
  let (entity, generated, lit) = *camera;
  *age += time.delta_secs();
  match (generated, lit) {
    (Some(generated), true) if time.elapsed_secs() > MIRROR_WARMUP && *drawn >= 2 => {
      *kept = Some(generated.clone());
      commands.entity(entity).remove::<GeneratedEnvironmentMapLight>();
      *age = 0.0;
      *drawn = 0;
    }
    (Some(_), true) => *drawn += 1,
    (None, true) if *age > MIRROR_REFRESH => {
      if let Some(generated) = kept.clone() {
        commands.entity(entity).insert(generated);
      }
    }
    _ => {}
  }
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Clock>()
    .init_resource::<Daylight>()
    .add_systems(Startup, spawn_sky)
    .add_systems(Update, (cycle_day, shade_close_lights, refresh_mirror));
}
