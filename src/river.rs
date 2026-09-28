use {crate::{model::{Piece, block},
             noise,
             place::{self, Marker},
             stuff::{Stuff, Stuffs},
             terrain::{self, smooth},
             texture},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            math::Affine2,
            mesh::{Indices, PrimitiveTopology},
            platform::collections::HashMap,
            prelude::*},
     std::{collections::{BinaryHeap, VecDeque},
           f32::consts::{PI, TAU},
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
  Lake(usize),
  River(usize)
}

struct Course {
  path: Vec<Vec2>,
  widths: Vec<f32>,
  mouth: End
}

const CELL: f32 = 24.0;
const CELLS: usize = (2.0 * terrain::WORLD / CELL) as usize;
const SPRING_AREA: f32 = 0.55e6;
const SHORTEST: usize = 8;
const SWING: f32 = 14.0;
const STEP: f32 = 4.0;
const BIN: f32 = 64.0;
const DEPTH: f32 = 1.7;
const SIDES: f32 = 0.4;
const VALLEY: f32 = 200.0;
const DROP: f32 = 0.012;
const GRADE: f32 = 0.04;

fn cell_center(index: usize) -> Vec2 {
  (Vec2::new((index % CELLS) as f32, (index / CELLS) as f32) + 0.5) * CELL
    - terrain::WORLD
}

fn neighbours(index: usize) -> impl Iterator<Item = usize> {
  let (x, y) = ((index % CELLS) as i32, (index / CELLS) as i32);
  [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
    .into_iter()
    .map(move |(dx, dy)| (x + dx, y + dy))
    .filter(|&(x, y)| (0..CELLS as i32).contains(&x) && (0..CELLS as i32).contains(&y))
    .map(|(x, y)| y as usize * CELLS + x as usize)
}

fn routing_height(at: Vec2) -> f32 {
  let settled = place::named()
    .filter(|place| {
      matches!(place.marker(), Marker::Town | Marker::City | Marker::Farm | Marker::Fort)
    })
    .fold(0.0_f32, |bump, place| {
      bump.max(smooth(place.flat() * 1.8, place.flat() * 1.1, at.distance(place.spot())))
    });
  terrain::land(at, 1.0) + 14.0 * settled + 4.0 * noise::fbm(at / 170.0, 3, 331)
}

struct Drainage {
  down: Vec<Option<usize>>,
  area: Vec<f32>,
  lake: Vec<Option<usize>>
}

#[derive(PartialEq)]
struct Lowest(f32, usize);

impl Eq for Lowest {}

impl Ord for Lowest {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    other.0.total_cmp(&self.0).then(other.1.cmp(&self.1))
  }
}

impl PartialOrd for Lowest {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

static DRAINAGE: LazyLock<Drainage> = LazyLock::new(|| {
  let count = CELLS * CELLS;
  let heights = terrain::in_parallel(&(0..count).collect::<Vec<_>>(), |&index| {
    routing_height(cell_center(index))
  });
  let mut down = vec![None; count];
  let mut lake: Vec<Option<usize>> = (0..count)
    .map(|index| {
      BASINS.iter().position(|basin| {
        cell_center(index).distance(basin.center) < basin.radius * 0.7 + CELL * 0.75
      })
    })
    .collect();
  let mut seen: Vec<bool> = lake.iter().map(Option::is_some).collect();
  let mut rising: BinaryHeap<Lowest> = (0..count)
    .filter(|&index| seen[index])
    .map(|index| Lowest(heights[index], index))
    .collect();
  let mut order = Vec::with_capacity(count);
  while let Some(Lowest(level, index)) = rising.pop() {
    order.push(index);
    for next in neighbours(index) {
      if !seen[next] {
        seen[next] = true;
        down[next] = Some(index);
        lake[next] = lake[index];
        rising.push(Lowest(heights[next].max(level + 0.01), next));
      }
    }
  }
  let area = order.iter().rev().fold(vec![CELL * CELL; count], |mut area, &index| {
    if let Some(below) = down[index] {
      area[below] += area[index];
    }
    area
  });
  Drainage { down, area, lake }
});

fn chaikin(points: Vec<Vec3>) -> Vec<Vec3> {
  points
    .first()
    .copied()
    .into_iter()
    .chain(
      points
        .windows(2)
        .flat_map(|pair| [pair[0].lerp(pair[1], 0.25), pair[0].lerp(pair[1], 0.75)])
    )
    .chain(points.last().copied())
    .collect()
}

fn width_for(area: f32) -> f32 { (1.2 + 1.1 * (area / 1.0e6).sqrt()).min(8.0) }

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
      let wander = noise::fbm(Vec2::new(length / 180.0, seed as f32 * 7.3), 3, seed);
      path[index] + side * wander * swing * calm
    })
    .collect()
}

