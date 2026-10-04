use {crate::{noise::Roll,
             place::{self, Marker, Spot},
             river,
             terrain::{self, BOUND}},
     bevy::{platform::collections::HashSet, prelude::*},
     std::{f32::consts::TAU, sync::LazyLock}};

const STEP: f32 = 165.0;
const APART: f32 = 120.0;
const PIT: f32 = 12.0;
const PIT_DEPTH: f32 = 3.4;
const UPLAND: f32 = 170.0;
const SUMMIT: f32 = 430.0;
const PEAK: f32 = 800.0;
const TRIES: usize = 10;

const LOWLAND: [(Marker, f32); 9] = [
  (Marker::Camp, 3.0),
  (Marker::Shack, 2.0),
  (Marker::Farm, 2.0),
  (Marker::Ruin, 2.0),
  (Marker::Cave, 2.0),
  (Marker::Barrow, 0.4),
  (Marker::Tower, 1.5),
  (Marker::Shrine, 1.0),
  (Marker::Stone, 1.0)
];

const HIGHLAND: [(Marker, f32); 5] = [
  (Marker::Ruin, 2.0),
  (Marker::Cave, 2.0),
  (Marker::Shrine, 1.5),
  (Marker::Tower, 1.0),
  (Marker::Barrow, 2.5)
];

const FIRST: [&str; 24] = [
  "Raven", "Frost", "Bleak", "Ash", "Wolf", "Iron", "Grey", "Black", "Silver", "Bear",
  "Elk", "Thorn", "Mist", "Storm", "Crow", "Rime", "Ember", "Dusk", "Cold", "Bitter",
  "Hawk", "Pine", "Old", "Stone"
];

const SECOND: [&str; 20] = [
  "heath", "fell", "crag", "wood", "vale", "mire", "garth", "brook", "scar", "holm",
  "beck", "gate", "tooth", "ridge", "water", "hollow", "shade", "field", "mark", "fen"
];

const PATRONS: [&str; 12] = [
  "Wanderer", "Hunter", "Raven", "Wolf", "Elk", "Eagle", "Oak", "Frost", "Serpent",
  "Bear", "Mother", "Smith"
];

fn flat(marker: Marker) -> f32 {
  match marker {
    Marker::Farm => 30.0,
    Marker::Ruin => 22.0,
    Marker::Barrow => 16.0,
    Marker::Shrine => 8.0,
    Marker::Stone => 11.0,
    Marker::Shack => 13.0,
    _ => 14.0
  }
}

fn title(marker: Marker, base: &str) -> String {
  match marker {
    Marker::Camp => format!("{base} Camp"),
    Marker::Shack => format!("{base} Shack"),
    Marker::Farm => format!("{base} Farm"),
    Marker::Tower => format!("{base} Watch"),
    Marker::Barrow => format!("{base} Barrow"),
    Marker::Cave => format!("{base} Cave"),
    Marker::Ruin => format!("{base} Ruins"),
    Marker::Shrine => format!("{base} Shrine"),
    _ => format!("The {base} Stone")
  }
}

fn pick(choices: &[(Marker, f32)], roll: &mut Roll) -> Marker {
  let aim = roll.next() * choices.iter().map(|&(_, weight)| weight).sum::<f32>();
  choices
    .iter()
    .scan(0.0, |upto, &(marker, weight)| {
      *upto += weight;
      Some((marker, *upto))
    })
    .find(|&(_, upto)| upto >= aim)
    .map_or(choices[0].0, |(marker, _)| marker)
}

fn steepness(at: Vec2, reach: f32) -> f32 {
  let middle = terrain::natural_height(at);
  (0..8)
    .map(|step| {
      let toward = Vec2::from_angle(step as f32 / 8.0 * TAU) * reach;
      (terrain::natural_height(at + toward) - middle).abs() / reach
    })
    .fold(0.0, f32::max)
}

fn gentle(marker: Marker) -> f32 {
  match marker {
    Marker::Farm => 0.2,
    Marker::Ruin => 0.32,
    Marker::Barrow => 0.42,
    _ => 0.4
  }
}

