use {bevy::{platform::collections::HashMap, prelude::*},
     std::sync::LazyLock};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Marker {
  Cave,
  Barrow,
  Tower,
  Stone,
  Camp,
  Town,
  City,
  Farm,
  Fort,
  Ruin,
  Shack,
  Shrine
}

#[derive(Clone, Copy, Debug)]
pub struct Spot {
  pub name: &'static str,
  pub at: Vec2,
  pub marker: Marker,
  pub flat: f32,
  pub sunk: f32,
  pub rise: f32
}

impl Spot {
  pub const fn new(name: &'static str, at: Vec2, marker: Marker, flat: f32) -> Self {
    Spot { name, at, marker, flat, sunk: 0.0, rise: 0.0 }
  }

  pub const fn sunk(self, sunk: f32) -> Self { Spot { sunk, ..self } }

  const fn rise(self, rise: f32) -> Self { Spot { rise, ..self } }
}

const NAMED: [Spot; 15] = [
  Spot::new("Hollowcrag Barrow", Vec2::new(-250.0, -170.0), Marker::Barrow, 36.0)
    .sunk(9.0),
  Spot::new("Fellhound Den", Vec2::new(302.8, -216.2), Marker::Cave, 24.0).sunk(6.0),
  Spot::new("Greymoor Watch", Vec2::new(175.0, 70.0), Marker::Tower, 16.0),
  Spot::new("The Warrior Stone", Vec2::new(-10.0, 70.0), Marker::Stone, 11.0),
  Spot::new("Rotfen Camp", Vec2::new(-150.0, 20.0), Marker::Camp, 14.0),
  Spot::new("Kjeldholm", Vec2::new(2080.0, 420.0), Marker::City, 100.0).rise(26.0),
  Spot::new("Brookhollow", Vec2::new(330.0, 1330.0), Marker::Town, 62.0),
  Spot::new("Frostmere", Vec2::new(-1950.0, -620.0), Marker::Town, 60.0),
  Spot::new("Aldvik Farm", Vec2::new(1660.0, 730.0), Marker::Farm, 36.0),
  Spot::new("Hallgrim Farm", Vec2::new(610.0, 1190.0), Marker::Farm, 32.0),
  Spot::new("Fort Greyhelm", Vec2::new(-1300.0, -1720.0), Marker::Fort, 44.0),
  Spot::new("Fort Skarn", Vec2::new(1910.0, -1320.0), Marker::Fort, 44.0),
  Spot::new("Blackbriar Camp", Vec2::new(560.0, 800.0), Marker::Camp, 14.0),
  Spot::new("Wolfskull Camp", Vec2::new(-880.0, 420.0), Marker::Camp, 14.0),
  Spot::new("Snowgate Watch", Vec2::new(-640.0, -40.0), Marker::Tower, 16.0)
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Place(u16);

impl Place {
  pub const HOLLOWCRAG: Place = Place(0);
  pub const FELLHOUND: Place = Place(1);
  pub const GREYMOOR: Place = Place(2);
  pub const WARRIOR_STONE: Place = Place(3);
  pub const ROTFEN: Place = Place(4);
  pub const KJELDHOLM: Place = Place(5);
  pub const BROOKHOLLOW: Place = Place(6);
  pub const FROSTMERE: Place = Place(7);
  pub const ALDVIK: Place = Place(8);
  pub const HALLGRIM: Place = Place(9);
  pub const GREYHELM: Place = Place(10);
  pub const SKARN: Place = Place(11);
  pub const BLACKBRIAR: Place = Place(12);
  pub const WOLFSKULL: Place = Place(13);
  pub const SNOWGATE: Place = Place(14);

  fn info(self) -> &'static Spot {
    let index = usize::from(self.0);
    NAMED.get(index).unwrap_or_else(|| &crate::site::SITES[index - NAMED.len()])
  }

  pub fn name(self) -> &'static str { self.info().name }

  pub fn spot(self) -> Vec2 { self.info().at }

  pub fn marker(self) -> Marker { self.info().marker }

  pub fn flat(self) -> f32 { self.info().flat }

  pub fn sunk(self) -> f32 { self.info().sunk }

  pub fn rise(self) -> f32 { self.info().rise }

  pub fn clearance(self) -> f32 {
    match self.marker() {
      Marker::Town | Marker::City | Marker::Farm | Marker::Fort => 1.25,
      _ => 1.4
    }
  }
}

pub fn named() -> impl Iterator<Item = Place> { (0..NAMED.len() as u16).map(Place) }

pub fn sites() -> impl Iterator<Item = Place> {
  (NAMED.len() as u16..(NAMED.len() + crate::site::SITES.len()) as u16).map(Place)
}

pub fn all() -> impl Iterator<Item = Place> {
  (0..(NAMED.len() + crate::site::SITES.len()) as u16).map(Place)
}

const AROUND_BIN: f32 = 128.0;

