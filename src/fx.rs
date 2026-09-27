use {crate::{combat::Struck, creature::Foe, signal::FoeKind, texture},
     bevy::prelude::*,
     bevy_hanabi::{AccelModifier, AlphaMode as Blending, Attribute,
                   ColorOverLifetimeModifier, EffectAsset, EffectMaterial, ExprWriter,
                   Gradient, HanabiPlugin, LinearDragModifier, OrientMode,
                   OrientModifier, ParticleEffect, ParticleTextureModifier,
                   SetAttributeModifier, SetPositionSphereModifier, ShapeDimension,
                   SimulationSpace, SizeOverLifetimeModifier, SpawnerSettings,
                   VectorType}};

#[derive(Clone, Copy)]
struct Plume {
  thrust: Vec3,
  spread: f32,
  source: f32,
  girth: f32,
  life: f32,
  lift: f32,
  drag: f32,
  space: SimulationSpace,
  blend: Blending
}

fn fire_colors() -> Gradient<Vec4> {
  Gradient::from_keys([
    (0.0, Vec4::new(9.0, 4.2, 1.2, 1.0)),
    (0.3, Vec4::new(6.0, 1.9, 0.3, 1.0)),
    (0.7, Vec4::new(2.0, 0.4, 0.06, 0.6)),
    (1.0, Vec4::new(0.3, 0.06, 0.02, 0.0))
  ])
}

fn blood_colors() -> Gradient<Vec4> {
  Gradient::from_keys([
    (0.0, Vec4::new(0.35, 0.02, 0.01, 1.0)),
    (1.0, Vec4::new(0.18, 0.01, 0.0, 0.0))
  ])
}

fn spark_colors() -> Gradient<Vec4> {
  Gradient::from_keys([
    (0.0, Vec4::new(12.0, 8.0, 3.0, 1.0)),
    (1.0, Vec4::new(3.0, 0.8, 0.1, 0.0))
  ])
}

fn dust_colors() -> Gradient<Vec4> {
  Gradient::from_keys([
    (0.0, Vec4::new(0.9, 0.95, 1.0, 0.0)),
    (0.1, Vec4::new(0.85, 0.9, 1.0, 0.3)),
    (1.0, Vec4::new(0.7, 0.75, 0.85, 0.0))
  ])
}

fn build(plume: Plume, spawner: SpawnerSettings, colors: Gradient<Vec4>) -> EffectAsset {
  let writer = ExprWriter::new();
  let drift = (writer.rand(VectorType::VEC3F) * writer.lit(2.0) - writer.lit(1.0))
    * writer.lit(plume.spread);
  let thrust = writer.add_property("thrust", plume.thrust.into());
  let velocity =
    SetAttributeModifier::new(Attribute::VELOCITY, (drift + writer.prop(thrust)).expr());
  let age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
  let lifetime = SetAttributeModifier::new(
    Attribute::LIFETIME,
    writer.lit(plume.life * 0.6).uniform(writer.lit(plume.life)).expr()
  );
  let drag = LinearDragModifier::new(writer.lit(plume.drag).expr());
  let lift = AccelModifier::new(writer.lit(Vec3::Y * plume.lift).expr());
  let size = SizeOverLifetimeModifier {
    gradient: Gradient::from_keys([
      (0.0, Vec3::splat(plume.girth * 0.5)),
      (0.3, Vec3::splat(plume.girth)),
      (1.0, Vec3::splat(plume.girth * 0.3))
    ]),
    screen_space_size: false
  };
  let slot = writer.lit(0u32).expr();
  let position = SetPositionSphereModifier {
    center: writer.lit(Vec3::ZERO).expr(),
    radius: writer.lit(plume.source).expr(),
    dimension: ShapeDimension::Volume
  };
  let mut module = writer.finish();
  module.add_texture_slot("puff");
  EffectAsset::new(8192, spawner, module)
    .with_simulation_space(plume.space)
    .with_alpha_mode(plume.blend)
    .init(position)
    .init(velocity)
    .init(age)
    .init(lifetime)
    .update(drag)
    .update(lift)
    .render(ColorOverLifetimeModifier::new(colors))
    .render(size)
    .render(OrientModifier::new(OrientMode::FaceCameraPosition))
    .render(ParticleTextureModifier::new(slot))
}

#[derive(Resource)]
pub struct Effects {
  pub campfire: Handle<EffectAsset>,
  pub torch: Handle<EffectAsset>,
  pub breath: Handle<EffectAsset>,
  pub blood: Handle<EffectAsset>,
  pub sparks: Handle<EffectAsset>,
  pub gust: Handle<EffectAsset>,
  pub puff: Handle<Image>
}

