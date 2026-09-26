use {crate::{model::{Piece, block},
             noise,
             place::{self, ROADS},
             stuff::{Stuff, Stuffs},
             terrain::{self, smooth},
             texture},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            math::Affine2,
            mesh::{Indices, PrimitiveTopology},
            platform::collections::HashMap,
            prelude::*},
     std::{f32::consts::{PI, TAU},
           sync::LazyLock}};

#[derive(Clone, Copy)]
pub struct Basin {
  pub center: Vec2,
  pub radius: f32,
  pub cap: f32
}

pub const BASINS: [Basin; 9] = [
  Basin { center: Vec2::new(-150.0, 230.0), radius: 95.0, cap: f32::MAX },
  Basin { center: Vec2::new(-2450.0, 250.0), radius: 190.0, cap: f32::MAX },
  Basin { center: Vec2::new(-2150.0, 1600.0), radius: 90.0, cap: f32::MAX },
  Basin { center: Vec2::new(1450.0, 2150.0), radius: 200.0, cap: f32::MAX },
  Basin { center: Vec2::new(250.0, 2350.0), radius: 120.0, cap: f32::MAX },
  Basin { center: Vec2::new(2600.0, 950.0), radius: 70.0, cap: f32::MAX },
  Basin { center: Vec2::new(-2550.0, -1350.0), radius: 110.0, cap: f32::MAX },
  Basin { center: Vec2::new(2500.0, 1650.0), radius: 100.0, cap: 34.5 },
  Basin { center: Vec2::new(1050.0, 2750.0), radius: 70.0, cap: f32::MAX }
];

pub const START_LAKE_LEVEL: f32 = 4.0;

pub static LAKE_LEVELS: LazyLock<Vec<f32>> = LazyLock::new(|| {
  BASINS
    .iter()
    .enumerate()
    .map(|(index, &Basin { center, radius, cap })| {
      (index == 0).then_some(START_LAKE_LEVEL).unwrap_or_else(|| {
        (0..32)
          .flat_map(|step| {
            [1.0, 1.3, 1.6, 2.0].map(|ring| {
              terrain::wild_height(
                center + Vec2::from_angle(step as f32 / 32.0 * TAU) * radius * ring
              )
            })
          })
          .fold(terrain::wild_height(center) + 3.0, f32::min)
          .min(cap + 1.5)
          - 1.5
      })
    })
    .collect()
});

#[derive(Clone, Copy, Debug)]
pub struct Lake {
  pub center: Vec2,
  pub radius: f32,
  pub level: f32
}

pub fn lakes() -> impl Iterator<Item = Lake> {
  BASINS
    .iter()
    .zip(LAKE_LEVELS.iter())
    .map(|(&Basin { center, radius, .. }, &level)| Lake { center, radius, level })
}

pub fn lake_near(at: Vec2, reach: f32) -> Option<Lake> {
  BASINS
    .iter()
    .position(|&Basin { center, radius, .. }| at.distance(center) < radius * reach)
    .map(|index| Lake {
      center: BASINS[index].center,
      radius: BASINS[index].radius,
      level: LAKE_LEVELS[index]
    })
}

#[derive(Clone, Copy)]
enum End {
  Spring,
  Lake(usize),
  River(usize)
}

struct Course {
  points: &'static [Vec2],
  source: End,
  mouth: End,
  widths: [f32; 2],
  swing: f32
}

