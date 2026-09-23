use {crate::texture,
     bevy::{math::Affine2, prelude::*},
     enum_assoc::Assoc};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Assoc)]
#[func(pub const fn roughness(self) -> f32)]
#[func(pub const fn metallic(self) -> f32 { 0.0 })]
#[func(pub const fn reflectance(self) -> f32 { 0.3 })]
#[func(pub const fn grain(self) -> Grain)]
#[func(pub const fn tiling(self) -> f32 { 1.0 })]
#[func(pub const fn glow(self) -> LinearRgba { LinearRgba::BLACK })]
#[func(pub const fn two_sided(self) -> bool { false })]
pub enum Stuff {
  #[assoc(roughness = 0.75, grain = Grain::Plain)]
  Skin,
  #[assoc(roughness = 0.95, grain = Grain::Fur, tiling = 2.0)]
  Fur,
  #[assoc(roughness = 0.8, grain = Grain::Leather, tiling = 2.0)]
  Leather,
  #[assoc(roughness = 0.92, grain = Grain::Leather, tiling = 3.0)]
  Cloth,
  #[assoc(roughness = 0.42, metallic = 0.9, reflectance = 0.5, grain = Grain::Metal)]
  Iron,
  #[assoc(roughness = 0.28, metallic = 1.0, reflectance = 0.6, grain = Grain::Metal)]
  Steel,
  #[assoc(roughness = 0.3, metallic = 1.0, reflectance = 0.6, grain = Grain::Metal)]
  Gold,
  #[assoc(roughness = 0.7, grain = Grain::Plain)]
  Bone,
  #[assoc(roughness = 0.85, grain = Grain::Wood, tiling = 1.0)]
  Wood,
  #[assoc(roughness = 0.95, grain = Grain::Bark, tiling = 1.0)]
  Bark,
  #[assoc(roughness = 0.9, grain = Grain::Needles, tiling = 2.0, two_sided = true)]
  Needles,
  #[assoc(roughness = 0.88, grain = Grain::Rock, tiling = 1.0)]
  Stone,
  #[assoc(roughness = 0.6, grain = Grain::Plain, glow = LinearRgba::rgb(700.0, 2400.0, 5200.0))]
  Frost,
  #[assoc(roughness = 0.6, grain = Grain::Plain, glow = LinearRgba::rgb(9000.0, 3200.0, 600.0))]
  Ember,
  #[assoc(roughness = 0.2, grain = Grain::Plain, reflectance = 0.5)]
  Gloss
}

impl Stuff {
  pub const ALL: [Stuff; 15] = [
    Stuff::Skin,
    Stuff::Fur,
    Stuff::Leather,
    Stuff::Cloth,
    Stuff::Iron,
    Stuff::Steel,
    Stuff::Gold,
    Stuff::Bone,
    Stuff::Wood,
    Stuff::Bark,
    Stuff::Needles,
    Stuff::Stone,
    Stuff::Frost,
    Stuff::Ember,
    Stuff::Gloss
  ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grain {
  Plain,
  Fur,
  Leather,
  Metal,
  Wood,
  Bark,
  Needles,
  Rock
}

#[derive(Resource)]
pub struct Stuffs(Vec<(Stuff, Handle<StandardMaterial>)>);

impl Stuffs {
  pub fn of(&self, stuff: Stuff) -> Handle<StandardMaterial> {
    self
      .0
      .iter()
      .find(|(each, _)| *each == stuff)
      .map(|(_, handle)| handle.clone())
      .expect("every stuff has a material")
  }
}

fn prepare(
  mut commands: Commands,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<StandardMaterial>>
) {
  let grains = [
    (Grain::Fur, texture::fur()),
    (Grain::Leather, texture::leather()),
    (Grain::Metal, texture::metal()),
    (Grain::Wood, texture::wood()),
    (Grain::Bark, texture::bark()),
    (Grain::Needles, texture::needles()),
    (Grain::Rock, texture::rock())
  ]
  .map(|(grain, image)| (grain, images.add(image)));
  let made = Stuff::ALL
    .into_iter()
    .map(|stuff| {
      let grain = stuff.grain();
      (
        stuff,
        materials.add(StandardMaterial {
          base_color_texture: grains
            .iter()
            .find(|(each, _)| *each == grain)
            .map(|(_, handle)| handle.clone()),
          uv_transform: Affine2::from_scale(Vec2::splat(stuff.tiling())),
          perceptual_roughness: stuff.roughness(),
          metallic: stuff.metallic(),
          reflectance: stuff.reflectance(),
          emissive: stuff.glow(),
          double_sided: stuff.two_sided(),
          cull_mode: (!stuff.two_sided()).then_some(bevy::render::render_resource::Face::Back),
          ..default()
        })
      )
    })
    .collect();
  commands.insert_resource(Stuffs(made));
}

pub fn plugin(app: &mut App) { app.add_systems(PreStartup, prepare); }
