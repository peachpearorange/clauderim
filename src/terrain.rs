use {crate::{noise,
             place::{self, LAKE, LAKE_LEVEL, LAKE_RADIUS, Place},
             texture},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            color::Mix,
            math::Affine2,
            mesh::{Indices, PrimitiveTopology},
            prelude::*},
     std::sync::Arc};

pub const HALF: f32 = 768.0;
pub const SPACING: f32 = 2.0;
pub const CELLS: usize = (2.0 * HALF / SPACING) as usize;
pub const GRID: usize = CELLS + 1;
pub const BOUND: f32 = HALF - 60.0;
const CHUNK: usize = 64;
const FAR_HALF: f32 = 7680.0;
const FAR_SPACING: f32 = 48.0;
const FAR_SUNK: f32 = 30.0;
const GRAIN_TILE: f32 = 7.0;
const THROAT: Vec2 = Vec2::new(900.0, -2600.0);

pub const fn srgb(red: f32, green: f32, blue: f32) -> LinearRgba {
  const fn decode(value: f32) -> f32 { value * value * (0.8 + 0.2 * value) }
  LinearRgba::rgb(decode(red), decode(green), decode(blue))
}

const MEADOW: LinearRgba = srgb(0.36, 0.42, 0.20);
const TUNDRA: LinearRgba = srgb(0.56, 0.52, 0.30);
const FOREST_FLOOR: LinearRgba = srgb(0.27, 0.27, 0.15);
const DIRT: LinearRgba = srgb(0.40, 0.33, 0.24);
const PEBBLES: LinearRgba = srgb(0.46, 0.44, 0.40);
const ROCK: LinearRgba = srgb(0.43, 0.43, 0.43);
const DARK_ROCK: LinearRgba = srgb(0.30, 0.30, 0.31);
const SNOW: LinearRgba = srgb(0.93, 0.95, 1.0);
const SEABED: LinearRgba = srgb(0.24, 0.24, 0.20);

fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
  let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

fn wild_height(at: Vec2) -> f32 {
  let warp =
    Vec2::new(noise::fbm(at / 520.0, 3, 11), noise::fbm(at / 520.0 + 9.3, 3, 12)) * 110.0;
  let bent = at + warp;
  let ring = smooth(0.5, 1.05, (bent / Vec2::new(780.0, 690.0)).length());
  let hills = noise::fbm(bent / 300.0, 5, 3) * 32.0 + noise::fbm(bent / 70.0, 4, 5) * 3.5;
  let peaks = noise::ridged(bent / 430.0, 7, 7);
  let throat_reach = at.distance(THROAT) / 1500.0;
  let throat = (-throat_reach * throat_reach).exp() * (1100.0 + 300.0 * peaks);
  24.0 + hills + ring * (40.0 + peaks * 330.0) + throat
}

fn lake_height(at: Vec2) -> f32 {
  let shore = LAKE_RADIUS * (1.0 + 0.25 * noise::fbm(at / 90.0, 3, 21));
  let reach = at.distance(LAKE);
  let bowl = smooth(shore * 1.6, shore * 0.55, reach);
  wild_height(at).lerp(LAKE_LEVEL - 9.0, bowl)
}

pub fn height_at(at: Vec2) -> f32 {
  Place::ALL.into_iter().fold(lake_height(at), |height, place| {
    let reach = at.distance(place.spot());
    let level = lake_height(place.spot());
    let flatten = smooth(place.flat() * 1.9, place.flat(), reach);
    let pit = smooth(place.flat() * 0.95, place.flat() * 0.6, reach) * place.sunk();
    height.lerp(level, flatten) - pit
  })
}

pub fn forest(at: Vec2) -> f32 {
  let clearing = Place::ALL
    .into_iter()
    .map(|place| {
      smooth(place.flat() * 2.2, place.flat() * 1.2, at.distance(place.spot()))
    })
    .fold(smooth(10.0, 4.0, place::road_distance(at)), f32::max);
  (smooth(-0.1, 0.35, noise::fbm(at / 160.0, 4, 31)) - clearing).max(0.0)
}

fn paint(at: Vec2, height: f32, normal: Vec3) -> LinearRgba {
  let patch = smooth(-0.3, 0.4, noise::fbm(at / 120.0, 4, 41));
  let grass = MEADOW.mix(&TUNDRA, patch).mix(&FOREST_FLOOR, forest(at) * 0.8);
  let road =
    smooth(3.4, 1.8, place::road_distance(at) + noise::fbm(at / 6.0, 2, 43) * 1.2);
  let shore = smooth(LAKE_LEVEL + 2.2, LAKE_LEVEL + 0.6, height);
  let drowned = smooth(LAKE_LEVEL - 0.5, LAKE_LEVEL - 4.0, height);
  let snow_line = 150.0 + 40.0 * noise::fbm(at / 200.0, 3, 45);
  let snow = smooth(snow_line, snow_line + 30.0, height) * smooth(0.45, 0.7, normal.y);
  let cliff = smooth(0.82, 0.64, normal.y + 0.06 * noise::fbm(at / 9.0, 2, 47));
  let stone = ROCK.mix(&DARK_ROCK, smooth(-0.2, 0.3, noise::fbm(at / 40.0, 3, 49)));
  grass
    .mix(&DIRT, road * 0.85)
    .mix(&PEBBLES, shore)
    .mix(&SEABED, drowned)
    .mix(&stone, cliff)
    .mix(&SNOW, snow)
}