const COURSES: [Course; 13] = [
  Course {
    points: &[
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
    ],
    source: End::Spring,
    mouth: End::Lake(0),
    widths: [3.0, 7.0],
    swing: 0.0
  },
  Course {
    points: &[
      Vec2::new(-2330.0, -1780.0),
      Vec2::new(-2420.0, -1600.0),
      Vec2::new(-2500.0, -1450.0),
      Vec2::new(-2550.0, -1350.0)
    ],
    source: End::Spring,
    mouth: End::Lake(6),
    widths: [2.5, 4.5],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(-2450.0, 150.0),
      Vec2::new(-2410.0, -40.0),
      Vec2::new(-2290.0, -360.0),
      Vec2::new(-2260.0, -660.0),
      Vec2::new(-2440.0, -900.0),
      Vec2::new(-2480.0, -1150.0),
      Vec2::new(-2520.0, -1350.0)
    ],
    source: End::Lake(1),
    mouth: End::Lake(6),
    widths: [5.0, 6.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(-2440.0, 300.0),
      Vec2::new(-2410.0, 520.0),
      Vec2::new(-2350.0, 820.0),
      Vec2::new(-2260.0, 1150.0),
      Vec2::new(-2190.0, 1440.0),
      Vec2::new(-2160.0, 1560.0)
    ],
    source: End::Lake(1),
    mouth: End::Lake(2),
    widths: [7.0, 8.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(2760.0, 560.0),
      Vec2::new(2690.0, 760.0),
      Vec2::new(2620.0, 900.0)
    ],
    source: End::Spring,
    mouth: End::Lake(5),
    widths: [2.5, 4.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(2590.0, 960.0),
      Vec2::new(2540.0, 1180.0),
      Vec2::new(2560.0, 1420.0),
      Vec2::new(2510.0, 1620.0)
    ],
    source: End::Lake(5),
    mouth: End::Lake(7),
    widths: [4.0, 5.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(2490.0, 1660.0),
      Vec2::new(2300.0, 1830.0),
      Vec2::new(2060.0, 1930.0),
      Vec2::new(1800.0, 2040.0),
      Vec2::new(1500.0, 2140.0)
    ],
    source: End::Lake(7),
    mouth: End::Lake(3),
    widths: [5.0, 7.5],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(1830.0, 880.0),
      Vec2::new(1740.0, 1120.0),
      Vec2::new(1680.0, 1420.0),
      Vec2::new(1560.0, 1720.0),
      Vec2::new(1470.0, 2000.0),
      Vec2::new(1450.0, 2120.0)
    ],
    source: End::Spring,
    mouth: End::Lake(3),
    widths: [2.5, 6.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(270.0, 2350.0),
      Vec2::new(480.0, 2360.0),
      Vec2::new(700.0, 2320.0),
      Vec2::new(960.0, 2300.0),
      Vec2::new(1210.0, 2260.0),
      Vec2::new(1400.0, 2160.0)
    ],
    source: End::Lake(4),
    mouth: End::Lake(3),
    widths: [5.0, 8.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(820.0, 1420.0),
      Vec2::new(870.0, 1660.0),
      Vec2::new(830.0, 1900.0),
      Vec2::new(760.0, 2120.0),
      Vec2::new(700.0, 2320.0)
    ],
    source: End::Spring,
    mouth: End::River(8),
    widths: [2.5, 5.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(1290.0, 240.0),
      Vec2::new(1330.0, 520.0),
      Vec2::new(1260.0, 800.0),
      Vec2::new(1150.0, 1030.0),
      Vec2::new(1120.0, 1180.0),
      Vec2::new(1030.0, 1480.0),
      Vec2::new(930.0, 1760.0),
      Vec2::new(850.0, 1880.0)
    ],
    source: End::Spring,
    mouth: End::River(9),
    widths: [2.0, 4.0],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(-1640.0, -1150.0),
      Vec2::new(-1950.0, -1210.0),
      Vec2::new(-2250.0, -1300.0),
      Vec2::new(-2480.0, -1340.0)
    ],
    source: End::Spring,
    mouth: End::Lake(6),
    widths: [2.0, 4.5],
    swing: 70.0
  },
  Course {
    points: &[
      Vec2::new(-1250.0, -660.0),
      Vec2::new(-1500.0, -480.0),
      Vec2::new(-1800.0, -250.0),
      Vec2::new(-2100.0, -50.0),
      Vec2::new(-2300.0, 150.0),
      Vec2::new(-2400.0, 230.0)
    ],
    source: End::Spring,
    mouth: End::Lake(1),
    widths: [2.0, 5.5],
    swing: 70.0
  }
];

