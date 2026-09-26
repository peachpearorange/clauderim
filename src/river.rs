use {crate::{model::{Piece, block},
             place::{self, LAKE, LAKE_LEVEL, LAKE_RADIUS, ROADS},
             stuff::{Stuff, Stuffs},
             terrain::{self, smooth},
             texture},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            math::Affine2,
            mesh::{Indices, PrimitiveTopology},
            platform::collections::HashMap,
            prelude::*},
     std::{f32::consts::PI, sync::LazyLock}};

const COURSE: [Vec2; 19] = [
  Vec2::new(-120.0, 1440.0),
  Vec2::new(-20.0, 1420.0),
  Vec2::new(90.0, 1400.0),
  Vec2::new(190.0, 1372.0),
  Vec2::new(244.0, 1320.0),
  Vec2::new(258.0, 1245.0),
  Vec2::new(252.0, 1160.0),
  Vec2::new(238.0, 1070.0),
  Vec2::new(222.0, 980.0),
  Vec2::new(204.0, 880.0),
  Vec2::new(190.0, 780.0),
  Vec2::new(186.0, 680.0),
  Vec2::new(182.0, 580.0),
  Vec2::new(168.0, 480.0),
  Vec2::new(128.0, 410.0),
  Vec2::new(60.0, 378.0),
  Vec2::new(-30.0, 345.0),
  Vec2::new(-100.0, 305.0),
  Vec2::new(-150.0, 250.0)
];

const STEP: f32 = 4.0;
const BIN: f32 = 64.0;
const DEPTH: f32 = 1.7;
const SIDES: f32 = 0.4;
const VALLEY: f32 = 200.0;
const DROP: f32 = 0.012;

pub static PATH: LazyLock<Vec<Vec2>> = LazyLock::new(|| {
  place::smoothed(&COURSE)
    .windows(2)
    .flat_map(|pair| {
      let steps = (pair[0].distance(pair[1]) / STEP).ceil().max(1.0) as usize;
      (0..steps).map(move |step| pair[0].lerp(pair[1], step as f32 / steps as f32))
    })
    .chain(COURSE.last().copied())
    .collect()
});

fn width_at(index: usize) -> f32 { 3.0 + 4.0 * (index as f32 / PATH.len() as f32).sqrt() }

type Bins = HashMap<IVec2, Vec<usize>>;

fn binned(reach: f32) -> Bins {
  (0..PATH.len() - 1).fold(HashMap::default(), |mut bins, index| {
    let (from, to) = (PATH[index], PATH[index + 1]);
    let low = ((from.min(to) - reach) / BIN).floor().as_ivec2();
    let high = ((from.max(to) + reach) / BIN).floor().as_ivec2();
    (low.y..=high.y).for_each(|y| {
      (low.x..=high.x).for_each(|x| bins.entry(IVec2::new(x, y)).or_default().push(index))
    });
    bins
  })
}

static BINS: LazyLock<Bins> = LazyLock::new(|| binned(VALLEY));

fn closest(at: Vec2) -> Option<(f32, usize, f32)> {
  BINS.get(&(at / BIN).floor().as_ivec2()).and_then(|segments| {
    segments
      .iter()
      .map(|&index| {
        let (from, to) = (PATH[index], PATH[index + 1]);
        let along =
          ((at - from).dot(to - from) / (to - from).length_squared()).clamp(0.0, 1.0);
        (at.distance(from.lerp(to, along)), index, along)
      })
      .min_by(|a, b| a.0.total_cmp(&b.0))
  })
}

pub fn course_distance(at: Vec2) -> f32 {
  closest(at).map_or(f32::MAX, |(distance, ..)| distance)
}

static LEVELS: LazyLock<Vec<f32>> = LazyLock::new(|| {
  PATH
    .iter()
    .scan(f32::MAX, |level, &at| {
      *level =
        (*level - DROP * STEP).min(terrain::natural_height(at) - 1.3).max(LAKE_LEVEL);
      Some(*level)
    })
    .collect()
});

#[derive(Clone, Copy, Debug)]
pub struct Reach {
  pub distance: f32,
  pub level: f32,
  pub width: f32
}

pub fn reach(at: Vec2) -> Option<Reach> {
  closest(at).map(|(distance, index, along)| Reach {
    distance,
    level: LEVELS[index].lerp(LEVELS[index + 1], along),
    width: width_at(index)
  })
}