static AROUND: LazyLock<HashMap<IVec2, Vec<Place>>> = LazyLock::new(|| {
  all().fold(HashMap::default(), |mut bins, place| {
    let reach = place.flat() * 2.5 + 16.0;
    let low = ((place.spot() - reach) / AROUND_BIN).floor().as_ivec2();
    let high = ((place.spot() + reach) / AROUND_BIN).floor().as_ivec2();
    (low.y..=high.y).for_each(|y| {
      (low.x..=high.x).for_each(|x| bins.entry(IVec2::new(x, y)).or_default().push(place))
    });
    bins
  })
});

pub fn around(at: Vec2) -> &'static [Place] {
  AROUND.get(&(at / AROUND_BIN).floor().as_ivec2()).map_or(&[], Vec::as_slice)
}

pub const START: Vec2 = Vec2::new(70.0, 290.0);
pub const START_FACING: Vec2 = Vec2::new(-0.25, -1.0);

pub const ROAD: [Vec2; 9] = [
  Vec2::new(90.0, 360.0),
  Vec2::new(60.0, 270.0),
  Vec2::new(20.0, 170.0),
  Vec2::new(-5.0, 85.0),
  Vec2::new(-70.0, 30.0),
  Vec2::new(-140.0, -60.0),
  Vec2::new(-200.0, -120.0),
  Vec2::new(-205.0, -125.0),
  Vec2::new(-215.0, -135.0)
];