fn course(river: usize, cells: &[usize], mouth: End) -> Course {
  let area = &DRAINAGE.area;
  let last = cells.len() - 1;
  let points: Vec<Vec3> = cells
    .iter()
    .enumerate()
    .rev()
    .filter(|&(step, _)| step % 2 == 0 || step == last)
    .map(|(_, &index)| cell_center(index).extend(width_for(area[index])))
    .collect();
  let smoothed = (0..4).fold(points, |points, _| chaikin(points));
  let bent = meandered(
    smoothed.iter().map(|point| point.xy()).collect(),
    SWING,
    300 + river as u32
  );
  let (path, widths) = bent
    .windows(2)
    .zip(smoothed.windows(2))
    .flat_map(|(pair, sizes)| {
      let steps = (pair[0].distance(pair[1]) / STEP).ceil().max(1.0) as usize;
      let (from, to, wide, wider) = (pair[0], pair[1], sizes[0].z, sizes[1].z);
      (0..steps).map(move |step| {
        let t = step as f32 / steps as f32;
        (from.lerp(to, t), wide.lerp(wider, t))
      })
    })
    .chain(bent.last().copied().zip(smoothed.last().map(|point| point.z)))
    .unzip();
  Course { path, widths, mouth }
}

static COURSES: LazyLock<Vec<Course>> = LazyLock::new(|| {
  let Drainage { down, area, lake } = &*DRAINAGE;
  let big = |index: usize| area[index] >= SPRING_AREA;
  let pooled = |index: usize| lake[index].is_some() && down[index].is_none();
  let feeding = |current: usize| {
    neighbours(current).filter(move |&next| down[next] == Some(current) && big(next))
  };
  let mut waiting: VecDeque<(usize, End)> = (0..CELLS * CELLS)
    .filter(|&index| !pooled(index) && big(index) && down[index].is_some_and(pooled))
    .filter_map(|index| lake[index].map(|lake| (index, End::Lake(lake))))
    .collect();
  let mut traced: Vec<(Vec<usize>, End)> = Vec::new();
  while let Some((start, mouth)) = waiting.pop_front() {
    let river = traced.len();
    let mut cells: Vec<usize> = down[start].into_iter().chain([start]).collect();
    let mut current = start;
    while let Some(next) = feeding(current).max_by(|a, b| area[*a].total_cmp(&area[*b])) {
      for other in feeding(current).filter(|&other| other != next) {
        waiting.push_back((other, End::River(river)))
      }
      cells.push(next);
      current = next;
    }
    traced.push((cells, mouth));
  }
  traced
    .into_iter()
    .enumerate()
    .fold(
      (Vec::new(), Vec::new()),
      |(mut kept, mut renamed): (Vec<Course>, Vec<Option<usize>>),
       (river, (cells, mouth))| {
        let mouth = match mouth {
          End::River(parent) => renamed[parent].map(End::River),
          lake => Some(lake)
        };
        renamed.push(mouth.filter(|_| cells.len() >= SHORTEST).map(|mouth| {
          kept.push(course(river, &cells, mouth));
          kept.len() - 1
        }));
        (kept, renamed)
      }
    )
    .0
});

pub static PATHS: LazyLock<Vec<Vec<Vec2>>> =
  LazyLock::new(|| COURSES.iter().map(|course| course.path.clone()).collect());

pub fn rivers() -> usize { COURSES.len() }

fn width_at(river: usize, index: usize) -> f32 { COURSES[river].widths[index] }

type Bins = HashMap<IVec2, Vec<(usize, usize)>>;

