use {crate::{humanoid::{Build, Joint, Kit},
             model::{self, Piece, curve, sculpt, taper, tube},
             noise::{self, Roll},
             stuff::Stuff},
     bevy::{asset::RenderAssetUsages,
            mesh::{Indices, PrimitiveTopology},
            prelude::*},
     std::f32::consts::{FRAC_PI_2, PI}};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Race {
  Northman,
  Orc,
  SunElf,
  AshElf,
  Saurian
}

impl Race {
  pub fn of(seed: u32) -> Race {
    match Roll::new(seed * 7 + 3).next() {
      odds if odds < 0.5 => Race::Northman,
      odds if odds < 0.62 => Race::Orc,
      odds if odds < 0.72 => Race::SunElf,
      odds if odds < 0.88 => Race::AshElf,
      _ => Race::Saurian
    }
  }

  pub fn named(name: &str) -> Option<Race> {
    match name {
      "northman" => Some(Race::Northman),
      "orc" => Some(Race::Orc),
      "altmer" | "sunelf" => Some(Race::SunElf),
      "dunmer" | "ashelf" => Some(Race::AshElf),
      "saurian" => Some(Race::Saurian),
      _ => None
    }
  }

  fn skins(self) -> Vec<Srgba> {
    match self {
      Race::Northman => vec![
        Srgba::new(0.8, 0.62, 0.52, 1.0),
        Srgba::new(0.74, 0.56, 0.46, 1.0),
        Srgba::new(0.7, 0.5, 0.4, 1.0),
        Srgba::new(0.84, 0.68, 0.58, 1.0),
      ],
      Race::Orc => vec![
        Srgba::new(0.46, 0.5, 0.36, 1.0),
        Srgba::new(0.4, 0.43, 0.32, 1.0),
        Srgba::new(0.52, 0.54, 0.42, 1.0),
        Srgba::new(0.36, 0.36, 0.28, 1.0),
      ],
      Race::SunElf => vec![
        Srgba::new(0.7, 0.57, 0.34, 1.0),
        Srgba::new(0.64, 0.52, 0.3, 1.0),
        Srgba::new(0.74, 0.62, 0.4, 1.0),
      ],
      Race::AshElf => vec![
        Srgba::new(0.4, 0.42, 0.42, 1.0),
        Srgba::new(0.35, 0.37, 0.39, 1.0),
        Srgba::new(0.46, 0.46, 0.44, 1.0),
        Srgba::new(0.31, 0.31, 0.33, 1.0),
      ],
      Race::Saurian => vec![
        Srgba::new(0.36, 0.4, 0.22, 1.0),
        Srgba::new(0.44, 0.34, 0.24, 1.0),
        Srgba::new(0.3, 0.34, 0.2, 1.0),
        Srgba::new(0.5, 0.36, 0.3, 1.0),
      ]
    }
  }

  fn irises(self) -> Vec<Srgba> {
    match self {
      Race::Northman => vec![
        Srgba::new(0.3, 0.45, 0.6, 1.0),
        Srgba::new(0.45, 0.5, 0.55, 1.0),
        Srgba::new(0.35, 0.45, 0.3, 1.0),
        Srgba::new(0.4, 0.3, 0.2, 1.0),
      ],
      Race::Orc => vec![
        Srgba::new(0.75, 0.6, 0.15, 1.0),
        Srgba::new(0.6, 0.12, 0.08, 1.0),
        Srgba::new(0.55, 0.45, 0.3, 1.0),
      ],
      Race::SunElf => vec![
        Srgba::new(0.8, 0.65, 0.1, 1.0),
        Srgba::new(0.5, 0.7, 0.15, 1.0),
        Srgba::new(0.85, 0.45, 0.1, 1.0),
      ],
      Race::AshElf => {
        vec![Srgba::new(0.75, 0.08, 0.05, 1.0), Srgba::new(0.6, 0.1, 0.1, 1.0)]
      }
      Race::Saurian => vec![
        Srgba::new(0.85, 0.7, 0.2, 1.0),
        Srgba::new(0.9, 0.45, 0.15, 1.0),
        Srgba::new(0.7, 0.75, 0.4, 1.0),
      ]
    }
  }

