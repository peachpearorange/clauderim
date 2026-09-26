use {crate::{noise,
             place::{self, Marker, Place},
             player::{MainCamera, Player},
             river, settlement,
             signal::Pending,
             texture,
             work::{self, Job, Work}},
     avian3d::prelude::*,
     bevy::{asset::RenderAssetUsages,
            color::Mix,
            mesh::{Indices, PrimitiveTopology},
            platform::collections::{HashMap, HashSet},
            prelude::*},
     std::sync::{Arc, RwLock}};

pub const WORLD: f32 = 3072.0;
pub const BOUND: f32 = WORLD - 60.0;
pub const SPACING: f32 = 2.0;
const CHUNK: i32 = 64;
const CHUNK_SIZE: f32 = CHUNK as f32 * SPACING;
const FAR_HALF: i32 = 12288;
const ROOT: i32 = 4096;
const LEAF_CELLS: usize = 64;
const FINEST: i32 = 128;
const SPLIT: f32 = 1.25;
const ANCHOR_STEP: f32 = 64.0;
const SOLID_REACH: f32 = 340.0;
const SOLID_NOW: f32 = 40.0;
const GRAIN_TILE: f32 = 7.0;
const THROAT: Vec2 = Vec2::new(900.0, -2600.0);
const THROAT_REACH: f32 = 1700.0;
const THROAT_RISE: f32 = 1000.0;
const THROAT_DETAIL: i32 = 512;
const LEDGE: f32 = 38.0;

pub const fn srgb(red: f32, green: f32, blue: f32) -> LinearRgba {
  const fn decode(value: f32) -> f32 { value * value * (0.8 + 0.2 * value) }
  LinearRgba::rgb(decode(red), decode(green), decode(blue))
}

const MEADOW: LinearRgba = srgb(0.33, 0.36, 0.23);
const TUNDRA: LinearRgba = srgb(0.47, 0.42, 0.30);
const FOREST_FLOOR: LinearRgba = srgb(0.25, 0.24, 0.18);
const DIRT: LinearRgba = srgb(0.40, 0.33, 0.24);
const TRODDEN: LinearRgba = srgb(0.36, 0.31, 0.25);
const COBBLE: LinearRgba = srgb(0.44, 0.43, 0.41);
const PEBBLES: LinearRgba = srgb(0.46, 0.44, 0.40);
const ROCK: LinearRgba = srgb(0.43, 0.43, 0.43);
const DARK_ROCK: LinearRgba = srgb(0.30, 0.30, 0.31);
const PALE_ROCK: LinearRgba = srgb(0.55, 0.54, 0.51);
const RUST_ROCK: LinearRgba = srgb(0.42, 0.37, 0.32);
const CRAG: LinearRgba = srgb(0.29, 0.29, 0.31);
const SNOW: LinearRgba = srgb(0.93, 0.95, 1.0);
const SEABED: LinearRgba = srgb(0.24, 0.24, 0.20);

pub fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
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

fn settled(place: Place) -> bool {
  matches!(place.marker(), Marker::Town | Marker::City | Marker::Farm | Marker::Fort)
}

fn highland(at: Vec2, bent: Vec2, far: f32, pass: f32) -> f32 {
  let reach = at.length();
  let ring = smooth(0.45, 1.15, (bent / Vec2::new(780.0, 690.0)).length())
    * smooth(1700.0, 1100.0, reach);
  let ranges =
    smooth(-0.02, 0.32, noise::fbm(bent / 1500.0, 3, 91)) * smooth(900.0, 1500.0, reach);
  let open =
    place::named().filter(|&place| settled(place)).fold(1.0_f32, |open, place| {
      open.min(smooth(place.flat() * 2.0, place.flat() * 6.0, at.distance(place.spot())))
    });
  let massif = smooth(2700.0, 1700.0, at.distance(THROAT));
  (ring.max(ranges).max(massif) * pass.min(open)).max(far)
}

