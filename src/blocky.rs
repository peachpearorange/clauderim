use {crate::noise::hash,
     bevy::{asset::RenderAssetUsages,
            image::ImageSampler,
            mesh::{Indices, PrimitiveTopology},
            prelude::*,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat}},
     std::f32::consts::{PI, TAU}};

const ATLAS: u32 = 256;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Facet {
  Top,
  Bottom,
  Left,
  Front,
  Right,
  Back,
  Rim,
  Cap
}

impl Facet {
  const BOX: [Facet; 6] =
    [Facet::Top, Facet::Bottom, Facet::Left, Facet::Front, Facet::Right, Facet::Back];

  fn axes(self) -> (Vec3, Vec3, Vec3) {
    match self {
      Facet::Bottom => (Vec3::NEG_Y, Vec3::X, Vec3::Z),
      Facet::Left => (Vec3::NEG_X, Vec3::Z, Vec3::Y),
      Facet::Front => (Vec3::Z, Vec3::X, Vec3::Y),
      Facet::Right => (Vec3::X, Vec3::NEG_Z, Vec3::Y),
      Facet::Back => (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
      Facet::Top | Facet::Rim | Facet::Cap => (Vec3::Y, Vec3::X, Vec3::NEG_Z)
    }
  }
}

pub fn srgb(hex: u32) -> Srgba {
  Srgba::rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub struct Texel {
  pub tone: Srgba,
  pub glow: Srgba
}

impl Texel {
  pub fn flat(hex: u32) -> Self { Self { tone: srgb(hex), glow: Srgba::BLACK } }

  pub fn lit(hex: u32) -> Self { Self { tone: srgb(hex), glow: srgb(hex) } }
}

#[derive(Clone, Copy)]
pub struct Pixel {
  pub facet: Facet,
  pub at: UVec2,
  pub size: UVec2,
  pub seed: UVec2
}

impl Pixel {
  pub fn edge(self) -> bool {
    let Pixel { at, size, .. } = self;
    at.x == 0 || at.y == 0 || at.x + 1 == size.x || at.y + 1 == size.y
  }

  pub fn grain(self, cell: u32, salt: u32) -> f32 {
    let spot = (self.seed + self.at) / cell;
    hash(spot.x as i32, spot.y as i32, salt)
  }

  pub fn column(self, salt: u32) -> f32 {
    hash((self.seed.x + self.at.x) as i32, self.seed.y as i32, salt)
  }

  pub fn speck(self) -> f32 { self.grain(1, 11) }

  pub fn side(self) -> bool {
    matches!(self.facet, Facet::Left | Facet::Front | Facet::Right | Facet::Back)
  }

  pub fn middle(self) -> Vec2 {
    (self.at.as_vec2() + 0.5 - self.size.as_vec2() / 2.0).abs()
  }

  pub fn shade(self, [dark, base, light, fleck]: [u32; 4]) -> u32 {
    let speck = self.speck();
    match () {
      _ if self.edge() => dark,
      _ if speck < 0.05 => fleck,
      _ if speck > 0.96 || (self.at.y == 1 && self.side()) => light,
      _ => base
    }
  }
}

#[derive(Clone)]
struct Patch {
  facet: Facet,
  size: UVec2,
  polygons: Vec<Vec<(Vec3, Vec2)>>
}

fn frustum_patches(base: Vec2, top: Vec2, height: f32) -> Vec<Patch> {
  let half = |y: f32| (if y > 0.0 { top } else { base }).extend(height).xzy() / 2.0;
  Facet::BOX
    .into_iter()
    .filter_map(|facet| {
      let (normal, right, up) = facet.axes();
      let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(s, t)| {
        let unit = normal + right * s + up * t;
        (unit * half(unit.y), t)
      });
      let across = corners.iter().map(|(at, _)| right.dot(*at).abs()).fold(0.0, f32::max);
      let tall =
        ((corners[2].0 + corners[3].0) - (corners[0].0 + corners[1].0)).length() / 2.0;
      (across * tall > 1e-3).then(|| {
        let size = Vec2::new(across * 2.0, tall).round().max(Vec2::ONE);
        Patch {
          facet,
          size: size.as_uvec2(),
          polygons: vec![
            corners
              .iter()
              .map(|&(at, t)| {
                (
                  at,
                  size * Vec2::new((right.dot(at) / across + 1.0) / 2.0, (1.0 - t) / 2.0)
                )
              })
              .collect(),
          ]
        }
      })
    })
    .collect()
}

fn prism_patches(sides: u32, radius: f32, top_radius: f32, length: f32) -> Vec<Patch> {
  let ring = |index: u32, reach: f32, y: f32| {
    let angle = TAU * (index as f32 + 0.5) / sides as f32;
    Vec3::new(angle.cos() * reach, y, -angle.sin() * reach)
  };
  let (low, high) = (-length / 2.0, length / 2.0);
  let facet = (2.0 * radius.max(top_radius) * (PI / sides as f32).sin()).round().max(1.0);
  let slant = ring(0, radius, low).distance(ring(0, top_radius, high)).round().max(1.0);
  let rim = Patch {
    facet: Facet::Rim,
    size: UVec2::new(facet as u32 * sides, slant as u32),
    polygons: (0..sides)
      .map(|side| {
        let (left, right) = (side as f32 * facet, (side + 1) as f32 * facet);
        vec![
          (ring(side, radius, low), Vec2::new(left, slant)),
          (ring(side + 1, radius, low), Vec2::new(right, slant)),
          (ring(side + 1, top_radius, high), Vec2::new(right, 0.0)),
          (ring(side, top_radius, high), Vec2::new(left, 0.0)),
        ]
      })
      .collect()
  };
  let cap = |reach: f32, y: f32, order: &dyn Fn(u32) -> u32| {
    let span = (2.0 * reach).round().max(1.0);
    (reach > 0.0).then(|| Patch {
      facet: Facet::Cap,
      size: UVec2::splat(span as u32),
      polygons: vec![
        (0..sides)
          .map(|index| {
            let at = ring(order(index), reach, y);
            (at, (at.xz() / reach + 1.0) / 2.0 * span)
          })
          .collect(),
      ]
    })
  };
  [
    Some(rim),
    cap(top_radius, high, &|index| index),
    cap(radius, low, &|index| sides - index)
  ]
  .into_iter()
  .flatten()
  .collect()
}

#[derive(Clone)]
pub struct Shape<S> {
  skin: S,
  patches: Vec<Patch>,
  center: Vec3,
  turn: Quat
}

impl<S: Copy> Shape<S> {
  fn new(skin: S, patches: Vec<Patch>) -> Self {
    Self { skin, patches, center: Vec3::ZERO, turn: Quat::IDENTITY }
  }