static BINS: LazyLock<Bins> = LazyLock::new(|| {
  PATHS.iter().enumerate().fold(HashMap::default(), |bins, (river, path)| {
    (0..path.len() - 1).fold(bins, |mut bins, index| {
      let (from, to) = (path[index], path[index + 1]);
      let low = ((from.min(to) - VALLEY) / BIN).floor().as_ivec2();
      let high = ((from.max(to) + VALLEY) / BIN).floor().as_ivec2();
      for y in low.y..=high.y {
        for x in low.x..=high.x {
          bins.entry(IVec2::new(x, y)).or_default().push((river, index))
        }
      }
      bins
    })
  })
});

const NEARBY: usize = 8;

type Closest = [Option<(usize, f32, usize, f32)>; NEARBY];

fn closest(at: Vec2) -> Closest {
  BINS.get(&(at / BIN).floor().as_ivec2()).map_or([None; NEARBY], |segments| {
    segments.iter().fold([None; NEARBY], |mut best, &(river, index)| {
      let (from, to) = (PATHS[river][index], PATHS[river][index + 1]);
      let along =
        ((at - from).dot(to - from) / (to - from).length_squared()).clamp(0.0, 1.0);
      let distance = at.distance(from.lerp(to, along));
      let slot = best
        .iter()
        .position(|found| found.is_none_or(|(known, ..)| known == river))
        .unwrap_or_else(|| {
          (0..NEARBY)
            .max_by(|&a, &b| {
              let far =
                |slot: usize| best[slot].map_or(0.0, |(_, distance, ..)| distance);
              far(a).total_cmp(&far(b))
            })
            .unwrap_or(0)
        });
      if best[slot].is_none_or(|(_, known, ..)| distance < known) {
        best[slot] = Some((river, distance, index, along));
      }
      best
    })
  })
}

pub fn course_distance(at: Vec2) -> f32 {
  closest(at)
    .into_iter()
    .flatten()
    .fold(f32::MAX, |nearest, (_, distance, ..)| nearest.min(distance))
}

pub fn closest_point(at: Vec2) -> Option<Vec2> {
  closest(at).into_iter().flatten().min_by(|a, b| a.1.total_cmp(&b.1)).map(
    |(river, _, index, along)| PATHS[river][index].lerp(PATHS[river][index + 1], along)
  )
}

