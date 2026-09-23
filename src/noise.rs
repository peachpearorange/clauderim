use bevy::prelude::*;

pub fn hash(x: i32, y: i32, seed: u32) -> f32 {
  let mixed = (x as u32).wrapping_mul(0x8da6_b343)
    ^ (y as u32).wrapping_mul(0xd816_3841)
    ^ seed.wrapping_mul(0xcb1a_b31f);
  let mixed = (mixed ^ (mixed >> 13)).wrapping_mul(0x5bd1_e995);
  ((mixed ^ (mixed >> 15)) >> 8) as f32 / (1 << 24) as f32
}

fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }

pub fn perlin(at: Vec2, seed: u32) -> f32 {
  let cell = at.floor();
  let (x, y) = (cell.x as i32, cell.y as i32);
  let local = at - cell;
  let slope = |dx: i32, dy: i32| {
    let angle = hash(x + dx, y + dy, seed) * std::f32::consts::TAU;
    Vec2::from_angle(angle).dot(local - Vec2::new(dx as f32, dy as f32))
  };
  let (u, v) = (fade(local.x), fade(local.y));
  let bottom = slope(0, 0).lerp(slope(1, 0), u);
  let top = slope(0, 1).lerp(slope(1, 1), u);
  bottom.lerp(top, v) * 1.4
}

pub fn value(at: Vec2, seed: u32) -> f32 {
  let cell = at.floor();
  let (x, y) = (cell.x as i32, cell.y as i32);
  let (u, v) = (fade(at.x - cell.x), fade(at.y - cell.y));
  let bottom = hash(x, y, seed).lerp(hash(x + 1, y, seed), u);
  let top = hash(x, y + 1, seed).lerp(hash(x + 1, y + 1, seed), u);
  bottom.lerp(top, v)
}

const TWIST: Mat2 = Mat2::from_cols_array(&[1.6, 1.2, -1.2, 1.6]);

pub fn fbm(at: Vec2, octaves: u32, seed: u32) -> f32 {
  (0..octaves)
    .fold((0.0, at, 1.0, 0.0), |(sum, at, amplitude, total), octave| {
      (
        sum + perlin(at, seed.wrapping_add(octave * 131)) * amplitude,
        TWIST * at,
        amplitude * 0.5,
        total + amplitude
      )
    })
    .0
    / 1.2
}

pub fn ridged(at: Vec2, octaves: u32, seed: u32) -> f32 {
  (0..octaves)
    .fold((0.0, at, 1.0, 0.0, 1.0), |(sum, at, amplitude, total, weight), octave| {
      let crest = 1.0 - perlin(at, seed.wrapping_add(octave * 173)).abs();
      let crest = crest * crest * weight;
      (sum + crest * amplitude, TWIST * at, amplitude * 0.5, total + amplitude, crest)
    })
    .0
    / 1.9
}

pub fn value3(at: Vec3, seed: u32) -> f32 {
  let cell = at.floor();
  let local = at - cell;
  let (x, y, z) = (cell.x as i32, cell.y as i32, cell.z as i32);
  let corner = |dx: i32, dy: i32, dz: i32| {
    hash(x + dx, (y + dy).wrapping_mul(8191).wrapping_add((z + dz).wrapping_mul(131_071)), seed)
  };
  let (u, v, w) = (fade(local.x), fade(local.y), fade(local.z));
  let plane = |dz: i32| {
    corner(0, 0, dz)
      .lerp(corner(1, 0, dz), u)
      .lerp(corner(0, 1, dz).lerp(corner(1, 1, dz), u), v)
  };
  plane(0).lerp(plane(1), w)
}

pub fn fbm3(at: Vec3, octaves: u32, seed: u32) -> f32 {
  (0..octaves)
    .fold((0.0, at, 1.0), |(sum, at, amplitude), octave| {
      (
        sum + (value3(at, seed.wrapping_add(octave * 97)) - 0.5) * amplitude,
        at * 2.03 + Vec3::splat(17.1),
        amplitude * 0.5
      )
    })
    .0
}

pub struct Roll(u32);

impl Roll {
  pub fn new(seed: u32) -> Self { Self(seed.wrapping_mul(2_654_435_761) ^ 0x9e37_79b9) }

  pub fn next(&mut self) -> f32 {
    self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let mixed = self.0 ^ (self.0 >> 15);
    (mixed.wrapping_mul(2_246_822_519) >> 8) as f32 / (1 << 24) as f32
  }

  pub fn range(&mut self, low: f32, high: f32) -> f32 { low + (high - low) * self.next() }

  pub fn spread(&mut self, reach: f32) -> f32 { self.range(-reach, reach) }

  pub fn chance(&mut self, odds: f32) -> bool { self.next() < odds }

  pub fn below(&mut self, bound: usize) -> usize {
    ((self.next() * bound as f32) as usize).min(bound - 1)
  }
}