  pub fn frustum(base: Vec2, top: Vec2, height: f32, skin: S) -> Self {
    Self::new(skin, frustum_patches(base, top, height))
  }

  pub fn block(size: Vec3, skin: S) -> Self {
    Self::frustum(size.xz(), size.xz(), size.y, skin)
  }

  pub fn prism(sides: u32, radius: f32, top_radius: f32, length: f32, skin: S) -> Self {
    Self::new(skin, prism_patches(sides, radius, top_radius, length))
  }

  pub fn boxed(from: Vec3, to: Vec3, skin: S) -> Self {
    Self::block((to - from).abs(), skin).at((from + to) / 2.0)
  }

  pub fn rod(from: Vec3, to: Vec3, thick: f32, skin: S) -> Self {
    Self::block(Vec3::new(thick, from.distance(to), thick), skin).along(from, to)
  }

  pub fn at(self, offset: Vec3) -> Self { Self { center: self.center + offset, ..self } }

  pub fn turned(self, turn: Quat) -> Self { Self { turn: turn * self.turn, ..self } }

  pub fn along(self, from: Vec3, to: Vec3) -> Self {
    Self {
      center: (from + to) / 2.0,
      turn: Quat::from_rotation_arc(Vec3::Y, (to - from).normalize()) * self.turn,
      ..self
    }
  }