pub fn carve(at: Vec2, height: f32) -> f32 {
  reach(at).map_or(height, |Reach { distance, level, width }| {
    let side = (distance - width).max(0.0);
    let bed = level + 0.3 - (DEPTH + 0.3) * smooth(width, width * 0.25, distance)
      + side * SIDES * smooth(0.0, 5.0, side)
      + ((side / 45.0).exp() - 1.0) * 3.0;
    let carved = height.min(bed).lerp(height, smooth(VALLEY - 40.0, VALLEY, distance));
    carved.lerp(bed, smooth(width + 2.0, width, distance))
  })
}

pub fn water_level(at: Vec2) -> Option<f32> {
  (at.distance(LAKE) < LAKE_RADIUS * 2.0).then_some(LAKE_LEVEL).or_else(|| {
    reach(at).filter(|reach| reach.distance < reach.width + 1.0).map(|reach| reach.level)
  })
}

pub fn bank(at: Vec2) -> f32 {
  reach(at).map_or(0.0, |Reach { distance, width, .. }| {
    smooth(width + 3.5, width + 1.0, distance)
  })
}

fn water_mesh() -> Mesh {
  let rows: Vec<[(Vec3, Vec2); 2]> = PATH
    .iter()
    .enumerate()
    .map(|(index, &at)| {
      let before = PATH[index.saturating_sub(1)];
      let after = PATH[(index + 1).min(PATH.len() - 1)];
      let side = (after - before).normalize_or(Vec2::X).perp();
      let width = width_at(index) + 1.6;
      let level = LEVELS[index];
      let along = index as f32 * STEP / 12.0;
      [-1.0, 1.0].map(|edge| {
        (
          (at + side * edge * width).extend(level).xzy(),
          Vec2::new(edge * 0.5 + 0.5, along)
        )
      })
    })
    .collect();
  let positions: Vec<Vec3> = rows.iter().flat_map(|row| row.map(|(at, _)| at)).collect();
  let uvs: Vec<Vec2> = rows.iter().flat_map(|row| row.map(|(_, uv)| uv)).collect();
  let indices: Vec<u32> = (0..rows.len() as u32 - 1)
    .flat_map(|row| {
      let first = row * 2;
      [first, first + 1, first + 2, first + 1, first + 3, first + 2]
    })
    .collect();
  Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_smooth_normals()
    .with_generated_tangents()
    .expect("river tangents")
}

#[derive(Component)]
struct Flow;

fn crossings() -> Vec<(Vec2, Vec2, f32)> {
  ROADS
    .iter()
    .flat_map(|road| {
      road.path.windows(2).map(|pair| (pair[0], pair[1])).collect::<Vec<_>>()
    })
    .flat_map(|(from, to)| {
      PATH.windows(2).enumerate().filter_map(move |(index, pair)| {
        let (a, b) = (pair[0], pair[1]);
        let (road, stream) = (to - from, b - a);
        let across = road.perp_dot(stream);
        let t = (a - from).perp_dot(stream) / across;
        let u = (a - from).perp_dot(road) / across;
        (across.abs() > 1e-4 && (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u))
          .then(|| (from + road * t, road.normalize(), width_at(index)))
      })
    })
    .fold(Vec::<(Vec2, Vec2, f32)>::new(), |mut found, crossing| {
      if found.iter().all(|other| other.0.distance(crossing.0) > 20.0) {
        found.push(crossing);
      }
      found
    })
}

