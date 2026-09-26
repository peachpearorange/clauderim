use {crate::{noise,
             place::{Paving, ROADS},
             terrain::{height_at, smooth},
             texture},
     bevy::{asset::RenderAssetUsages,
            camera::visibility::VisibilityRange,
            light::NotShadowCaster,
            mesh::{Indices, PrimitiveTopology},
            prelude::*}};

const STEP: f32 = 1.2;
const STRETCH: usize = 60;
const ACROSS: usize = 8;
const LIFT: f32 = 0.07;
const SHOWN: f32 = 360.0;

pub fn decay(at: Vec2) -> f32 {
  let neglect = smooth(-0.35, 0.45, noise::fbm(at / 170.0, 2, 611));
  let patches = smooth(0.0, 0.5, noise::fbm(at / 7.0, 3, 613));
  (patches * (0.15 + 0.6 * neglect)).clamp(0.0, 1.0)
}

fn wear(at: Vec2, across: f32) -> f32 {
  let rim = smooth(0.75, 1.05, across.abs() + 0.25 * noise::fbm(at / 1.7, 2, 617));
  1.0 - 0.5 * (decay(at) + rim).clamp(0.0, 1.0)
}

fn resampled(path: &[Vec2]) -> Vec<Vec2> {
  path
    .windows(2)
    .flat_map(|pair| {
      let (from, to) = (pair[0], pair[1]);
      let steps = (from.distance(to) / STEP).ceil().max(1.0) as usize;
      (0..steps).map(move |step| from.lerp(to, step as f32 / steps as f32))
    })
    .chain(path.last().copied())
    .collect()
}

fn ribbon(spine: &[Vec2], start_length: f32, width: f32) -> (Vec3, Mesh) {
  let middle = spine[spine.len() / 2];
  let center = middle.extend(height_at(middle)).xzy();
  let along = |index: usize| {
    let (before, after) =
      (spine[index.saturating_sub(1)], spine[(index + 1).min(spine.len() - 1)]);
    (after - before).normalize_or(Vec2::X)
  };
  let lengths: Vec<f32> = spine
    .windows(2)
    .scan(start_length, |length, pair| {
      *length += pair[0].distance(pair[1]);
      Some(*length)
    })
    .collect();
  let rows: Vec<(Vec<Vec3>, Vec<(Vec2, [f32; 4])>)> = spine
    .iter()
    .enumerate()
    .map(|(index, &point)| {
      let side = along(index).perp();
      let length = index.checked_sub(1).map_or(start_length, |before| lengths[before]);
      (0..=ACROSS)
        .map(|slot| {
          let across = slot as f32 / ACROSS as f32;
          let offset = (across - 0.5) * 2.0 * width;
          let crown = 0.05 * (1.0 - (2.0 * across - 1.0).powi(2)) - 0.04;
          let at = point + side * offset;
          let dirt = 1.0 - 0.25 * decay(at);
          (
            at.extend(
              crate::river::deck(at)
                .map_or(height_at(at), |deck| deck.max(height_at(at)))
                + LIFT
                + crown
            )
            .xzy()
              - center,
            (Vec2::new(across, length / (2.0 * width)), [
              dirt,
              dirt * 0.97,
              dirt * 0.92,
              wear(at, 2.0 * across - 1.0)
            ])
          )
        })
        .unzip()
    })
    .collect();
  let columns = ACROSS as u32 + 1;
  let indices: Vec<u32> = (0..rows.len() as u32 - 1)
    .flat_map(|row| {
      (0..ACROSS as u32).flat_map(move |column| {
        let first = row * columns + column;
        let next = first + columns;
        [first, first + 1, next, first + 1, next + 1, next]
      })
    })
    .collect();
  let (positions, (uvs, colors)): (Vec<Vec3>, (Vec<Vec2>, Vec<[f32; 4]>)) =
    rows.into_iter().flat_map(|(row, looks)| row.into_iter().zip(looks)).unzip();
  let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_smooth_normals()
    .with_generated_tangents()
    .expect("paving tangents");
  (center, mesh)
}

fn lay_roads(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut images: ResMut<Assets<Image>>
) {
  let material = materials.add(StandardMaterial {
    base_color_texture: Some(images.add(texture::cobbles())),
    normal_map_texture: Some(images.add(texture::cobble_bumps())),
    alpha_mode: AlphaMode::Mask(0.5),
    perceptual_roughness: 0.82,
    reflectance: 0.25,
    depth_bias: 40.0,
    ..default()
  });
  let stretches: Vec<(Vec<Vec2>, f32, f32)> = ROADS
    .iter()
    .filter(|road| road.paving == Paving::Stone)
    .flat_map(|road| {
      let spine = resampled(&road.path);
      let width = road.paving.half_width();
      (0..spine.len().saturating_sub(1))
        .step_by(STRETCH)
        .map(|start| {
          let end = (start + STRETCH + 1).min(spine.len());
          (spine[start..end].to_vec(), start as f32 * STEP, width)
        })
        .collect::<Vec<_>>()
    })
    .collect();
  crate::terrain::in_parallel(&stretches, |(spine, start, width)| {
    ribbon(spine, *start, *width)
  })
  .into_iter()
  .for_each(|(center, mesh)| {
    commands.spawn((
      Name::new("Paving"),
      Mesh3d(meshes.add(mesh)),
      MeshMaterial3d(material.clone()),
      Transform::from_translation(center),
      VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: SHOWN..SHOWN + 40.0,
        use_aabb: true
      },
      NotShadowCaster
    ));
  });
}

pub fn plugin(app: &mut App) { app.add_systems(Startup, lay_roads); }