  pub fn placed(self, at: Vec3, turn: Quat) -> Self {
    Self { center: at + turn * self.center, turn: turn * self.turn, ..self }
  }
}

pub struct Bone<P, S> {
  pub part: P,
  pub parent: Option<P>,
  pub pivot: Vec3,
  pub shapes: Vec<Shape<S>>
}

#[derive(Clone)]
pub struct Part<P> {
  pub part: P,
  pub parent: Option<P>,
  pub rest: Vec3,
  pub mesh: Handle<Mesh>
}

#[derive(Clone)]
pub struct Kit<P> {
  pub parts: Vec<Part<P>>,
  pub material: Handle<StandardMaterial>
}

fn pack(sizes: &[UVec2]) -> (Vec<UVec2>, UVec2) {
  let width =
    sizes.iter().map(|size| size.x).max().unwrap_or(1).max(ATLAS).next_power_of_two();
  let mut order: Vec<usize> = (0..sizes.len()).collect();
  order.sort_by_key(|&index| std::cmp::Reverse(sizes[index].y));
  let (spots, cursor, row) = order.into_iter().fold(
    (vec![UVec2::ZERO; sizes.len()], UVec2::ZERO, 0),
    |(mut spots, cursor, row), index| {
      let size = sizes[index];
      let wrapped = cursor.x + size.x > width;
      let at = if wrapped { UVec2::new(0, cursor.y + row) } else { cursor };
      spots[index] = at;
      (spots, at + UVec2::X * size.x, if wrapped { size.y } else { row.max(size.y) })
    }
  );
  (spots, UVec2::new(width, (cursor.y + row).next_power_of_two()))
}

fn image(texels: Vec<[u8; 4]>, size: UVec2) -> Image {
  let mut image = Image::new(
    Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
    TextureDimension::D2,
    texels.concat(),
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::RENDER_WORLD
  );
  image.sampler = ImageSampler::nearest();
  image
}

fn mesh(polygons: &[(Vec<(Vec3, Vec2)>, Vec3)]) -> Mesh {
  let corners: Vec<(Vec3, Vec2, Vec3)> = polygons
    .iter()
    .flat_map(|(corners, normal)| corners.iter().map(move |&(at, uv)| (at, uv, *normal)))
    .collect();
  let indices = polygons
    .iter()
    .scan(0u32, |first, (corners, _)| {
      let start = *first;
      *first += corners.len() as u32;
      Some(
        (1..corners.len() as u32 - 1)
          .flat_map(move |fan| [start, start + fan, start + fan + 1])
      )
    })
    .flatten()
    .collect();
  Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(
      Mesh::ATTRIBUTE_POSITION,
      corners.iter().map(|c| c.0).collect::<Vec<_>>()
    )
    .with_inserted_attribute(
      Mesh::ATTRIBUTE_NORMAL,
      corners.iter().map(|c| c.2).collect::<Vec<_>>()
    )
    .with_inserted_attribute(
      Mesh::ATTRIBUTE_UV_0,
      corners.iter().map(|c| c.1).collect::<Vec<_>>()
    )
    .with_inserted_indices(Indices::U32(indices))
}

fn newell(corners: &[Vec3]) -> Vec3 {
  corners
    .iter()
    .zip(corners.iter().cycle().skip(1))
    .fold(Vec3::ZERO, |sum, (a, b)| {
      sum
        + Vec3::new(
          (a.y - b.y) * (a.z + b.z),
          (a.z - b.z) * (a.x + b.x),
          (a.x - b.x) * (a.y + b.y)
        )
    })
    .normalize_or(Vec3::Y)
}

pub fn kits<P: Copy + PartialEq, S: Copy, F: Fn(S, Pixel) -> Texel>(
  bones: &[Bone<P, S>],
  px: f32,
  painters: impl IntoIterator<Item = F>,
  glow: f32,
  meshes: &mut Assets<Mesh>,
  images: &mut Assets<Image>,
  materials: &mut Assets<StandardMaterial>
) -> Vec<Kit<P>> {
  let patches: Vec<(usize, &Shape<S>, &Patch)> = bones
    .iter()
    .enumerate()
    .flat_map(|(index, bone)| {
      bone.shapes.iter().flat_map(move |shape| {
        shape.patches.iter().map(move |patch| (index, shape, patch))
      })
    })
    .collect();
  let (spots, atlas) =
    pack(&patches.iter().map(|(_, _, patch)| patch.size).collect::<Vec<_>>());
  let pivot = |part: Option<P>| {
    part
      .and_then(|part| bones.iter().find(|bone| bone.part == part))
      .map_or(Vec3::ZERO, |bone| bone.pivot)
  };
  let parts: Vec<Part<P>> = bones
    .iter()
    .enumerate()
    .map(|(index, bone)| {
      let polygons: Vec<(Vec<(Vec3, Vec2)>, Vec3)> = patches
        .iter()
        .zip(&spots)
        .filter(|((owner, _, _), _)| *owner == index)
        .flat_map(|((_, shape, patch), &spot)| {
          patch.polygons.iter().map(move |polygon| {
            let placed: Vec<(Vec3, Vec2)> = polygon
              .iter()
              .map(|&(at, uv)| {
                (
                  (shape.turn * at + shape.center - bone.pivot) * px,
                  (spot.as_vec2() + uv) / atlas.as_vec2()
                )
              })
              .collect();
            let normal = newell(&placed.iter().map(|(at, _)| *at).collect::<Vec<_>>());
            (placed, normal)
          })
        })
        .collect();
      Part {
        part: bone.part,
        parent: bone.parent,
        rest: (bone.pivot - pivot(bone.parent)) * px,
        mesh: meshes.add(mesh(&polygons))
      }
    })
    .collect();
  painters
    .into_iter()
    .map(|painter| {
      let blank = vec![[0, 0, 0, 255]; (atlas.x * atlas.y) as usize];
      let (tone, glow_map) = patches.iter().zip(&spots).fold(
        (blank.clone(), blank),
        |(mut tone, mut lit), (&(_, shape, patch), &spot)| {
          (0..patch.size.y)
            .flat_map(|y| (0..patch.size.x).map(move |x| UVec2::new(x, y)))
            .for_each(|at| {
              let texel = painter(shape.skin, Pixel {
                facet: patch.facet,
                at,
                size: patch.size,
                seed: spot
              });
              let index = ((spot.y + at.y) * atlas.x + spot.x + at.x) as usize;
              tone[index] = texel.tone.to_u8_array();
              lit[index] = texel.glow.to_u8_array();
            });
          (tone, lit)
        }
      );
      Kit {
        parts: parts.clone(),
        material: materials.add(StandardMaterial {
          base_color_texture: Some(images.add(image(tone, atlas))),
          emissive_texture: Some(images.add(image(glow_map, atlas))),
          emissive: LinearRgba::rgb(glow, glow, glow),
          perceptual_roughness: 0.75,
          ..default()
        })
      }
    })
    .collect()
}

impl<P: Copy + PartialEq> Kit<P> {
  pub fn spawn(
    &self,
    commands: &mut Commands,
    placed: Transform
  ) -> (Entity, Vec<(P, Entity, Vec3)>) {
    let root = commands.spawn((placed, Visibility::default())).id();
    let parts =
      self.parts.iter().fold(Vec::<(P, Entity, Vec3)>::new(), |mut spawned, part| {
        let parent = part
          .parent
          .and_then(|parent| spawned.iter().find(|(owner, ..)| *owner == parent))
          .map_or(root, |&(_, entity, _)| entity);
        let entity = commands
          .spawn((
            Mesh3d(part.mesh.clone()),
            MeshMaterial3d(self.material.clone()),
            Transform::from_translation(part.rest),
            ChildOf(parent)
          ))
          .id();
        spawned.push((part.part, entity, part.rest));
        spawned
      });
    (root, parts)
  }
}

pub fn knee(hip: Vec3, foot: Vec3, upper: f32, lower: f32, bend: Vec3) -> Vec3 {
  let span = foot - hip;
  let reach =
    span.length().clamp(0.01, (upper + lower) * 0.999).max((upper - lower).abs() * 1.001);
  let along = span.normalize_or(Vec3::NEG_Y);
  let out = (bend - along * bend.dot(along)).normalize_or(along.any_orthonormal_vector());
  let toward = (upper * upper - lower * lower + reach * reach) / (2.0 * reach);
  hip + along * toward + out * (upper * upper - toward * toward).max(0.0).sqrt()
}

pub fn aim(from: Vec3, to: Vec3, up: Vec3) -> Transform {
  Transform::from_translation(from).looking_to(to - from, up)
}
