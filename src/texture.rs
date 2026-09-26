use {crate::terrain::smooth,
     bevy::{asset::RenderAssetUsages,
            image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
            prelude::*,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat}}};

pub fn tile_noise(u: f32, v: f32, period: i32, seed: u32) -> f32 {
  stretched_noise(u, v, period, period, seed)
}

pub fn stretched_noise(u: f32, v: f32, across: i32, along: i32, seed: u32) -> f32 {
  let hash =
    |x: i32, y: i32| crate::noise::hash(x.rem_euclid(across), y.rem_euclid(along), seed);
  let (x, y) = (u * across as f32, v * along as f32);
  let (column, row) = (x.floor() as i32, y.floor() as i32);
  let ease = |t: f32| t * t * (3.0 - 2.0 * t);
  let (fx, fy) = (ease(x - column as f32), ease(y - row as f32));
  let corner = |dx: i32, dy: i32| hash(column + dx, row + dy);
  corner(0, 0).lerp(corner(1, 0), fx).lerp(corner(0, 1).lerp(corner(1, 1), fx), fy)
}

pub fn tile_fbm(u: f32, v: f32, base: i32, octaves: u32, seed: u32) -> f32 {
  (0..octaves)
    .fold((0.0, 0.0, 1.0), |(sum, total, amplitude), octave| {
      (
        sum + tile_noise(u, v, base << octave, seed + octave) * amplitude,
        total + amplitude,
        amplitude * 0.55
      )
    })
    .0
    / (0..octaves).map(|octave| 0.55f32.powi(octave as i32)).sum::<f32>()
}

fn image(
  size: u32,
  format: TextureFormat,
  pixels: impl Fn(f32, f32) -> [u8; 4]
) -> Image {
  let data = (0..size * size)
    .flat_map(|index| {
      pixels((index % size) as f32 / size as f32, (index / size) as f32 / size as f32)
    })
    .collect();
  let mut image = Image::new(
    Extent3d { width: size, height: size, depth_or_array_layers: 1 },
    TextureDimension::D2,
    data,
    format,
    RenderAssetUsages::RENDER_WORLD
  );
  image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
    address_mode_u: ImageAddressMode::Repeat,
    address_mode_v: ImageAddressMode::Repeat,
    anisotropy_clamp: 8,
    ..ImageSamplerDescriptor::linear()
  });
  image
}

pub fn shade(size: u32, pixels: impl Fn(f32, f32) -> f32) -> Image {
  image(size, TextureFormat::Rgba8UnormSrgb, |u, v| {
    let level = (pixels(u, v).clamp(0.0, 1.0) * 255.0) as u8;
    [level, level, level, 255]
  })
}

pub fn bumps(size: u32, strength: f32, height: impl Fn(f32, f32) -> f32) -> Image {
  let step = 1.0 / size as f32;
  image(size, TextureFormat::Rgba8Unorm, |u, v| {
    let slope = Vec2::new(
      height(u + step, v) - height(u - step, v),
      height(u, v + step) - height(u, v - step)
    ) / (2.0 * step);
    let normal = (-slope * strength).extend(1.0).normalize();
    let level = |axis: f32| ((0.5 + 0.5 * axis) * 255.0) as u8;
    [level(normal.x), level(normal.y), level(normal.z), 255]
  })
}

pub fn ground_grain(u: f32, v: f32) -> f32 {
  let coarse = tile_fbm(u, v, 8, 4, 3);
  let fine = tile_fbm(u, v, 64, 3, 9);
  let pebbles = (tile_noise(u, v, 96, 21) - 0.62).max(0.0) * 2.2;
  0.55 * coarse + 0.35 * fine + pebbles * 0.4
}

pub fn ground() -> Image { shade(512, |u, v| 0.8 + 0.3 * (ground_grain(u, v) - 0.5)) }

pub fn ground_bumps() -> Image { bumps(512, 0.005, ground_grain) }