pub static LEVELS: LazyLock<Vec<Vec<f32>>> = LazyLock::new(|| {
  COURSES.iter().zip(PATHS.iter()).fold(
    Vec::new(),
    |mut done: Vec<Vec<f32>>, (course, path)| {
      let level_of = |end: End, at: Vec2| match end {
        End::Lake(lake) => Some(LAKE_LEVELS[lake]),
        End::River(river) => PATHS[river]
          .iter()
          .enumerate()
          .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)))
          .map(|(index, _)| done[river][index])
      };
      let floor = level_of(course.mouth, path[path.len() - 1]).expect("river mouth");
      let levels = path
        .iter()
        .enumerate()
        .scan(f32::MAX, |level, (index, &at)| {
          let steepest = matches!(course.mouth, End::River(_))
            .then(|| floor + (path.len() - 1 - index) as f32 * STEP * GRADE)
            .unwrap_or(f32::MAX);
          *level = (*level - DROP * STEP)
            .min(terrain::natural_height(at) - 1.3)
            .min(steepest)
            .max(floor);
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
  closest(at).into_iter().flatten().map(|(river, distance, index, along)| Reach {
    distance,
    level: LEVELS[river][index].lerp(LEVELS[river][index + 1], along),
    width: width_at(river, index)
  })
}

pub fn reach_at(at: Vec2) -> Option<Reach> {
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

pub struct Bridge {
  at: Vec2,
  along: Vec2,
  reach: f32,
  half_wide: f32,
  deck: f32,
  low: f32
}

const RAMP: f32 = 0.16;
const EMBANKMENT: f32 = 9.0;

impl Bridge {
  fn local(&self, at: Vec2) -> Vec2 {
    let offset = at - self.at;
    Vec2::new(offset.dot(self.along), offset.perp_dot(self.along))
  }

  fn near(&self, at: Vec2) -> bool {
    at.distance(self.at) < self.reach + (self.deck - self.low) / RAMP + 60.0
  }
}

pub static BRIDGES: LazyLock<Vec<Bridge>> = LazyLock::new(|| {
  let crossings = place::ways().flat_map(|road| {
    road.path.windows(2).filter_map(|pair| {
      let (from, to) = (pair[0], pair[1]);
      let road_line = to - from;
      BINS.get(&(((from + to) / 2.0) / BIN).floor().as_ivec2()).and_then(|segments| {
        segments.iter().find_map(|&(river, index)| {
          let (a, b) = (PATHS[river][index], PATHS[river][index + 1]);
          let stream = b - a;
          let across = road_line.perp_dot(stream);
          let t = (a - from).perp_dot(stream) / across;
          let u = (a - from).perp_dot(road_line) / across;
          (across.abs() > 1e-4 && (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u))
            .then(|| {
              let along = road_line.normalize();
              let slant = along.perp_dot(stream.normalize()).abs().max(0.5);
              (
                from + road_line * t,
                along,
                (width_at(river, index) + 2.5) / slant + 3.0,
                road.paving.half_width() + 0.6
              )
            })
        })
      })
    })
  });
  crossings
    .fold(Vec::<(Vec2, Vec2, f32, f32)>::new(), |mut found, crossing| {
      if found.iter().all(|other| other.0.distance(crossing.0) > 30.0) {
        found.push(crossing);
      }
      found
    })
    .into_iter()
    .map(|(at, along, reach, half_wide)| {
      let water = reach_at(at).map_or(0.0, |reach| reach.level);
      let bank = |side: f32| terrain::unbridged_height(at + along * side * reach);
      let deck = bank(1.0).max(bank(-1.0)).max(water + 2.6) + 0.1;
      Bridge { at, along, reach, half_wide, deck, low: water - DEPTH }
    })
    .collect()
});

pub fn ramp(at: Vec2, height: f32) -> f32 {
  BRIDGES.iter().filter(|bridge| bridge.near(at)).fold(height, |height, bridge| {
    let local = bridge.local(at);
    let beyond = local.x.abs() - (bridge.reach - 1.2);
    let target = bridge.deck - 0.05 - beyond.max(0.0) * RAMP;
    let beside = smooth(
      bridge.half_wide + EMBANKMENT,
      bridge.half_wide + 1.0,
      local.y.abs() - (target - height).max(0.0) * 0.5
    );
    height.lerp(height.max(target), beside * smooth(-0.6, 0.4, beyond))
  })
}

pub fn deck(at: Vec2) -> Option<f32> {
  BRIDGES
    .iter()
    .filter(|bridge| bridge.at.distance(at) < bridge.reach + 4.0)
    .find(|bridge| {
      let local = bridge.local(at);
      local.x.abs() < bridge.reach && local.y.abs() < bridge.half_wide
    })
    .map(|bridge| bridge.deck + 0.1)
}

fn bridge(
  &Bridge { reach, half_wide, deck, low, .. }: &Bridge
) -> (Vec<(Stuff, Piece)>, Vec<(Vec3, Collider)>) {
  let stone = Srgba::new(0.55, 0.54, 0.51, 1.0);
  let (span, wide) = (reach * 2.0, half_wide * 2.0);
  let arches = 9;
  let radius = reach - 1.5;
  let rise = (deck - 1.0 - low).clamp(1.5, radius) / radius;
  let ring: Vec<(Stuff, Piece)> = (0..arches)
    .map(|index| {
      let angle = PI * (index as f32 + 0.5) / arches as f32;
      let at = Vec3::new(0.0, low + radius * angle.sin() * rise, -radius * angle.cos());
      (
        Stuff::Masonry,
        Piece::new(block(wide, 1.0, radius * PI / arches as f32 + 0.4), stone * 0.92)
          .planar(1.6)
          .pitched((angle - PI / 2.0) * rise)
          .at(at)
      )
    })
    .collect();
  let deck_piece = (
    Stuff::Masonry,
    Piece::new(block(wide, 1.1, span), stone).planar(1.8).at_xyz(0.0, deck - 0.45, 0.0)
  );
  let parapets = [-1.0, 1.0].map(|side| {
    (
      Stuff::Masonry,
      Piece::new(block(0.5, 0.9, span), stone * 0.9).planar(1.4).at_xyz(
        side * (half_wide + 0.25),
        deck + 0.45,
        0.0
      )
    )
  });
  let abutments = [-1.0, 1.0].map(|side| {
    let tall = deck - low + 3.0;
    (
      Stuff::Masonry,
      Piece::new(block(wide + 1.0, tall, 3.0), stone * 0.85).planar(1.6).at_xyz(
        0.0,
        deck - 0.4 - tall / 2.0,
        side * (reach - 1.5)
      )
    )
  });
  (ring.into_iter().chain([deck_piece]).chain(parapets).chain(abutments).collect(), vec![
    (Vec3::new(0.0, deck - 0.45, 0.0), Collider::cuboid(wide, 1.1, span)),
    (Vec3::new(half_wide + 0.25, deck + 0.45, 0.0), Collider::cuboid(0.5, 0.9, span)),
    (Vec3::new(-half_wide - 0.25, deck + 0.45, 0.0), Collider::cuboid(0.5, 0.9, span)),
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
  for river in 0..rivers() {
    commands.spawn((
      Name::new("River"),
      Flow,
      Mesh3d(meshes.add(water_mesh(river))),
      MeshMaterial3d(water.clone()),
      Transform::IDENTITY
    ));
  }
  for built in BRIDGES.iter() {
    let &Bridge { at, along, .. } = built;
    let origin = at.extend(0.0).xzy();
    let (parts, solids) = bridge(built);
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
    for (stuff, pieces) in parts
      .into_iter()
      .fold(Vec::<(Stuff, Vec<Piece>)>::new(), |mut groups, (stuff, piece)| {
        match groups.iter_mut().find(|(each, _)| *each == stuff) {
          Some((_, list)) => list.push(piece),
          None => groups.push((stuff, vec![piece]))
        }
        groups
      })
      .into_iter()
    {
      let mesh =
        crate::model::merge(pieces).with_generated_tangents().expect("bridge uvs");
      commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(stuffs.of(stuff)),
        ChildOf(root)
      ));
    }
  }
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
  for Lake { center, radius, level } in lakes() {
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
  }
}

#[derive(Component)]
struct Ripples;

fn ripple(
  time: Res<Time>,
  lakes: Query<&MeshMaterial3d<StandardMaterial>, With<Ripples>>,
  mut materials: ResMut<Assets<StandardMaterial>>
) {
  for lake in lakes.iter() {
    if let Some(mut water) = materials.get_mut(&lake.0) {
      water.uv_transform.translation = Vec2::new(0.013, 0.007) * time.elapsed_secs();
    }
  }
}

fn flow(
  time: Res<Time>,
  rivers: Query<&MeshMaterial3d<StandardMaterial>, With<Flow>>,
  mut materials: ResMut<Assets<StandardMaterial>>
) {
  for river in rivers.iter() {
    if let Some(mut water) = materials.get_mut(&river.0) {
      water.uv_transform.translation = Vec2::new(0.0, -0.09 * time.elapsed_secs());
    }
  }
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
    for bridge in BRIDGES.iter() {
      println!(
        "bridge at {} along {} reach {:.1} deck {:.1} low {:.1}",
        bridge.at, bridge.along, bridge.reach, bridge.deck, bridge.low
      )
    }
    for (river, course) in COURSES.iter().enumerate() {
      let mouth = match course.mouth {
        End::Lake(lake) => format!("lake {lake}"),
        End::River(other) => format!("river {other}")
      };
      println!(
        "course {river}: {} -> {} into {mouth}, {} points",
        course.path[0],
        course.path[course.path.len() - 1],
        course.path.len()
      )
    }
    for (river, levels) in LEVELS.iter().enumerate() {
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
    }
    for at in [Vec2::new(1130.0, 2280.0), Vec2::new(1100.0, 2350.0)] {
      println!(
        "{at}: height {} natural {} water {:?} reach {:?}",
        terrain::height_at(at),
        terrain::natural_height(at),
        water_level(at),
        reach_at(at)
      )
    }
  }
}
