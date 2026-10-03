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
#[func(pub const fn glow_follows_grain(self) -> bool { false })]
#[func(pub const fn translucency(self) -> f32 { 0.0 })]
pub enum Stuff {
  #[assoc(roughness = 0.6, reflectance = 0.35, grain = Grain::Plain)]
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
  #[assoc(roughness = 0.7, grain = Grain::Birch, tiling = 1.0)]
  Birch,
  #[assoc(roughness = 0.9, grain = Grain::Needles, tiling = 2.0, two_sided = true)]
  Needles,
  #[assoc(roughness = 0.88, grain = Grain::Rock, tiling = 1.0)]
  Stone,
  #[assoc(roughness = 0.9, grain = Grain::Cliff, tiling = 1.2)]
  Cliff,
  #[assoc(roughness = 0.6, grain = Grain::Plain, glow = LinearRgba::rgb(1.4, 4.8, 10.4))]
  Frost,
  #[assoc(roughness = 0.6, grain = Grain::Plain, glow = LinearRgba::rgb(18.0, 6.4, 1.2))]
  Ember,
  #[assoc(roughness = 0.2, grain = Grain::Plain, reflectance = 0.5)]
  Gloss,
  #[assoc(roughness = 0.75, grain = Grain::Leather, tiling = 4.0, two_sided = true, translucency = 0.6)]
  Membrane,
  #[assoc(roughness = 0.62, grain = Grain::Scales, reflectance = 0.4)]
  Scales,
  #[assoc(roughness = 0.9, grain = Grain::Cracks, tiling = 2.0, glow = LinearRgba::rgb(9.0, 2.6, 0.35), glow_follows_grain = true)]
  Cinder,
  #[assoc(roughness = 0.97, grain = Grain::Thatch, reflectance = 0.2)]
  Thatch,
  #[assoc(roughness = 0.88, grain = Grain::Shingles)]
  Shingle,
  #[assoc(roughness = 0.9, grain = Grain::Masonry, reflectance = 0.25)]
  Masonry,
  #[assoc(roughness = 0.85, grain = Grain::Planks)]
  Planks,
  #[assoc(roughness = 0.12, grain = Grain::Ice, reflectance = 0.5, tiling = 0.7, glow = LinearRgba::rgb(0.5, 1.7, 2.6), glow_follows_grain = true)]
  Ice
}

impl Stuff {
  pub const fn bumpy(self) -> bool {
    matches!(
      self.grain(),
      Grain::Wood
        | Grain::Bark
        | Grain::Birch
        | Grain::Rock
        | Grain::Cliff
        | Grain::Scales
        | Grain::Thatch
        | Grain::Shingles
        | Grain::Masonry
        | Grain::Planks
        | Grain::Ice
    )
  }

  pub fn fitted(self, mut mesh: Mesh) -> Mesh {
    if self.bumpy() {
      mesh.generate_tangents().ok();
    }
    mesh
  }

  pub const ALL: [Stuff; 25] = [
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
    Stuff::Birch,
    Stuff::Needles,
    Stuff::Stone,
    Stuff::Cliff,
    Stuff::Frost,
    Stuff::Ember,
    Stuff::Gloss,
    Stuff::Membrane,
    Stuff::Scales,
    Stuff::Cinder,
    Stuff::Thatch,
    Stuff::Shingle,
    Stuff::Masonry,
    Stuff::Planks,
    Stuff::Ice
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
  Birch,
  Needles,
  Rock,
  Cliff,
  Cracks,
  Scales,
  Thatch,
  Shingles,
  Masonry,
  Planks,
  Ice
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
  let plain = |image: Image| (image, None);
  let relieved = |texture::Textured { tone, relief }| (tone, Some(relief));
  let grains = [
    (Grain::Fur, plain(texture::fur())),
    (Grain::Leather, plain(texture::leather())),
    (Grain::Metal, plain(texture::metal())),
    (Grain::Wood, relieved(texture::wood())),
    (Grain::Bark, relieved(texture::bark())),
    (Grain::Birch, relieved(texture::birch())),
    (Grain::Needles, plain(texture::needles())),
    (Grain::Rock, relieved(texture::rock())),
    (Grain::Cliff, (texture::cliff(), Some(texture::cliff_bumps()))),
    (Grain::Cracks, plain(texture::cracks())),
    (Grain::Scales, (texture::scales(), Some(texture::scale_bumps()))),
    (Grain::Thatch, relieved(texture::thatch())),
    (Grain::Shingles, (texture::shingles(), Some(texture::shingle_bumps()))),
    (Grain::Masonry, relieved(texture::masonry())),
    (Grain::Planks, relieved(texture::planks())),
    (Grain::Ice, (texture::ice(), Some(texture::ice_bumps())))
  ]
  .map(|(grain, (tone, relief))| {
    (grain, images.add(tone), relief.map(|relief| images.add(relief)))
  });
  let made = Stuff::ALL
    .into_iter()
    .map(|stuff| {
      let grain = stuff.grain();
      let (texture, relief) = grains
        .iter()
        .find(|(each, ..)| *each == grain)
        .map_or((None, None), |(_, tone, relief)| (Some(tone.clone()), relief.clone()));
      (
        stuff,
        materials.add(StandardMaterial {
          emissive_texture: texture.clone().filter(|_| stuff.glow_follows_grain()),
          base_color_texture: texture,
          uv_transform: Affine2::from_scale(Vec2::splat(stuff.tiling())),
          perceptual_roughness: stuff.roughness(),
          metallic: stuff.metallic(),
          reflectance: stuff.reflectance(),
          emissive: stuff.glow(),
          normal_map_texture: relief,
          diffuse_transmission: stuff.translucency(),
          double_sided: stuff.two_sided(),
          cull_mode: (!stuff.two_sided())
            .then_some(bevy::render::render_resource::Face::Back),
          ..default()
        })
      )
    })
    .collect();
  commands.insert_resource(Stuffs(made));
}

pub fn plugin(app: &mut App) { app.add_systems(PreStartup, prepare); }