#[derive(Resource, Clone)]
pub struct Ground(Arc<Vec<f32>>);

impl Ground {
  fn sample(&self, column: usize, row: usize) -> f32 {
    self.0[row.min(CELLS) * GRID + column.min(CELLS)]
  }

  pub fn height(&self, at: Vec2) -> f32 {
    let grid =
      ((at + HALF) / SPACING).clamp(Vec2::ZERO, Vec2::splat(CELLS as f32 - 0.001));
    let (column, row) = (grid.x as usize, grid.y as usize);
    let (fx, fy) = (grid.x.fract(), grid.y.fract());
    let near = self.sample(column, row).lerp(self.sample(column + 1, row), fx);
    let far = self.sample(column, row + 1).lerp(self.sample(column + 1, row + 1), fx);
    near.lerp(far, fy)
  }

  pub fn normal(&self, at: Vec2) -> Vec3 {
    let slope = |offset: Vec2| self.height(at + offset) - self.height(at - offset);
    Vec3::new(-slope(Vec2::X * SPACING), 2.0 * SPACING, -slope(Vec2::Y * SPACING))
      .normalize()
  }

  pub fn surface(&self, at: Vec2) -> Vec3 { at.extend(self.height(at)).xzy() }
}

fn heights() -> Vec<f32> {
  let bands = std::thread::available_parallelism().map_or(8, usize::from);
  let rows_per_band = GRID.div_ceil(bands);
  std::thread::scope(|scope| {
    (0..bands)
      .map(|band| {
        scope.spawn(move || {
          (band * rows_per_band..((band + 1) * rows_per_band).min(GRID))
            .flat_map(|row| {
              (0..GRID).map(move |column| {
                height_at(Vec2::new(column as f32, row as f32) * SPACING - HALF)
              })
            })
            .collect::<Vec<_>>()
        })
      })
      .collect::<Vec<_>>()
      .into_iter()
      .flat_map(|handle| handle.join().expect("height band"))
      .collect()
  })
}