pub fn rock_grain(u: f32, v: f32) -> f32 {
  let strata =
    (v * 3.0 * std::f32::consts::TAU + 2.5 * tile_fbm(u, v, 4, 3, 41)).sin() * 0.5 + 0.5;
  0.5 * tile_fbm(u, v, 6, 5, 40) + 0.3 * strata + 0.2 * tile_noise(u, v, 128, 44)
}

pub fn rock() -> Image { shade(512, |u, v| 0.64 + 0.5 * (rock_grain(u, v) - 0.5)) }

pub fn rock_bumps() -> Image { bumps(512, 0.02, rock_grain) }

fn stretched_fbm(
  u: f32,
  v: f32,
  across: i32,
  along: i32,
  octaves: u32,
  seed: u32
) -> f32 {
  (0..octaves)
    .map(|octave| {
      stretched_noise(u, v, across << octave, along << octave, seed + octave)
        * 0.5f32.powi(octave as i32)
    })
    .sum::<f32>()
    / (0..octaves).map(|octave| 0.5f32.powi(octave as i32)).sum::<f32>()
}

fn cliff_grain(u: f32, v: f32) -> (f32, f32) {
  let streak = stretched_fbm(u, v, 20, 2, 4, 61);
  let crack = |u: f32, seed: u32| {
    let wander = 0.06 * (stretched_fbm(u, v, 4, 4, 2, seed + 1) - 0.5);
    let ridge = 1.0 - (2.0 * stretched_fbm(u + wander, v, 5, 1, 3, seed) - 1.0).abs();
    ridge.powf(16.0)
  };
  let cracks = (crack(u, 62) + 0.6 * crack(u + v, 66)).min(1.0);
  let patches = tile_fbm(u, v, 3, 3, 63);
  let fine = tile_fbm(u, v, 48, 3, 65);
  (
    0.45 * streak + 0.3 * patches + 0.25 * fine - 0.35 * cracks,
    0.82 + 0.16 * (patches - 0.5) - 0.28 * smooth(0.5, 0.75, streak) + 0.1 * (fine - 0.5)
      - 0.4 * cracks
  )
}

pub fn cliff() -> Image { shade(512, |u, v| cliff_grain(u, v).1) }

pub fn cliff_bumps() -> Image { bumps(512, 0.02, |u, v| cliff_grain(u, v).0) }

pub fn cracks() -> Image {
  shade(256, |u, v| {
    let ridge = 1.0 - (2.0 * tile_fbm(u, v, 5, 4, 91) - 1.0).abs();
    let finer = 1.0 - (2.0 * tile_fbm(u, v, 11, 3, 92) - 1.0).abs();
    0.06 + 0.94 * (ridge.powf(9.0) + 0.6 * finer.powf(14.0)).min(1.0)
  })
}

pub fn scale_grain(u: f32, v: f32) -> (f32, f32) {
  const PERIOD: i32 = 16;
  const REACH: f32 = 0.64;
  let (x, y) = (u * PERIOD as f32, v * PERIOD as f32);
  let row = (y * 2.0).floor() as i32;
  (row - 2..=row + 1)
    .flat_map(|rank| {
      let shift = (rank.rem_euclid(2) as f32) * 0.5;
      let column = (x - shift).round() as i32;
      (column - 1..=column + 1).map(move |column| (rank, column, shift))
    })
    .find_map(|(rank, column, shift)| {
      let reach =
        Vec2::new(x - column as f32 - shift, y - rank as f32 * 0.5).length() / REACH;
      (reach < 1.0).then(|| {
        let tint =
          crate::noise::hash(column.rem_euclid(PERIOD), rank.rem_euclid(PERIOD * 2), 133);
        ((1.0 - reach.powi(3)).sqrt(), tint)
      })
    })
    .unwrap_or((0.0, 0.5))
}

pub fn scales() -> Image {
  shade(512, |u, v| {
    let (swell, tint) = scale_grain(u, v);
    0.48 + 0.36 * swell + 0.16 * (tint - 0.5) + 0.12 * (tile_fbm(u, v, 8, 3, 134) - 0.5)
  })
}

