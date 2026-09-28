use {crate::terrain::smooth,
     bevy::{asset::RenderAssetUsages,
            image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
            prelude::*,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat,
                                      TextureViewDescriptor, TextureViewDimension}}};

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

fn grid<Made: Send>(size: u32, pixel: impl Fn(f32, f32) -> Made + Sync) -> Vec<Made> {
  let rows: Vec<u32> = (0..size).collect();
  crate::terrain::in_parallel(&rows, |&row| {
    (0..size)
      .map(|column| pixel(column as f32 / size as f32, row as f32 / size as f32))
      .collect::<Vec<_>>()
  })
  .into_iter()
  .flatten()
  .collect()
}

fn halved(size: u32, pixels: &[[u8; 4]]) -> Vec<[u8; 4]> {
  let half = size / 2;
  (0..half * half)
    .map(|index| {
      let (x, y) = (index % half * 2, index / half * 2);
      let at = |dx: u32, dy: u32| pixels[((y + dy) * size + x + dx) as usize];
      let corners = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
      std::array::from_fn(|channel| {
        (corners.iter().map(|pixel| pixel[channel] as u32).sum::<u32>() / 4) as u8
      })
    })
    .collect()
}

fn chain(size: u32, pixels: Vec<[u8; 4]>, mipped: bool) -> Vec<Vec<[u8; 4]>> {
  std::iter::successors(Some((size, pixels)), |(size, pixels)| {
    (mipped && *size > 1).then(|| (size / 2, halved(*size, pixels)))
  })
  .map(|(_, pixels)| pixels)
  .collect()
}

fn upload(size: u32, format: TextureFormat, layers: Vec<Vec<Vec<[u8; 4]>>>) -> Image {
  let (count, levels) = (layers.len() as u32, layers[0].len() as u32);
  let mut image = Image::new_uninit(
    Extent3d { width: size, height: size, depth_or_array_layers: count },
    TextureDimension::D2,
    format,
    RenderAssetUsages::RENDER_WORLD
  );
  image.data = Some(layers.into_iter().flatten().flatten().flatten().collect());
  image.texture_descriptor.mip_level_count = levels;
  image.texture_view_descriptor = (count > 1).then(|| TextureViewDescriptor {
    dimension: Some(TextureViewDimension::D2Array),
    ..default()
  });
  image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
    address_mode_u: ImageAddressMode::Repeat,
    address_mode_v: ImageAddressMode::Repeat,
    anisotropy_clamp: 8,
    ..ImageSamplerDescriptor::linear()
  });
  image
}

fn image(
  size: u32,
  format: TextureFormat,
  pixels: impl Fn(f32, f32) -> [u8; 4] + Sync
) -> Image {
  upload(size, format, vec![chain(size, grid(size, pixels), true)])
}

fn unmipped(
  size: u32,
  format: TextureFormat,
  pixels: impl Fn(f32, f32) -> [u8; 4] + Sync
) -> Image {
  upload(size, format, vec![chain(size, grid(size, pixels), false)])
}

