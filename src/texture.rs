use bevy::{asset::RenderAssetUsages,
           image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
           prelude::*,
           render::render_resource::{Extent3d, TextureDimension, TextureFormat}};

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