pub fn scale_bumps() -> Image { bumps(512, 0.0035, |u, v| scale_grain(u, v).0) }

pub fn fur() -> Image {
  shade(256, |u, v| {
    let strands = stretched_noise(u, v, 16, 256, 71);
    let clumps = tile_fbm(u, v, 8, 3, 72);
    0.55 + 0.35 * strands + 0.25 * (clumps - 0.5)
  })
}

pub fn bark() -> Image {
  shade(256, |u, v| {
    let furrows = stretched_noise(u, v, 24, 3, 81);
    let flakes = tile_fbm(u, v, 16, 3, 82);
    0.35 + 0.55 * furrows.powf(0.6) * (0.7 + 0.5 * flakes)
  })
}

pub fn leather() -> Image {
  shade(256, |u, v| {
    0.78 + 0.22 * tile_fbm(u, v, 32, 3, 91) - 0.1 * tile_noise(u, v, 8, 92)
  })
}

pub fn metal() -> Image {
  shade(256, |u, v| {
    let scratches = stretched_noise(u, v, 4, 384, 101);
    let blotch = tile_fbm(u, v, 6, 4, 102);
    0.72 + 0.2 * blotch + 0.12 * (scratches - 0.5)
  })
}

pub fn wood() -> Image {
  shade(256, |u, v| {
    let rings =
      ((u * 11.0 + 1.6 * tile_fbm(u, v, 4, 3, 111)) * std::f32::consts::TAU).sin();
    0.62 + 0.18 * rings + 0.2 * stretched_noise(u, v, 6, 64, 112)
  })
}

pub fn needles() -> Image {
  shade(256, |u, v| {
    let tufts = tile_noise(u, v, 48, 121);
    let clusters = tile_fbm(u, v, 6, 3, 122);
    0.45 + 0.4 * tufts * tufts + 0.35 * (clusters - 0.3)
  })
}

pub fn voronoi(u: f32, v: f32, period: i32, seed: u32) -> (f32, f32, IVec2) {
  let grid = Vec2::new(u, v) * period as f32;
  let base = grid.floor().as_ivec2();
  let (first, second, cell) = (-1..=1)
    .flat_map(|dy| (-1..=1).map(move |dx| base + IVec2::new(dx, dy)))
    .map(|cell| {
      let wrapped = cell.rem_euclid(IVec2::splat(period));
      let jitter = Vec2::new(
        crate::noise::hash(wrapped.x, wrapped.y, seed),
        crate::noise::hash(wrapped.x, wrapped.y, seed + 1)
      );
      (grid.distance(cell.as_vec2() + 0.15 + 0.7 * jitter), wrapped)
    })
    .fold(
      (f32::MAX, f32::MAX, IVec2::ZERO),
      |(first, second, cell), (distance, each)| match (
        distance < first,
        distance < second
      ) {
        (true, _) => (distance, first, each),
        (false, true) => (first, distance, cell),
        _ => (first, second, cell)
      }
    );
  (0.5 * (second - first), first, cell)
}

const COBBLES: i32 = 10;

fn cobble_grain(u: f32, v: f32) -> (f32, IVec2) {
  let wobble = 0.05 * (tile_fbm(u, v, 20, 3, 152) - 0.5);
  let (edge, first, cell) = voronoi(u + wobble, v - wobble, COBBLES, 151);
  let girth = 0.42 + 0.16 * crate::noise::hash(cell.x, cell.y, 154);
  let (border, round) = (edge - 0.035, girth - first);
  let blend = 0.09;
  let body = (border + round - ((border - round).powi(2) + blend * blend).sqrt()) / 2.0;
  let dome = (body / 0.16).clamp(0.0, 1.0);
  ((1.0 - (1.0 - dome).powi(2)).sqrt() + 0.12 * (tile_fbm(u, v, 40, 2, 156) - 0.5), cell)
}