pub fn land(at: Vec2, pass: f32) -> f32 {
  let warp =
    Vec2::new(noise::fbm(at / 520.0, 3, 11), noise::fbm(at / 520.0 + 9.3, 3, 12)) * 110.0;
  let bent = at + warp;
  let far = smooth(WORLD - 250.0, WORLD + 450.0, at.abs().max_element());
  let hills = noise::fbm(bent / 300.0, 5, 3) * 32.0
    + noise::fbm(bent / 70.0, 4, 5) * 3.5
    + (noise::fbm(bent / 1100.0, 3, 93) * 0.5 + 0.5)
      * 70.0
      * smooth(600.0, 1300.0, at.length());
  let crags = noise::crags(bent / 500.0, 9, 7, 1.0 - 0.5 * far);
  let range = smooth(900.0, 3200.0, at.length());
  let throat_calm =
    1.0 - 0.8 * smooth(1.1 * THROAT_REACH, 0.4 * THROAT_REACH, at.distance(THROAT));
  let border = (far > 0.0).then(|| far * throat_calm * spires(at)).unwrap_or(0.0);
  let massif =
    highland(at, bent, far, pass) * (40.0 + crags * (320.0 + 110.0 * range)) + border;
  24.0 + hills + massif.lerp(ledged(massif, bent), 0.25 * far) + throat_height(at, crags)
}

pub fn wild_height(at: Vec2) -> f32 {
  land(at, smooth(55.0, 320.0, place::route_distance(at)))
}

pub fn natural_height(at: Vec2) -> f32 {
  river::BASINS
    .iter()
    .enumerate()
    .filter(|(_, basin)| at.distance(basin.center) < basin.radius * 2.0)
    .fold(wild_height(at), |height, (index, &river::Basin { center, radius, .. })| {
      let shore = radius * (1.0 + 0.25 * noise::fbm(at / 90.0, 3, 21 + index as u32));
      let bowl = smooth(shore * 1.6, shore * 0.55, at.distance(center));
      height.lerp(river::LAKE_LEVELS[index] - 9.0, bowl)
    })
}

fn graded_height(at: Vec2) -> f32 {
  let road = place::nearest_road(at);
  let width = road.paving.half_width();
  let bed = smooth(width + 7.0, width + 1.0, road.distance);
  let natural = natural_height(at);
  (bed > 0.0).then(|| natural.lerp(natural_height(road.point), bed)).unwrap_or(natural)
}

fn place_level(place: Place) -> f32 { natural_height(place.spot()) + place.rise() }

pub fn height_at(at: Vec2) -> f32 { river::ramp(at, unbridged_height(at)) }

pub fn unbridged_height(at: Vec2) -> f32 {
  let near = |place: &Place| at.distance(place.spot()) < place.flat() * 1.9;
  let settled = place::around(at).iter().copied().filter(near).fold(
    graded_height(at),
    |height, place| {
      let reach = at.distance(place.spot());
      let flatten = smooth(place.flat() * 1.9, place.flat(), reach);
      let pit = smooth(place.flat() * 0.95, place.flat() * 0.6, reach) * place.sunk();
      height.lerp(place_level(place), flatten) - pit
    }
  );
  river::carve(at, settled)
}

pub fn forest(at: Vec2) -> f32 {
  let clearing = place::around(at)
    .iter()
    .filter(|place| at.distance(place.spot()) < place.flat() * 2.2)
    .map(|place| {
      smooth(place.flat() * 2.2, place.flat() * 1.2, at.distance(place.spot()))
    })
    .fold(smooth(10.0, 4.0, place::road_distance(at)), f32::max)
    .max(smooth(16.0, 8.0, river::course_distance(at)))
    .max(settlement::tilled(at));
  let lowland_woods = 0.12 * smooth(1200.0, 700.0, at.length());
  (smooth(-0.1 + lowland_woods, 0.35, noise::fbm(at / 160.0, 4, 31)) - clearing).max(0.0)
}

