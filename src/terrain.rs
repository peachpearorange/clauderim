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
const TILE: f32 = 384.0;
const GRAIN_TILE: f32 = 7.0;
const THROAT: Vec2 = Vec2::new(900.0, -2600.0);
const THROAT_REACH: f32 = 1700.0;
const THROAT_RISE: f32 = 1000.0;
const LEDGE: f32 = 38.0;

pub const fn srgb(red: f32, green: f32, blue: f32) -> LinearRgba {
  const fn decode(value: f32) -> f32 { value * value * (0.8 + 0.2 * value) }
  LinearRgba::rgb(decode(red), decode(green), decode(blue))
}

const MEADOW: LinearRgba = srgb(0.33, 0.36, 0.23);
const TUNDRA: LinearRgba = srgb(0.47, 0.42, 0.30);
const FOREST_FLOOR: LinearRgba = srgb(0.25, 0.24, 0.18);
const DIRT: LinearRgba = srgb(0.40, 0.33, 0.24);
const PEBBLES: LinearRgba = srgb(0.46, 0.44, 0.40);
const ROCK: LinearRgba = srgb(0.43, 0.43, 0.43);
const DARK_ROCK: LinearRgba = srgb(0.30, 0.30, 0.31);
const PALE_ROCK: LinearRgba = srgb(0.55, 0.54, 0.51);
const RUST_ROCK: LinearRgba = srgb(0.42, 0.37, 0.32);
const CRAG: LinearRgba = srgb(0.29, 0.29, 0.31);
const SNOW: LinearRgba = srgb(0.93, 0.95, 1.0);
const SEABED: LinearRgba = srgb(0.24, 0.24, 0.20);

fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
  let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

fn throat_height(at: Vec2, crags: f32) -> f32 {
  let offset = at - THROAT;
  let foothills = offset.length() / 1500.0;
  let base = (-foothills * foothills).exp() * 1100.0;
  let cone = (1.0 - offset.length() / THROAT_REACH).max(0.0);
  let spurs = (offset.to_angle() * 5.0 + 2.4 * noise::fbm(at / 700.0, 3, 61)).sin();
  let arete = (1.0 - spurs.abs()).powi(2) * cone * (1.0 - cone) * 4.0;
  base + THROAT_RISE * cone.powf(2.4) + 220.0 * arete + 200.0 * (crags - 0.45) * cone
}

fn spires(at: Vec2) -> f32 {
  let warp =
    Vec2::new(noise::fbm(at / 900.0, 3, 81), noise::fbm(at / 900.0 + 4.7, 3, 82)) * 320.0;
  noise::crags((at + warp) / 760.0, 8, 83, 1.0).max(0.0) * 900.0
}

fn ledged(height: f32, at: Vec2) -> f32 {
  let shift = noise::fbm(at / 400.0, 2, 87);
  let thickness = LEDGE * (1.0 + 0.45 * noise::fbm(at / 350.0, 2, 89));
  let level = height / thickness + shift;
  (level.floor() + smooth(0.0, 1.0, level.fract()) - shift) * thickness
}

fn wild_height(at: Vec2) -> f32 {
  let warp =
    Vec2::new(noise::fbm(at / 520.0, 3, 11), noise::fbm(at / 520.0 + 9.3, 3, 12)) * 110.0;
  let bent = at + warp;
  let far = smooth(BOUND, BOUND + 400.0, at.abs().max_element());
  let ring = smooth(0.5, 1.05, (bent / Vec2::new(780.0, 690.0)).length());
  let hills = noise::fbm(bent / 300.0, 5, 3) * 32.0 + noise::fbm(bent / 70.0, 4, 5) * 3.5;
  let crags = noise::crags(bent / 430.0, 9, 7, 1.0 - 0.5 * far);
  let range = smooth(900.0, 3200.0, at.length());
  let throat_calm =
    1.0 - 0.8 * smooth(1.1 * THROAT_REACH, 0.4 * THROAT_REACH, at.distance(THROAT));
  let massif =
    ring * (40.0 + crags * (400.0 + 120.0 * range)) + far * throat_calm * spires(at);
  24.0 + hills + massif.lerp(ledged(massif, bent), 0.25 * far) + throat_height(at, crags)
}

fn lake_height(at: Vec2) -> f32 {
  let shore = LAKE_RADIUS * (1.0 + 0.25 * noise::fbm(at / 90.0, 3, 21));
  let reach = at.distance(LAKE);
  let bowl = smooth(shore * 1.6, shore * 0.55, reach);
  wild_height(at).lerp(LAKE_LEVEL - 9.0, bowl)
}