pub fn cobbles() -> Image {
  image(512, TextureFormat::Rgba8UnormSrgb, |u, v| {
    let (dome, cell) = cobble_grain(u, v);
    let hash = |seed: u32| crate::noise::hash(cell.x, cell.y, seed);
    let speck = tile_fbm(u, v, 48, 3, 157);
    let lichen = tile_fbm(u, v, 12, 3, 158);
    let stone = (0.3 + 0.16 * hash(153) + 0.12 * (speck - 0.5)) * (0.72 + 0.28 * dome);
    let warm = hash(155);
    let stone =
      Vec3::new(stone * (1.0 + 0.12 * warm), stone, stone * (0.96 - 0.1 * warm));
    let moss = smooth(0.55, 0.75, lichen) * smooth(0.6, 0.1, dome) * hash(159);
    let tone = stone.lerp(Vec3::new(0.24, 0.27, 0.15), moss * 0.7);
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0) as u8;
    let solid = (dome > 0.02) as u8 as f32 * (0.52 + 0.48 * hash(160));
    [byte(tone.x), byte(tone.y), byte(tone.z), byte(solid)]
  })
}

pub fn cobble_bumps() -> Image { bumps(512, 0.01, |u, v| cobble_grain(u, v).0) }

pub fn thatch() -> Image {
  shade(512, |u, v| {
    let straws = stretched_noise(u, v, 160, 6, 161);
    let bundles = stretched_noise(u, v, 24, 3, 162);
    let courses =
      ((v * 10.0 + 0.3 * tile_fbm(u, v, 4, 2, 163)) * std::f32::consts::TAU).sin();
    0.5 + 0.35 * straws.powf(0.7) + 0.12 * bundles - 0.08 * courses.max(0.0)
  })
}

fn shingle_grain(u: f32, v: f32) -> (f32, f32) {
  const ROWS: f32 = 10.0;
  const COLUMNS: f32 = 6.0;
  let row = (v * ROWS).floor();
  let shift = (row.rem_euclid(2.0)) * 0.5;
  let x = u * COLUMNS + shift;
  let column = x.floor();
  let (across, down) = (x.fract(), (v * ROWS).fract());
  let gap = ((across.min(1.0 - across)) / 0.06).clamp(0.0, 1.0);
  let tint = crate::noise::hash(column as i32 % COLUMNS as i32, row as i32, 171);
  (gap * (0.55 + 0.45 * down), tint)
}

pub fn shingles() -> Image {
  shade(512, |u, v| {
    let (lift, tint) = shingle_grain(u, v);
    let grain = stretched_noise(u, v, 96, 12, 173);
    0.35 + 0.35 * lift + 0.15 * tint + 0.15 * grain
  })
}

pub fn shingle_bumps() -> Image { bumps(512, 0.004, |u, v| shingle_grain(u, v).0) }

fn masonry_grain(u: f32, v: f32) -> (f32, f32) {
  let (edge, _, cell) = voronoi(u * 1.6, v, 5, 181);
  let dome = ((edge - 0.035) / 0.18).clamp(0.0, 1.0);
  (
    (1.0 - (1.0 - dome).powi(2)).sqrt() + 0.15 * tile_fbm(u, v, 16, 3, 183),
    crate::noise::hash(cell.x, cell.y, 185)
  )
}

pub fn masonry() -> Image {
  shade(512, |u, v| {
    let (dome, tint) = masonry_grain(u, v);
    let mortar = (dome < 0.12) as u8 as f32;
    (0.55 + 0.25 * tint + 0.2 * (dome - 0.5)) * (1.0 - 0.4 * mortar)
  })
}

pub fn masonry_bumps() -> Image { bumps(512, 0.012, |u, v| masonry_grain(u, v).0) }

pub fn planks() -> Image {
  shade(256, |u, v| {
    let board = (u * 8.0).floor();
    let seam = ((u * 8.0).fract() - 0.5).abs() > 0.46;
    let grain = stretched_noise(u, v, 128, 6, 191 + board as u32);
    let tone = crate::noise::hash(board as i32, 0, 193);
    (0.5 + 0.2 * tone + 0.25 * grain) * if seam { 0.45 } else { 1.0 }
  })
}