fn paint(at: Vec2, height: f32, normal: Vec3, hollow: f32) -> LinearRgba {
  let patch = smooth(-0.3, 0.4, noise::fbm(at / 120.0, 4, 41));
  let grass = MEADOW.mix(&TUNDRA, patch).mix(&FOREST_FLOOR, forest(at) * 0.8);
  let road = place::nearest_road(at);
  let edge = road.edge() + noise::fbm(at / 6.0, 2, 43) * 1.2;
  let (surface, paved) = match road.paving {
    place::Paving::Dirt => (DIRT, smooth(0.8, -0.8, edge)),
    place::Paving::Stone => (
      COBBLE.mix(&TRODDEN, 0.35 + 0.5 * crate::paving::decay(at)),
      smooth(0.6, -0.6, edge)
    )
  };
  let verge = smooth(2.5, 0.0, edge) * 0.5;
  let (worn, soil) = (settlement::worn(at), settlement::tilled(at));
  let (shore, drowned) = river::lake_near(at, 2.4).map_or(
    (0.0, 0.0),
    |river::Lake { center, radius, level }| {
      let lakeside = smooth(radius * 2.4, radius * 2.0, at.distance(center));
      (
        smooth(level + 2.2, level + 0.6, height) * lakeside,
        smooth(level - 0.5, level - 4.0, height) * lakeside
      )
    }
  );
  let snow_line = 150.0 + 40.0 * noise::fbm(at / 200.0, 3, 45);
  let alpine = smooth(snow_line + 150.0, snow_line + 900.0, height);
  let gully = hollow.clamp(-1.0, 1.0);
  let drift = normal.y
    + 0.1 * noise::fbm(at / 40.0, 3, 53)
    + 0.14 * noise::fbm(at / 190.0, 2, 59)
    + 0.35 * gully;
  let snow_hold = 0.8 - 0.45 * alpine;
  let snow = smooth(snow_line, snow_line + 40.0, height)
    * smooth(snow_hold - 0.05, snow_hold + 0.05, drift)
    * (1.0 - paved * 0.45);
  let cliff = smooth(0.82, 0.64, normal.y + 0.06 * noise::fbm(at / 9.0, 2, 47));
  let face = smooth(0.7, 0.35, normal.y + 0.08 * noise::fbm(at / 15.0, 2, 55));
  let strata = (height / 9.0 + 3.0 * noise::fbm(at / 260.0, 3, 51)).sin();
  let stone = ROCK
    .mix(&DARK_ROCK, smooth(-0.2, 0.3, noise::fbm(at / 40.0, 3, 49)))
    .mix(&PALE_ROCK, smooth(0.3, 0.9, strata) * 0.6)
    .mix(&RUST_ROCK, smooth(0.4, 0.9, noise::fbm(at / 90.0, 3, 57)) * 0.3)
    .mix(&CRAG, face * (0.5 + 0.5 * smooth(0.0, -0.4, gully)));
  grass
    .mix(&TRODDEN, worn.max(verge))
    .mix(&settlement::SOIL, soil)
    .mix(&surface, paved * 0.9)
    .mix(&PEBBLES, shore.max(river::bank(at) * 0.85))
    .mix(&SEABED, drowned)
    .mix(&stone, cliff.max(alpine) * (1.0 - paved * 0.8))
    .mix(&SNOW, snow)
}

type Chunks = HashMap<IVec2, Arc<[f32]>>;

#[derive(Resource, Clone, Default)]
pub struct Ground(Arc<RwLock<Chunks>>);

fn chunk_of(at: Vec2) -> IVec2 { (at / CHUNK_SIZE).floor().as_ivec2() }

impl Ground {
  fn sample(chunks: &Chunks, point: IVec2) -> f32 {
    let chunk = point.div_euclid(IVec2::splat(CHUNK));
    let local = point - chunk * CHUNK;
    chunks.get(&chunk).map_or_else(
      || height_at(point.as_vec2() * SPACING),
      |heights| heights[(local.y * (CHUNK + 1) + local.x) as usize]
    )
  }

  pub fn height(&self, at: Vec2) -> f32 {
    let grid = at / SPACING;
    let base = grid.floor();
    let (fraction, cell) = (grid - base, base.as_ivec2());
    let chunks = self.0.read().expect("ground chunks");
    let sample = |offset: IVec2| Self::sample(&chunks, cell + offset);
    let near = sample(IVec2::ZERO).lerp(sample(IVec2::X), fraction.x);
    let far = sample(IVec2::Y).lerp(sample(IVec2::ONE), fraction.x);
    near.lerp(far, fraction.y)
  }

  pub fn normal(&self, at: Vec2) -> Vec3 {
    let slope = |offset: Vec2| self.height(at + offset) - self.height(at - offset);
    Vec3::new(-slope(Vec2::X * SPACING), 2.0 * SPACING, -slope(Vec2::Y * SPACING))
      .normalize()
  }

  pub fn surface(&self, at: Vec2) -> Vec3 { at.extend(self.height(at)).xzy() }