pub const TRAIL: [Vec2; 6] = [
  Vec2::new(-5.0, 85.0),
  Vec2::new(70.0, 90.0),
  Vec2::new(150.0, 80.0),
  Vec2::new(215.0, 10.0),
  Vec2::new(270.0, -120.0),
  Vec2::new(290.0, -188.0)
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paving {
  Dirt,
  Stone
}

impl Paving {
  pub const fn half_width(self) -> f32 {
    match self {
      Paving::Dirt => 2.6,
      Paving::Stone => 3.4
    }
  }
}

pub struct Road {
  pub paving: Paving,
  pub path: Vec<Vec2>
}

const SOUTH_ROAD: [Vec2; 8] = [
  Vec2::new(90.0, 360.0),
  Vec2::new(104.0, 520.0),
  Vec2::new(110.0, 700.0),
  Vec2::new(140.0, 880.0),
  Vec2::new(185.0, 1040.0),
  Vec2::new(250.0, 1160.0),
  Vec2::new(300.0, 1255.0),
  Vec2::new(330.0, 1330.0)
];

const EAST_ROAD: [Vec2; 10] = [
  Vec2::new(60.0, 270.0),
  Vec2::new(180.0, 300.0),
  Vec2::new(420.0, 330.0),
  Vec2::new(700.0, 370.0),
  Vec2::new(1000.0, 420.0),
  Vec2::new(1300.0, 470.0),
  Vec2::new(1600.0, 485.0),
  Vec2::new(1850.0, 455.0),
  Vec2::new(1985.0, 425.0),
  Vec2::new(2080.0, 420.0)
];

const WEST_ROAD: [Vec2; 11] = [
  Vec2::new(-5.0, 85.0),
  Vec2::new(-80.0, 100.0),
  Vec2::new(-250.0, 110.0),
  Vec2::new(-450.0, 60.0),
  Vec2::new(-610.0, 5.0),
  Vec2::new(-800.0, -45.0),
  Vec2::new(-1050.0, -150.0),
  Vec2::new(-1350.0, -300.0),
  Vec2::new(-1650.0, -450.0),
  Vec2::new(-1860.0, -570.0),
  Vec2::new(-1950.0, -620.0)
];

const GREYHELM_TRAIL: [Vec2; 5] = [
  Vec2::new(-1050.0, -150.0),
  Vec2::new(-1130.0, -470.0),
  Vec2::new(-1220.0, -870.0),
  Vec2::new(-1290.0, -1270.0),
  Vec2::new(-1300.0, -1660.0)
];

const SKARN_TRAIL: [Vec2; 6] = [
  Vec2::new(1300.0, 470.0),
  Vec2::new(1420.0, 150.0),
  Vec2::new(1560.0, -250.0),
  Vec2::new(1720.0, -650.0),
  Vec2::new(1850.0, -1000.0),
  Vec2::new(1905.0, -1265.0)
];

const ALDVIK_TRAIL: [Vec2; 3] =
  [Vec2::new(1600.0, 485.0), Vec2::new(1625.0, 610.0), Vec2::new(1660.0, 730.0)];

const HALLGRIM_TRAIL: [Vec2; 3] =
  [Vec2::new(300.0, 1255.0), Vec2::new(450.0, 1215.0), Vec2::new(610.0, 1190.0)];

const BLACKBRIAR_TRAIL: [Vec2; 3] =
  [Vec2::new(140.0, 880.0), Vec2::new(380.0, 860.0), Vec2::new(548.0, 806.0)];

const WOLFSKULL_TRAIL: [Vec2; 3] =
  [Vec2::new(-450.0, 60.0), Vec2::new(-650.0, 250.0), Vec2::new(-868.0, 412.0)];

pub fn smoothed(path: &[Vec2]) -> Vec<Vec2> {
  let key = |index: isize| path[index.clamp(0, path.len() as isize - 1) as usize];
  (0..path.len() as isize - 1)
    .flat_map(|segment| {
      let [before, from, to, after] = [-1, 0, 1, 2].map(|offset| key(segment + offset));
      let steps = (from.distance(to) / 10.0).ceil().max(1.0) as usize;
      (0..steps).map(move |step| {
        let t = step as f32 / steps as f32;
        0.5
          * (2.0 * from
            + (to - before) * t
            + (2.0 * before - 5.0 * from + 4.0 * to - after) * t * t
            + (3.0 * from - before - 3.0 * to + after) * t * t * t)
      })
    })
    .chain(path.last().copied())
    .collect()
}

fn wandered(path: Vec<Vec2>) -> Vec<Vec2> {
  path
    .into_iter()
    .map(|at| {
      let settled = named()
        .map(|place| {
          crate::terrain::smooth(
            place.flat() * 1.3,
            place.flat() * 1.3 + 160.0,
            at.distance(place.spot())
          )
        })
        .fold(1.0, f32::min);
      let sway = |seed: u32| {
        crate::noise::fbm(at / 420.0, 2, seed) * 60.0
          + crate::noise::fbm(at / 110.0, 2, seed + 7) * 12.0
      };
      at + Vec2::new(sway(701), sway(703)) * settled
    })
    .collect()
}

pub static ROADS: LazyLock<Vec<Road>> = LazyLock::new(|| {
  let road = |paving, path: &[Vec2]| Road { paving, path: wandered(smoothed(path)) };
  [
    road(Paving::Dirt, &ROAD),
    road(Paving::Dirt, &TRAIL),
    road(Paving::Stone, &SOUTH_ROAD),
    road(Paving::Stone, &EAST_ROAD),
    road(Paving::Stone, &WEST_ROAD),
    road(Paving::Dirt, &GREYHELM_TRAIL),
    road(Paving::Dirt, &SKARN_TRAIL),
    road(Paving::Dirt, &ALDVIK_TRAIL),
    road(Paving::Dirt, &HALLGRIM_TRAIL),
    road(Paving::Dirt, &BLACKBRIAR_TRAIL),
    road(Paving::Dirt, &WOLFSKULL_TRAIL)
  ]
  .into_iter()
  .chain(crate::settlement::streets())
  .collect()
});

#[derive(Clone, Copy)]
struct Segment {
  from: Vec2,
  to: Vec2,
  paving: Paving
}

#[derive(Clone, Copy, Debug)]
pub struct Nearest {
  pub distance: f32,
  pub point: Vec2,
  pub paving: Paving
}

impl Nearest {
  const NONE: Nearest =
    Nearest { distance: REACH, point: Vec2::ZERO, paving: Paving::Dirt };

  pub fn edge(self) -> f32 { self.distance - self.paving.half_width() }
}

const BIN: f32 = 128.0;
pub const REACH: f32 = 420.0;

struct Bins {
  near: HashMap<IVec2, Vec<Segment>>,
  wide: HashMap<IVec2, Vec<Segment>>
}

fn binned(segments: &[Segment], reach: f32) -> HashMap<IVec2, Vec<Segment>> {
  segments.iter().fold(HashMap::default(), |mut bins, &segment| {
    let low = ((segment.from.min(segment.to) - reach) / BIN).floor().as_ivec2();
    let high = ((segment.from.max(segment.to) + reach) / BIN).floor().as_ivec2();
    (low.y..=high.y).for_each(|y| {
      (low.x..=high.x)
        .for_each(|x| bins.entry(IVec2::new(x, y)).or_default().push(segment))
    });
    bins
  })
}

static BINS: LazyLock<Bins> = LazyLock::new(|| {
  let segments: Vec<Segment> = ROADS
    .iter()
    .flat_map(|road| {
      road.path.windows(2).map(|pair| Segment {
        from: pair[0],
        to: pair[1],
        paving: road.paving
      })
    })
    .collect();
  Bins { near: binned(&segments, 24.0), wide: binned(&segments, REACH) }
});

fn nearest_in(bins: &HashMap<IVec2, Vec<Segment>>, at: Vec2) -> Nearest {
  bins.get(&(at / BIN).floor().as_ivec2()).map_or(Nearest::NONE, |segments| {
    segments.iter().fold(Nearest::NONE, |best, &Segment { from, to, paving }| {
      let along =
        ((at - from).dot(to - from) / (to - from).length_squared()).clamp(0.0, 1.0);
      let point = from + (to - from) * along;
      let distance = at.distance(point);
      (distance < best.distance)
        .then_some(Nearest { distance, point, paving })
        .unwrap_or(best)
    })
  })
}

pub fn nearest_road(at: Vec2) -> Nearest { nearest_in(&BINS.near, at) }

pub fn road_distance(at: Vec2) -> f32 {
  (nearest_road(at).edge() + Paving::Dirt.half_width()).min(24.0)
}

pub fn route_distance(at: Vec2) -> f32 { nearest_in(&BINS.wide, at).distance }