pub const RIVERS: usize = COURSES.len();
const STEP: f32 = 4.0;
const BIN: f32 = 64.0;
const DEPTH: f32 = 1.7;
const SIDES: f32 = 0.4;
const VALLEY: f32 = 200.0;
const DROP: f32 = 0.012;
const GRADE: f32 = 0.04;

fn meandered(path: Vec<Vec2>, swing: f32, seed: u32) -> Vec<Vec2> {
  let lengths: Vec<f32> = path
    .iter()
    .scan((path[0], 0.0), |(last, length), &at| {
      *length += last.distance(at);
      *last = at;
      Some(*length)
    })
    .collect();
  let total = lengths[lengths.len() - 1];
  (0..path.len())
    .map(|index| {
      let before = path[index.saturating_sub(1)];
      let after = path[(index + 1).min(path.len() - 1)];
      let side = (after - before).normalize_or(Vec2::X).perp();
      let length = lengths[index];
      let calm = smooth(0.0, 120.0, length) * smooth(total, total - 120.0, length);
      let wander = noise::fbm(Vec2::new(length / 260.0, seed as f32 * 7.3), 3, seed);
      path[index] + side * wander * swing * calm
    })
    .collect()
}

pub static PATHS: LazyLock<Vec<Vec<Vec2>>> = LazyLock::new(|| {
  COURSES
    .iter()
    .enumerate()
    .map(|(river, course)| {
      meandered(place::smoothed(course.points), course.swing, 300 + river as u32)
        .windows(2)
        .flat_map(|pair| {
          let steps = (pair[0].distance(pair[1]) / STEP).ceil().max(1.0) as usize;
          (0..steps).map(move |step| pair[0].lerp(pair[1], step as f32 / steps as f32))
        })
        .chain(course.points.last().copied())
        .collect()
    })
    .collect()
});

fn width_at(river: usize, index: usize) -> f32 {
  let [source, mouth] = COURSES[river].widths;
  source.lerp(mouth, (index as f32 / PATHS[river].len() as f32).sqrt())
}

type Bins = HashMap<IVec2, Vec<(usize, usize)>>;

static BINS: LazyLock<Bins> = LazyLock::new(|| {
  PATHS.iter().enumerate().fold(HashMap::default(), |bins, (river, path)| {
    (0..path.len() - 1).fold(bins, |mut bins, index| {
      let (from, to) = (path[index], path[index + 1]);
      let low = ((from.min(to) - VALLEY) / BIN).floor().as_ivec2();
      let high = ((from.max(to) + VALLEY) / BIN).floor().as_ivec2();
      (low.y..=high.y).for_each(|y| {
        (low.x..=high.x)
          .for_each(|x| bins.entry(IVec2::new(x, y)).or_default().push((river, index)))
      });
      bins
    })
  })
});

type Closest = [Option<(f32, usize, f32)>; RIVERS];

fn closest(at: Vec2) -> Closest {
  BINS.get(&(at / BIN).floor().as_ivec2()).map_or([None; RIVERS], |segments| {
    segments.iter().fold([None; RIVERS], |mut best, &(river, index)| {
      let (from, to) = (PATHS[river][index], PATHS[river][index + 1]);
      let along =
        ((at - from).dot(to - from) / (to - from).length_squared()).clamp(0.0, 1.0);
      let distance = at.distance(from.lerp(to, along));
      if best[river].is_none_or(|(known, ..)| distance < known) {
        best[river] = Some((distance, index, along));
      }
      best
    })
  })
}

pub fn course_distance(at: Vec2) -> f32 {
  closest(at)
    .into_iter()
    .flatten()
    .fold(f32::MAX, |nearest, (distance, ..)| nearest.min(distance))
}