  pub fn filling(&self, chunk: IVec2) -> Work<Arc<[f32]>> {
    let known = self.0.read().expect("ground chunks").get(&chunk).cloned();
    let ground = self.clone();
    let first = chunk * CHUNK;
    known.map_or_else(
      || {
        work::rows(CHUNK as usize + 1, move |row| {
          (0..=CHUNK)
            .map(|column| {
              height_at((first + IVec2::new(column, row as i32)).as_vec2() * SPACING)
            })
            .collect::<Vec<f32>>()
        })
        .map(move |rows| {
          let made: Arc<[f32]> = rows.into_iter().flatten().collect();
          ground.0.write().expect("ground chunks").insert(chunk, made.clone());
          made
        })
      },
      |known| work::once(move || known)
    )
  }

  pub fn filling_at(&self, at: Vec2) -> Work<Arc<[f32]>> { self.filling(chunk_of(at)) }

  pub fn chunk(&self, chunk: IVec2) -> Arc<[f32]> { work::run(self.filling(chunk)) }
}

pub fn in_parallel<Item: Sync, Made: Send>(
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

fn surface(corners: usize, rows: Vec<Vec<(Vec3, Vec3, [f32; 4])>>) -> Mesh {
  let (positions, (normals, colors)): (Vec<Vec3>, (Vec<Vec3>, Vec<[f32; 4]>)) = rows
    .into_iter()
    .flatten()
    .map(|(position, normal, color)| (position, (normal, color)))
    .unzip();
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

fn hollowness(around: f32, height: f32, reach: f32) -> f32 {
  (around / 4.0 - height) / reach.max(6.0) * 4.0
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Leaf {
  corner: IVec2,
  size: i32,
  seams: [i32; 4]
}

fn splits(corner: IVec2, size: i32, anchor: Vec2) -> bool {
  let (low, high) = (corner.as_vec2(), (corner + size).as_vec2());
  let gap = |to: Vec2| (low - to).max(to - high).max(Vec2::ZERO).length();
  size > FINEST
    && (gap(anchor) < size as f32 * SPLIT
      || (size > THROAT_DETAIL && gap(THROAT) < 0.8 * THROAT_REACH))
}

fn quarters(corner: IVec2, size: i32) -> [IVec2; 4] {
  let half = size / 2;
  [IVec2::ZERO, IVec2::X, IVec2::Y, IVec2::ONE].map(|step| corner + step * half)
}

fn tiles(corner: IVec2, size: i32, anchor: Vec2) -> Vec<(IVec2, i32)> {
  splits(corner, size, anchor)
    .then(|| {
      quarters(corner, size)
        .into_iter()
        .flat_map(|quarter| tiles(quarter, size / 2, anchor))
        .collect()
    })
    .unwrap_or_else(|| vec![(corner, size)])
}

fn tile_size_at(point: Vec2, anchor: Vec2) -> Option<i32> {
  let root = (point / ROOT as f32).floor().as_ivec2() * ROOT;
  (point.abs().max_element() < FAR_HALF as f32).then(|| {
    std::iter::successors(Some((root, ROOT)), |&(corner, size)| {
      splits(corner, size, anchor).then(|| {
        let half = size / 2;
        let step = ((point - corner.as_vec2()) / half as f32)
          .floor()
          .as_ivec2()
          .clamp(IVec2::ZERO, IVec2::ONE);
        (corner + step * half, half)
      })
    })
    .last()
    .map_or(ROOT, |(_, size)| size)
  })
}

fn leaves(anchor: Vec2) -> Vec<Leaf> {
  let roots = FAR_HALF / ROOT;
  (-roots..roots)
    .flat_map(|z| (-roots..roots).map(move |x| IVec2::new(x, z) * ROOT))
    .flat_map(|root| tiles(root, ROOT, anchor))
    .map(|(corner, size)| {
      let middle = corner.as_vec2() + size as f32 / 2.0;
      let across = |side: Vec2| {
        tile_size_at(middle + side * (size as f32 / 2.0 + 1.0), anchor).unwrap_or(size)
      };
      Leaf {
        corner,
        size,
        seams: [Vec2::NEG_X, Vec2::X, Vec2::NEG_Y, Vec2::Y].map(across)
      }
    })
    .collect()
}

fn tiling(leaf: Leaf) -> Work<Mesh> {
  let Leaf { corner, size, seams } = leaf;
  let spacing = size as f32 / LEAF_CELLS as f32;
  let cells = LEAF_CELLS;
  let pad = ((6.0 / spacing).ceil() as usize).max(1);
  let span = cells + 1 + 2 * pad;
  let origin = corner.as_vec2();
  work::rows(span, move |row| {
    (0..span)
      .map(|column| {
        let step = Vec2::new(column as f32, row as f32) - pad as f32;
        height_at(origin + step * spacing)
      })
      .collect::<Vec<f32>>()
  })
  .then(move |rows| {
    let heights: Vec<f32> = rows.into_iter().flatten().collect();
    work::rows(cells + 3, move |row| {
      let raw = |x: usize, z: usize| heights[z * span + x];
      let height = |column: usize, row: usize| raw(column + pad, row + pad);
      let [west, east, south, north] =
        seams.map(|neighbour| (neighbour / size).max(1) as usize);
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
      (0..cells + 3)
        .map(|column| {
          let skirt = rim(column) + 1 != column || rim(row) + 1 != row;
          let spot = origin + Vec2::new(rim(column) as f32, rim(row) as f32) * spacing;
          let lift =
            seam(rim(column), rim(row)) - skirt.then_some(2.0 * spacing).unwrap_or(0.0);
          let (x, z) = (rim(column) + pad, rim(row) + pad);
          let around =
            raw(x - pad, z) + raw(x + pad, z) + raw(x, z - pad) + raw(x, z + pad);
          let normal = Vec3::new(
            raw(x - 1, z) - raw(x + 1, z),
            2.0 * spacing,
            raw(x, z - 1) - raw(x, z + 1)
          )
          .normalize();
          let hollow = hollowness(around, raw(x, z), pad as f32 * spacing);
          let position = spot.extend(lift).xzy();
          (position, normal, paint(spot, lift, normal, hollow).to_f32_array())
        })
        .collect::<Vec<_>>()
    })
  })
  .map(move |rows| surface(cells + 3, rows))
}

#[derive(Resource)]
pub struct Surfaces {
  pub rock: Handle<StandardMaterial>,
  land: Handle<StandardMaterial>
}

#[derive(Resource, Default)]
struct Lands {
  anchor: Option<IVec2>,
  wanted: Vec<Leaf>,
  shown: HashMap<Leaf, Entity>,
  ready: HashMap<Leaf, Handle<Mesh>>,
  making: Vec<(Leaf, Job<Mesh>)>
}

fn anchor_of(at: Vec2) -> IVec2 { (at / ANCHOR_STEP).round().as_ivec2() }

impl Leaf {
  fn overlaps(self, other: Leaf) -> bool {
    let (low, high) = (self.corner, self.corner + self.size);
    let (other_low, other_high) = (other.corner, other.corner + other.size);
    low.cmplt(other_high).all() && other_low.cmplt(high).all()
  }

  fn gap(self, to: Vec2) -> f32 {
    let (low, high) = (self.corner.as_vec2(), (self.corner + self.size).as_vec2());
    (low - to).max(to - high).max(Vec2::ZERO).length()
  }
}

fn swaps(stale: &[Leaf], fresh: &[Leaf]) -> Vec<(Vec<Leaf>, Vec<Leaf>)> {
  let mut claimed = (vec![false; stale.len()], vec![false; fresh.len()]);
  (0..stale.len() + fresh.len())
    .filter_map(|seed| {
      let seen = |claimed: &(Vec<bool>, Vec<bool>), node: usize| {
        (node < stale.len())
          .then(|| claimed.0[node])
          .unwrap_or_else(|| claimed.1[node - stale.len()])
      };
      (!seen(&claimed, seed)).then(|| {
        let mut group = (Vec::new(), Vec::new());
        let mut frontier = vec![seed];
        while let Some(node) = frontier.pop() {
          if !seen(&claimed, node) {
            if node < stale.len() {
              claimed.0[node] = true;
              group.0.push(stale[node]);
              frontier.extend(
                (0..fresh.len())
                  .filter(|&other| fresh[other].overlaps(stale[node]))
                  .map(|other| other + stale.len())
              )
            } else {
              let index = node - stale.len();
              claimed.1[index] = true;
              group.1.push(fresh[index]);
              frontier.extend(
                (0..stale.len()).filter(|&other| stale[other].overlaps(fresh[index]))
              )
            }
          }
        }
        group
      })
    })
    .collect()
}

fn tend_lands(
  mut commands: Commands,
  mut lands: ResMut<Lands>,
  mut meshes: ResMut<Assets<Mesh>>,
  surfaces: Res<Surfaces>,
  camera: Single<&Transform, With<MainCamera>>
) {
  let anchor = anchor_of(camera.translation.xz());
  let middle = anchor.as_vec2() * ANCHOR_STEP;
  let Lands { anchor: last, wanted, shown, ready, making } = &mut *lands;
  if *last != Some(anchor) {
    *last = Some(anchor);
    *wanted = leaves(middle);
    let wanting: HashSet<Leaf> = wanted.iter().copied().collect();
    making.retain(|(leaf, _)| wanting.contains(leaf));
    ready.retain(|leaf, _| wanting.contains(leaf));
    let fresh: Vec<Leaf> = wanted
      .iter()
      .filter(|leaf| {
        !shown.contains_key(*leaf)
          && !ready.contains_key(*leaf)
          && !making.iter().any(|(making, _)| making == *leaf)
      })
      .copied()
      .collect();
    match shown.is_empty() {
      true => {
        for (leaf, mesh) in
          in_parallel(&fresh, |&leaf| (leaf, work::run(tiling(leaf)))).into_iter()
        {
          ready.insert(leaf, meshes.add(mesh));
        }
      }
      false => {
        making.extend(fresh.into_iter().map(|leaf| (leaf, work::spawn(tiling(leaf)))))
      }
    }
    making.sort_by(|(a, _), (b, _)| a.gap(middle).total_cmp(&b.gap(middle)));
  }
  making.retain_mut(|(leaf, task)| {
    task
      .done()
      .map(|mesh| {
        ready.insert(*leaf, meshes.add(mesh));
      })
      .is_none()
  });
  if !ready.is_empty() {
    let wanting: HashSet<Leaf> = wanted.iter().copied().collect();
    let stale: Vec<Leaf> =
      shown.keys().filter(|leaf| !wanting.contains(*leaf)).copied().collect();
    let fresh: Vec<Leaf> =
      wanted.iter().filter(|leaf| !shown.contains_key(*leaf)).copied().collect();
    for (stale, fresh) in swaps(&stale, &fresh)
      .into_iter()
      .filter(|(_, fresh)| fresh.iter().all(|leaf| ready.contains_key(leaf)))
      .collect::<Vec<_>>()
      .into_iter()
    {
      for leaf in stale.iter() {
        shown.remove(leaf).map(|entity| commands.entity(entity).despawn());
      }
      for leaf in fresh {
        let mesh = ready.remove(&leaf).expect("ready leaf");
        let entity = commands
          .spawn((
            Name::new("Land"),
            Mesh3d(mesh),
            MeshMaterial3d(surfaces.land.clone()),
            Transform::IDENTITY
          ))
          .id();
        shown.insert(leaf, entity);
      }
    }
  }
}

#[derive(Resource, Default)]
pub struct Footing {
  solid: HashMap<IVec2, (Entity, bool)>,
  making: HashMap<IVec2, Job<Collider>>
}

impl Footing {
  pub fn firm(&self, at: Vec2) -> bool {
    self.solid.get(&chunk_of(at)).is_some_and(|&(_, ready)| ready)
  }
}

fn chunk_collider(heights: &[f32]) -> Collider {
  let side = CHUNK as usize + 1;
  let rows = (0..side)
    .map(|column| (0..side).map(|row| heights[row * side + column]).collect())
    .collect();
  Collider::heightfield(rows, Vec3::new(CHUNK_SIZE, 1.0, CHUNK_SIZE))
}

fn chunk_gap(chunk: IVec2, at: Vec2) -> f32 {
  let low = chunk.as_vec2() * CHUNK_SIZE;
  (low - at).max(at - low - CHUNK_SIZE).max(Vec2::ZERO).length()
}

fn tend_footing(
  mut commands: Commands,
  mut footing: ResMut<Footing>,
  ground: Res<Ground>,
  players: Query<&Transform, With<Player>>
) {
  if let Ok(player) = players.single() {
    let here = player.translation.xz();
    let Footing { solid, making } = &mut *footing;
    let (low, high) = (chunk_of(here - SOLID_REACH), chunk_of(here + SOLID_REACH));
    let wanted: Vec<IVec2> = (low.y..=high.y)
      .flat_map(|y| (low.x..=high.x).map(move |x| IVec2::new(x, y)))
      .filter(|&chunk| {
        chunk_gap(chunk, here) < SOLID_REACH
          && !solid.contains_key(&chunk)
          && !making.contains_key(&chunk)
      })
      .collect();
    let (urgent, later): (Vec<IVec2>, Vec<IVec2>) =
      wanted.into_iter().partition(|&chunk| chunk_gap(chunk, here) < SOLID_NOW);
    let mut place = |chunk: IVec2, collider: Collider| {
      let entity = commands
        .spawn((
          Name::new("Ground"),
          RigidBody::Static,
          collider,
          Friction::new(0.8),
          Transform::from_translation(
            (chunk.as_vec2() * CHUNK_SIZE + CHUNK_SIZE / 2.0).extend(0.0).xzy()
          )
        ))
        .id();
      solid.insert(chunk, (entity, false));
    };
    for (chunk, collider) in
      in_parallel(&urgent, |&chunk| (chunk, chunk_collider(&ground.chunk(chunk))))
        .into_iter()
    {
      place(chunk, collider)
    }
    for chunk in later {
      let ground = ground.clone();
      making.insert(
        chunk,
        work::spawn(ground.filling(chunk).map(|heights| chunk_collider(&heights)))
      );
    }
    let done: Vec<(IVec2, Collider)> = making
      .iter_mut()
      .filter_map(|(&chunk, task)| task.done().map(|collider| (chunk, collider)))
      .collect();
    for (chunk, collider) in done {
      making.remove(&chunk);
      place(chunk, collider)
    }
    solid.retain(|&chunk, &mut (entity, _)| {
      let keep = chunk_gap(chunk, here) < SOLID_REACH + CHUNK_SIZE;
      if !keep {
        commands.entity(entity).despawn();
      }
      keep
    });
  }
}

fn report_streaming(
  lands: Res<Lands>,
  footing: Res<Footing>,
  mut pending: ResMut<Pending>
) {
  pending.0.insert(
    "lands",
    lands.making.len() + lands.ready.len() + lands.anchor.is_none() as usize
  );
  pending.0.insert("footing", footing.making.len());
}

fn settle_footing(mut footing: ResMut<Footing>) {
  for (_, ready) in footing.solid.values_mut() {
    *ready = true
  }
}

fn prepare_terrain(
  mut commands: Commands,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut images: ResMut<Assets<Image>>
) {
  commands.insert_resource(Surfaces {
    rock: materials.add(StandardMaterial {
      base_color_texture: Some(images.add(texture::rock())),
      normal_map_texture: Some(images.add(texture::rock_bumps())),
      perceptual_roughness: 0.9,
      reflectance: 0.25,
      ..default()
    }),
    land: materials.add(StandardMaterial {
      base_color_texture: Some(images.add(texture::ground())),
      normal_map_texture: Some(images.add(texture::ground_bumps())),
      perceptual_roughness: 0.93,
      reflectance: 0.2,
      ..default()
    })
  });
  commands.insert_resource(Ground::default());
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Lands>()
    .init_resource::<Footing>()
    .add_systems(PreStartup, prepare_terrain)
    .add_systems(PreUpdate, tend_footing)
    .add_systems(FixedPostUpdate, settle_footing.after(PhysicsSystems::Last))
    .add_systems(Update, (tend_lands, report_streaming.after(tend_lands)));
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
    for (step, from, to) in [
      (SPACING, Vec2::new(-760.0, 700.0), Vec2::new(760.0, 740.0)),
      (SPACING, Vec2::new(-700.0, -760.0), Vec2::new(-740.0, 760.0)),
      (8.0, Vec2::new(-2900.0, 1300.0), Vec2::new(2900.0, 900.0)),
      (8.0, THROAT - Vec2::X * 1600.0, THROAT - Vec2::X * 200.0)
    ]
    .into_iter()
    {
      let slope = steepest(step, from, to);
      assert!(slope < 6.0, "{from} to {to}: {slope}");
    }
  }

  #[test]
  fn chunk_collider_matches_ground() {
    let ground = Ground::default();
    for at in
      [Vec2::new(-250.0, -170.0), Vec2::new(300.0, 60.0), Vec2::new(-1340.0, 2290.0)]
        .into_iter()
    {
      let chunk = chunk_of(at);
      let collider = chunk_collider(&ground.chunk(chunk));
      let center = (chunk.as_vec2() * CHUNK_SIZE + CHUNK_SIZE / 2.0).extend(0.0).xzy();
      let hit = collider
        .cast_ray(
          center,
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
    }
  }

  #[test]
  #[ignore]
  fn mesh_matches_ground() {
    let ground = Ground::default();
    let anchor = place::START;
    let near: Vec<Leaf> = leaves(anchor)
      .into_iter()
      .filter(|leaf| {
        let middle = leaf.corner.as_vec2() + leaf.size as f32 / 2.0;
        (middle - anchor).abs().max_element() < 200.0 + leaf.size as f32 / 2.0
      })
      .collect();
    for &leaf in near.iter() {
      let mesh = work::run(tiling(leaf));
      let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
      else {
        panic!()
      };
      let worst = positions
        .iter()
        .map(|&[x, y, z]| (ground.height(Vec2::new(x, z)) - y).abs())
        .filter(|gap| *gap < 1.5 * SPACING * leaf.size as f32 / FINEST as f32)
        .fold(0.0f32, f32::max);
      println!("{:?} size {} worst {worst:.3}", leaf.corner, leaf.size);
    }
  }

  #[test]
  #[ignore]
  fn map() {
    println!(
      "{} places: {:?}",
      place::all().count(),
      place::all().fold(HashMap::<String, usize>::default(), |mut counts, place| {
        *counts.entry(format!("{:?}", place.marker())).or_default() += 1;
        counts
      })
    );
    println!("{} rivers", river::rivers());
    map_image("screenshots/map.png", |at| {
      place::named()
        .filter(|&place| settled(place))
        .any(|place| at.distance(place.spot()) < place.flat())
        .then_some(LinearRgba::rgb(1.0, 0.85, 0.0))
        .or_else(|| {
          place::all()
            .any(|place| at.distance(place.spot()) < 14.0)
            .then_some(LinearRgba::rgb(1.0, 0.0, 0.0))
        })
        .or_else(|| {
          (river::course_distance(at) < 5.0).then_some(LinearRgba::rgb(0.1, 0.5, 1.0))
        })
        .or_else(|| {
          (place::nearest_road(at).edge() < 2.0)
            .then_some(LinearRgba::rgb(0.2, 0.1, 0.05))
        })
    });
  }
}

#[cfg(test)]
pub const MAP_SIDE: usize = 1024;

#[cfg(test)]
pub fn map_pixel(at: Vec2) -> Option<usize> {
  let grid = ((at + WORLD) / (2.0 * WORLD) * MAP_SIDE as f32).floor();
  (grid.min_element() >= 0.0 && grid.max_element() < MAP_SIDE as f32)
    .then(|| grid.y as usize * MAP_SIDE + grid.x as usize)
}

#[cfg(test)]
pub fn map_image(path: &str, mark: impl Fn(Vec2) -> Option<LinearRgba> + Sync) {
  let side = MAP_SIDE;
  let step = 2.0 * WORLD / side as f32;
  let spot =
    |index: usize| Vec2::new((index % side) as f32, (index / side) as f32) * step - WORLD;
  let heights: Vec<f32> =
    in_parallel(&(0..side * side).collect::<Vec<_>>(), |&index| height_at(spot(index)));
  let pixels: Vec<u8> = in_parallel(&(0..side * side).collect::<Vec<_>>(), |&index| {
    let (x, z) = (index % side, index / side);
    let height = |x: usize, z: usize| heights[z.min(side - 1) * side + x.min(side - 1)];
    let normal = Vec3::new(
      height(x.saturating_sub(1), z) - height(x + 1, z),
      2.0 * step,
      height(x, z.saturating_sub(1)) - height(x, z + 1)
    )
    .normalize();
    let light = 0.55 + 0.6 * normal.dot(Vec3::new(-0.5, 0.7, -0.4).normalize()).max(0.0);
    let at = spot(index);
    let ground = height(x, z);
    let gridline = (at + WORLD).rem_euclid(Vec2::splat(500.0)).min_element() < step;
    let land = paint(at, ground, normal, 0.0) * light;
    let wet = river::water_level(at)
      .filter(|&level| level > ground)
      .map_or(land, |_| LinearRgba::rgb(0.05, 0.14, 0.4));
    let tone = mark(at).unwrap_or(wet * (1.0 - 0.35 * f32::from(u8::from(gridline))));
    Srgba::from(tone).to_u8_array()
  })
  .concat();
  Image::new(
    bevy::render::render_resource::Extent3d {
      width: side as u32,
      height: side as u32,
      depth_or_array_layers: 1
    },
    bevy::render::render_resource::TextureDimension::D2,
    pixels,
    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::MAIN_WORLD
  )
  .try_into_dynamic()
  .expect("map image")
  .to_rgb8()
  .save(path)
  .expect("map saved");
}