fn bridge(
  width: f32,
  deck: f32,
  low: f32
) -> (Vec<(Stuff, Piece)>, Vec<(Vec3, Collider)>) {
  let stone = Srgba::new(0.55, 0.54, 0.51, 1.0);
  let span = width * 2.0 + 10.0;
  let arches = 7;
  let ring: Vec<(Stuff, Piece)> = (0..arches)
    .map(|index| {
      let angle = PI * (index as f32 + 0.5) / arches as f32;
      let radius = width + 1.6;
      let at = Vec3::new(0.0, low + radius * angle.sin() * 0.7, -radius * angle.cos());
      (
        Stuff::Masonry,
        Piece::new(block(4.6, 1.0, radius * PI / arches as f32 + 0.3), stone * 0.92)
          .planar(1.6)
          .pitched(angle - PI / 2.0)
          .at(at)
      )
    })
    .collect();
  let deck_piece = (
    Stuff::Masonry,
    Piece::new(block(4.6, 1.1, span), stone).planar(1.8).at_xyz(0.0, deck - 0.45, 0.0)
  );
  let parapets = [-2.1, 2.1].map(|x| {
    (
      Stuff::Masonry,
      Piece::new(block(0.5, 0.9, span), stone * 0.9).planar(1.4).at_xyz(
        x,
        deck + 0.45,
        0.0
      )
    )
  });
  let piers = [-1.0, 1.0].map(|side| {
    let z = side * (width + 3.0);
    let tall = deck - low + 2.0;
    (
      Stuff::Masonry,
      Piece::new(block(5.0, tall, 3.4), stone * 0.85).planar(1.6).at_xyz(
        0.0,
        deck - tall / 2.0,
        z
      )
    )
  });
  (ring.into_iter().chain([deck_piece]).chain(parapets).chain(piers).collect(), vec![
    (Vec3::new(0.0, deck - 0.45, 0.0), Collider::cuboid(4.6, 1.1, span)),
    (Vec3::new(2.1, deck + 0.45, 0.0), Collider::cuboid(0.5, 0.9, span)),
    (Vec3::new(-2.1, deck + 0.45, 0.0), Collider::cuboid(0.5, 0.9, span)),
  ])
}

fn spill(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut images: ResMut<Assets<Image>>,
  stuffs: Res<Stuffs>
) {
  let ripples = images.add(texture::bumps(256, 0.012, |u, v| {
    texture::stretched_noise(u, v, 16, 8, 211) + 0.5 * texture::tile_fbm(u, v, 24, 2, 213)
  }));
  commands.spawn((
    Name::new("River"),
    Flow,
    Mesh3d(meshes.add(water_mesh())),
    MeshMaterial3d(materials.add(StandardMaterial {
      base_color: Color::srgba(0.10, 0.14, 0.13, 0.85),
      normal_map_texture: Some(ripples),
      perceptual_roughness: 0.14,
      reflectance: 0.4,
      alpha_mode: AlphaMode::Blend,
      uv_transform: Affine2::from_scale(Vec2::new(2.0, 3.0)),
      ..default()
    })),
    Transform::IDENTITY
  ));
  crossings().into_iter().for_each(|(at, along, width)| {
    let deck = terrain::height_at(at + along * (width + 9.0))
      .max(terrain::height_at(at - along * (width + 9.0)))
      .max(reach(at).map_or(0.0, |reach| reach.level + 2.2))
      + 0.1;
    let low = reach(at).map_or(deck - 4.0, |reach| reach.level - DEPTH);
    let origin = at.extend(0.0).xzy();
    let (parts, solids) = bridge(width, deck, low);
    let root = commands
      .spawn((
        Name::new("Bridge"),
        Transform::from_translation(origin)
          .with_rotation(Quat::from_rotation_y(f32::atan2(along.x, along.y))),
        Visibility::Inherited,
        RigidBody::Static,
        Collider::compound(
          solids
            .into_iter()
            .map(|(at, collider)| (Position(at), Quat::IDENTITY, collider))
            .collect()
        )
      ))
      .id();
    parts
      .into_iter()
      .fold(Vec::<(Stuff, Vec<Piece>)>::new(), |mut groups, (stuff, piece)| {
        match groups.iter_mut().find(|(each, _)| *each == stuff) {
          Some((_, list)) => list.push(piece),
          None => groups.push((stuff, vec![piece]))
        }
        groups
      })
      .into_iter()
      .for_each(|(stuff, pieces)| {
        let mesh =
          crate::model::merge(pieces).with_generated_tangents().expect("bridge uvs");
        commands.spawn((
          Mesh3d(meshes.add(mesh)),
          MeshMaterial3d(stuffs.of(stuff)),
          ChildOf(root)
        ));
      });
  });
}

fn flow(
  time: Res<Time>,
  rivers: Query<&MeshMaterial3d<StandardMaterial>, With<Flow>>,
  mut materials: ResMut<Assets<StandardMaterial>>
) {
  rivers.iter().for_each(|river| {
    if let Some(mut water) = materials.get_mut(&river.0) {
      water.uv_transform.translation = Vec2::new(0.0, -0.09 * time.elapsed_secs());
    }
  });
}

pub fn plugin(app: &mut App) {
  app.add_systems(Startup, spill).add_systems(Update, flow);
}