pub fn closest_point(at: Vec2) -> Option<Vec2> {
  closest(at)
    .into_iter()
    .enumerate()
    .filter_map(|(river, found)| found.map(|found| (river, found)))
    .min_by(|a, b| a.1.0.total_cmp(&b.1.0))
    .map(|(river, (_, index, along))| {
      PATHS[river][index].lerp(PATHS[river][index + 1], along)
    })
}

pub static LEVELS: LazyLock<Vec<Vec<f32>>> = LazyLock::new(|| {
  COURSES.iter().zip(PATHS.iter()).fold(
    Vec::new(),
    |mut done: Vec<Vec<f32>>, (course, path)| {
      let level_of = |end: End, at: Vec2| match end {
        End::Spring => None,
        End::Lake(lake) => Some(LAKE_LEVELS[lake]),
        End::River(river) => PATHS[river]
          .iter()
          .enumerate()
          .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)))
          .map(|(index, _)| done[river][index])
      };
      let brim = |at: Vec2| match course.source {
        End::Lake(lake) => (at.distance(BASINS[lake].center) < BASINS[lake].radius * 2.0)
          .then_some(LAKE_LEVELS[lake])
          .unwrap_or(f32::MIN),
        _ => f32::MIN
      };
      let source = level_of(course.source, path[0]).unwrap_or(f32::MAX);
      let floor = level_of(course.mouth, path[path.len() - 1]).expect("river mouth");
      let levels = path
        .iter()
        .enumerate()
        .scan(source, |level, (index, &at)| {
          let steepest = matches!(course.mouth, End::River(_))
            .then(|| floor + (path.len() - 1 - index) as f32 * STEP * GRADE)
            .unwrap_or(f32::MAX);
          *level = (*level - DROP * STEP)
            .min(terrain::natural_height(at) - 1.3)
            .min(steepest)
            .max(floor)
            .max(brim(at));
          Some(*level)
        })
        .collect();
      done.push(levels);
      done
    }
  )
});

#[derive(Clone, Copy, Debug)]
pub struct Reach {
  pub distance: f32,
  pub level: f32,
  pub width: f32
}

fn reaches(at: Vec2) -> impl Iterator<Item = Reach> {
  closest(at).into_iter().enumerate().filter_map(|(river, found)| {
    found.map(|(distance, index, along)| Reach {
      distance,
      level: LEVELS[river][index].lerp(LEVELS[river][index + 1], along),
      width: width_at(river, index)
    })
  })
}

pub fn reach(at: Vec2) -> Option<Reach> {
  reaches(at).min_by(|a, b| a.distance.total_cmp(&b.distance))
}

pub fn carve(at: Vec2, height: f32) -> f32 {
  reaches(at).fold(height, |height, Reach { distance, level, width }| {
    let side = (distance - width).max(0.0);
    let bed = level + 0.3 - (DEPTH + 0.3) * smooth(width, width * 0.25, distance)
      + side * SIDES * smooth(0.0, 5.0, side)
      + ((side / 45.0).exp() - 1.0) * 3.0;
    let carved = height.min(bed).lerp(height, smooth(VALLEY - 40.0, VALLEY, distance));
    carved.lerp(bed, smooth(width + 2.0, width, distance))
  })
}

pub fn water_level(at: Vec2) -> Option<f32> {
  lake_near(at, 2.0)
    .map(|lake| lake.level)
    .into_iter()
    .chain(
      reaches(at)
        .filter(|reach| reach.distance < reach.width + 1.0)
        .map(|reach| reach.level)
    )
    .reduce(f32::max)
}

pub fn bank(at: Vec2) -> f32 {
  reaches(at).fold(0.0, |bank, Reach { distance, width, .. }| {
    bank.max(smooth(width + 3.5, width + 1.0, distance))
  })
}