  fn manes(self) -> Vec<Srgba> {
    match self {
      Race::Northman => vec![
        Srgba::new(0.55, 0.42, 0.22, 1.0),
        Srgba::new(0.3, 0.2, 0.11, 1.0),
        Srgba::new(0.45, 0.2, 0.1, 1.0),
        Srgba::new(0.62, 0.52, 0.32, 1.0),
        Srgba::new(0.6, 0.58, 0.54, 1.0),
      ],
      Race::Orc => vec![
        Srgba::new(0.12, 0.1, 0.08, 1.0),
        Srgba::new(0.22, 0.16, 0.1, 1.0),
        Srgba::new(0.5, 0.48, 0.45, 1.0),
      ],
      Race::SunElf => vec![
        Srgba::new(0.72, 0.6, 0.36, 1.0),
        Srgba::new(0.8, 0.78, 0.72, 1.0),
        Srgba::new(0.62, 0.36, 0.26, 1.0),
      ],
      Race::AshElf => vec![
        Srgba::new(0.08, 0.07, 0.07, 1.0),
        Srgba::new(0.2, 0.14, 0.1, 1.0),
        Srgba::new(0.5, 0.2, 0.12, 1.0),
        Srgba::new(0.8, 0.76, 0.7, 1.0),
      ],
      Race::Saurian => vec![
        Srgba::new(0.3, 0.2, 0.14, 1.0),
        Srgba::new(0.2, 0.18, 0.3, 1.0),
        Srgba::new(0.45, 0.2, 0.12, 1.0),
      ]
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ear {
  Round,
  Pointed,
  Long
}

#[derive(Clone, Copy)]
pub struct Visage {
  pub jaw: f32,
  pub length: f32,
  pub brow: f32,
  pub cheek: f32,
  pub hollow: f32,
  pub nose: f32,
  pub nostrils: f32,
  pub chin: f32,
  pub lips: f32,
  pub slant: f32,
  pub ear: Ear,
  pub tusks: f32
}

impl Visage {
  fn of(race: Race, woman: bool) -> Visage {
    let base = match race {
      Race::Northman => Visage {
        jaw: 1.04,
        length: 1.0,
        brow: 1.0,
        cheek: 1.0,
        hollow: 0.4,
        nose: 1.0,
        nostrils: 1.0,
        chin: 1.1,
        lips: 1.0,
        slant: 0.0,
        ear: Ear::Round,
        tusks: 0.0
      },
      Race::Orc => Visage {
        jaw: 1.2,
        length: 1.02,
        brow: 2.2,
        cheek: 1.3,
        hollow: 0.0,
        nose: 0.7,
        nostrils: 1.9,
        chin: 1.5,
        lips: 1.3,
        slant: 0.08,
        ear: Ear::Pointed,
        tusks: 1.0
      },
      Race::SunElf => Visage {
        jaw: 0.88,
        length: 1.12,
        brow: 1.1,
        cheek: 1.7,
        hollow: 1.3,
        nose: 1.15,
        nostrils: 0.8,
        chin: 1.2,
        lips: 0.8,
        slant: 0.2,
        ear: Ear::Long,
        tusks: 0.0
      },
      Race::AshElf => Visage {
        jaw: 0.92,
        length: 1.06,
        brow: 1.5,
        cheek: 1.5,
        hollow: 1.0,
        nose: 1.0,
        nostrils: 0.9,
        chin: 1.0,
        lips: 0.9,
        slant: 0.22,
        ear: Ear::Long,
        tusks: 0.0
      },
      Race::Saurian => Visage {
        jaw: 1.0,
        length: 1.0,
        brow: 1.0,
        cheek: 0.0,
        hollow: 0.0,
        nose: 0.0,
        nostrils: 0.0,
        chin: 0.0,
        lips: 0.0,
        slant: 0.0,
        ear: Ear::Round,
        tusks: 0.0
      }
    };
    woman
      .then(|| Visage {
        jaw: base.jaw * 0.9,
        length: base.length * 0.96,
        brow: base.brow * 0.35,
        chin: base.chin * 0.7,
        lips: base.lips * 1.35,
        nose: base.nose * 0.85,
        nostrils: base.nostrils * 0.8,
        tusks: base.tusks * 0.45,
        ..base
      })
      .unwrap_or(base)
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
  Bald,
  Cropped,
  Swept,
  Long,
  Braided,
  Mohawk,
  Topknot
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Beard {
  Stubble,
  Goatee,
  Full,
  Long
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint {
  HalfFace,
  EyeBand,
  Claws,
  Chevron,
  Tears,
  Sigil,
  Stripes
}

#[derive(Clone, Copy)]
pub struct Person {
  pub race: Race,
  pub woman: bool,
  pub skin: Srgba,
  pub mane: Srgba,
  pub iris: Srgba,
  pub glowing: bool,
  pub style: Style,
  pub beard: Option<Beard>,
  pub paint: Option<(Paint, Srgba)>,
  pub age: f32,
  pub visage: Visage,
  pub seed: u32
}

impl Person {
  pub fn roll(race: Race, woman: bool, seed: u32) -> Person {
    let mut roll = Roll::new(seed * 13 + 5);
    let mut pick = |colors: &[Srgba]| colors[roll.below(colors.len())];
    let (skin, mane, iris) =
      (pick(&race.skins()), pick(&race.manes()), pick(&race.irises()));
    let mut roll = Roll::new(seed * 29 + 11);
    let styles: &[Style] = match (race, woman) {
      (Race::Saurian, _) => &[Style::Bald],
      (Race::Orc, false) => &[Style::Mohawk, Style::Topknot, Style::Cropped, Style::Bald],
      (Race::Orc, true) => &[Style::Topknot, Style::Swept, Style::Mohawk],
      (Race::AshElf, false) => {
        &[Style::Mohawk, Style::Long, Style::Swept, Style::Cropped]
      }
      (Race::AshElf, true) => &[Style::Long, Style::Mohawk, Style::Braided, Style::Swept],
      (Race::SunElf, false) => &[Style::Long, Style::Swept, Style::Topknot],
      (Race::SunElf, true) => &[Style::Long, Style::Swept, Style::Topknot],
      (Race::Northman, false) => {
        &[Style::Long, Style::Swept, Style::Braided, Style::Bald, Style::Cropped]
      }
      (Race::Northman, true) => &[Style::Long, Style::Braided, Style::Swept]
    };
    let style = styles[roll.below(styles.len())];
    let beards: &[Option<Beard>] = match (race, woman) {
      (Race::Saurian, _) | (_, true) => &[None],
      (Race::Northman, false) => &[
        Some(Beard::Full),
        Some(Beard::Long),
        Some(Beard::Stubble),
        Some(Beard::Full),
        Some(Beard::Goatee)
      ],
      (Race::Orc, false) => {
        &[Some(Beard::Stubble), None, Some(Beard::Full), Some(Beard::Goatee)]
      }
      _ => &[None, Some(Beard::Goatee), Some(Beard::Stubble), None]
    };
    let beard = beards[roll.below(beards.len())];
    let paints: &[Paint] = match race {
      Race::Northman => &[Paint::HalfFace, Paint::EyeBand, Paint::Claws],
      Race::Orc => &[Paint::Chevron, Paint::Stripes, Paint::Claws],
      Race::SunElf => &[Paint::Sigil],
      Race::AshElf => &[Paint::Tears, Paint::Stripes, Paint::Chevron],
      Race::Saurian => &[Paint::Stripes, Paint::Tears]
    };
    let inks = [
      Srgba::new(0.22, 0.36, 0.4, 1.0),
      Srgba::new(0.5, 0.12, 0.08, 1.0),
      Srgba::new(0.2, 0.14, 0.1, 1.0),
      Srgba::new(0.62, 0.45, 0.12, 1.0)
    ];
    let paint = roll
      .chance(0.35)
      .then(|| (paints[roll.below(paints.len())], inks[roll.below(inks.len())]));
    let age = roll.range(0.0, 1.0).powi(2);
    let visage = Visage::of(race, woman);
    let mut vary = |value: f32, spread: f32| value * (1.0 + roll.spread(spread));
    let visage = Visage {
      jaw: vary(visage.jaw, 0.06),
      length: vary(visage.length, 0.04),
      brow: vary(visage.brow, 0.3),
      cheek: vary(visage.cheek, 0.3),
      hollow: visage.hollow + age * 0.8,
      nose: vary(visage.nose, 0.2),
      nostrils: vary(visage.nostrils, 0.2),
      chin: vary(visage.chin, 0.3),
      lips: vary(visage.lips, 0.2),
      ..visage
    };
    Person {
      race,
      woman,
      skin,
      mane: mane.mix(&Srgba::new(0.62, 0.6, 0.57, 1.0), (age - 0.5).max(0.0) * 1.6),
      iris,
      glowing: false,
      style,
      beard,
      paint,
      age,
      visage,
      seed
    }
  }

  pub fn flesh(&self) -> Stuff {
    (self.race == Race::Saurian).then_some(Stuff::Scales).unwrap_or(Stuff::Skin)
  }

  pub fn wight(seed: u32) -> Person {
    let base = Person::roll(Race::Northman, false, seed);
    Person {
      skin: Srgba::new(0.36, 0.33, 0.27, 1.0),
      mane: Srgba::new(0.66, 0.64, 0.58, 1.0),
      iris: Srgba::new(0.5, 0.8, 1.0, 1.0),
      glowing: true,
      style: [Style::Long, Style::Bald, Style::Braided][seed as usize % 3],
      beard: Some([Beard::Long, Beard::Full][seed as usize % 2]),
      paint: None,
      age: 1.0,
      visage: Visage {
        hollow: 2.6,
        cheek: 1.6,
        brow: 1.6,
        nose: 0.6,
        lips: 0.3,
        ..base.visage
      },
      ..base
    }
  }
}

struct Profile {
  y: f32,
  wide: f32,
  front: f32,
  back: f32,
  shift: f32
}

const fn ring(y: f32, wide: f32, front: f32, back: f32, shift: f32) -> Profile {
  Profile { y, wide, front, back, shift }
}

const HUMAN: [Profile; 13] = [
  ring(0.036, 0.004, 0.004, 0.004, -0.07),
  ring(0.042, 0.022, 0.018, 0.02, -0.066),
  ring(0.056, 0.047, 0.03, 0.05, -0.05),
  ring(0.075, 0.06, 0.064, 0.074, -0.022),
  ring(0.095, 0.065, 0.082, 0.086, -0.011),
  ring(0.115, 0.067, 0.088, 0.092, -0.007),
  ring(0.135, 0.071, 0.092, 0.097, -0.004),
  ring(0.158, 0.075, 0.094, 0.101, -0.002),
  ring(0.185, 0.077, 0.092, 0.104, 0.0),
  ring(0.215, 0.074, 0.083, 0.101, 0.003),
  ring(0.245, 0.061, 0.063, 0.083, 0.006),
  ring(0.268, 0.036, 0.036, 0.05, 0.009),
  ring(0.278, 0.003, 0.003, 0.004, 0.01)
];

const SAURIAN: [Profile; 13] = [
  ring(0.03, 0.004, 0.004, 0.004, -0.04),
  ring(0.038, 0.03, 0.05, 0.03, -0.04),
  ring(0.055, 0.046, 0.1, 0.05, -0.03),
  ring(0.075, 0.048, 0.135, 0.07, -0.02),
  ring(0.092, 0.045, 0.155, 0.08, -0.012),
  ring(0.108, 0.043, 0.162, 0.086, -0.008),
  ring(0.125, 0.048, 0.15, 0.09, -0.005),
  ring(0.145, 0.066, 0.125, 0.096, -0.002),
  ring(0.168, 0.07, 0.105, 0.1, 0.0),
  ring(0.195, 0.064, 0.08, 0.1, 0.004),
  ring(0.22, 0.052, 0.06, 0.09, 0.01),
  ring(0.238, 0.03, 0.035, 0.06, 0.015),
  ring(0.245, 0.003, 0.003, 0.004, 0.018)
];

const COLUMNS: usize = 112;

#[derive(Clone, Copy)]
struct Bump {
  at: Vec3,
  reach: Vec3,
  rise: f32,
  tilt: f32,
  sharp: f32
}

fn bump(x: f32, y: f32, z: f32, wide: f32, high: f32, rise: f32) -> Bump {
  Bump {
    at: Vec3::new(x, y, z),
    reach: Vec3::new(wide, high, 0.05),
    rise,
    tilt: 0.0,
    sharp: 2.0
  }
}

impl Bump {
  fn deep(self, reach: f32) -> Self { Self { reach: self.reach.with_z(reach), ..self } }

  fn tilted(self, tilt: f32) -> Self { Self { tilt, ..self } }

  fn sharp(self, sharp: f32) -> Self { Self { sharp, ..self } }

  fn one(&self, at: Vec3, tilt: f32) -> f32 {
    let offset = at - self.at;
    let (sin, cos) = tilt.sin_cos();
    let turned = Vec3::new(
      offset.x * cos + offset.y * sin,
      offset.y * cos - offset.x * sin,
      offset.z
    );
    (-(turned / self.reach).length_squared().powf(self.sharp / 2.0)).exp()
  }

  fn weight(&self, at: Vec3) -> f32 {
    let mirror = Vec3::new(-at.x, at.y, at.z);
    let paired = (self.at.x > 0.0).then(|| self.one(mirror, self.tilt)).unwrap_or(0.0);
    self.one(at, self.tilt) + paired
  }

  fn lift(&self, at: Vec3) -> f32 { self.rise * self.weight(at) }
}

fn refined(keys: &[Vec3], steps: impl Fn(usize) -> usize) -> Vec<Vec3> {
  let key = |index: isize| keys[index.clamp(0, keys.len() as isize - 1) as usize];
  (0..keys.len() - 1)
    .flat_map(|segment| {
      let index = segment as isize;
      let [before, from, to, after] =
        [key(index - 1), key(index), key(index + 1), key(index + 2)];
      let count = steps(segment);
      (0..count).map(move |step| {
        let t = step as f32 / count as f32;
        0.5
          * (from * 2.0
            + (to - before) * t
            + (before * 2.0 - from * 5.0 + to * 4.0 - after) * t * t
            + (from * 3.0 - before - to * 3.0 + after) * t * t * t)
      })
    })
    .chain(keys.last().copied())
    .collect()
}

fn sign_pow(value: f32, power: f32) -> f32 { value.signum() * value.abs().powf(power) }

fn warp(t: f32) -> f32 { t * (0.35 + 0.65 * t * t) }

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
  let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

fn table(points: &[(f32, f32)], at: f32) -> f32 {
  points
    .windows(2)
    .find(|pair| at <= pair[1].0)
    .map(|pair| {
      let t = ((at - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0);
      pair[0].1.lerp(pair[1].1, t)
    })
    .unwrap_or(points[points.len() - 1].1)
}

struct Skull {
  rows: usize,
  base: Vec<Vec3>,
  angles: Vec<f32>
}

impl Skull {
  fn shaped(profile: &[Profile], visage: &Visage) -> Skull {
    let squash = |y: f32| 0.19 - (0.19 - y) * visage.length;
    let upper: Vec<Vec3> =
      profile.iter().map(|ring| Vec3::new(squash(ring.y), ring.shift, 0.0)).collect();
    let girth: Vec<Vec3> = profile
      .iter()
      .map(|ring| {
        let jaw = 1.0 + (visage.jaw - 1.0) * smoothstep(0.15, 0.06, ring.y);
        Vec3::new(ring.wide * jaw, ring.front, ring.back)
      })
      .collect();
    let steps = |index: usize| {
      let middle = (profile[index].y + profile[index + 1].y) / 2.0;
      (0.07..0.19).contains(&middle).then_some(11).unwrap_or(4)
    };
    let (heights, girths) = (refined(&upper, steps), refined(&girth, steps));
    let angles: Vec<f32> = (0..COLUMNS)
      .map(|column| PI * warp(column as f32 / COLUMNS as f32 * 2.0 - 1.0))
      .collect();
    let base = heights
      .iter()
      .zip(&girths)
      .flat_map(|(&Vec3 { x: y, y: shift, .. }, &Vec3 { x: wide, y: front, z: back })| {
        angles.iter().map(move |&angle| {
          let (sin, cos) = angle.sin_cos();
          let (depth, power) =
            (cos > 0.0).then_some((front, 2.0 / 2.7)).unwrap_or((back, 1.0));
          Vec3::new(wide * sign_pow(sin, power), y, shift - depth * sign_pow(cos, power))
        })
      })
      .collect();
    Skull { rows: heights.len(), base, angles }
  }

  fn within(&self, low: f32, high: f32) -> Skull {
    let kept: Vec<usize> = (0..self.rows)
      .filter(|&row| (low..=high).contains(&self.base[row * COLUMNS].y))
      .collect();
    Skull {
      rows: kept.len(),
      base: kept
        .iter()
        .flat_map(|&row| self.base[row * COLUMNS..(row + 1) * COLUMNS].iter().copied())
        .collect(),
      angles: self.angles.clone()
    }
  }

  fn nearest(&self, target: Vec3) -> Vec3 {
    self
      .base
      .iter()
      .copied()
      .min_by(|a, b| a.distance_squared(target).total_cmp(&b.distance_squared(target)))
      .unwrap_or(target)
  }

  fn surface(
    &self,
    lift: impl Fn(Vec3, f32) -> Vec3,
    paint: impl Fn(Vec3) -> LinearRgba
  ) -> Mesh {
    let columns = COLUMNS;
    let placed: Vec<Vec3> = self
      .base
      .iter()
      .enumerate()
      .map(|(index, &at)| lift(at, self.angles[index % columns]))
      .collect();
    let at = |row: usize, column: usize| {
      placed[row.min(self.rows - 1) * columns + (column + columns) % columns]
    };
    let normals: Vec<Vec3> = (0..self.rows)
      .flat_map(|row| {
        let at = &at;
        (0..columns).map(move |column| {
          let up = at(row + 1, column) - at(row.saturating_sub(1), column);
          let around = at(row, column + 1) - at(row, column + columns - 1);
          up.cross(around).normalize_or(Vec3::NEG_Y * (row == 0) as i32 as f32 + Vec3::Y)
        })
      })
      .collect();
    let uvs: Vec<[f32; 2]> = (0..self.rows)
      .flat_map(|row| {
        (0..columns).map(move |column| {
          [column as f32 / columns as f32 * 4.0, row as f32 / self.rows as f32 * 2.0]
        })
      })
      .collect();
    let colors: Vec<[f32; 4]> =
      placed.iter().map(|&at| paint(at).to_f32_array()).collect();
    let indices: Vec<u32> = (0..self.rows - 1)
      .flat_map(|row| {
        (0..columns).flat_map(move |column| {
          let next = (column + 1) % columns;
          let [a, b, c, d] = [
            row * columns + column,
            row * columns + next,
            (row + 1) * columns + column,
            (row + 1) * columns + next
          ]
          .map(|index| index as u32);
          [a, c, b, b, c, d]
        })
      })
      .collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
      .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, placed)
      .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
      .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
      .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
      .with_inserted_indices(Indices::U32(indices))
  }
}

fn skinned(mesh: Mesh) -> Piece {
  let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).cloned();
  let Piece(mut mesh) = Piece::new(mesh, Srgba::WHITE);
  if let Some(colors) = colors {
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
  }
  Piece(mesh)
}

fn lin(color: Srgba) -> LinearRgba { LinearRgba::from(color) }

fn tint(color: LinearRgba, red: f32, green: f32, blue: f32) -> LinearRgba {
  LinearRgba::rgb(color.red * red, color.green * green, color.blue * blue)
}

fn features(person: &Person) -> Vec<Bump> {
  let Visage { brow, cheek, hollow, nose, nostrils, chin, lips, slant, .. } =
    person.visage;
  let front = -0.09;
  let eye = bump(0.032, 0.158, front, 0.016, 0.0045, -0.012).sharp(4.0).tilted(slant);
  let wrinkles = person.age * (!person.woman) as i32 as f32;
  vec![
    bump(0.0, 0.215, -0.08, 0.05, 0.03, 0.003).deep(0.06),
    bump(0.068, 0.19, -0.03, 0.015, 0.02, -0.004).deep(0.03),
    bump(0.03, 0.178, front, 0.03, 0.008, 0.006 * brow).tilted(-slant * 0.5),
    bump(0.0, 0.172, front, 0.012, 0.01, 0.003 * brow),
    bump(0.032, 0.16, front, 0.022, 0.016, -0.004),
    eye,
    bump(0.032, 0.165, front, 0.016, 0.0028, 0.0022).tilted(slant),
    bump(0.034, 0.147, front, 0.012, 0.003, -0.0012 * wrinkles).tilted(slant),
    bump(0.0, 0.145, front, 0.0085, 0.028, 0.013 * nose),
    bump(0.0, 0.118, front, 0.011 + 0.002 * nostrils, 0.011, 0.024 * nose),
    bump(0.014, 0.112, front, 0.007 + 0.003 * nostrils, 0.007, 0.008 * nostrils),
    bump(0.052, 0.138, -0.07, 0.02, 0.012, 0.006 * cheek),
    bump(0.052, 0.103, -0.07, 0.018, 0.02, -0.006 * hollow),
    bump(0.028, 0.103, front, 0.003, 0.014, -0.0025 * wrinkles).tilted(-0.5),
    bump(0.0, 0.0935, front, 0.02, 0.005, 0.007 * lips),
    bump(0.0, 0.0825, front, 0.017, 0.006, 0.008 * lips),
    bump(0.0, 0.0885, front, 0.023, 0.0016, -0.006),
    bump(0.024, 0.088, front, 0.004, 0.004, -0.003),
    bump(0.0, 0.071, front, 0.016, 0.004, -0.003),
    bump(0.0, 0.052, -0.085, 0.02, 0.014, 0.008 * chin),
    bump(0.062, 0.075, 0.02, 0.015, 0.02, 0.004 * person.visage.jaw).deep(0.03),
  ]
}

fn saurian_features(person: &Person) -> Vec<Bump> {
  let brow = person.visage.brow;
  vec![
    bump(0.052, 0.176, -0.065, 0.024, 0.008, 0.013 * brow).deep(0.04),
    bump(0.058, 0.158, -0.06, 0.016, 0.012, -0.013).sharp(3.0).deep(0.03),
    bump(0.05, 0.145, -0.07, 0.02, 0.006, 0.005).deep(0.04),
    bump(0.042, 0.083, -0.1, 0.02, 0.0028, -0.006).deep(0.12),
    bump(0.012, 0.118, -0.16, 0.006, 0.005, -0.004).deep(0.02),
    bump(0.0, 0.2, -0.04, 0.012, 0.04, 0.006).deep(0.1),
    bump(0.045, 0.13, -0.12, 0.012, 0.03, 0.004).deep(0.05),
    bump(0.03, 0.06, -0.08, 0.03, 0.015, -0.004).deep(0.08),
  ]
}

fn eyeball(person: &Person) -> Mesh {
  let slit = person.race == Race::Saurian;
  let sclera = match person.race {
    Race::Saurian => person.iris,
    Race::AshElf => Srgba::new(0.55, 0.3, 0.28, 1.0),
    _ => Srgba::new(0.8, 0.76, 0.7, 1.0)
  };
  let iris = person.iris;
  let mesh: Mesh = Sphere::new(1.0).mesh().uv(28, 20);
  let colors: Vec<[f32; 4]> = mesh
    .attribute(Mesh::ATTRIBUTE_POSITION)
    .and_then(|values| values.as_float3())
    .map(|values| {
      values
        .iter()
        .map(|&at| {
          let at = Vec3::from(at);
          let facing = -at.z;
          let pupil = match slit {
            true => (at.x.abs() < 0.12 && facing > 0.5) as i32 as f32,
            false => smoothstep(0.972, 0.98, facing)
          };
          let ring = smoothstep(0.84, 0.86, facing);
          let rim = 1.0 - 0.5 * smoothstep(0.92, 0.85, facing) * ring;
          let color = lin(sclera).mix(&(lin(iris) * rim), ring);
          color.mix(&LinearRgba::rgb(0.01, 0.01, 0.01), pupil).to_f32_array()
        })
        .collect()
    })
    .unwrap_or_default();
  mesh.with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
}

fn ear(person: &Person) -> Mesh {
  let keys: Vec<(Vec3, Vec3)> = match person.visage.ear {
    Ear::Round => vec![
      (Vec3::new(0.0, -0.028, 0.004), Vec3::new(0.006, 0.004, 0.004)),
      (Vec3::new(0.004, -0.02, 0.006), Vec3::new(0.013, 0.007, 0.006)),
      (Vec3::new(0.007, 0.0, 0.01), Vec3::new(0.019, 0.008, 0.005)),
      (Vec3::new(0.008, 0.02, 0.012), Vec3::new(0.017, 0.008, 0.005)),
      (Vec3::new(0.006, 0.03, 0.01), Vec3::new(0.006, 0.004, 0.003)),
    ],
    Ear::Pointed => vec![
      (Vec3::new(0.0, -0.026, 0.004), Vec3::new(0.006, 0.004, 0.004)),
      (Vec3::new(0.005, -0.016, 0.008), Vec3::new(0.012, 0.006, 0.005)),
      (Vec3::new(0.009, 0.004, 0.014), Vec3::new(0.016, 0.006, 0.004)),
      (Vec3::new(0.012, 0.024, 0.022), Vec3::new(0.01, 0.005, 0.004)),
      (Vec3::new(0.014, 0.038, 0.032), Vec3::new(0.002, 0.002, 0.002)),
    ],
    Ear::Long => vec![
      (Vec3::new(0.0, -0.024, 0.004), Vec3::new(0.006, 0.004, 0.004)),
      (Vec3::new(0.005, -0.012, 0.008), Vec3::new(0.012, 0.006, 0.005)),
      (Vec3::new(0.01, 0.01, 0.016), Vec3::new(0.014, 0.006, 0.004)),
      (Vec3::new(0.016, 0.034, 0.03), Vec3::new(0.009, 0.004, 0.003)),
      (Vec3::new(0.022, 0.058, 0.05), Vec3::new(0.002, 0.002, 0.002)),
    ]
  };
  sculpt(&keys, 5, 10)
}

fn hairline(style: Style, angle: f32) -> f32 {
  let around = angle.abs();
  match matches!(style, Style::Long | Style::Braided) {
    true => table(
      &[
        (0.0, 0.212),
        (0.55, 0.207),
        (0.9, 0.195),
        (1.2, 0.165),
        (1.5, 0.1),
        (2.3, 0.07),
        (PI, 0.065)
      ],
      around
    ),
    false => table(
      &[
        (0.0, 0.212),
        (0.55, 0.207),
        (0.9, 0.195),
        (1.1, 0.172),
        (1.2, 0.135),
        (1.3, 0.135),
        (1.4, 0.175),
        (1.8, 0.175),
        (2.3, 0.075),
        (PI, 0.068)
      ],
      around
    )
  }
}

fn beard_line(angle: f32) -> f32 {
  table(
    &[(0.0, 0.103), (0.3, 0.11), (0.75, 0.122), (1.15, 0.14), (1.32, 0.142), (1.42, 0.0)],
    angle.abs()
  )
}

fn beard_depth(beard: Beard, at: Vec3, angle: f32) -> f32 {
  let below = smoothstep(beard_line(angle) + 0.002, beard_line(angle) - 0.016, at.y);
  let lips = smoothstep(0.009, 0.001, (at.y - 0.0875).abs() - 0.006)
    * smoothstep(0.032, 0.018, at.x.abs())
    * (at.z < 0.0) as i32 as f32;
  let chin = smoothstep(0.1, 0.05, at.y) * smoothstep(0.05, 0.02, at.x.abs());
  let shape = below * (1.0 - lips);
  match beard {
    Beard::Stubble => 0.0,
    Beard::Goatee => {
      shape
        * smoothstep(0.038, 0.012, at.x.abs())
        * (smoothstep(0.08, 0.07, at.y) + 0.6 * smoothstep(0.094, 0.1, at.y))
        * 0.009
    }
    Beard::Full => shape * (0.008 + 0.012 * chin),
    Beard::Long => shape * (0.01 + 0.018 * chin)
  }
}

pub fn head(kit: &mut Kit, person: &Person, build: &Build) {
  let saurian = person.race == Race::Saurian;
  let visage =
    Visage { hollow: person.visage.hollow + (1.0 - build.cheeks) * 6.0, ..person.visage };
  let person = &Person { visage, ..*person };
  let skull =
    Skull::shaped(saurian.then_some(&SAURIAN[..]).unwrap_or(&HUMAN[..]), &visage);
  let bumps =
    saurian.then(|| saurian_features(person)).unwrap_or_else(|| features(person));
  let eye_rim =
    bump(0.032, 0.158, -0.09, 0.019, 0.0062, 1.0).sharp(4.0).tilted(visage.slant);
  let outward = |at: Vec3| (at - Vec3::new(0.0, 0.16, 0.0)).normalize_or(Vec3::Y);
  let lift = |at: Vec3| bumps.iter().map(|bump| bump.lift(at)).sum::<f32>();
  let sculpted = |at: Vec3| at + at.with_y(0.0).normalize_or(Vec3::Z) * lift(at);
  let skin = lin(person.skin);
  let mane = lin(person.mane);
  let seed = person.seed;
  let lip_tone = match person.race {
    Race::Orc => tint(lin(person.skin), 0.8, 0.72, 0.8),
    Race::AshElf | Race::SunElf => tint(lin(person.skin), 0.9, 0.78, 0.8),
    _ => tint(lin(person.skin), 0.95, 0.7, 0.68)
  };
  let paint = |at: Vec3| -> LinearRgba {
    let mottle = 1.0 + 0.12 * noise::fbm3(at * 60.0, 3, seed);
    let side = Vec2::new(at.x.abs(), at.y);
    let front = smoothstep(-0.02, -0.06, at.z);
    let near = |x: f32, y: f32, wide: f32, high: f32| {
      (-((side - Vec2::new(x, y)) / Vec2::new(wide, high)).length_squared()).exp() * front
    };
    let lips = near(0.0, 0.0875, 0.018, 0.0075).clamp(0.0, 1.0);
    let brows =
      (1.6 * near(0.03, 0.176, 0.02, 0.0045) * (1.0 - smoothstep(0.052, 0.058, side.x)))
        .clamp(0.0, 1.0)
        * (!saurian) as i32 as f32;
    let flush = near(0.045, 0.12, 0.02, 0.02)
      * 0.25
      * (person.race == Race::Northman) as i32 as f32;
    let shadow = near(0.032, 0.158, 0.02, 0.012)
      * match (person.woman, person.race) {
        (_, Race::AshElf) | (true, _) => 0.55,
        (_, Race::SunElf) => 0.35,
        _ => 0.15
      };
    let stubble = person.beard.filter(|_| !saurian).map_or(0.0, |_| {
      smoothstep(
        beard_line(at.x.atan2(-at.z)) + 0.004,
        beard_line(at.x.atan2(-at.z)) - 0.01,
        at.y
      ) * front.max(0.3)
        * 0.5
    });
    let ink = person.paint.map_or(0.0, |(paint, _)| {
      let (x, y) = (at.x, at.y);
      let mark = match paint {
        Paint::HalfFace => smoothstep(-0.004, 0.004, x) * smoothstep(0.1, 0.11, y),
        Paint::EyeBand => smoothstep(0.014, 0.01, (y - 0.158).abs()),
        Paint::Claws => (0..3)
          .map(|stripe| {
            let lane = 0.04 + stripe as f32 * 0.01;
            smoothstep(0.004, 0.002, (x - lane + (y - 0.13) * 0.4).abs())
              * smoothstep(0.08, 0.1, y)
              * smoothstep(0.155, 0.145, y)
          })
          .sum(),
        Paint::Chevron => {
          smoothstep(0.006, 0.003, (y - 0.195 + x.abs() * 0.9).abs())
            * smoothstep(0.13, 0.14, y)
        }
        Paint::Tears => {
          smoothstep(0.004, 0.002, (x.abs() - 0.034).abs())
            * smoothstep(0.155, 0.145, y)
            * smoothstep(0.09, 0.1, y)
        }
        Paint::Sigil => near(0.0, 0.205, 0.008, 0.012) / front.max(0.01),
        Paint::Stripes => {
          (0..2)
            .map(|stripe| {
              smoothstep(
                0.004,
                0.002,
                (y - 0.2 - stripe as f32 * 0.012 + x.abs() * 0.2).abs()
              )
            })
            .sum::<f32>()
            + smoothstep(0.004, 0.002, x.abs())
              * smoothstep(0.215, 0.205, y)
              * smoothstep(0.16, 0.17, y)
        }
      };
      mark.clamp(0.0, 1.0) * front
    });
    let cavity = (lift(at) * 28.0).clamp(-0.45, 0.12);
    let liner = (eye_rim.weight(at) * (1.0 - eye_rim.weight(at))).clamp(0.0, 0.25)
      * 2.4
      * (!saurian) as i32 as f32;
    let base = skin * mottle * (1.0 + cavity) * (1.0 - liner);
    let base = saurian
      .then(|| {
        skin
          * (0.85 + 0.35 * noise::value3(at * 180.0, seed + 3))
          * (1.0 + 0.6 * smoothstep(0.09, 0.03, at.y))
      })
      .unwrap_or(base);
    let toned = base.mix(&(tint(base, 1.05, 0.8, 0.75)), flush);
    let toned = toned.mix(&lip_tone, lips * (!saurian) as i32 as f32);
    let toned = toned * (1.0 - shadow * 0.5);
    let toned = toned.mix(&(mane * 0.55), brows * 0.9);
    let toned = toned.mix(&(toned * 0.62), stubble);
    let toned =
      person.paint.map_or(toned, |(_, color)| toned.mix(&lin(color), ink * 0.85));
    toned.with_alpha(1.0)
  };
  let face_mesh = skull.surface(|at, _| sculpted(at), paint);
  let stuff = saurian.then_some(Stuff::Scales).unwrap_or(Stuff::Skin);
  kit.add(Joint::Head, stuff, skinned(face_mesh));
  let neck = model::loft(
    &[
      model::Hoop::new(-0.05, 0.062, 0.058, 0.062).shifted(0.0, 0.008),
      model::Hoop::new(0.02, 0.054, 0.05, 0.054).shifted(0.0, 0.0),
      model::Hoop::new(0.07, 0.05, 0.046, 0.05).shifted(0.0, -0.005),
      model::Hoop::new(0.11, 0.046, 0.04, 0.05).shifted(0.0, 0.005)
    ]
    .map(|hoop| {
      hoop.scaled(build.limbs.sqrt() * (person.woman.then_some(0.86).unwrap_or(1.0)))
    }),
    20
  );
  kit.add(Joint::Head, stuff, Piece::new(neck, person.skin));
  let socket = Vec3::new(0.032, 0.158, 0.0);
  let eye_center = socket.with_z(skull.nearest(socket.with_z(-0.12)).z);
  let eye_center = saurian
    .then(|| {
      let side = skull.nearest(Vec3::new(0.09, 0.158, -0.06));
      side - side.with_y(0.0).normalize() * 0.012
    })
    .unwrap_or(eye_center + Vec3::Z * 0.0135);
  let eye_size = saurian.then_some(0.0155).unwrap_or(0.0125);
  let eye_stuff = person.glowing.then_some(Stuff::Frost).unwrap_or(Stuff::Gloss);
  let turn = saurian.then_some(0.7).unwrap_or(0.0);
  kit.both(
    Joint::Head,
    eye_stuff,
    skinned(eyeball(person))
      .sized(Vec3::splat(eye_size))
      .yawed(-turn)
      .at(eye_center.with_x(eye_center.x.abs()))
  );
  (!saurian).then(|| {
    let side = skull.nearest(Vec3::new(0.1, 0.14, 0.004));
    kit.both(
      Joint::Head,
      Stuff::Skin,
      Piece::new(ear(person), person.skin * 0.94)
        .yawed(-0.25)
        .rolled(-0.12)
        .at(side + Vec3::new(-0.004, 0.0, 0.0))
    );
  });
  (person.visage.tusks > 0.0).then(|| {
    let size = person.visage.tusks;
    let root = skull.nearest(Vec3::new(0.022, 0.079, -0.12));
    let path = curve(
      Vec3::ZERO,
      Vec3::new(0.002, 0.016, -0.006) * size,
      Vec3::new(0.007, 0.03, 0.0) * size,
      8
    );
    kit.both(
      Joint::Head,
      Stuff::Bone,
      Piece::new(
        tube(&path, &taper(8, 0.0065 * size.sqrt(), 0.0012), 8),
        Srgba::new(0.85, 0.8, 0.66, 1.0)
      )
      .at(root + Vec3::new(0.0, 0.0, -0.002))
    );
  });
  saurian.then(|| crest(kit, person, &skull));
  let hair_mask = |at: Vec3, angle: f32| {
    let line = hairline(person.style, angle);
    smoothstep(line - 0.002, line + 0.018, at.y)
  };
  let shell_paint = |grain: f32, at: Vec3| {
    let streak = 0.8
      + 0.4 * noise::value3(Vec3::new(at.x * 300.0, at.y * 30.0, at.z * 300.0), seed + 1);
    (mane * streak * grain).with_alpha(1.0)
  };
  (!saurian && person.style != Style::Bald).then(|| {
    let style = person.style;
    let hair = skull.within(0.06, 1.0).surface(
      |at, angle| {
        let sculpted = sculpted(at);
        let mask = hair_mask(at, angle);
        let mohawk = (style == Style::Mohawk)
          .then(|| smoothstep(0.026, 0.016, at.x.abs()))
          .unwrap_or(1.0);
        let crown = smoothstep(0.19, 0.27, at.y);
        let thick = match style {
          Style::Mohawk => 0.006 + 0.03 * crown,
          Style::Cropped => 0.004 + 0.002 * crown,
          _ => 0.007 + 0.007 * crown
        };
        let strands =
          1.0 + 0.35 * noise::fbm3(Vec3::new(angle * 22.0, at.y * 4.0, 0.0), 3, seed + 2);
        let cover = mask * mohawk;
        let depth = cover * thick * strands - 0.003 * (1.0 - cover);
        sculpted + outward(at) * depth
      },
      |at| shell_paint(1.0, at)
    );
    kit.add(Joint::Head, Stuff::Fur, skinned(hair));
    locks(kit, person, &skull);
  });
  (!saurian).then(|| {
    person.beard.filter(|&beard| beard != Beard::Stubble).map(|beard| {
      let whiskers = skull.within(0.0, 0.15).surface(
        |at, angle| {
          let sculpted = sculpted(at);
          let depth = beard_depth(beard, at, angle);
          let bristle = 1.0
            + 0.4 * noise::fbm3(Vec3::new(angle * 30.0, at.y * 60.0, 0.0), 2, seed + 4);
          sculpted
            + outward(at) * (depth * bristle - 0.003 * (depth <= 0.0005) as i32 as f32)
            + Vec3::NEG_Y * depth * 0.6 * smoothstep(0.1, 0.05, at.y)
        },
        |at| shell_paint(0.95, at)
      );
      kit.add(Joint::Head, Stuff::Fur, skinned(whiskers));
      (beard == Beard::Long).then(|| {
        let chin = skull.nearest(Vec3::new(0.0, 0.05, -0.12));
        let mesh = sculpt(
          &[
            (chin + Vec3::new(0.0, 0.03, 0.02), Vec3::new(0.04, 0.02, 0.02)),
            (chin + Vec3::new(0.0, -0.01, 0.0), Vec3::new(0.042, 0.022, 0.02)),
            (chin + Vec3::new(0.0, -0.06, 0.005), Vec3::new(0.034, 0.018, 0.016)),
            (chin + Vec3::new(0.0, -0.11, 0.015), Vec3::new(0.02, 0.012, 0.01)),
            (chin + Vec3::new(0.0, -0.14, 0.022), Vec3::new(0.004, 0.004, 0.004))
          ],
          5,
          16
        );
        kit.add(
          Joint::Head,
          Stuff::Fur,
          Piece::new(
            model::ruffled(mesh, 0.006, Vec3::new(160.0, 12.0, 160.0), seed),
            person.mane * 0.95
          )
        );
      });
    });
  });
}

fn locks(kit: &mut Kit, person: &Person, skull: &Skull) {
  let seed = person.seed;
  let color = person.mane;
  let hang = |length: f32| {
    let mesh = model::loft(
      &[
        model::Hoop::new(0.235, 0.07, 0.0, 0.09).shifted(0.0, 0.012),
        model::Hoop::new(0.17, 0.088, 0.0, 0.11).shifted(0.0, 0.004),
        model::Hoop::new(0.1, 0.086, 0.0, 0.1).shifted(0.0, 0.018),
        model::Hoop::new(0.03, 0.08, 0.0, 0.075).shifted(0.0, 0.045),
        model::Hoop::new(0.03 - length, 0.072, 0.0, 0.04).shifted(0.0, 0.07),
        model::Hoop::pole(0.02 - length).shifted(0.0, 0.075)
      ],
      28
    );
    Piece::new(model::ruffled(mesh, 0.01, Vec3::new(140.0, 5.0, 140.0), seed), color)
  };
  let braid = |side: f32| {
    let top = skull.nearest(Vec3::new(0.075 * side, 0.12, -0.01));
    let path: Vec<Vec3> = (0..=14)
      .map(|step| {
        let t = step as f32 / 14.0;
        top
          + Vec3::new(0.012 * side * t, -0.2 * t, -0.012 * t)
          + Vec3::new((t * 40.0).sin() * 0.003, 0.0, (t * 40.0).cos() * 0.003)
      })
      .collect();
    Piece::new(tube(&path, &taper(15, 0.011, 0.006), 10), color * 0.92)
  };
  match person.style {
    Style::Long => {
      kit.add(Joint::Head, Stuff::Fur, hang(0.14 + 0.1 * person.woman as i32 as f32));
    }
    Style::Braided => {
      kit
        .add(Joint::Head, Stuff::Fur, hang(0.08))
        .add(Joint::Head, Stuff::Fur, braid(1.0))
        .add(Joint::Head, Stuff::Fur, braid(-1.0));
    }
    Style::Topknot => {
      let crown = skull.nearest(Vec3::new(0.0, 0.3, 0.04));
      kit
        .add(
          Joint::Head,
          Stuff::Fur,
          Piece::new(model::lump(seed, 0.2, 2), color)
            .sized(Vec3::new(0.022, 0.03, 0.022))
            .at(crown + Vec3::new(0.0, 0.03, 0.01))
        )
        .add(
          Joint::Head,
          Stuff::Leather,
          Piece::new(model::rod(0.016, 0.012), Srgba::new(0.3, 0.2, 0.12, 1.0))
            .at(crown + Vec3::new(0.0, 0.014, 0.008))
        );
    }
    Style::Swept => {
      kit.add(Joint::Head, Stuff::Fur, hang(0.02));
    }
    _ => {}
  }
}

fn crest(kit: &mut Kit, person: &Person, skull: &Skull) {
  let horn = person.mane.mix(&Srgba::new(0.8, 0.74, 0.6, 1.0), 0.7);
  let mut roll = Roll::new(person.seed * 5 + 1);
  let count = 2 + roll.below(3);
  for index in 0..count {
    let t = index as f32 / count as f32;
    let root =
      skull.nearest(Vec3::new(0.045 - t * 0.02, 0.21 - t * 0.04, 0.03 + t * 0.05));
    let length = roll.range(0.05, 0.1) * (1.0 - t * 0.3);
    let path = curve(
      Vec3::ZERO,
      Vec3::new(0.01, 0.4, 0.6) * length,
      Vec3::new(0.02, 0.35 + roll.spread(0.3), 1.0) * length,
      8
    );
    kit.both(
      Joint::Head,
      Stuff::Bone,
      Piece::new(tube(&path, &taper(8, 0.009, 0.0015), 8), horn).at(root)
    );
  }
  roll.chance(0.6).then(|| {
    let frill = person.mane;
    for index in 0..5 {
      let t = index as f32 / 4.0;
      let root = skull.nearest(Vec3::new(0.0, 0.24 - t * 0.08, 0.02 + t * 0.08));
      kit.add(
        Joint::Head,
        Stuff::Membrane,
        Piece::new(
          model::fan(
            &[
              Vec2::new(0.0, -0.01),
              Vec2::new(0.05, 0.0),
              Vec2::new(0.06, 0.02),
              Vec2::new(0.0, 0.012)
            ],
            0.003
          ),
          frill
        )
        .yawed(FRAC_PI_2)
        .pitched(0.8 + t * 0.6)
        .at(root)
      );
    }
  });
}

#[cfg(test)]
pub(crate) mod tests {
  use {super::*,
       crate::humanoid::{self, Build, Calling, MAN, WOMAN},
       bevy::mesh::VertexAttributeValues,
       std::{io::Write, path::Path}};

  pub(crate) fn dump(path: &str, parts: Vec<(usize, Stuff, Mesh)>) {
    let floats = |mesh: &Mesh, attribute| match mesh.attribute(attribute) {
      Some(VertexAttributeValues::Float32x3(values)) => values.concat(),
      Some(VertexAttributeValues::Float32x4(values)) => values.concat(),
      _ => Vec::new()
    };
    let mut bytes = Vec::new();
    bytes.extend((parts.len() as u32).to_le_bytes());
    for (_, stuff, mesh) in parts.iter() {
      let indices: Vec<u32> = mesh
        .indices()
        .map(|indices| indices.iter().map(|index| index as u32).collect())
        .unwrap_or_default();
      let stuff = Stuff::ALL.iter().position(|each| each == stuff).unwrap_or(0) as u32;
      bytes.extend(stuff.to_le_bytes());
      bytes.extend((mesh.count_vertices() as u32).to_le_bytes());
      bytes.extend((indices.len() as u32).to_le_bytes());
      for attribute in
        [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL, Mesh::ATTRIBUTE_COLOR]
      {
        bytes
          .extend(floats(mesh, attribute).iter().flat_map(|value| value.to_le_bytes()));
      }
      bytes.extend(indices.iter().flat_map(|index| index.to_le_bytes()));
    }
    Path::new(path).parent().map(|folder| std::fs::create_dir_all(folder).unwrap());
    std::fs::File::create(path).unwrap().write_all(&bytes).unwrap();
  }

  #[test]
  #[ignore]
  fn heads() {
    let races = [Race::Northman, Race::Orc, Race::SunElf, Race::AshElf, Race::Saurian];
    for seed in
      0..std::env::var("SEEDS").ok().and_then(|seeds| seeds.parse().ok()).unwrap_or(2u32)
    {
      for race in races {
        for woman in [false, true] {
          let person = Person::roll(race, woman, seed);
          let mut kit = Kit::new();
          head(&mut kit, &person, &Build::HALE);
          let frame = woman.then_some(WOMAN).unwrap_or(MAN);
          dump(
            &format!(
              "screenshots/heads/{race:?}-{}-{seed}.bin",
              woman.then_some("f").unwrap_or("m")
            ),
            humanoid::tailor(kit, &frame)
          );
        }
      }
    }
    let wight = humanoid::wight(1);
    dump("screenshots/heads/wight.bin", humanoid::tailor(wight, &MAN));
    let farmer =
      humanoid::villager(Calling::Farmer, &Person::roll(Race::Northman, false, 3));
    dump("screenshots/heads/farmer.bin", humanoid::tailor(farmer, &MAN));
  }
}
