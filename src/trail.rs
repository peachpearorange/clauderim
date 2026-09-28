use {crate::{river, terrain},
     bevy::{platform::collections::HashMap, prelude::*},
     std::{cmp::Reverse, collections::BinaryHeap}};

const CELL: f32 = 14.0;
const TURN: f32 = 30.0;
const FORD: f32 = 180.0;
const EAGERNESS: f32 = 1.4;

const MOVES: [IVec2; 16] = [
  IVec2::new(1, 0),
  IVec2::new(-1, 0),
  IVec2::new(0, 1),
  IVec2::new(0, -1),
  IVec2::new(1, 1),
  IVec2::new(1, -1),
  IVec2::new(-1, 1),
  IVec2::new(-1, -1),
  IVec2::new(2, 1),
  IVec2::new(2, -1),
  IVec2::new(-2, 1),
  IVec2::new(-2, -1),
  IVec2::new(1, 2),
  IVec2::new(-1, 2),
  IVec2::new(1, -2),
  IVec2::new(-1, -2)
];

pub struct Climb {
  pub from: Vec2,
  pub reach: f32,
  pub steepest: f32
}

fn chaikin(path: Vec<Vec2>) -> Vec<Vec2> {
  path
    .first()
    .copied()
    .into_iter()
    .chain(
      path
        .windows(2)
        .flat_map(|pair| [pair[0].lerp(pair[1], 0.25), pair[0].lerp(pair[1], 0.75)])
    )
    .chain(path.last().copied())
    .collect()
}

impl Climb {
  pub fn toward(&self, remaining: impl Fn(Vec2) -> f32) -> Option<Vec<Vec2>> {
    let &Climb { from, reach, steepest } = self;
    let origin = (from / CELL).round().as_ivec2();
    let spot = |cell: IVec2| cell.as_vec2() * CELL;
    let mut heights: HashMap<IVec2, f32> = HashMap::default();
    let mut height = |cell: IVec2| {
      *heights.entry(cell).or_insert_with(|| terrain::natural_height(spot(cell)))
    };
    let passable = |cell: IVec2| {
      let at = spot(cell);
      at.distance(from) < reach
        && at.abs().max_element() < terrain::BOUND - 40.0
        && river::lake_near(at, 1.1).is_none()
    };
    let mut best: HashMap<IVec2, (f32, IVec2)> = HashMap::default();
    let mut open = BinaryHeap::new();
    best.insert(origin, (0.0, origin));
    open.push((Reverse(0_u32), origin.to_array()));
    let key = |cost: f32| Reverse((cost * 10.0) as u32);
    let mut arrived = None;
    while let Some((_, cell)) =
      open.pop().map(|(key, cell)| (key, IVec2::from_array(cell)))
      && arrived.is_none()
    {
      let (spent, before) = best[&cell];
      let heading = (cell - before).as_vec2().normalize_or_zero();
      if remaining(spot(cell)) < CELL {
        arrived = Some(cell);
      }
      let here = height(cell);
      for step in MOVES {
        let next = cell + step;
        let length = step.as_vec2().length() * CELL;
        let there = height(next);
        let halfway = terrain::natural_height((spot(cell) + spot(next)) / 2.0);
        let grade = ((halfway - here).abs().max((there - halfway).abs())) * 2.0 / length;
        let fording = river::course_distance(spot(next)) < 8.0;
        let turning =
          TURN * (1.0 - heading.dot(step.as_vec2().normalize())) * heading.length();
        let cost = spent
          + turning
          + length * (1.0 + 4.0 * (grade / steepest).powi(2))
          + fording.then_some(FORD).unwrap_or(0.0);
        if grade <= steepest
          && passable(next)
          && best.get(&next).is_none_or(|&(known, _)| cost < known)
        {
          best.insert(next, (cost, cell));
          open.push((key(cost + remaining(spot(next)) * EAGERNESS), next.to_array()));
        }
      }
    }
    arrived.map(|end| {
      let cells: Vec<IVec2> = std::iter::successors(Some(end), |cell| {
        Some(best[cell].1).filter(|&before| before != *cell)
      })
      .collect();
      chaikin(chaikin(cells.into_iter().rev().map(spot).collect()))
    })
  }
}