fn water_mesh(river: usize) -> Mesh {
  let (path, levels) = (&PATHS[river], &LEVELS[river]);
  let rows: Vec<[(Vec3, Vec2); 2]> = path
    .iter()
    .enumerate()
    .map(|(index, &at)| {
      let before = path[index.saturating_sub(1)];
      let after = path[(index + 1).min(path.len() - 1)];
      let side = (after - before).normalize_or(Vec2::X).perp();
      let width = width_at(river, index) + 1.6;
      let level = levels[index];
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
      PATHS.iter().enumerate().flat_map(move |(river, path)| {
        path.windows(2).enumerate().filter_map(move |(index, pair)| {
          let (a, b) = (pair[0], pair[1]);
          let (road, stream) = (to - from, b - a);
          let across = road.perp_dot(stream);
          let t = (a - from).perp_dot(stream) / across;
          let u = (a - from).perp_dot(road) / across;
          (across.abs() > 1e-4 && (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u))
            .then(|| (from + road * t, road.normalize(), width_at(river, index)))
        })
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
  let water = materials.add(StandardMaterial {
    base_color: Color::srgba(0.10, 0.14, 0.13, 0.85),
    normal_map_texture: Some(ripples),
    perceptual_roughness: 0.14,
    reflectance: 0.4,
    alpha_mode: AlphaMode::Blend,
    uv_transform: Affine2::from_scale(Vec2::new(2.0, 3.0)),
    ..default()
  });
  (0..RIVERS).for_each(|river| {
    commands.spawn((
      Name::new("River"),
      Flow,
      Mesh3d(meshes.add(water_mesh(river))),
      MeshMaterial3d(water.clone()),
      Transform::IDENTITY
    ));
  });
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

fn fill_lakes(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut images: ResMut<Assets<Image>>
) {
  let ripples = images.add(texture::bumps(256, 0.02, |u, v| {
    texture::tile_fbm(u, v, 6, 4, 131) + 0.5 * texture::tile_fbm(u, v, 24, 2, 133)
  }));
  let water = materials.add(StandardMaterial {
    base_color: Color::srgba(0.10, 0.16, 0.18, 0.86),
    normal_map_texture: Some(ripples),
    perceptual_roughness: 0.06,
    reflectance: 0.6,
    alpha_mode: AlphaMode::Blend,
    uv_transform: Affine2::from_scale(Vec2::splat(18.0)),
    ..default()
  });
  lakes().for_each(|Lake { center, radius, level }| {
    commands.spawn((
      Name::new("Lake"),
      Ripples,
      Mesh3d(
        meshes.add(
          Plane3d::default()
            .mesh()
            .size(radius * 4.0, radius * 4.0)
            .subdivisions(8)
            .build()
            .with_generated_tangents()
            .expect("lake tangents")
        )
      ),
      MeshMaterial3d(water.clone()),
      Transform::from_translation(center.extend(level).xzy())
    ));
  });
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
  app.add_systems(Startup, (spill, fill_lakes)).add_systems(Update, (flow, ripple));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[ignore]
  fn levels() {
    println!("lakes {:?}", *LAKE_LEVELS);
    LEVELS.iter().enumerate().for_each(|(river, levels)| {
      let floating = PATHS[river]
        .iter()
        .zip(levels)
        .filter(|(at, level)| **level > terrain::natural_height(**at) + 0.5)
        .count();
      println!(
        "river {river}: {:.1} -> {:.1}, {} points, {floating} floating",
        levels[0],
        levels[levels.len() - 1],
        levels.len()
      )
    });
    [Vec2::new(1130.0, 2280.0), Vec2::new(1100.0, 2350.0)].into_iter().for_each(|at| {
      println!(
        "{at}: height {} natural {} water {:?} reach {:?}",
        terrain::height_at(at),
        terrain::natural_height(at),
        water_level(at),
        reach(at)
      )
    });
  }
}