fn byte(value: f32) -> u8 { (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8 }

pub fn shade(size: u32, pixels: impl Fn(f32, f32) -> f32 + Sync) -> Image {
  image(size, TextureFormat::Rgba8UnormSrgb, |u, v| {
    let level = byte(pixels(u, v));
    [level, level, level, 255]
  })
}

fn facing(size: u32, strength: f32, heights: &[f32], at: u32) -> Vec3 {
  let (x, y) = (at % size, at / size);
  let sample =
    |dx: u32, dy: u32| heights[(((y + dy) % size) * size + (x + dx) % size) as usize];
  let slope =
    Vec2::new(sample(1, 0) - sample(size - 1, 0), sample(0, 1) - sample(0, size - 1))
      * size as f32
      / 2.0;
  (-slope * strength).extend(1.0).normalize()
}

fn normal_bytes(normal: Vec3, alpha: f32) -> [u8; 4] {
  let level = |axis: f32| byte(0.5 + 0.5 * axis);
  [level(normal.x), level(normal.y), level(normal.z), byte(alpha)]
}

pub fn bumps(size: u32, strength: f32, height: impl Fn(f32, f32) -> f32 + Sync) -> Image {
  let heights = grid(size, height);
  image(size, TextureFormat::Rgba8Unorm, |u, v| {
    let at = (v * size as f32) as u32 * size + (u * size as f32) as u32;
    normal_bytes(facing(size, strength, &heights, at), 1.0)
  })
}

#[derive(Clone, Copy)]
pub struct Texel {
  pub tone: Vec3,
  pub height: f32,
  pub rough: f32
}

pub struct Textured {
  pub tone: Image,
  pub relief: Image
}

fn texel_pixels(
  size: u32,
  strength: f32,
  grain: impl Fn(f32, f32) -> Texel + Sync
) -> (Vec<[u8; 4]>, Vec<[u8; 4]>) {
  let grains = grid(size, grain);
  let heights: Vec<f32> = grains.iter().map(|grain| grain.height).collect();
  let reliefs: Vec<u32> = (0..size * size).collect();
  let relief = crate::terrain::in_parallel(&reliefs, |&at| {
    normal_bytes(facing(size, strength, &heights, at), grains[at as usize].rough)
  });
  let tone = grains
    .iter()
    .map(|&Texel { tone, height, .. }| {
      [byte(tone.x), byte(tone.y), byte(tone.z), byte(height)]
    })
    .collect();
  (tone, relief)
}

pub fn textured(
  size: u32,
  strength: f32,
  grain: impl Fn(f32, f32) -> Texel + Sync
) -> Textured {
  let (tone, relief) = texel_pixels(size, strength, grain);
  Textured {
    tone: upload(size, TextureFormat::Rgba8UnormSrgb, vec![chain(size, tone, true)]),
    relief: upload(size, TextureFormat::Rgba8Unorm, vec![chain(size, relief, true)])
  }
}

pub fn stacked(
  size: u32,
  layers: Vec<(f32, &(dyn Fn(f32, f32) -> Texel + Sync))>
) -> Textured {
  let (tones, reliefs): (Vec<_>, Vec<_>) = layers
    .into_iter()
    .map(|(strength, grain)| {
      let (tone, relief) = texel_pixels(size, strength, grain);
      (chain(size, tone, true), chain(size, relief, true))
    })
    .unzip();
  Textured {
    tone: upload(size, TextureFormat::Rgba8UnormSrgb, tones),
    relief: upload(size, TextureFormat::Rgba8Unorm, reliefs)
  }
}

pub fn rough_stone(u: f32, v: f32) -> Texel {
  let strata =
    (v * 3.0 * std::f32::consts::TAU + 2.5 * tile_fbm(u, v, 4, 3, 41)).sin() * 0.5 + 0.5;
  let blotch = tile_fbm(u, v, 6, 5, 40);
  let fine = tile_fbm(u, v, 48, 3, 44);
  let crack = (ridge(tile_fbm(u, v, 5, 4, 45)).powf(60.0)
    * smooth(0.45, 0.6, tile_fbm(u, v, 3, 2, 46)))
    * 0.6;
  let pits = smooth(0.66, 0.76, tile_noise(u, v, 96, 47));
  let (_, spot, dot) = voronoi(u, v, 36, 48);
  let lichen = smooth(0.52, 0.66, tile_fbm(u, v, 5, 3, 49))
    * smooth(0.36, 0.22, spot)
    * (crate::noise::hash(dot.x, dot.y, 50) < 0.7) as u8 as f32;
  let moss = smooth(0.6, 0.75, tile_fbm(u, v, 4, 4, 51)) * smooth(0.3, 0.7, fine);
  let pale = crate::noise::hash(dot.x, dot.y, 52) < 0.45;
  let lichen_tone =
    if pale { Vec3::new(1.0, 0.98, 0.92) } else { Vec3::new(1.0, 0.9, 0.5) };
  let base = Vec3::new(1.0, 0.98, 0.95)
    * (0.62 + 0.3 * (blotch - 0.5) + 0.1 * strata + 0.12 * (fine - 0.5) - 0.12 * pits)
    * (1.0 - 0.5 * crack.min(1.0));
  Texel {
    tone: base
      .lerp(lichen_tone * 0.8, lichen * 0.7)
      .lerp(Vec3::new(0.42, 0.5, 0.3), moss * 0.7),
    height: 0.5 * blotch + 0.2 * strata + 0.2 * fine - 0.1 * pits - 0.4 * crack.min(1.0)
      + 0.08 * (lichen + moss),
    rough: 0.88
  }
}
pub fn rock() -> Textured { textured(512, 0.015, rough_stone) }

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

fn ridge(value: f32) -> f32 { 1.0 - (2.0 * value - 1.0).abs() }

pub fn pine_bark(u: f32, v: f32) -> Texel {
  let wander = 0.04 * (stretched_fbm(u, v, 2, 3, 2, 80) - 0.5);
  let furrow = ridge(stretched_fbm(u + wander, v, 11, 2, 3, 81)).powf(3.0);
  let fine_furrow = ridge(stretched_fbm(u + wander, v, 19, 2, 2, 83)).powf(5.0);
  let breaks = ridge(stretched_fbm(u, v, 3, 9, 2, 84)).powf(14.0) * (1.0 - furrow);
  let flakes = tile_fbm(u, v, 24, 3, 82) * 0.6 + 0.4 * tile_fbm(u, v, 64, 2, 86);
  let plate = (1.0 - furrow) * (1.0 - 0.5 * fine_furrow) * (1.0 - 0.6 * breaks);
  let lichen = smooth(0.58, 0.7, tile_fbm(u, v, 4, 4, 85)) * smooth(0.3, 0.7, plate);
  let tone =
    Vec3::new(1.0, 0.9, 0.82) * (0.3 + 0.6 * plate.powf(0.7) + 0.15 * (flakes - 0.5));
  Texel {
    tone: tone.lerp(Vec3::new(0.78, 0.84, 0.72) * (0.8 + 0.3 * flakes), lichen * 0.8),
    height: 0.8 * plate + 0.15 * flakes + 0.05 * lichen,
    rough: 0.95
  }
}

pub fn birch_bark(u: f32, v: f32) -> Texel {
  const ACROSS: i32 = 5;
  const ALONG: i32 = 40;
  let at = Vec2::new(u * ACROSS as f32, v * ALONG as f32);
  let row = at.y.floor() as i32;
  let lenticel = (-1..=1).any(|shift| {
    let column = at.x.floor() as i32 + shift;
    let roll = |seed: u32| {
      crate::noise::hash(column.rem_euclid(ACROSS), row.rem_euclid(ALONG), seed)
    };
    let reach = 0.2 + 0.5 * roll(87);
    let offset =
      at - Vec2::new(column as f32 + roll(88), row as f32 + 0.2 + 0.6 * roll(89));
    let span = offset.x / reach;
    roll(90) < 0.7 && span.abs() < 1.0 && offset.y.abs() < 0.12 * (1.0 - span * span)
  });
  let peel = smooth(0.6, 0.7, stretched_fbm(u, v, 3, 12, 3, 91));
  let smudge = stretched_fbm(u, v, 6, 20, 3, 92);
  let tone = Vec3::new(1.0, 0.98, 0.95).lerp(Vec3::new(0.95, 0.85, 0.72), peel * 0.6)
    * (0.85 + 0.15 * smudge);
  Texel {
    tone: if lenticel { tone * 0.28 } else { tone },
    height: 0.5 + 0.2 * peel + 0.1 * smudge - if lenticel { 0.3 } else { 0.0 },
    rough: 0.7
  }
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

pub fn wood_grain(u: f32, v: f32) -> Texel {
  let rings =
    ((u * 11.0 + 1.6 * tile_fbm(u, v, 4, 3, 111)) * std::f32::consts::TAU).sin();
  let fibres = stretched_noise(u, v, 6, 64, 112);
  let checks = ridge(stretched_noise(u, v, 24, 3, 113)).powf(60.0) * 0.6;
  let level = 0.62 + 0.18 * rings + 0.2 * fibres;
  Texel {
    tone: Vec3::splat(level * (1.0 - 0.5 * checks)),
    height: 0.5 + 0.2 * rings + 0.2 * fibres - 0.5 * checks,
    rough: 0.85
  }
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
  unmipped(512, TextureFormat::Rgba8UnormSrgb, |u, v| {
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
    let solid = (dome > 0.02) as u8 as f32 * (0.52 + 0.48 * hash(160));
    [byte(tone.x), byte(tone.y), byte(tone.z), byte(solid)]
  })
}

pub fn cobble_bumps() -> Image { bumps(512, 0.01, |u, v| cobble_grain(u, v).0) }

pub fn thatch_straw(u: f32, v: f32) -> Texel {
  let straws = stretched_noise(u, v, 160, 6, 161);
  let bundles = stretched_noise(u, v, 24, 3, 162);
  let courses =
    ((v * 10.0 + 0.3 * tile_fbm(u, v, 4, 2, 163)) * std::f32::consts::TAU).sin();
  let rot = smooth(0.55, 0.8, stretched_fbm(u, v, 3, 1, 4, 164)) * 0.35;
  let level = 0.5 + 0.35 * straws.powf(0.7) + 0.12 * bundles - 0.08 * courses.max(0.0);
  Texel {
    tone: Vec3::splat(level).lerp(Vec3::new(0.55, 0.55, 0.45) * level, rot * 0.7),
    height: 0.4 * straws + 0.3 * bundles - 0.25 * courses.max(0.0),
    rough: 0.97
  }
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

pub fn masonry_stone(u: f32, v: f32) -> Texel {
  const COURSES: i32 = 6;
  let warp = Vec2::new(tile_fbm(u, v, 6, 3, 181), tile_fbm(u, v, 6, 3, 182)) - 0.5;
  let (x, rise) = (u + 0.035 * warp.x, (v + 0.035 * warp.y) * COURSES as f32);
  let seam = |index: i32| {
    index as f32 + 0.4 * (crate::noise::hash(index.rem_euclid(COURSES), 7, 180) - 0.5)
  };
  let low = rise.floor() as i32;
  let course = match rise {
    rise if rise < seam(low) => low - 1,
    rise if rise >= seam(low + 1) => low + 1,
    _ => low
  };
  let (bottom, top) = (seam(course), seam(course + 1));
  let y_in = ((rise - bottom) / (top - bottom)).clamp(0.0, 1.0);
  let row = |seed: u32| crate::noise::hash(course.rem_euclid(COURSES), 0, seed);
  let stones = 3 + (row(183) * 2.0) as i32;
  let slot = (x + row(184)) * stones as f32;
  let bound = |index: i32| {
    index as f32 + 0.6 * (crate::noise::hash(index.rem_euclid(stones), course, 186) - 0.5)
  };
  let near = slot.floor() as i32;
  let index = match slot {
    slot if slot < bound(near) => near - 1,
    slot if slot >= bound(near + 1) => near + 1,
    _ => near
  };
  let pick = |seed: u32| crate::noise::hash(index.rem_euclid(stones), course, seed);
  let (left, right) = (bound(index), bound(index + 1));
  let across = (slot - left).min(right - slot) / stones as f32;
  let down = y_in.min(1.0 - y_in) * (top - bottom) / COURSES as f32;
  let chip = 0.022 * tile_fbm(u, v, 16, 4, 187);
  let inset = across * down / (across * across + down * down).sqrt() - 0.006 - chip;
  let dome = (inset / 0.03).clamp(0.0, 1.0);
  let dome = (1.0 - (1.0 - dome).powi(2)).sqrt();
  let face = tile_fbm(u + pick(188), v + pick(189), 20, 4, 190);
  let pits = smooth(0.7, 0.78, tile_noise(u, v, 90, 191)) * 0.6;
  let lean = (pick(192) - 0.5) * ((slot - left) / (right - left) - 0.5);
  let moss_patch = tile_fbm(u, v, 4, 4, 193);
  let grime = stretched_noise(u, v, 24, 2, 194);
  let stone_hue = Vec3::new(1.0, 0.98, 0.94).lerp(Vec3::new(0.93, 0.96, 1.0), pick(195));
  let stone = stone_hue
    * (0.66 + 0.2 * pick(196) + 0.22 * (face - 0.5) - 0.12 * pits)
    * (0.78 + 0.22 * dome);
  let mortar = Vec3::new(0.46, 0.44, 0.4) * (0.8 + 0.4 * tile_noise(u, v, 128, 197));
  let bare = if inset > 0.0 { stone } else { mortar };
  let moss = smooth(0.5, 0.68, moss_patch + 0.25 * (1.0 - dome))
    * smooth(0.3, 0.6, tile_fbm(u, v, 32, 2, 198));
  let tone = bare.lerp(Vec3::new(0.5, 0.56, 0.36), moss * 0.55) * (0.82 + 0.18 * grime);
  Texel {
    tone,
    height: if inset > 0.0 {
      0.45 + 0.35 * dome + 0.15 * (face - 0.5) + 0.2 * lean - 0.08 * pits
    } else {
      0.1 * tile_noise(u, v, 128, 199)
    } + 0.04 * moss,
    rough: 0.9
  }
}

pub fn masonry() -> Textured { textured(512, 0.01, masonry_stone) }

pub fn bark() -> Textured { textured(512, 0.01, pine_bark) }

pub fn birch() -> Textured { textured(512, 0.006, birch_bark) }

pub fn wood() -> Textured { textured(256, 0.005, wood_grain) }

pub fn thatch() -> Textured { textured(512, 0.003, thatch_straw) }

pub fn plank_boards(u: f32, v: f32) -> Texel {
  let board = (u * 8.0).floor();
  let seam = ((u * 8.0).fract() - 0.5).abs() > 0.46;
  let grain = stretched_noise(u, v, 128, 6, 191 + board as u32);
  let tone = crate::noise::hash(board as i32, 0, 193);
  let knot = smooth(
    0.08,
    0.0,
    voronoi(u, v, 6, 194).1 * (0.5 + crate::noise::hash(board as i32, 1, 195))
  );
  let level = (0.5 + 0.2 * tone + 0.25 * grain) * (1.0 - 0.4 * knot);
  Texel {
    tone: Vec3::splat(if seam { level * 0.45 } else { level }),
    height: if seam { 0.0 } else { 0.6 + 0.2 * grain - 0.2 * knot },
    rough: 0.85
  }
}
#[derive(Clone, Copy)]
struct Hit {
  rise: f32,
  along: f32,
  across: f32,
  pick: f32
}

struct Strokes {
  period: i32,
  count: i32,
  seed: u32,
  length: f32,
  bend: f32
}

impl Strokes {
  fn hit(
    &self,
    u: f32,
    v: f32,
    girth: impl Fn(f32) -> f32,
    kept: impl Fn(Vec2) -> f32
  ) -> Option<Hit> {
    let &Strokes { period, count, seed, length, bend } = self;
    let at = Vec2::new(u, v) * period as f32;
    let base = at.floor().as_ivec2();
    (-1..=1)
      .flat_map(|dy| (-1..=1).map(move |dx| base + IVec2::new(dx, dy)))
      .flat_map(|cell| (0..count).map(move |index| (cell, index)))
      .filter_map(|(cell, index)| {
        let wrapped = cell.rem_euclid(IVec2::splat(period));
        let roll = |k: u32| {
          crate::noise::hash(wrapped.x + 7919 * index, wrapped.y, seed + 101 * k)
        };
        let root = cell.as_vec2() + Vec2::new(roll(0), roll(1));
        let direction = Vec2::from_angle(roll(2) * std::f32::consts::TAU);
        let reach = length * (0.6 + 0.4 * roll(3));
        let offset = at - root;
        let along = offset.dot(direction) / reach;
        let sway = bend * (roll(4) - 0.5) * along * along * reach;
        let half = girth(along.clamp(0.0, 1.0));
        let across = (direction.perp_dot(offset) - sway) / half.max(1e-4);
        ((0.0..1.0).contains(&along)
          && across.abs() < 1.0
          && roll(7) < kept(root / period as f32))
        .then(|| Hit { rise: roll(5) * 0.5 + 0.5 * along, along, across, pick: roll(6) })
      })
      .max_by(|a, b| a.rise.total_cmp(&b.rise))
  }
}

fn taper(width: f32) -> impl Fn(f32) -> f32 { move |t| width * (1.0 - t).powf(0.7) }

fn leaf(width: f32) -> impl Fn(f32) -> f32 {
  move |t| width * (std::f32::consts::PI * t).sin().powf(0.6)
}

pub fn grass(u: f32, v: f32) -> Texel {
  let mottle = tile_fbm(u, v, 24, 3, 302);
  let moss = smooth(0.45, 0.75, tile_fbm(u, v, 10, 3, 303)) * 0.6;
  let soil = Vec3::new(0.5, 0.42, 0.32).lerp(Vec3::new(0.6, 0.66, 0.42), moss)
    * (0.75 + 0.4 * mottle);
  let density = |at: Vec2| smooth(0.15, 0.6, tile_noise(at.x, at.y, 6, 301)) * 0.5 + 0.5;
  let blade = |hit: Hit, lift: f32| {
    let hue = if hit.pick < 0.2 {
      Vec3::new(1.0, 0.9, 0.6)
    } else {
      Vec3::new(0.82, 0.92, 0.66).lerp(Vec3::new(0.93, 0.96, 0.76), hit.pick)
    };
    Texel {
      tone: hue * (0.6 + 0.4 * hit.along - 0.1 * hit.across),
      height: lift + 0.35 * hit.rise + 0.1 * (1.0 - hit.across.abs()),
      rough: 0.82
    }
  };
  let tall = Strokes { period: 12, count: 4, seed: 311, length: 0.95, bend: 0.8 };
  let short = Strokes { period: 22, count: 5, seed: 321, length: 0.9, bend: 0.6 };
  let fine = Strokes { period: 40, count: 5, seed: 331, length: 0.9, bend: 0.5 };
  tall
    .hit(u, v, taper(0.09), density)
    .map(|hit| blade(hit, 0.5))
    .or_else(|| short.hit(u, v, taper(0.1), density).map(|hit| blade(hit, 0.3)))
    .or_else(|| fine.hit(u, v, taper(0.12), |_| 1.0).map(|hit| blade(hit, 0.15)))
    .unwrap_or(Texel { tone: soil, height: 0.1 + 0.15 * mottle, rough: 0.95 })
}

pub fn litter(u: f32, v: f32) -> Texel {
  let mottle = tile_fbm(u, v, 12, 4, 351);
  let fine = tile_fbm(u, v, 64, 2, 352);
  let moss = smooth(0.58, 0.72, tile_fbm(u, v, 7, 4, 353)) * (0.55 + 0.45 * fine);
  let humus = Texel {
    tone: Vec3::new(0.5, 0.41, 0.33) * (0.7 + 0.5 * mottle),
    height: 0.1 + 0.2 * mottle,
    rough: 0.95
  };
  let ground = match moss > 0.0 {
    true => Texel {
      tone: humus.tone.lerp(Vec3::new(0.66, 0.74, 0.46) * (0.75 + 0.45 * fine), moss),
      height: humus.height + 0.3 * moss * (0.6 + 0.4 * fine),
      rough: 0.97
    },
    false => humus
  };
  let needles = Strokes { period: 40, count: 6, seed: 361, length: 0.95, bend: 0.15 };
  let leaves = Strokes { period: 10, count: 1, seed: 371, length: 0.45, bend: 0.2 };
  let twigs = Strokes { period: 5, count: 1, seed: 381, length: 0.95, bend: 0.4 };
  let everywhere = |_: Vec2| 1.0;
  twigs
    .hit(u, v, |_| 0.025, |_| 0.45)
    .map(|hit| Texel {
      tone: Vec3::new(0.5, 0.42, 0.36) * (0.8 + 0.25 * hit.across),
      height: 0.85,
      rough: 0.9
    })
    .or_else(|| {
      leaves
        .hit(u, v, leaf(0.16), |at| smooth(0.4, 0.8, tile_noise(at.x, at.y, 4, 372)))
        .map(|hit| Texel {
          tone: Vec3::new(1.0, 0.82, 0.45).lerp(Vec3::new(0.8, 0.5, 0.3), hit.pick)
            * (0.85 + 0.15 * (1.0 - hit.across.abs())),
          height: 0.7 + 0.1 * hit.rise,
          rough: 0.8
        })
    })
    .or_else(|| {
      needles.hit(u, v, |_| 0.06, everywhere).filter(|_| moss < 0.35).map(|hit| Texel {
        tone: Vec3::new(0.95, 0.7, 0.5).lerp(Vec3::new(0.72, 0.64, 0.56), hit.pick)
          * (0.8 + 0.2 * hit.along),
        height: 0.4 + 0.3 * hit.rise,
        rough: 0.9
      })
    })
    .unwrap_or(ground)
}

pub fn dirt(u: f32, v: f32) -> Texel {
  let mottle = tile_fbm(u, v, 8, 4, 401);
  let grit = tile_noise(u, v, 256, 402);
  let clods = tile_fbm(u, v, 40, 2, 403);
  let crack = (1.0 - (2.0 * tile_fbm(u, v, 7, 3, 405) - 1.0).abs()).powf(10.0)
    * smooth(0.5, 0.7, tile_fbm(u, v, 3, 2, 409))
    * 0.6;
  let soil = Texel {
    tone: Vec3::new(0.86, 0.79, 0.7)
      * (0.75 + 0.35 * mottle + 0.1 * (grit - 0.5))
      * (1.0 - 0.35 * crack),
    height: 0.2 * mottle + 0.2 * clods + 0.05 * grit - 0.15 * crack,
    rough: 0.97
  };
  let (_, first, cell) = voronoi(u, v, 22, 404);
  let pick = |seed: u32| crate::noise::hash(cell.x, cell.y, seed);
  let reach = 0.15 + 0.2 * pick(406);
  match pick(407) < 0.3 && first < reach {
    true => {
      let dome = (1.0 - (first / reach).powi(2)).sqrt();
      Texel {
        tone: Vec3::new(0.9, 0.9, 0.92) * (0.7 + 0.35 * pick(408)) * (0.8 + 0.2 * dome),
        height: 0.45 + 0.4 * dome,
        rough: 0.85
      }
    }
    false => soil
  }
}

pub fn gravel(u: f32, v: f32) -> Texel {
  let wobble = 0.04 * (tile_fbm(u, v, 12, 3, 502) - 0.5);
  let (edge, first, cell) = voronoi(u + wobble, v - wobble, 16, 501);
  let pick = |seed: u32| crate::noise::hash(cell.x, cell.y, seed);
  let girth = 0.36 + 0.14 * pick(503);
  let (border, round) = (edge - 0.04, girth - first);
  let body = (border + round - ((border - round).powi(2) + 0.01).sqrt()) / 2.0;
  let dome = (body / 0.15).clamp(0.0, 1.0);
  let dome = (1.0 - (1.0 - dome).powi(2)).sqrt();
  let hues = [
    Vec3::new(0.86, 0.86, 0.86),
    Vec3::new(0.92, 0.86, 0.78),
    Vec3::new(0.66, 0.66, 0.68),
    Vec3::new(0.8, 0.82, 0.86),
    Vec3::new(0.95, 0.93, 0.9)
  ];
  let speck = tile_noise(u, v, 192, 504);
  let (_, grain, bit) = voronoi(u, v, 56, 505);
  let bit = crate::noise::hash(bit.x, bit.y, 506);
  let sand = Vec3::new(0.6, 0.56, 0.5) * (0.8 + 0.4 * bit * smooth(0.45, 0.2, grain));
  match dome > 0.0 {
    true => Texel {
      tone: hues[(pick(507) * hues.len() as f32) as usize % hues.len()]
        * (0.8 + 0.2 * dome + 0.12 * (speck - 0.5)),
      height: 0.35 + 0.6 * dome + 0.05 * speck,
      rough: 0.72
    },
    false => {
      Texel { tone: sand, height: 0.15 * bit * smooth(0.45, 0.2, grain), rough: 0.95 }
    }
  }
}

pub fn rock_face(u: f32, v: f32) -> Texel {
  let warp = Vec2::new(tile_fbm(u, v, 4, 3, 601), tile_fbm(u, v, 4, 3, 602)) - 0.5;
  let (edge, first, cell) = voronoi(u + 0.15 * warp.x, v + 0.15 * warp.y, 4, 603);
  let pick = |seed: u32| crate::noise::hash(cell.x, cell.y, seed);
  let fine = tile_fbm(u, v, 40, 4, 604);
  let blotch = tile_fbm(u, v, 6, 4, 612);
  let strata =
    ((v * 7.0 + 2.5 * tile_fbm(u, v, 3, 3, 605)) * std::f32::consts::TAU).sin();
  let ridge =
    |base: i32, seed: u32| 1.0 - (2.0 * tile_fbm(u, v, base, 4, seed) - 1.0).abs();
  let fissure = ridge(5, 613).powf(40.0) * smooth(0.4, 0.6, tile_fbm(u, v, 3, 2, 614));
  let seam = smooth(0.035, 0.0, edge) * smooth(0.3, 0.7, pick(615) + 0.5 * blotch);
  let crack = fissure.max(seam);
  let tilt = (pick(606) - 0.5) * first;
  let (_, spot, dot) = voronoi(u, v, 48, 607);
  let lichen = smooth(0.55, 0.68, tile_fbm(u, v, 6, 3, 608))
    * smooth(0.34, 0.2, spot)
    * (crate::noise::hash(dot.x, dot.y, 609) < 0.6) as u8 as f32;
  let pale = crate::noise::hash(dot.x, dot.y, 610) < 0.5;
  let stone = Vec3::splat(
    0.8
      + 0.1 * (pick(611) - 0.5)
      + 0.3 * (blotch - 0.5)
      + 0.15 * (fine - 0.5)
      + 0.04 * strata
  ) * Vec3::new(1.0, 0.99, 0.96).lerp(Vec3::new(0.95, 0.97, 1.0), pick(616));
  let lichen_tone =
    if pale { Vec3::new(0.97, 0.97, 0.9) } else { Vec3::new(0.92, 0.9, 0.55) };
  Texel {
    tone: stone.lerp(lichen_tone, lichen * 0.8) * (1.0 - 0.6 * crack),
    height: 0.5 + 0.35 * tilt + 0.3 * (fine - 0.5) + 0.2 * (blotch - 0.5) + 0.05 * strata
      - 0.4 * crack
      + 0.05 * lichen,
    rough: 0.85 - 0.1 * lichen
  }
}

pub fn snow(u: f32, v: f32) -> Texel {
  let drift = tile_fbm(u, v, 3, 4, 701);
  let ripple = ((v * 16.0 + u * 3.0 + 3.0 * tile_fbm(u, v, 3, 3, 702))
    * std::f32::consts::TAU)
    .sin()
    * 0.5
    + 0.5;
  let rippled = smooth(0.4, 0.7, tile_fbm(u, v, 3, 3, 704));
  let lumps = tile_fbm(u, v, 12, 3, 703);
  let crust = tile_fbm(u, v, 48, 2, 706);
  let sparkle = tile_noise(u, v, 512, 705) > 0.93;
  let height = 0.55 * drift + 0.1 * ripple * rippled + 0.2 * lumps + 0.05 * crust;
  Texel {
    tone: Vec3::new(0.86, 0.9, 0.97).lerp(Vec3::ONE, smooth(0.2, 0.6, height)),
    height,
    rough: if sparkle { 0.12 } else { 0.5 + 0.15 * crust }
  }
}

pub fn planks() -> Textured { textured(256, 0.006, plank_boards) }

pub fn land() -> Textured {
  stacked(512, vec![
    (0.004, &grass),
    (0.004, &litter),
    (0.006, &dirt),
    (0.006, &gravel),
    (0.012, &rock_face),
    (0.006, &snow),
  ])
}

#[cfg(test)]
mod tests {
  use super::*;

  pub fn swatches(
    path: &str,
    size: u32,
    layers: &[(f32, Vec3, &(dyn Fn(f32, f32) -> Texel + Sync))]
  ) {
    let light = Vec3::new(-0.5, -0.6, 0.6).normalize();
    let lit: Vec<Vec<Vec3>> = layers
      .iter()
      .map(|&(strength, tint, texel)| {
        let texels = grid(size, texel);
        let heights: Vec<f32> = texels.iter().map(|texel| texel.height).collect();
        (0..size * size)
          .map(|at| {
            let normal = facing(size, strength, &heights, at);
            texels[at as usize].tone * tint * (0.35 + 0.8 * normal.dot(light).max(0.0))
          })
          .collect()
      })
      .collect();
    let (width, height) = (size * layers.len() as u32, size * 2);
    let pixels: Vec<u8> = (0..width * height)
      .flat_map(|index| {
        let (x, y) = (index % width, index / width);
        let (layer, column) = ((x / size) as usize, x % size);
        let tone = match y < size {
          true => lit[layer][(y * size + column) as usize],
          false => lit[layer][((y * 2) % size * size + (column * 2) % size) as usize]
        };
        [byte(tone.x), byte(tone.y), byte(tone.z), 255]
      })
      .collect();
    Image::new(
      Extent3d { width, height, depth_or_array_layers: 1 },
      TextureDimension::D2,
      pixels,
      TextureFormat::Rgba8UnormSrgb,
      RenderAssetUsages::MAIN_WORLD
    )
    .try_into_dynamic()
    .expect("swatch image")
    .to_rgb8()
    .save(path)
    .expect("swatches saved");
  }

  #[test]
  #[ignore]
  fn stuff_swatches() {
    swatches("screenshots/stuff.png", 384, &[
      (0.01, Vec3::new(0.55, 0.53, 0.5) * 1.4, &masonry_stone),
      (0.015, Vec3::new(0.5, 0.5, 0.48) * 1.4, &rough_stone),
      (0.01, Vec3::new(0.42, 0.3, 0.22) * 1.6, &pine_bark),
      (0.006, Vec3::new(0.9, 0.9, 0.88), &birch_bark),
      (0.005, Vec3::new(0.5, 0.36, 0.24) * 1.4, &wood_grain),
      (0.003, Vec3::new(0.7, 0.6, 0.4) * 1.2, &thatch_straw),
      (0.006, Vec3::new(0.5, 0.36, 0.24) * 1.4, &plank_boards)
    ]);
  }

  #[test]
  #[ignore]
  fn land_swatches() {
    swatches("screenshots/land.png", 384, &[
      (0.004, Vec3::new(0.33, 0.36, 0.23) * 1.6, &grass),
      (0.004, Vec3::new(0.25, 0.24, 0.18) * 1.6, &litter),
      (0.006, Vec3::new(0.40, 0.33, 0.24) * 1.6, &dirt),
      (0.006, Vec3::new(0.46, 0.44, 0.40) * 1.6, &gravel),
      (0.012, Vec3::new(0.43, 0.43, 0.43) * 1.6, &rock_face),
      (0.01, Vec3::new(0.93, 0.95, 1.0), &snow)
    ]);
  }
}