fn preference(marker: Marker, at: Vec2, steep: f32) -> f32 {
  match marker {
    Marker::Barrow => terrain::natural_height(at),
    _ => -steep
  }
}

fn uncarved(at: Vec2, reach: f32) -> bool {
  (0..=8)
    .map(|step| {
      at + Vec2::from_angle(step as f32 / 8.0 * TAU) * reach * (step > 0) as u8 as f32
    })
    .all(|spot| {
      let height = terrain::natural_height(spot);
      river::carve(spot, height) > height - 1.0
    })
}

fn welcoming(at: Vec2, marker: Marker) -> bool {
  let room = flat(marker);
  terrain::natural_height(at)
    < (marker == Marker::Barrow).then_some(PEAK).unwrap_or(SUMMIT)
    && place::named().all(|place| at.distance(place.spot()) > place.flat() * 2.5 + 110.0)
    && place::nearest_main_road(at).edge() > room + 12.0
    && river::course_distance(at) > room + 30.0
    && river::lake_near(at, 2.3).is_none()
    && uncarved(at, room * 1.9)
}

fn named(marker: Marker, roll: &mut Roll, used: &HashSet<String>) -> String {
  let base = |roll: &mut Roll| match marker {
    Marker::Stone => PATRONS[roll.below(PATRONS.len())].to_string(),
    _ => format!("{}{}", FIRST[roll.below(FIRST.len())], SECOND[roll.below(SECOND.len())])
  };
  let fresh = |name: &String| {
    !used.contains(name)
      && place::named().all(|place| !place.name().contains(name.as_str()))
  };
  (0..12).map(|_| base(roll)).find(fresh).unwrap_or_else(|| {
    format!("{}{}", base(roll), ["heim", "stad", "vik"][roll.below(3)])
  })
}

pub static SITES: LazyLock<Vec<Spot>> = LazyLock::new(|| {
  let span = BOUND - 150.0;
  let cells = (2.0 * span / STEP).floor() as i32;
  let candidates: Vec<(Vec2, Marker, u32)> = (0..cells * cells)
    .filter_map(|cell| {
      let seed = cell as u32 * 7919 + 101;
      let mut roll = Roll::new(seed);
      let corner = Vec2::new((cell % cells) as f32, (cell / cells) as f32) * STEP - span;
      let middle = corner + Vec2::splat(STEP / 2.0);
      let marker = pick(
        (terrain::natural_height(middle) > UPLAND)
          .then_some(&HIGHLAND[..])
          .unwrap_or(&LOWLAND[..]),
        &mut roll
      );
      (0..TRIES)
        .map(|_| corner + Vec2::new(roll.range(0.1, 0.9), roll.range(0.1, 0.9)) * STEP)
        .filter(|&at| welcoming(at, marker))
        .map(|at| (at, steepness(at, flat(marker) * 1.4)))
        .filter(|&(_, steep)| steep < gentle(marker))
        .map(|(at, steep)| (at, preference(marker, at, steep)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(at, _)| (at, marker, seed))
    })
    .collect();
  let kept = candidates.into_iter().fold(
    Vec::new(),
    |mut kept: Vec<(Vec2, Marker, u32)>, site| {
      if kept.iter().all(|&(other, ..)| other.distance(site.0) > APART) {
        kept.push(site);
      }
      kept
    }
  );
  kept
    .into_iter()
    .fold((Vec::new(), HashSet::new()), |(mut sites, mut used), (at, marker, seed)| {
      let mut roll = Roll::new(seed ^ 0x5eed);
      let base = named(marker, &mut roll, &used);
      used.insert(base.clone());
      let name = title(marker, &base).leak();
      sites.push(
        (marker == Marker::Barrow && roll.chance(0.35))
          .then(|| Spot::new(name, at, marker, PIT).sunk(PIT_DEPTH))
          .unwrap_or_else(|| Spot::new(name, at, marker, flat(marker)))
      );
      (sites, used)
    })
    .0
});

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[ignore]
  fn census() {
    println!("{} sites", SITES.len());
    for site in SITES.iter() {
      println!(
        "{:?} {} {} {} {}",
        site.marker,
        site.name,
        site.at,
        site.sunk,
        terrain::natural_height(site.at)
      )
    }
  }
}