impl Effects {
  pub fn emit(&self, effect: &Handle<EffectAsset>) -> impl Bundle {
    (ParticleEffect::new(effect.clone()), EffectMaterial {
      images: vec![self.puff.clone()]
    })
  }
}

fn puff() -> Image {
  texture::shade(64, |u, v| {
    let away = Vec2::new(u - 0.5, v - 0.5).length() * 2.0;
    let ragged = away * (0.8 + 0.4 * texture::tile_noise(u, v, 6, 3));
    let body = ((1.0 - ragged) / 0.6).clamp(0.0, 1.0);
    body * body * (3.0 - 2.0 * body)
  })
}

fn prepare(
  mut commands: Commands,
  mut effects: ResMut<Assets<EffectAsset>>,
  mut images: ResMut<Assets<Image>>
) {
  let flame = Plume {
    thrust: Vec3::Y * 1.3,
    spread: 0.3,
    source: 0.3,
    girth: 0.5,
    life: 0.9,
    lift: 0.8,
    drag: 1.0,
    space: SimulationSpace::Global,
    blend: Blending::Add
  };
  let mut puff = puff();
  puff.texture_descriptor.format =
    bevy::render::render_resource::TextureFormat::Rgba8Unorm;
  let alpha_puff = {
    let data = puff.data.clone().unwrap_or_default();
    let reshaped: Vec<u8> =
      data.chunks_exact(4).flat_map(|pixel| [255, 255, 255, pixel[0]]).collect();
    puff.data = Some(reshaped);
    puff
  };
  commands.insert_resource(Effects {
    campfire: effects.add(build(
      flame,
      SpawnerSettings::rate(90.0.into()),
      fire_colors()
    )),
    torch: effects.add(build(
      Plume {
        thrust: Vec3::Y * 0.9,
        spread: 0.12,
        source: 0.05,
        girth: 0.16,
        life: 0.45,
        ..flame
      },
      SpawnerSettings::rate(50.0.into()),
      fire_colors()
    )),
    breath: effects.add(build(
      Plume {
        thrust: Vec3::NEG_Z * 26.0,
        spread: 3.2,
        source: 0.5,
        girth: 2.6,
        life: 0.9,
        lift: 1.5,
        drag: 1.4,
        ..flame
      },
      SpawnerSettings::rate(420.0.into()),
      fire_colors()
    )),
    blood: effects.add(build(
      Plume {
        thrust: Vec3::Y * 1.2,
        spread: 2.2,
        source: 0.1,
        girth: 0.09,
        life: 0.6,
        lift: -9.0,
        drag: 0.5,
        blend: Blending::Blend,
        ..flame
      },
      SpawnerSettings::once(40.0.into()),
      blood_colors()
    )),
    sparks: effects.add(build(
      Plume {
        thrust: Vec3::Y * 1.5,
        spread: 4.0,
        source: 0.05,
        girth: 0.05,
        life: 0.35,
        lift: -9.0,
        drag: 0.5,
        ..flame
      },
      SpawnerSettings::once(50.0.into()),
      spark_colors()
    )),
    gust: effects.add(build(
      Plume {
        thrust: Vec3::NEG_Z * 26.0 + Vec3::Y * 1.0,
        spread: 3.2,
        source: 0.4,
        girth: 1.8,
        life: 0.9,
        lift: 0.0,
        drag: 2.2,
        space: SimulationSpace::Local,
        blend: Blending::Blend
      },
      SpawnerSettings::once(420.0.into()),
      dust_colors()
    )),
    puff: images.add(alpha_puff)
  });
}

#[derive(Component)]
pub struct Fleeting(pub f32);

fn fade_out(
  time: Res<Time>,
  mut commands: Commands,
  mut fleeting: Query<(Entity, &mut Fleeting)>
) {
  for (entity, mut left) in fleeting.iter_mut() {
    left.0 -= time.delta_secs();
    if left.0 <= 0.0 {
      commands.entity(entity).despawn();
    }
  }
}

fn splatter(
  mut struck: MessageReader<Struck>,
  effects: Option<Res<Effects>>,
  foes: Query<&Foe>,
  mut commands: Commands
) {
  if let Some(effects) = effects {
    for hit in struck.read() {
      let dry = foes
        .get(hit.target)
        .is_ok_and(|foe| matches!(foe.kind, FoeKind::Draugr | FoeKind::DraugrOverlord));
      let effect = if hit.blocked || dry { &effects.sparks } else { &effects.blood };
      commands.spawn((
        effects.emit(effect),
        Fleeting(1.5),
        Transform::from_translation(hit.at)
      ));
    }
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_plugins(HanabiPlugin)
    .add_systems(PreStartup, prepare)
    .add_systems(Update, (fade_out, splatter));
}
