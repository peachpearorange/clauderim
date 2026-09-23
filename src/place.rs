use {bevy::prelude::*, enum_assoc::Assoc};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Marker {
  Cave,
  Barrow,
  Tower,
  Stone,
  Camp
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Assoc)]
#[func(pub const fn name(self) -> &'static str)]
#[func(pub const fn spot(self) -> Vec2)]
#[func(pub const fn marker(self) -> Marker)]
#[func(pub const fn flat(self) -> f32)]
#[func(pub const fn sunk(self) -> f32 { 0.0 })]
pub enum Place {
  #[assoc(name = "Hollowcrag Barrow", spot = Vec2::new(-250.0, -170.0), marker = Marker::Barrow, flat = 38.0, sunk = 14.0)]
  Hollowcrag,
  #[assoc(name = "Fellhound Den", spot = Vec2::new(300.0, -210.0), marker = Marker::Cave, flat = 26.0, sunk = 10.0)]
  Fellhound,
  #[assoc(name = "Greymoor Watch", spot = Vec2::new(175.0, 70.0), marker = Marker::Tower, flat = 16.0)]
  Greymoor,
  #[assoc(name = "The Warrior Stone", spot = Vec2::new(-10.0, 70.0), marker = Marker::Stone, flat = 11.0)]
  WarriorStone,
  #[assoc(name = "Rotfen Camp", spot = Vec2::new(-150.0, 20.0), marker = Marker::Camp, flat = 14.0)]
  Rotfen
}

impl Place {
  pub const ALL: [Place; 5] =
    [Place::Hollowcrag, Place::Fellhound, Place::Greymoor, Place::WarriorStone, Place::Rotfen];
}

pub const START: Vec2 = Vec2::new(70.0, 290.0);
pub const START_FACING: Vec2 = Vec2::new(-0.25, -1.0);

pub const LAKE: Vec2 = Vec2::new(-150.0, 230.0);
pub const LAKE_RADIUS: f32 = 95.0;
pub const LAKE_LEVEL: f32 = 4.0;

pub const ROAD: [Vec2; 9] = [
  Vec2::new(90.0, 360.0),
  Vec2::new(60.0, 270.0),
  Vec2::new(20.0, 170.0),
  Vec2::new(-5.0, 85.0),
  Vec2::new(-70.0, 30.0),
  Vec2::new(-140.0, -60.0),
  Vec2::new(-200.0, -120.0),
  Vec2::new(-232.0, -148.0),
  Vec2::new(-240.0, -156.0)
];

pub const TRAIL: [Vec2; 6] = [
  Vec2::new(-5.0, 85.0),
  Vec2::new(70.0, 90.0),
  Vec2::new(150.0, 80.0),
  Vec2::new(215.0, 10.0),
  Vec2::new(270.0, -120.0),
  Vec2::new(290.0, -188.0)
];

pub fn road_distance(at: Vec2) -> f32 {
  [&ROAD[..], &TRAIL[..]]
    .into_iter()
    .flat_map(|path| path.windows(2))
    .map(|pair| {
      let (a, b) = (pair[0], pair[1]);
      let along = ((at - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
      at.distance(a + (b - a) * along)
    })
    .fold(f32::MAX, f32::min)
}