pub fn height_at(at: Vec2) -> f32 {
  let near = |place: &Place| at.distance(place.spot()) < place.flat() * 1.9;
  Place::ALL.into_iter().filter(near).fold(lake_height(at), |height, place| {
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

fn paint(at: Vec2, height: f32, normal: Vec3, hollow: f32) -> LinearRgba {
  let patch = smooth(-0.3, 0.4, noise::fbm(at / 120.0, 4, 41));
  let grass = MEADOW.mix(&TUNDRA, patch).mix(&FOREST_FLOOR, forest(at) * 0.8);
  let road =
    smooth(3.4, 1.8, place::road_distance(at) + noise::fbm(at / 6.0, 2, 43) * 1.2);
  let shore = smooth(LAKE_LEVEL + 2.2, LAKE_LEVEL + 0.6, height);
  let drowned = smooth(LAKE_LEVEL - 0.5, LAKE_LEVEL - 4.0, height);
  let snow_line = 150.0 + 40.0 * noise::fbm(at / 200.0, 3, 45);
  let alpine = smooth(snow_line + 150.0, snow_line + 900.0, height);
  let gully = hollow.clamp(-1.0, 1.0);
  let drift = normal.y
    + 0.1 * noise::fbm(at / 40.0, 3, 53)
    + 0.14 * noise::fbm(at / 190.0, 2, 59)
    + 0.35 * gully;
  let snow_hold = 0.8 - 0.45 * alpine;
  let snow = smooth(snow_line, snow_line + 40.0, height)
    * smooth(snow_hold - 0.05, snow_hold + 0.05, drift);
  let cliff = smooth(0.82, 0.64, normal.y + 0.06 * noise::fbm(at / 9.0, 2, 47));
  let face = smooth(0.7, 0.35, normal.y + 0.08 * noise::fbm(at / 15.0, 2, 55));
  let strata = (height / 9.0 + 3.0 * noise::fbm(at / 260.0, 3, 51)).sin();
  let stone = ROCK
    .mix(&DARK_ROCK, smooth(-0.2, 0.3, noise::fbm(at / 40.0, 3, 49)))
    .mix(&PALE_ROCK, smooth(0.3, 0.9, strata) * 0.6)
    .mix(&RUST_ROCK, smooth(0.4, 0.9, noise::fbm(at / 90.0, 3, 57)) * 0.3)
    .mix(&CRAG, face * (0.5 + 0.5 * smooth(0.0, -0.4, gully)));
  grass
    .mix(&DIRT, road * 0.85)
    .mix(&PEBBLES, shore)
    .mix(&SEABED, drowned)
    .mix(&stone, cliff.max(alpine))
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

fn in_parallel<Item: Sync, Made: Send>(
  items: &[Item],
  make: impl Fn(&Item) -> Made + Sync
) -> Vec<Made> {
  let threads = std::thread::available_parallelism().map_or(8, usize::from);
  let share = items.len().div_ceil(4 * threads).max(1);
  crate::par::scope(|scope| {
    items
      .chunks(share)
      .map(|part| scope.spawn(|| part.iter().map(&make).collect::<Vec<_>>()))
      .collect::<Vec<_>>()
      .into_iter()
      .flat_map(|handle| handle.join().expect("parallel work"))
      .collect()
  })
}

fn heights() -> Vec<f32> {
  in_parallel(&(0..GRID).collect::<Vec<_>>(), |&row| {
    (0..GRID)
      .map(|column| height_at(Vec2::new(column as f32, row as f32) * SPACING - HALF))
      .collect::<Vec<_>>()
  })
  .concat()
}

fn surface_mesh(
  corners: usize,
  spot: impl Fn(usize, usize) -> Vec2,
  lift: impl Fn(usize, usize) -> f32,
  shape: impl Fn(usize, usize) -> (Vec3, f32)
) -> Mesh {
  let cells = (0..corners).flat_map(|row| (0..corners).map(move |column| (column, row)));
  let positions: Vec<Vec3> = cells
    .clone()
    .map(|(column, row)| spot(column, row).extend(lift(column, row)).xzy())
    .collect();
  let shapes: Vec<(Vec3, f32)> =
    cells.clone().map(|(column, row)| shape(column, row)).collect();
  let colors: Vec<[f32; 4]> = positions
    .iter()
    .zip(&shapes)
    .map(|(&position, &(normal, hollow))| {
      paint(position.xz(), position.y, normal, hollow).to_f32_array()
    })
    .collect();
  let normals: Vec<Vec3> = shapes.into_iter().map(|(normal, _)| normal).collect();
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

const HOLLOW_REACH: usize = 3;

fn hollowness(around: f32, height: f32, reach: f32) -> f32 {
  (around / 4.0 - height) / reach.max(6.0) * 4.0
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
      let around = height(x.saturating_sub(HOLLOW_REACH), z)
        + height(x + HOLLOW_REACH, z)
        + height(x, z.saturating_sub(HOLLOW_REACH))
        + height(x, z + HOLLOW_REACH);
      (
        Vec3::new(
          height(x.saturating_sub(1), z) - height(x + 1, z),
          2.0 * SPACING,
          height(x, z.saturating_sub(1)) - height(x, z + 1)
        )
        .normalize(),
        hollowness(around, height(x, z), HOLLOW_REACH as f32 * SPACING)
      )
    }
  )
}

const DETAIL: [(f32, f32); 3] = [(1.0, 4.0), (800.0, 8.0), (2400.0, 16.0)];
const COARSE: f32 = 32.0;
const THROAT_DETAIL: f32 = 8.0;

fn tile_spacing(tile: IVec2) -> f32 {
  let center = (tile.as_vec2() + 0.5) * TILE;
  let gap = center.abs().max_element() - HALF - 0.5 * TILE;
  let spacing = DETAIL
    .into_iter()
    .find(|&(reach, _)| gap < reach)
    .map_or(COARSE, |(_, spacing)| spacing);
  (center.distance(THROAT) < 0.8 * THROAT_REACH)
    .then_some(spacing.min(THROAT_DETAIL))
    .unwrap_or(spacing)
}

fn tile_mesh(tile: IVec2) -> Mesh {
  let spacing = tile_spacing(tile);
  let cells = (TILE / spacing) as usize;
  let span = cells + 3;
  let origin = tile.as_vec2() * TILE;
  let spot =
    |column: usize, row: usize| origin + Vec2::new(column as f32, row as f32) * spacing;
  let heights: Vec<f32> = (0..span * span)
    .map(|index| height_at(spot(index % span, index / span) - spacing))
    .collect();
  let height = |column: usize, row: usize| heights[(row + 1) * span + column + 1];
  let coarsening = |side: IVec2| (tile_spacing(tile + side) / spacing).max(1.0) as usize;
  let (west, east, south, north) = (
    coarsening(IVec2::NEG_X),
    coarsening(IVec2::X),
    coarsening(IVec2::NEG_Y),
    coarsening(IVec2::Y)
  );
  let stitch = |along: usize, step: usize, sample: &dyn Fn(usize) -> f32| {
    let offset = along % step;
    let start = along - offset;
    sample(start).lerp(sample((start + step).min(cells)), offset as f32 / step as f32)
  };
  let rim = |index: usize| index.clamp(1, cells + 1) - 1;
  let seam = |column: usize, row: usize| match (column, row) {
    (0, _) => stitch(row, west, &|row| height(0, row)),
    (_, _) if column == cells => stitch(row, east, &|row| height(cells, row)),
    (_, 0) => stitch(column, south, &|column| height(column, 0)),
    (_, _) if row == cells => stitch(column, north, &|column| height(column, cells)),
    _ => height(column, row)
  };
  surface_mesh(
    cells + 3,
    |column, row| spot(rim(column), rim(row)),
    |column, row| {
      let skirt = rim(column) + 1 != column || rim(row) + 1 != row;
      seam(rim(column), rim(row)) - skirt.then_some(2.0 * spacing).unwrap_or(0.0)
    },
    |column, row| {
      let (x, z) = (rim(column) + 1, rim(row) + 1);
      let raw = |x: usize, z: usize| heights[z * span + x];
      let around = raw(x - 1, z) + raw(x + 1, z) + raw(x, z - 1) + raw(x, z + 1);
      (
        Vec3::new(
          raw(x - 1, z) - raw(x + 1, z),
          2.0 * spacing,
          raw(x, z - 1) - raw(x, z + 1)
        )
        .normalize(),
        hollowness(around, raw(x, z), spacing)
      )
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
  let chunk_spots: Vec<UVec2> =
    (0..chunks).flat_map(|z| (0..chunks).map(move |x| UVec2::new(x, z))).collect();
  in_parallel(&chunk_spots, |&chunk| chunk_mesh(&ground, chunk)).into_iter().for_each(
    |mesh| {
      commands.spawn((
        Name::new("Terrain Chunk"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material.clone())
      ));
    }
  );

  let tiles_across = (FAR_HALF / TILE) as i32;
  let playable = (HALF / TILE) as i32;
  let tiles: Vec<IVec2> = (-tiles_across..tiles_across)
    .flat_map(|z| (-tiles_across..tiles_across).map(move |x| IVec2::new(x, z)))
    .filter(|tile| {
      !(-playable..playable).contains(&tile.x) || !(-playable..playable).contains(&tile.y)
    })
    .collect();
  in_parallel(&tiles, |&tile| tile_mesh(tile)).into_iter().for_each(|mesh| {
    commands.spawn((
      Name::new("Distant Lands"),
      Mesh3d(meshes.add(mesh)),
      MeshMaterial3d(material.clone())
    ));
  });

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
  fn mountains_have_no_walls() {
    let steepest = |step: f32, from: Vec2, to: Vec2| {
      (0..4000)
        .map(|index| {
          let at = from.lerp(to, index as f32 / 4000.0);
          let rise = |offset: Vec2| (height_at(at + offset * step) - height_at(at)).abs();
          rise(Vec2::X).max(rise(Vec2::Y)) / step
        })
        .fold(0.0, f32::max)
    };
    [
      (SPACING, Vec2::new(-760.0, 700.0), Vec2::new(760.0, 740.0)),
      (SPACING, Vec2::new(-700.0, -760.0), Vec2::new(-740.0, 760.0)),
      (8.0, Vec2::new(-900.0, 1300.0), Vec2::new(900.0, 900.0)),
      (8.0, THROAT - Vec2::X * 1600.0, THROAT - Vec2::X * 200.0)
    ]
    .into_iter()
    .for_each(|(step, from, to)| {
      let slope = steepest(step, from, to);
      assert!(slope < 6.0, "{from} to {to}: {slope}");
    });
  }

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