fn surface_mesh(
  corners: usize,
  spot: impl Fn(usize, usize) -> Vec2,
  lift: impl Fn(usize, usize) -> f32,
  normal: impl Fn(usize, usize) -> Vec3
) -> Mesh {
  let cells = (0..corners).flat_map(|row| (0..corners).map(move |column| (column, row)));
  let positions: Vec<Vec3> = cells
    .clone()
    .map(|(column, row)| spot(column, row).extend(lift(column, row)).xzy())
    .collect();
  let normals: Vec<Vec3> =
    cells.clone().map(|(column, row)| normal(column, row)).collect();
  let colors: Vec<[f32; 4]> = positions
    .iter()
    .zip(&normals)
    .map(|(&position, &normal)| paint(position.xz(), position.y, normal).to_f32_array())
    .collect();
  let span = corners as u32;
  let indices = (0..span - 1)
    .flat_map(|row| {
      (0..span - 1).flat_map(move |column| {
        let first = row * span + column;
        [first, first + span, first + 1, first + 1, first + span, first + span + 1]
      })
    })
    .collect();
  Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(
      Mesh::ATTRIBUTE_UV_0,
      positions.iter().map(|position| position.xz() / GRAIN_TILE).collect::<Vec<_>>()
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
    .with_generated_tangents()
    .expect("terrain tangents")
}

fn chunk_mesh(ground: &Ground, chunk: UVec2) -> Mesh {
  let origin = chunk * CHUNK as u32;
  let cell = move |column: usize, row: usize| {
    (origin.x as usize + column, origin.y as usize + row)
  };
  surface_mesh(
    CHUNK + 1,
    |column, row| {
      let (x, z) = cell(column, row);
      Vec2::new(x as f32, z as f32) * SPACING - HALF
    },
    |column, row| {
      let (x, z) = cell(column, row);
      ground.sample(x, z)
    },
    |column, row| {
      let (x, z) = cell(column, row);
      let height = |x: usize, z: usize| ground.sample(x, z);
      Vec3::new(
        height(x.saturating_sub(1), z) - height(x + 1, z),
        2.0 * SPACING,
        height(x, z.saturating_sub(1)) - height(x, z + 1)
      )
      .normalize()
    }
  )
}

fn far_mesh() -> Mesh {
  let corners = (2.0 * FAR_HALF / FAR_SPACING) as usize + 1;
  let spot = |column: usize, row: usize| {
    Vec2::new(column as f32, row as f32) * FAR_SPACING - FAR_HALF
  };
  let inside = |at: Vec2| at.x.abs() < HALF - 1.0 && at.y.abs() < HALF - 1.0;
  surface_mesh(
    corners,
    spot,
    |column, row| {
      let at = spot(column, row);
      height_at(at) - inside(at).then_some(FAR_SUNK).unwrap_or(0.0)
    },
    |column, row| {
      let at = spot(column, row);
      let slope = |offset: Vec2| height_at(at + offset) - height_at(at - offset);
      Vec3::new(-slope(Vec2::X * 24.0), 48.0, -slope(Vec2::Y * 24.0)).normalize()
    }
  )
}

#[derive(Resource)]
pub struct Surfaces {
  pub rock: Handle<StandardMaterial>
}

fn spawn_terrain(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut images: ResMut<Assets<Image>>
) {
  let ground = Ground(Arc::new(heights()));
  let material = materials.add(StandardMaterial {
    base_color_texture: Some(images.add(texture::ground())),
    normal_map_texture: Some(images.add(texture::ground_bumps())),
    perceptual_roughness: 0.93,
    reflectance: 0.2,
    ..default()
  });

  let chunks = (CELLS / CHUNK) as u32;
  (0..chunks).flat_map(|z| (0..chunks).map(move |x| UVec2::new(x, z))).for_each(
    |chunk| {
      commands.spawn((
        Name::new("Terrain Chunk"),
        Mesh3d(meshes.add(chunk_mesh(&ground, chunk))),
        MeshMaterial3d(material.clone())
      ));
    }
  );

  commands.spawn((
    Name::new("Distant Lands"),
    Mesh3d(meshes.add(far_mesh())),
    MeshMaterial3d(material),
    Transform::from_xyz(0.0, -0.5, 0.0)
  ));

  let rows = (0..GRID)
    .map(|column| (0..GRID).map(|row| ground.sample(column, row)).collect())
    .collect();
  commands.spawn((
    Name::new("Terrain Collider"),
    RigidBody::Static,
    Collider::heightfield(rows, Vec3::new(2.0 * HALF, 1.0, 2.0 * HALF)),
    Friction::new(0.8)
  ));

  commands.insert_resource(Surfaces {
    rock: materials.add(StandardMaterial {
      base_color_texture: Some(images.add(texture::rock())),
      normal_map_texture: Some(images.add(texture::rock_bumps())),
      perceptual_roughness: 0.9,
      reflectance: 0.25,
      ..default()
    })
  });
  commands.insert_resource(ground);
}

fn spawn_lake(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut images: ResMut<Assets<Image>>
) {
  let ripples = images.add(texture::bumps(256, 0.02, |u, v| {
    texture::tile_fbm(u, v, 6, 4, 131) + 0.5 * texture::tile_fbm(u, v, 24, 2, 133)
  }));
  commands.spawn((
    Name::new("Lake"),
    Ripples,
    Mesh3d(
      meshes.add(
        Plane3d::default()
          .mesh()
          .size(LAKE_RADIUS * 4.0, LAKE_RADIUS * 4.0)
          .subdivisions(8)
          .build()
          .with_generated_tangents()
          .expect("lake tangents")
      )
    ),
    MeshMaterial3d(materials.add(StandardMaterial {
      base_color: Color::srgba(0.10, 0.16, 0.18, 0.86),
      normal_map_texture: Some(ripples),
      perceptual_roughness: 0.06,
      reflectance: 0.6,
      alpha_mode: AlphaMode::Blend,
      uv_transform: Affine2::from_scale(Vec2::splat(18.0)),
      ..default()
    })),
    Transform::from_translation(LAKE.extend(LAKE_LEVEL).xzy())
  ));
}

#[derive(Component)]
struct Ripples;

fn ripple(
  time: Res<Time>,
  lakes: Query<&MeshMaterial3d<StandardMaterial>, With<Ripples>>,
  mut materials: ResMut<Assets<StandardMaterial>>
) {
  lakes.iter().for_each(|lake| {
    if let Some(mut water) = materials.get_mut(&lake.0) {
      water.uv_transform.translation = Vec2::new(0.013, 0.007) * time.elapsed_secs();
    }
  });
}

pub fn plugin(app: &mut App) {
  app.add_systems(PreStartup, (spawn_terrain, spawn_lake)).add_systems(Update, ripple);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn heightfield_matches_grid() {
    let ground = Ground(Arc::new(heights()));
    let rows = (0..GRID)
      .map(|column| (0..GRID).map(|row| ground.sample(column, row)).collect())
      .collect();
    let collider = Collider::heightfield(rows, Vec3::new(2.0 * HALF, 1.0, 2.0 * HALF));
    [Vec2::new(-250.0, -170.0), Vec2::new(300.0, 60.0), Vec2::new(40.0, 290.0)]
      .into_iter()
      .for_each(|at| {
        let hit = collider
          .cast_ray(
            Vec3::ZERO,
            Quat::IDENTITY,
            at.extend(2000.0).xzy(),
            Vec3::new(0.0001, -1.0, 0.0002).normalize(),
            4000.0,
            true
          )
          .expect("ray hits terrain");
        let found = 2000.0 - hit.0;
        assert!(
          (found - ground.height(at)).abs() < 0.5,
          "{at} {found} {}",
          ground.height(at)
        );
      });
  }
}
