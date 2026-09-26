use {crate::{landmark::{self, Flicker},
             model::{Piece, block, lathe, rod},
             noise::Roll,
             place::{Marker, Paving, Place, Road},
             player::MainCamera,
             signal::{FoeKind, Pending},
             stuff::{Stuff, Stuffs},
             terrain::{height_at, smooth, srgb}},
     avian3d::prelude::*,
     bevy::{light::NotShadowCaster,
            platform::collections::HashMap,
            prelude::*,
            tasks::{AsyncComputeTaskPool, Task, futures::check_ready}},
     std::{f32::consts::{FRAC_PI_2, TAU},
           sync::LazyLock}};

pub const SOIL: LinearRgba = srgb(0.26, 0.20, 0.15);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Roof {
  Thatch,
  Shingle,
  Gilded
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Walls {
  Timber,
  Logs,
  Stone
}

#[derive(Clone, Copy, Debug)]
pub struct House {
  pub at: Vec2,
  pub facing: f32,
  pub length: f32,
  pub depth: f32,
  pub tall: f32,
  pub roof: Roof,
  pub walls: Walls
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Crop {
  Cabbage,
  Wheat
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clutter {
  Barrels,
  Crates,
  Hay,
  Woodpile,
  Cart,
  Stall
}

#[derive(Clone, Debug)]
pub enum Work {
  House(House),
  Keep { at: Vec2, facing: f32 },
  Windmill { at: Vec2, facing: f32 },
  Well { at: Vec2 },
  Fence { path: Vec<Vec2> },
  Field { at: Vec2, facing: f32, size: Vec2, crop: Crop },
  Rampart { from: Vec2, to: Vec2, tall: f32 },
  Tower { at: Vec2, radius: f32, tall: f32 },
  Gate { at: Vec2, facing: f32, tall: f32 },
  Clutter { at: Vec2, facing: f32, kind: Clutter },
  Tent { at: Vec2, facing: f32 },
  Mill { at: Vec2, facing: f32 },
  Fire { at: Vec2 },
  Mound { at: Vec2, facing: f32, radius: f32 },
  Den { at: Vec2, facing: f32 },
  Shrine { at: Vec2, facing: f32 },
  Menhirs { at: Vec2 },
  Pillar { at: Vec2, tall: f32 },
  Rubble { at: Vec2 }
}

pub struct Layout {
  pub place: Place,
  pub works: Vec<Work>,
  pub streets: Vec<Road>,
  pub worn: Vec<(Vec2, f32)>,
  pub fields: Vec<(Vec2, f32, Vec2)>,
  pub foes: Vec<(Vec2, FoeKind)>
}

fn segment_gap(at: Vec2, from: Vec2, to: Vec2) -> f32 {
  let along = ((at - from).dot(to - from) / (to - from).length_squared()).clamp(0.0, 1.0);
  at.distance(from + (to - from) * along)
}

fn facing_toward(toward: Vec2) -> f32 { f32::atan2(toward.x, toward.y) }

fn local(facing: f32, offset: Vec2) -> Vec2 { Vec2::from_angle(-facing).rotate(offset) }

struct Plan {
  roll: Roll,
  center: Vec2,
  reach: f32,
  works: Vec<Work>,
  taken: Vec<(Vec2, f32)>,
  streets: Vec<(Vec2, Vec2)>,
  worn: Vec<(Vec2, f32)>
}

impl Plan {
  fn new(place: Place, seed: u32, reach: f32) -> Self {
    Plan {
      roll: Roll::new(seed),
      center: place.spot(),
      reach,
      works: Vec::new(),
      taken: Vec::new(),
      streets: Vec::new(),
      worn: Vec::new()
    }
  }

  fn clear(&self, at: Vec2, radius: f32, kerb: f32) -> bool {
    at.distance(self.center) + radius < self.reach
      && self.taken.iter().all(|&(spot, room)| spot.distance(at) > room + radius)
      && self.streets.iter().all(|&(from, to)| segment_gap(at, from, to) > kerb + 2.5)
      && crate::river::course_distance(at) > radius + 9.0
  }

  fn free(&self, at: Vec2, radius: f32) -> bool { self.clear(at, radius, radius) }

  fn claim(&mut self, at: Vec2, radius: f32, work: Work) -> bool {
    let free = self.free(at, radius);
    if free {
      self.taken.push((at, radius));
      self.works.push(work);
    }
    free
  }

  fn street(&mut self, from: Vec2, to: Vec2) { self.streets.push((from, to)); }

  fn line_houses(&mut self, from: Vec2, to: Vec2, roof: Roof, walls: Walls, size: f32) {
    let along = (to - from).normalize();
    let length = from.distance(to);
    [-1.0, 1.0].into_iter().for_each(|side| {
      let mut travelled = 12.0 + self.roll.range(0.0, 4.0);
      while travelled < length {
        let (house_length, depth) =
          (self.roll.range(9.0, 13.5) * size, self.roll.range(6.5, 8.2) * size);
        let setback = depth / 2.0 + self.roll.range(4.5, 6.5);
        let at = from + along * travelled + along.perp() * side * setback;
        let house = House {
          at,
          facing: facing_toward(-along.perp() * side),
          length: house_length,
          depth,
          tall: self.roll.range(2.9, 3.5),
          roof,
          walls
        };
        let radius = 0.5 * Vec2::new(house_length, depth).length();
        let fits = self.clear(at, radius * 0.85, depth / 2.0 + 1.0);
        if fits {
          self.taken.push((at, radius * 0.85));
          self.works.push(Work::House(house));
          self.worn.push((at - along.perp() * side * (depth / 2.0 + 2.0), 4.0));
          let behind = at + along.perp() * side * (depth / 2.0 + 3.5);
          let kind = [Clutter::Barrels, Clutter::Woodpile, Clutter::Hay, Clutter::Crates]
            [self.roll.below(4)];
          let spot = behind + along * self.roll.spread(house_length * 0.3);
          if self.roll.chance(0.7) && self.free(spot, 1.2) {
            self.works.push(Work::Clutter {
              at: spot,
              facing: self.roll.range(0.0, TAU),
              kind
            });
          }
        }
        travelled += house_length + self.roll.range(4.0, 9.0);
      }
    });
  }

  fn finish(
    self,
    place: Place,
    paving: Paving,
    fields: Vec<(Vec2, f32, Vec2)>,
    foes: Vec<(Vec2, FoeKind)>
  ) -> Layout {
    Layout {
      place,
      works: self.works,
      streets: self
        .streets
        .into_iter()
        .map(|(from, to)| Road { paving, path: vec![from, to] })
        .collect(),
      worn: self.worn,
      fields,
      foes
    }
  }
}

fn town(place: Place, seed: u32, entry: Vec2, roof: Roof, walls: Walls) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.2);
  let center = plan.center;
  let entry = entry.normalize();
  let cross = entry.perp().rotate(Vec2::from_angle(plan.roll.spread(0.35)));
  let reach = place.flat() * 1.05;
  let ends = [
    center + entry * reach,
    center - entry * reach * 0.8,
    center + cross * reach * 0.85,
    center - cross * reach * 0.7
  ];
  ends.iter().for_each(|&end| plan.street(center, end));
  plan.worn.push((center, 15.0));
  if let Some(bank) = crate::river::closest_point(center)
    .filter(|bank| bank.distance(center) < place.flat() * 1.6)
  {
    let inward = (center - bank).normalize();
    let at = bank + inward * 12.0;
    plan.works.push(Work::Mill { at, facing: facing_toward(inward.perp()) });
    plan.taken.push((at, 9.0));
    plan.worn.push((at + inward * 5.0, 5.0));
  }
  plan.works.push(Work::Well { at: center + cross * 5.0 });
  plan.taken.push((center, 9.0));
  let hall_at = center - entry * 17.0 + cross * 16.0;
  let hall = House {
    at: hall_at,
    facing: facing_toward(center - hall_at),
    length: 22.0,
    depth: 11.0,
    tall: 4.2,
    roof,
    walls: Walls::Timber
  };
  plan.claim(hall_at, 12.5, Work::House(hall));
  ends.into_iter().for_each(|end| plan.line_houses(center, end, roof, walls, 1.0));
  (0..6).for_each(|_| {
    let angle = plan.roll.range(0.0, TAU);
    let at = center + Vec2::from_angle(angle) * plan.roll.range(9.0, 14.0);
    let kind = [Clutter::Stall, Clutter::Cart, Clutter::Barrels][plan.roll.below(3)];
    if plan.free(at, 1.6) {
      plan.works.push(Work::Clutter { at, facing: facing_toward(center - at), kind });
    }
  });
  plan.finish(place, Paving::Dirt, Vec::new(), Vec::new())
}

fn city(place: Place, seed: u32, entry: Vec2) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 0.82);
  let center = plan.center;
  let entry = entry.normalize();
  let ring_reach = place.flat() * 0.88;
  let corners = 16;
  let gate_side = (0..corners)
    .map(|index| (index, Vec2::from_angle((index as f32 + 0.5) / corners as f32 * TAU)))
    .max_by(|a, b| a.1.dot(entry).total_cmp(&b.1.dot(entry)))
    .map_or(0, |(index, _)| index);
  let ring: Vec<Vec2> = (0..corners)
    .map(|index| {
      let angle = index as f32 / corners as f32 * TAU;
      center + Vec2::from_angle(angle) * ring_reach * (1.0 + 0.05 * (angle * 3.0).sin())
    })
    .collect();
  (0..corners).for_each(|index| {
    let (from, to) = (ring[index], ring[(index + 1) % corners]);
    match index == gate_side {
      true => {
        let middle = (from + to) / 2.0;
        let along = (to - from).normalize();
        plan.works.push(Work::Rampart { from, to: middle - along * 4.5, tall: 8.0 });
        plan.works.push(Work::Rampart { from: middle + along * 4.5, to, tall: 8.0 });
        plan.works.push(Work::Gate {
          at: middle,
          facing: facing_toward(middle - center),
          tall: 8.0
        });
      }
      false => plan.works.push(Work::Rampart { from, to, tall: 8.0 })
    }
    if index % 2 == 0 {
      plan.works.push(Work::Tower { at: from, radius: 3.6, tall: 11.0 });
    }
  });
  let keep_at = center - entry * 38.0;
  plan.works.push(Work::Keep { at: keep_at, facing: facing_toward(entry) });
  plan.taken.push((keep_at, 30.0));
  let gate = (ring[gate_side] + ring[(gate_side + 1) % corners]) / 2.0;
  plan.street(gate, center);
  plan.street(center, keep_at + entry * 24.0);
  let cross = entry.perp();
  plan.street(center - cross * 60.0, center + cross * 60.0);
  plan.worn.push((center, 16.0));
  plan.works.push(Work::Well { at: center });
  plan.taken.push((center, 8.0));
  let lanes: Vec<Vec2> = (0..=12)
    .map(|step| center + Vec2::from_angle(step as f32 / 12.0 * TAU + 0.26) * 56.0)
    .collect();
  lanes.windows(2).for_each(|pair| plan.street(pair[0], pair[1]));
  let streets = plan.streets.clone();
  streets.into_iter().for_each(|(from, to)| {
    plan.line_houses(from, to, Roof::Gilded, Walls::Timber, 1.1);
    plan.line_houses(to, from, Roof::Gilded, Walls::Timber, 1.1);
  });
  (0..10).for_each(|_| {
    let at =
      center + Vec2::from_angle(plan.roll.range(0.0, TAU)) * plan.roll.range(8.0, 15.0);
    if plan.free(at, 1.8) {
      plan.works.push(Work::Clutter {
        at,
        facing: facing_toward(center - at),
        kind: Clutter::Stall
      });
    }
  });
  plan.finish(place, Paving::Stone, Vec::new(), Vec::new())
}

fn farm(place: Place, seed: u32, entry: Vec2, windmill: bool) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let entry = entry.normalize();
  let side = entry.perp();
  let yard = center + entry * 6.0;
  plan.worn.push((yard, 11.0));
  let farmhouse = House {
    at: center - side * 12.0 + entry * 2.0,
    facing: facing_toward(side),
    length: 11.0,
    depth: 7.0,
    tall: 3.0,
    roof: Roof::Shingle,
    walls: Walls::Logs
  };
  plan.claim(farmhouse.at, 7.0, Work::House(farmhouse));
  let barn = House {
    at: center + side * 13.0 + entry * 3.0,
    facing: facing_toward(-side),
    length: 13.0,
    depth: 8.5,
    tall: 3.8,
    roof: Roof::Thatch,
    walls: Walls::Timber
  };
  plan.claim(barn.at, 8.0, Work::House(barn));
  if windmill {
    let mill_at = center - entry * 16.0 - side * 14.0;
    plan.claim(mill_at, 5.0, Work::Windmill {
      at: mill_at,
      facing: facing_toward(entry + side)
    });
  }
  plan.works.push(Work::Well { at: yard + side * 3.0 });
  let facing = facing_toward(entry);
  let fields: Vec<(Vec2, f32, Vec2)> = [
    (center - entry * 14.0 + side * 6.0, Vec2::new(14.0, 12.0), Crop::Cabbage),
    (center - entry * 30.0 - side * 4.0, Vec2::new(18.0, 13.0), Crop::Wheat),
    (center - entry * 30.0 + side * 17.0, Vec2::new(12.0, 13.0), Crop::Cabbage)
  ]
  .into_iter()
  .map(|(at, size, crop)| {
    plan.works.push(Work::Field { at, facing, size, crop });
    let corner = |x: f32, z: f32| {
      at + local(facing, Vec2::new(x * size.x / 2.0 + x, z * size.y / 2.0 + z))
    };
    plan.works.push(Work::Fence {
      path: vec![
        corner(0.2, 1.0),
        corner(1.0, 1.0),
        corner(1.0, -1.0),
        corner(-1.0, -1.0),
        corner(-1.0, 1.0),
        corner(-0.2, 1.0),
      ]
    });
    (at, facing, size)
  })
  .collect();
  [Clutter::Hay, Clutter::Hay, Clutter::Cart, Clutter::Woodpile, Clutter::Barrels]
    .into_iter()
    .for_each(|kind| {
      let at = yard + Vec2::new(plan.roll.spread(9.0), plan.roll.spread(6.0));
      if plan.free(at, 1.5) {
        plan.works.push(Work::Clutter { at, facing: plan.roll.range(0.0, TAU), kind });
      }
    });
  plan.street(center + entry * place.flat(), yard);
  plan.finish(place, Paving::Dirt, fields, Vec::new())
}

fn fort(place: Place, seed: u32, entry: Vec2, towers: [f32; 4], keep: f32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat());
  let center = plan.center;
  let entry = entry.normalize();
  let facing = facing_toward(entry);
  let half = Vec2::new(19.0, 16.0);
  let corner = |x: f32, z: f32| center + local(facing, Vec2::new(x * half.x, z * half.y));
  let corners =
    [corner(-1.0, 1.0), corner(1.0, 1.0), corner(1.0, -1.0), corner(-1.0, -1.0)];
  let front = (corners[0] + corners[1]) / 2.0;
  let along = (corners[1] - corners[0]).normalize();
  plan.works.push(Work::Rampart { from: corners[0], to: front - along * 3.5, tall: 8.0 });
  plan.works.push(Work::Rampart { from: front + along * 3.5, to: corners[1], tall: 8.0 });
  plan.works.push(Work::Gate { at: front, facing, tall: 8.0 });
  (1..4).for_each(|side| {
    plan.works.push(Work::Rampart {
      from: corners[side],
      to: corners[(side + 1) % 4],
      tall: 8.0
    })
  });
  corners
    .into_iter()
    .zip(towers)
    .for_each(|(at, tall)| plan.works.push(Work::Tower { at, radius: 5.0, tall }));
  let keep_at = corner(0.0, -0.45);
  plan.works.push(Work::Tower { at: keep_at, radius: 6.0, tall: keep });
  let fire = center + entry * 3.0;
  plan.works.push(Work::Fire { at: fire });
  plan.works.push(Work::Tent { at: corner(-0.6, 0.1), facing: facing + FRAC_PI_2 });
  plan.works.push(Work::Tent { at: corner(0.62, 0.05), facing: facing - FRAC_PI_2 });
  [
    (-0.75, 0.6, Clutter::Crates),
    (0.75, 0.6, Clutter::Barrels),
    (0.5, -0.55, Clutter::Woodpile)
  ]
  .into_iter()
  .for_each(|(x, z, kind)| {
    plan.works.push(Work::Clutter {
      at: corner(x, z),
      facing: plan.roll.range(0.0, TAU),
      kind
    })
  });
  plan.worn.push((center, 17.0));
  plan.street(center + entry * place.flat() * 1.2, front);
  let foes = vec![
    (fire + along * 2.5, FoeKind::Bandit),
    (fire - along * 2.5, FoeKind::Bandit),
    (corner(-0.4, -0.2), FoeKind::Bandit),
    (corner(0.4, -0.2), FoeKind::Bandit),
    (front - entry * 3.0, FoeKind::Bandit),
    (keep_at + entry * 8.0, FoeKind::BanditChief),
  ];
  plan.finish(place, Paving::Dirt, Vec::new(), foes)
}

fn around_fire(center: Vec2, count: usize, reach: f32, turn: f32) -> Vec<(Vec2, f32)> {
  (0..count)
    .map(|index| {
      let angle = turn + index as f32 / count as f32 * TAU;
      let at = center + Vec2::from_angle(angle) * reach;
      (at, facing_toward(center - at))
    })
    .collect()
}

fn camp(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let turn = plan.roll.range(0.0, TAU);
  let tents = 2 + plan.roll.below(2);
  plan.works.push(Work::Fire { at: center });
  around_fire(center, tents, 5.5, turn).into_iter().for_each(|(at, facing)| {
    plan.works.push(Work::Tent { at, facing: facing + FRAC_PI_2 })
  });
  [Clutter::Crates, Clutter::Barrels, Clutter::Woodpile].into_iter().for_each(|kind| {
    let at =
      center + Vec2::from_angle(plan.roll.range(0.0, TAU)) * plan.roll.range(7.5, 9.5);
    plan.works.push(Work::Clutter { at, facing: plan.roll.range(0.0, TAU), kind });
  });
  plan.worn.push((center, 9.0));
  let bandits = 2 + plan.roll.below(3);
  let chief = plan.roll.chance(0.35);
  let foes = around_fire(center, bandits, 2.6, turn + 0.6)
    .into_iter()
    .map(|(at, _)| (at, FoeKind::Bandit))
    .chain(chief.then_some((center + Vec2::from_angle(turn) * 8.0, FoeKind::BanditChief)))
    .collect();
  plan.finish(place, Paving::Dirt, Vec::new(), foes)
}

fn shack(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let facing = plan.roll.range(0.0, TAU);
  let roof = [Roof::Thatch, Roof::Shingle][plan.roll.below(2)];
  plan.works.push(Work::House(House {
    at: center,
    facing,
    length: plan.roll.range(7.0, 8.5),
    depth: plan.roll.range(5.2, 6.0),
    tall: 2.7,
    roof,
    walls: Walls::Logs
  }));
  let front = Vec2::from_angle(-facing).rotate(Vec2::Y);
  let side = front.perp();
  plan.works.push(Work::Clutter {
    at: center + side * 6.0 + front * 1.0,
    facing: facing + FRAC_PI_2,
    kind: Clutter::Woodpile
  });
  plan.works.push(Work::Fire { at: center + front * 7.0 - side * 2.0 });
  plan.works.push(Work::Clutter {
    at: center - side * 5.5 + front * 2.5,
    facing: plan.roll.range(0.0, TAU),
    kind: [Clutter::Barrels, Clutter::Crates, Clutter::Hay][plan.roll.below(3)]
  });
  plan.worn.push((center + front * 5.0, 6.0));
  plan.finish(place, Paving::Dirt, Vec::new(), Vec::new())
}

fn watch(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let turn = plan.roll.range(0.0, TAU);
  let tall = plan.roll.range(10.0, 14.0);
  plan.works.push(Work::Tower { at: center, radius: 3.6, tall });
  let fire = center + Vec2::from_angle(turn) * 7.5;
  plan.works.push(Work::Fire { at: fire });
  plan
    .works
    .push(Work::Tent { at: center + Vec2::from_angle(turn + 1.9) * 7.5, facing: turn });
  plan.works.push(Work::Clutter {
    at: center + Vec2::from_angle(turn - 1.6) * 6.5,
    facing: turn,
    kind: Clutter::Crates
  });
  plan.worn.push((center, 10.0));
  let foes = vec![
    (fire + Vec2::from_angle(turn + 1.0) * 2.4, FoeKind::Bandit),
    (fire + Vec2::from_angle(turn - 1.0) * 2.4, FoeKind::Bandit),
  ];
  plan.finish(place, Paving::Dirt, Vec::new(), foes)
}

fn ruin(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let turn = plan.roll.range(0.0, TAU);
  let reach = place.flat() * 0.7;
  let corners: Vec<Vec2> = (0..6)
    .map(|index| center + Vec2::from_angle(turn + index as f32 / 6.0 * TAU) * reach)
    .collect();
  (0..6).for_each(|side| {
    let (from, to) = (corners[side], corners[(side + 1) % 6]);
    if plan.roll.chance(0.65) {
      let cut = plan.roll.range(0.45, 0.85);
      plan.works.push(Work::Rampart {
        from,
        to: from.lerp(to, cut),
        tall: plan.roll.range(1.5, 4.5)
      });
    }
  });
  corners.iter().step_by(2).for_each(|&at| {
    if plan.roll.chance(0.6) {
      plan.works.push(Work::Tower { at, radius: 2.8, tall: plan.roll.range(3.5, 8.0) });
    }
  });
  (0..5).for_each(|_| {
    let at = center
      + Vec2::from_angle(plan.roll.range(0.0, TAU)) * plan.roll.range(2.0, reach * 0.8);
    plan.works.push(Work::Pillar { at, tall: plan.roll.range(1.2, 5.5) });
  });
  (0..6).for_each(|_| {
    let at = center
      + Vec2::from_angle(plan.roll.range(0.0, TAU)) * plan.roll.range(0.0, reach * 1.2);
    plan.works.push(Work::Rubble { at });
  });
  plan.worn.push((center, reach));
  let foes = around_fire(center, 2 + plan.roll.below(2), 4.0, turn)
    .into_iter()
    .map(|(at, _)| (at, FoeKind::Draugr))
    .collect();
  plan.finish(place, Paving::Dirt, Vec::new(), foes)
}

fn barrow(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let facing = plan.roll.range(0.0, TAU);
  let radius = place.flat() * 0.55;
  plan.works.push(Work::Mound { at: center, facing, radius });
  let front = Vec2::from_angle(-facing).rotate(Vec2::Y);
  [-1.0, 1.0].into_iter().for_each(|side| {
    plan.works.push(Work::Pillar {
      at: center + front * (radius + 4.0) + front.perp() * side * 3.0,
      tall: plan.roll.range(2.2, 3.2)
    })
  });
  plan.worn.push((center + front * (radius + 3.0), 4.0));
  let foes = (0..1 + plan.roll.below(2))
    .map(|index| {
      (
        center + front * (radius + 5.0) + front.perp() * (index as f32 * 2.5 - 1.0),
        FoeKind::Draugr
      )
    })
    .collect();
  plan.finish(place, Paving::Dirt, Vec::new(), foes)
}

fn lair(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let facing = plan.roll.range(0.0, TAU);
  plan.works.push(Work::Den { at: center, facing });
  let front = Vec2::from_angle(-facing).rotate(Vec2::Y);
  (0..3).for_each(|_| {
    let at =
      center + front * plan.roll.range(7.0, 11.0) + front.perp() * plan.roll.spread(6.0);
    plan.works.push(Work::Rubble { at });
  });
  plan.worn.push((center + front * 6.0, 5.0));
  let foes = (0..2 + plan.roll.below(2))
    .map(|index| {
      (
        center
          + front * (8.0 + index as f32 * 1.5)
          + front.perp() * plan.roll.spread(3.0),
        FoeKind::Wolf
      )
    })
    .collect();
  plan.finish(place, Paving::Dirt, Vec::new(), foes)
}

fn wayshrine(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  let facing = plan.roll.range(0.0, TAU);
  plan.works.push(Work::Shrine { at: center, facing });
  plan.worn.push((center, 4.5));
  plan.finish(place, Paving::Dirt, Vec::new(), Vec::new())
}

fn stones(place: Place, seed: u32) -> Layout {
  let mut plan = Plan::new(place, seed, place.flat() * 1.3);
  let center = plan.center;
  plan.works.push(Work::Menhirs { at: center });
  plan.worn.push((center, 7.5));
  plan.finish(place, Paving::Dirt, Vec::new(), Vec::new())
}

pub static LAYOUTS: LazyLock<Vec<Layout>> = LazyLock::new(|| {
  vec![
    city(Place::KJELDHOLM, 11, Vec2::new(-1.0, 0.05)),
    town(Place::BROOKHOLLOW, 23, Vec2::new(-0.25, -1.0), Roof::Thatch, Walls::Timber),
    town(Place::FROSTMERE, 37, Vec2::new(0.85, 0.5), Roof::Thatch, Walls::Stone),
    farm(Place::ALDVIK, 41, Vec2::new(-0.3, -1.0), true),
    farm(Place::HALLGRIM, 43, Vec2::new(-1.0, -0.25), false),
    fort(Place::GREYHELM, 53, Vec2::new(0.0, 1.0), [11.0, 11.0, 12.0, 12.0], 17.0),
    fort(Place::SKARN, 59, Vec2::new(-0.05, 1.0), [13.0, 10.0, 19.0, 11.0], 22.0),
  ]
});

pub static SITE_LAYOUTS: LazyLock<Vec<Layout>> = LazyLock::new(|| {
  crate::place::sites()
    .enumerate()
    .map(|(index, place)| {
      let seed = 1000 + index as u32 * 31;
      match place.marker() {
        Marker::Camp => camp(place, seed),
        Marker::Shack => shack(place, seed),
        Marker::Farm => {
          let entry = Vec2::from_angle(seed as f32 * 2.1);
          farm(place, seed, entry, index % 3 == 0)
        }
        Marker::Tower => watch(place, seed),
        Marker::Ruin => ruin(place, seed),
        Marker::Barrow => barrow(place, seed),
        Marker::Cave => lair(place, seed),
        Marker::Shrine => wayshrine(place, seed),
        _ => stones(place, seed)
      }
    })
    .collect()
});

pub fn layouts() -> impl Iterator<Item = &'static Layout> {
  LAYOUTS.iter().chain(SITE_LAYOUTS.iter())
}

fn layout(index: usize) -> &'static Layout {
  LAYOUTS.get(index).unwrap_or_else(|| &SITE_LAYOUTS[index - LAYOUTS.len()])
}

static LAYOUT_OF: LazyLock<HashMap<Place, usize>> = LazyLock::new(|| {
  layouts().enumerate().map(|(index, layout)| (layout.place, index)).collect()
});

fn layouts_around(at: Vec2) -> impl Iterator<Item = &'static Layout> {
  crate::place::around(at)
    .iter()
    .filter_map(|place| LAYOUT_OF.get(place))
    .map(|&index| layout(index))
    .filter(move |layout| near(layout, at))
}

pub fn streets() -> Vec<Road> {
  LAYOUTS
    .iter()
    .flat_map(|layout| {
      layout
        .streets
        .iter()
        .map(|road| Road { paving: road.paving, path: road.path.clone() })
    })
    .collect()
}

fn near(layout: &Layout, at: Vec2) -> bool {
  at.distance(layout.place.spot()) < layout.place.flat() * 1.6
}

pub fn worn(at: Vec2) -> f32 {
  layouts_around(at).fold(0.0, |worn: f32, layout| {
    layout.worn.iter().fold(worn, |worn, &(spot, radius)| {
      worn.max(smooth(radius, radius * 0.55, at.distance(spot)))
    })
  })
}

pub fn tilled(at: Vec2) -> f32 {
  layouts_around(at).fold(0.0, |tilled: f32, layout| {
    layout.fields.iter().fold(tilled, |tilled, &(spot, facing, size)| {
      let inside = (local(-facing, at - spot).abs() - size / 2.0).max_element();
      tilled.max(smooth(0.8, -0.4, inside))
    })
  })
}

const TIMBER: Srgba = Srgba::new(0.44, 0.35, 0.26, 1.0);
const BEAM: Srgba = Srgba::new(0.24, 0.18, 0.13, 1.0);
const STONEWORK: Srgba = Srgba::new(0.55, 0.55, 0.53, 1.0);
const STRAW: Srgba = Srgba::new(0.60, 0.53, 0.37, 1.0);
const SHINGLES: Srgba = Srgba::new(0.45, 0.41, 0.36, 1.0);
const GILT: Srgba = Srgba::new(0.66, 0.56, 0.32, 1.0);
const DOOR: Srgba = Srgba::new(0.20, 0.15, 0.11, 1.0);
const GLOOM: Srgba = Srgba::new(0.05, 0.05, 0.06, 1.0);
const WATTLE: Srgba = Srgba::new(0.42, 0.40, 0.36, 1.0);
const LEAF: Srgba = Srgba::new(0.42, 0.58, 0.34, 1.0);
const CANVAS: Srgba = Srgba::new(0.62, 0.55, 0.44, 1.0);

#[derive(Default)]
struct Works {
  parts: Vec<(Stuff, Piece)>,
  solids: Vec<(Vec3, Quat, Collider)>,
  spinners: Vec<(Transform, Vec<(Stuff, Piece)>)>,
  fires: Vec<Vec3>
}

impl Works {
  fn add(&mut self, stuff: Stuff, piece: Piece) { self.parts.push((stuff, piece)); }

  fn solid(&mut self, at: Vec3, turn: Quat, collider: Collider) {
    self.solids.push((at, turn, collider));
  }

  fn placed(self, frame: Transform) -> Works {
    Works {
      parts: self
        .parts
        .into_iter()
        .map(|(stuff, piece)| (stuff, piece.moved(frame)))
        .collect(),
      solids: self
        .solids
        .into_iter()
        .map(|(at, turn, collider)| {
          (frame.transform_point(at), frame.rotation * turn, collider)
        })
        .collect(),
      spinners: self
        .spinners
        .into_iter()
        .map(|(hub, parts)| (frame * hub, parts))
        .collect(),
      fires: self.fires.into_iter().map(|at| frame.transform_point(at)).collect()
    }
  }

  fn absorb(&mut self, other: Works) {
    self.parts.extend(other.parts);
    self.solids.extend(other.solids);
    self.spinners.extend(other.spinners);
    self.fires.extend(other.fires);
  }
}

struct Site {
  origin: Vec3,
  frame: Transform
}

impl Site {
  fn ground(&self, offset: Vec2) -> f32 {
    let world = self.origin + self.frame.transform_point(offset.extend(0.0).xzy());
    height_at(world.xz()) - self.origin.y - self.frame.translation.y
  }
}

type Ground<'a> = &'a dyn Fn(Vec2) -> f32;

fn spread(ground: Ground, half: Vec2) -> (f32, f32) {
  (-2..=2)
    .flat_map(|x| (-2..=2).map(move |z| Vec2::new(x as f32, z as f32) / 2.0 * half))
    .map(ground)
    .fold((f32::MAX, f32::MIN), |(low, high), height| (low.min(height), high.max(height)))
}

fn tinted(color: Srgba, roll: &mut Roll, spread: f32) -> Srgba {
  color * roll.range(1.0 - spread, 1.0 + spread)
}

fn slab(length: f32, thick: f32, width: f32, color: Srgba, tile: f32) -> Piece {
  Piece::new(block(length, thick, width), color).planar(tile)
}

fn beam(from: Vec3, to: Vec3, girth: f32, color: Srgba) -> Piece {
  Piece::new(block(girth, 1.0, girth), color).span(from, to)
}

fn house(house: &House, ground: Ground, roll: &mut Roll) -> Works {
  let House { length, depth, tall, roof, walls, .. } = *house;
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::new(length / 2.0 + 0.4, depth / 2.0 + 0.4));
  let floor = high + 0.35;
  let plinth = floor - low + 0.5;
  works.add(
    Stuff::Masonry,
    slab(length + 0.5, plinth, depth + 0.5, tinted(STONEWORK, roll, 0.08), 1.6).at_xyz(
      0.0,
      floor - plinth / 2.0,
      0.0
    )
  );
  let top = floor + tall;
  let timber = tinted(TIMBER, roll, 0.1);
  match walls {
    Walls::Timber => {
      works.add(
        Stuff::Planks,
        Piece::new(block(length, tall, depth), timber)
          .at_xyz(0.0, floor + tall / 2.0, 0.0)
          .planar(2.4)
      );
      let posts = (length / 3.2).ceil() as usize;
      (0..=posts).for_each(|post| {
        let x = -length / 2.0 + post as f32 * length / posts as f32;
        [-1.0, 1.0].into_iter().for_each(|side| {
          works.add(
            Stuff::Wood,
            Piece::new(block(0.3, tall + 0.1, 0.3), BEAM).at_xyz(
              x,
              floor + tall / 2.0,
              side * depth / 2.0
            )
          )
        });
      });
      [-1.0, 1.0].into_iter().for_each(|side| {
        works.add(
          Stuff::Wood,
          Piece::new(block(length + 0.3, 0.3, 0.34), BEAM).at_xyz(
            0.0,
            top,
            side * depth / 2.0
          )
        );
        works.add(
          Stuff::Wood,
          Piece::new(block(0.3, 0.3, depth), BEAM).at_xyz(
            side * length / 2.0,
            floor + tall * 0.55,
            0.0
          )
        );
      });
    }
    Walls::Logs => {
      works.add(
        Stuff::Planks,
        Piece::new(block(length - 0.3, tall, depth - 0.3), timber * 0.8)
          .at_xyz(0.0, floor + tall / 2.0, 0.0)
          .planar(2.0)
      );
      let courses = (tall / 0.34).round() as usize;
      (0..courses).for_each(|course| {
        let y = floor + 0.17 + course as f32 * 0.34;
        let shade = timber * roll.range(0.85, 1.1);
        [-1.0, 1.0].into_iter().for_each(|side| {
          works.add(
            Stuff::Bark,
            Piece::new(rod(0.19, length + 0.7), shade).rolled(FRAC_PI_2).at_xyz(
              0.0,
              y,
              side * depth / 2.0
            )
          );
          works.add(
            Stuff::Bark,
            Piece::new(rod(0.19, depth + 0.7), shade).pitched(FRAC_PI_2).at_xyz(
              side * length / 2.0,
              y + 0.17,
              0.0
            )
          );
        });
      });
    }
    Walls::Stone => {
      works.add(
        Stuff::Masonry,
        Piece::new(block(length, tall, depth), tinted(STONEWORK, roll, 0.1))
          .at_xyz(0.0, floor + tall / 2.0, 0.0)
          .planar(1.8)
      );
    }
  }
  let (pitch, overhang, thick, stuff, color, tile) = match roof {
    Roof::Thatch => (0.95_f32, 1.0, 0.5, Stuff::Thatch, tinted(STRAW, roll, 0.12), 2.2),
    Roof::Shingle => (0.8, 0.6, 0.18, Stuff::Shingle, tinted(SHINGLES, roll, 0.1), 2.0),
    Roof::Gilded => (0.82, 0.6, 0.2, Stuff::Shingle, tinted(GILT, roll, 0.1), 2.0)
  };
  let gable = 0.6;
  let half = depth / 2.0 + overhang;
  let slope = half / pitch.cos();
  let rise = depth / 2.0 * pitch.tan();
  let ridge = top + rise;
  [-1.0, 1.0].into_iter().for_each(|side| {
    let mid = half / 2.0;
    let y = top + (depth / 2.0 - mid) * pitch.tan() + thick / 2.0 / pitch.cos();
    works.add(
      stuff,
      slab(length + 2.0 * gable, thick, slope, color, tile).pitched(side * pitch).at_xyz(
        0.0,
        y,
        side * mid
      )
    );
  });
  works.add(
    stuff,
    Piece::new(rod(thick * 0.9 + 0.08, length + 2.0 * gable), color * 0.9)
      .rolled(FRAC_PI_2)
      .at_xyz(0.0, ridge + thick * 0.8, 0.0)
  );
  let eave = top - overhang * pitch.tan();
  [-1.0, 1.0].into_iter().for_each(|end| {
    let x = end * (length / 2.0 - 0.05);
    works.add(
      Stuff::Planks,
      Piece::new(
        crate::model::fan(
          &[
            Vec2::new(-depth / 2.0, top),
            Vec2::new(0.0, ridge),
            Vec2::new(depth / 2.0, top)
          ],
          0.12
        ),
        timber * 0.9
      )
      .yawed(FRAC_PI_2)
      .at_xyz(x, 0.0, 0.0)
      .planar(2.4)
    );
    let edge = end * (length / 2.0 + gable + 0.05);
    [-1.0, 1.0].into_iter().for_each(|side| {
      let foot = Vec3::new(edge, eave + thick, side * half);
      let peak = Vec3::new(edge, ridge + thick * 1.2, 0.0);
      let horn = peak + (peak - foot).normalize() * 1.3;
      works.add(Stuff::Wood, beam(foot, horn, 0.2, BEAM));
    });
  });
  let door = length * roll.range(-0.2, 0.2);
  works.add(
    Stuff::Wood,
    Piece::new(block(1.2, 2.1, 0.16), DOOR).at_xyz(
      door,
      floor + 1.05,
      depth / 2.0 + 0.06
    )
  );
  [-0.72, 0.72].into_iter().for_each(|side| {
    works.add(
      Stuff::Wood,
      Piece::new(block(0.2, 2.4, 0.24), BEAM).at_xyz(
        door + side,
        floor + 1.2,
        depth / 2.0 + 0.08
      )
    )
  });
  works.add(
    Stuff::Wood,
    Piece::new(block(1.7, 0.24, 0.26), BEAM).at_xyz(
      door,
      floor + 2.3,
      depth / 2.0 + 0.08
    )
  );
  [-1.0, 1.0].into_iter().for_each(|side| {
    [-0.32, 0.32]
      .into_iter()
      .map(|fraction| fraction * length)
      .filter(|x| (x - door).abs() > 1.6)
      .for_each(|x| {
        let z = side * (depth / 2.0 + 0.04);
        works.add(
          Stuff::Gloss,
          Piece::new(block(0.7, 0.6, 0.1), GLOOM).at_xyz(x, floor + 1.6, z)
        );
        [-0.55, 0.55].into_iter().for_each(|shutter| {
          works.add(
            Stuff::Planks,
            Piece::new(block(0.38, 0.72, 0.06), timber * 0.75).at_xyz(
              x + shutter,
              floor + 1.6,
              z + side * 0.04
            )
          )
        });
      });
  });
  if roll.chance(0.75) {
    let (x, z) = (length * 0.28, -depth * 0.2);
    let height = ridge + 1.3 - floor;
    works.add(
      Stuff::Masonry,
      slab(0.9, height, 0.9, STONEWORK * 0.9, 1.4).at_xyz(x, floor + height / 2.0, z)
    );
  }
  let front = ground(Vec2::new(door, depth / 2.0 + 1.5));
  let steps = ((floor - front) / 0.3).floor().max(0.0) as usize;
  (0..steps).for_each(|step| {
    let y = floor - 0.3 * (step as f32 + 1.0);
    works.add(
      Stuff::Masonry,
      slab(1.6, 0.3, 0.45, STONEWORK * 0.85, 1.4).at_xyz(
        door,
        y + 0.15,
        depth / 2.0 + 0.45 + step as f32 * 0.42
      )
    );
  });
  works.solid(
    Vec3::new(0.0, (top + low) / 2.0, 0.0),
    Quat::IDENTITY,
    Collider::cuboid(length + 0.5, top - low, depth + 0.5)
  );
  works
}

fn merlons(works: &mut Works, from: Vec3, to: Vec3, color: Srgba) {
  let length = from.distance(to);
  let count = (length / 1.7).floor().max(1.0) as usize;
  let along = (to - from).normalize();
  let turn = f32::atan2(along.x, along.z);
  (0..count).for_each(|index| {
    let at = from + along * (index as f32 + 0.5) * length / count as f32;
    works.add(
      Stuff::Masonry,
      slab(0.6, 1.0, 0.9, color, 1.4).yawed(turn).at(at + Vec3::Y * 0.5)
    );
  });
}

fn rampart(from: Vec2, to: Vec2, tall: f32, ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let length = from.distance(to);
  let steps = (length / 4.0).ceil() as usize;
  let lows: Vec<f32> =
    (0..=steps).map(|step| ground(from.lerp(to, step as f32 / steps as f32))).collect();
  let base = lows.iter().copied().fold(f32::MAX, f32::min) - 1.5;
  let crest = lows.iter().copied().fold(f32::MIN, f32::max) + tall;
  let middle = (from + to) / 2.0;
  let along = (to - from).normalize();
  let turn = f32::atan2(along.x, along.y) + FRAC_PI_2;
  let color = tinted(STONEWORK, roll, 0.06);
  works.add(
    Stuff::Masonry,
    Piece::new(block(length + 1.0, crest - base, 2.6), color)
      .planar(2.0)
      .yawed(turn)
      .at(middle.extend((crest + base) / 2.0).xzy())
  );
  let outward = along.perp();
  [1.0, -1.0].into_iter().for_each(|side| {
    let edge = |at: Vec2| (at + outward * side * 1.1).extend(crest).xzy();
    merlons(&mut works, edge(from), edge(to), color);
  });
  works.solid(
    middle.extend((crest + base) / 2.0).xzy(),
    Quat::from_rotation_y(turn),
    Collider::cuboid(length + 1.0, crest - base, 2.6)
  );
  works
}

fn tower(radius: f32, tall: f32, ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::splat(radius));
  let (base, crest) = (low - 1.5, high + tall);
  let color = tinted(STONEWORK, roll, 0.06);
  let profile = [
    Vec2::new(0.0, base),
    Vec2::new(radius + 0.5, base),
    Vec2::new(radius + 0.5, high + 1.2),
    Vec2::new(radius, high + 2.2),
    Vec2::new(radius, crest - 1.0),
    Vec2::new(radius + 0.4, crest - 0.6),
    Vec2::new(radius + 0.4, crest),
    Vec2::new(0.0, crest)
  ];
  let around = TAU * radius;
  works.add(
    Stuff::Masonry,
    Piece::new(lathe(&profile, 20), color)
      .tiled(Vec2::new(around / 2.2, (crest - base) / 2.2))
  );
  let count = ((around / 1.9).round() as usize).max(6);
  (0..count).for_each(|index| {
    let angle = index as f32 / count as f32 * TAU;
    let at = Vec3::new(angle.cos(), 0.0, angle.sin()) * (radius + 0.15)
      + Vec3::Y * (crest + 0.5);
    works.add(Stuff::Masonry, slab(0.8, 1.0, 0.7, color, 1.4).yawed(-angle).at(at));
  });
  (0..3).for_each(|slit| {
    let angle = roll.range(0.0, TAU);
    let y = high + 3.0 + slit as f32 * (tall - 5.0) / 3.0;
    works.add(
      Stuff::Gloss,
      Piece::new(block(0.12, 1.0, 0.3), GLOOM)
        .yawed(-angle)
        .at(Vec3::new(angle.cos(), 0.0, angle.sin()) * (radius + 0.02) + Vec3::Y * y)
    );
  });
  works.solid(
    Vec3::Y * (crest + base) / 2.0,
    Quat::IDENTITY,
    Collider::cylinder(radius + 0.4, crest - base)
  );
  works
}

fn gate(tall: f32, ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::new(5.0, 2.0));
  let color = tinted(STONEWORK, roll, 0.05) * 0.95;
  let crest = high + tall + 2.0;
  [-1.0, 1.0].into_iter().for_each(|side| {
    let x = side * 4.2;
    works.add(
      Stuff::Masonry,
      slab(3.0, crest - low + 1.5, 4.0, color, 2.0).at_xyz(
        x,
        (crest + low - 1.5) / 2.0,
        0.0
      )
    );
    works.solid(
      Vec3::new(x, (crest + low) / 2.0, 0.0),
      Quat::IDENTITY,
      Collider::cuboid(3.0, crest - low, 4.0)
    );
    merlons(
      &mut works,
      Vec3::new(x - 1.4, crest, 1.7),
      Vec3::new(x + 1.4, crest, 1.7),
      color
    );
    works.add(
      Stuff::Planks,
      Piece::new(block(2.6, 5.2, 0.2), DOOR * 1.3).planar(2.0).yawed(side * 1.2).at_xyz(
        side * (2.7 - 1.3 * 1.2f32.cos()),
        high + 2.6,
        2.1 + 1.3 * 1.2f32.sin()
      )
    );
  });
  works.add(Stuff::Masonry, slab(5.6, 2.4, 3.6, color, 2.0).at_xyz(0.0, high + 6.2, 0.0));
  works.add(
    Stuff::Masonry,
    slab(5.8, crest - high - 7.4, 3.8, color, 2.0).at_xyz(
      0.0,
      (crest + high + 7.4) / 2.0,
      0.0
    )
  );
  works.solid(
    Vec3::new(0.0, (crest + high + 5.0) / 2.0, 0.0),
    Quat::IDENTITY,
    Collider::cuboid(5.6, crest - high - 5.0, 3.6)
  );
  works
}

fn keep(ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::new(18.0, 24.0));
  let deck = high + 7.0;
  let color = tinted(STONEWORK, roll, 0.04);
  works.add(
    Stuff::Masonry,
    slab(36.0, deck - low + 1.0, 46.0, color, 2.4).at_xyz(
      0.0,
      (deck + low - 1.0) / 2.0,
      0.0
    )
  );
  works.solid(
    Vec3::new(0.0, (deck + low) / 2.0, 0.0),
    Quat::IDENTITY,
    Collider::cuboid(36.0, deck - low, 46.0)
  );
  let steps = ((deck - high) / 0.35).ceil() as usize;
  (0..steps).for_each(|step| {
    let y = deck - 0.35 * (step as f32 + 1.0);
    works.add(
      Stuff::Masonry,
      slab(9.0, 0.35, 0.6, color * 0.9, 1.6).at_xyz(
        0.0,
        y + 0.17,
        23.3 + step as f32 * 0.55
      )
    );
  });
  let run = steps as f32 * 0.55;
  let ramp = f32::atan2(deck - high, run);
  works.solid(
    Vec3::new(0.0, (deck + high) / 2.0 - 0.3, 23.0 + run / 2.0),
    Quat::from_rotation_x(ramp),
    Collider::cuboid(9.0, 0.5, (run * run + (deck - high).powi(2)).sqrt())
  );
  let flat: Ground = &|_| 0.0;
  let hall = House {
    at: Vec2::ZERO,
    facing: 0.0,
    length: 30.0,
    depth: 18.0,
    tall: 7.5,
    roof: Roof::Gilded,
    walls: Walls::Stone
  };
  let wing = House {
    at: Vec2::ZERO,
    facing: 0.0,
    length: 11.0,
    depth: 26.0,
    tall: 5.0,
    roof: Roof::Gilded,
    walls: Walls::Timber
  };
  [
    (hall, Vec3::new(0.0, deck, -2.0), 0.0),
    (wing, Vec3::new(0.0, deck, -14.0), FRAC_PI_2)
  ]
  .into_iter()
  .for_each(|(part, at, turn)| {
    let mut made = house(&part, flat, roll);
    made.solids.clear();
    works.absorb(made.placed(
      Transform::from_translation(at).with_rotation(Quat::from_rotation_y(turn))
    ));
  });
  works.solid(
    Vec3::new(0.0, deck + 6.0, -2.0),
    Quat::IDENTITY,
    Collider::cuboid(30.5, 12.0, 18.5)
  );
  let spire = high + 7.0 + 24.0;
  works.add(
    Stuff::Masonry,
    slab(7.0, spire - deck, 7.0, color, 2.0).at_xyz(11.0, (spire + deck) / 2.0, 12.0)
  );
  works.add(
    Stuff::Shingle,
    Piece::new(crate::model::cone(5.6, 7.0), GILT).at_xyz(11.0, spire + 3.5, 12.0)
  );
  works
}

fn windmill(ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::splat(3.2));
  let color = tinted(STONEWORK, roll, 0.06);
  let base = [
    Vec2::new(0.0, low - 1.0),
    Vec2::new(3.4, low - 1.0),
    Vec2::new(3.3, high + 3.2),
    Vec2::new(0.0, high + 3.2)
  ];
  works
    .add(Stuff::Masonry, Piece::new(lathe(&base, 18), color).tiled(Vec2::new(9.0, 2.0)));
  let body = [
    Vec2::new(0.0, high + 3.1),
    Vec2::new(3.0, high + 3.1),
    Vec2::new(2.3, high + 11.0),
    Vec2::new(0.0, high + 11.0)
  ];
  works.add(
    Stuff::Planks,
    Piece::new(lathe(&body, 8), tinted(TIMBER, roll, 0.08)).tiled(Vec2::new(6.0, 3.0))
  );
  (0..8).for_each(|side| {
    let angle = (side as f32 + 0.5) / 8.0 * TAU;
    let (foot, head) = (
      Vec3::new(angle.cos() * 3.02, high + 3.1, angle.sin() * 3.02),
      Vec3::new(angle.cos() * 2.32, high + 11.0, angle.sin() * 2.32)
    );
    works.add(Stuff::Wood, beam(foot, head, 0.22, BEAM));
  });
  let cap = [
    Vec2::new(0.0, high + 10.8),
    Vec2::new(2.9, high + 10.8),
    Vec2::new(1.2, high + 13.4),
    Vec2::new(0.1, high + 14.6)
  ];
  works.add(
    Stuff::Shingle,
    Piece::new(lathe(&cap, 12), tinted(SHINGLES, roll, 0.1)).tiled(Vec2::new(7.0, 2.0))
  );
  works.add(
    Stuff::Wood,
    Piece::new(block(1.1, 2.0, 0.14), DOOR).at_xyz(0.0, high + 1.0, 3.32)
  );
  let hub = Vec3::new(0.0, high + 9.8, 3.0);
  let sails: Vec<(Stuff, Piece)> = (0..4)
    .flat_map(|arm| {
      let turn = Quat::from_rotation_z(arm as f32 * FRAC_PI_2 + 0.3);
      let spar =
        Piece::new(block(0.2, 8.5, 0.2), BEAM).at_xyz(0.0, 4.4, 0.0).turned(turn);
      let lattice = (0..7).map(move |rung| {
        Piece::new(block(1.5, 0.08, 0.05), TIMBER * 0.9)
          .at_xyz(0.85, 1.6 + rung as f32 * 1.05, 0.05)
          .turned(turn)
      });
      let cloth =
        Piece::new(block(1.35, 6.4, 0.03), CANVAS).at_xyz(0.85, 4.8, 0.08).turned(turn);
      std::iter::once((Stuff::Wood, spar))
        .chain(lattice.map(|piece| (Stuff::Wood, piece)))
        .chain([(Stuff::Cloth, cloth)])
    })
    .chain([(Stuff::Wood, Piece::new(rod(0.35, 0.8), BEAM).pitched(FRAC_PI_2))])
    .collect();
  works.spinners.push((Transform::from_translation(hub), sails));
  works.solid(Vec3::Y * (high + 5.0), Quat::IDENTITY, Collider::cylinder(3.3, 12.0));
  works
}

fn well(ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::splat(1.3));
  let ring = [
    Vec2::new(0.9, low - 0.5),
    Vec2::new(1.25, low - 0.5),
    Vec2::new(1.25, high + 0.85),
    Vec2::new(0.9, high + 0.85),
    Vec2::new(0.9, high - 1.0)
  ];
  works.add(
    Stuff::Masonry,
    Piece::new(lathe(&ring, 14), tinted(STONEWORK, roll, 0.06))
      .tiled(Vec2::new(4.0, 1.0))
  );
  works.add(Stuff::Gloss, Piece::new(rod(0.9, 0.05), GLOOM).at_xyz(0.0, high + 0.2, 0.0));
  [-1.0, 1.0].into_iter().for_each(|side| {
    works.add(
      Stuff::Wood,
      Piece::new(block(0.18, 2.6, 0.18), BEAM).at_xyz(side * 1.05, high + 1.3, 0.0)
    );
    works.add(
      Stuff::Shingle,
      slab(2.6, 0.08, 1.2, SHINGLES, 1.5)
        .rolled(side * 0.6)
        .at_xyz(side * 0.5, high + 2.75, 0.0)
        .yawed(FRAC_PI_2)
    );
  });
  works.add(
    Stuff::Wood,
    Piece::new(rod(0.08, 2.2), TIMBER).rolled(FRAC_PI_2).at_xyz(0.0, high + 2.1, 0.0)
  );
  works.solid(Vec3::Y * high, Quat::IDENTITY, Collider::cylinder(1.3, 2.0));
  works
}

fn fence(path: &[Vec2], ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  path.windows(2).for_each(|pair| {
    let (from, to) = (pair[0], pair[1]);
    let length = from.distance(to);
    let posts = (length / 1.6).ceil().max(1.0) as usize;
    let along = (to - from).normalize();
    let turn = f32::atan2(along.x, along.y) + FRAC_PI_2;
    (0..=posts).for_each(|post| {
      let at = from.lerp(to, post as f32 / posts as f32);
      let tall = roll.range(1.05, 1.3);
      works.add(
        Stuff::Bark,
        Piece::new(rod(0.06, tall + 0.3), WATTLE * 0.8)
          .at(at.extend(ground(at) + tall / 2.0 - 0.1).xzy())
      );
    });
    (0..posts).for_each(|span| {
      let (a, b) = (
        from.lerp(to, span as f32 / posts as f32),
        from.lerp(to, (span + 1) as f32 / posts as f32)
      );
      let middle = (a + b) / 2.0;
      let lean = f32::atan2(ground(b) - ground(a), a.distance(b));
      works.add(
        Stuff::Bark,
        Piece::new(block(a.distance(b) + 0.05, 0.85, 0.09), tinted(WATTLE, roll, 0.1))
          .planar(0.6)
          .rolled(lean)
          .yawed(turn)
          .at(middle.extend(ground(middle) + 0.6).xzy())
      );
    });
    works.solid(
      ((from + to) / 2.0).extend(ground((from + to) / 2.0) + 0.6).xzy(),
      Quat::from_rotation_y(turn),
      Collider::cuboid(length, 1.2, 0.15)
    );
  });
  works
}

fn field(size: Vec2, crop: Crop, ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let rows = (size.y / 1.1).floor() as usize;
  let across = (size.x
    / match crop {
      Crop::Cabbage => 0.8,
      Crop::Wheat => 2.4
    })
  .floor() as usize;
  (0..rows).for_each(|row| {
    (0..across).for_each(|column| {
      let at = Vec2::new(
        (column as f32 + 0.5) / across as f32 - 0.5,
        (row as f32 + 0.5) / rows as f32 - 0.5
      ) * size
        + Vec2::new(roll.spread(0.15), roll.spread(0.1));
      let y = ground(at);
      match crop {
        Crop::Cabbage => works.add(
          Stuff::Leather,
          Piece::new(
            Sphere::new(0.26).mesh().ico(1).expect("cabbage"),
            tinted(LEAF, roll, 0.15)
          )
          .sized(Vec3::new(1.0, 0.7, 1.0))
          .at(at.extend(y + 0.12).xzy())
        ),
        Crop::Wheat => {
          if roll.chance(0.6) {
            works.add(
              Stuff::Thatch,
              Piece::new(crate::model::cone(0.45, 1.3), tinted(STRAW * 1.15, roll, 0.1))
                .planar(0.8)
                .at(at.extend(y + 0.6).xzy())
            )
          }
        }
      }
    });
  });
  works
}

fn clutter(kind: Clutter, ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let y = ground(Vec2::ZERO);
  let barrel = |at: Vec3, roll: &mut Roll| {
    let staves = lathe(
      &[
        Vec2::new(0.0, 0.0),
        Vec2::new(0.32, 0.0),
        Vec2::new(0.38, 0.45),
        Vec2::new(0.32, 0.9),
        Vec2::new(0.0, 0.9)
      ],
      12
    );
    [
      (Stuff::Wood, Piece::new(staves, tinted(TIMBER, roll, 0.1)).at(at)),
      (
        Stuff::Iron,
        Piece::new(rod(0.385, 0.06), Srgba::new(0.3, 0.3, 0.3, 1.0))
          .at(at + Vec3::Y * 0.25)
      ),
      (
        Stuff::Iron,
        Piece::new(rod(0.385, 0.06), Srgba::new(0.3, 0.3, 0.3, 1.0))
          .at(at + Vec3::Y * 0.65)
      )
    ]
  };
  let (parts, size): (Vec<(Stuff, Piece)>, Vec3) = match kind {
    Clutter::Barrels => (
      (0..3)
        .flat_map(|index| {
          barrel(Vec3::new(index as f32 * 0.8 - 0.8, y, roll.spread(0.2)), roll)
        })
        .collect(),
      Vec3::new(2.4, 0.9, 0.8)
    ),
    Clutter::Crates => (
      (0..4)
        .map(|index| {
          let stacked = (index == 3) as u8 as f32;
          (
            Stuff::Planks,
            slab(0.75, 0.75, 0.75, tinted(TIMBER * 1.15, roll, 0.12), 0.75)
              .yawed(roll.spread(0.3))
              .at_xyz(
                index as f32 % 3.0 * 0.85 - 0.85 + stacked * 0.4,
                y + 0.37 + stacked * 0.75,
                0.0
              )
          )
        })
        .collect(),
      Vec3::new(2.6, 1.5, 0.9)
    ),
    Clutter::Hay => (
      (0..3)
        .map(|index| {
          (
            Stuff::Thatch,
            slab(1.4, 0.7, 0.8, tinted(STRAW * 1.1, roll, 0.1), 0.9).at_xyz(
              0.0,
              y + 0.35 + (index == 2) as u8 as f32 * 0.7,
              index as f32 % 2.0 * 0.85 - 0.4
            )
          )
        })
        .collect(),
      Vec3::new(1.4, 1.4, 1.7)
    ),
    Clutter::Woodpile => (
      (0..14)
        .map(|index| {
          let (row, slot) = (index / 5, index % 5);
          (
            Stuff::Bark,
            Piece::new(rod(0.14, 1.1), tinted(TIMBER * 0.8, roll, 0.15))
              .pitched(FRAC_PI_2)
              .at_xyz(
                slot as f32 * 0.3 - 0.6 + row as f32 * 0.15,
                y + 0.14 + row as f32 * 0.26,
                0.0
              )
          )
        })
        .collect(),
      Vec3::new(1.6, 0.8, 1.1)
    ),
    Clutter::Cart => (
      [
        (Stuff::Planks, slab(2.6, 0.12, 1.4, TIMBER, 1.0).at_xyz(0.0, y + 0.8, 0.0)),
        (
          Stuff::Planks,
          slab(2.6, 0.4, 0.08, TIMBER * 0.9, 1.0).at_xyz(0.0, y + 1.05, 0.68)
        ),
        (
          Stuff::Planks,
          slab(2.6, 0.4, 0.08, TIMBER * 0.9, 1.0).at_xyz(0.0, y + 1.05, -0.68)
        ),
        (
          Stuff::Wood,
          Piece::new(rod(0.55, 0.1), BEAM).pitched(FRAC_PI_2).at_xyz(-0.3, y + 0.55, 0.8)
        ),
        (
          Stuff::Wood,
          Piece::new(rod(0.55, 0.1), BEAM).pitched(FRAC_PI_2).at_xyz(
            -0.3,
            y + 0.55,
            -0.8
          )
        ),
        (
          Stuff::Wood,
          beam(Vec3::new(1.3, y + 0.8, 0.3), Vec3::new(3.0, y + 0.2, 0.3), 0.1, BEAM)
        ),
        (
          Stuff::Wood,
          beam(Vec3::new(1.3, y + 0.8, -0.3), Vec3::new(3.0, y + 0.2, -0.3), 0.1, BEAM)
        ),
        (Stuff::Thatch, slab(1.4, 0.4, 1.1, STRAW, 0.9).at_xyz(-0.2, y + 1.05, 0.0))
      ]
      .into_iter()
      .collect(),
      Vec3::new(2.6, 1.3, 1.6)
    ),
    Clutter::Stall => {
      let cloth = [
        Srgba::new(0.55, 0.16, 0.12, 1.0),
        Srgba::new(0.2, 0.3, 0.45, 1.0),
        Srgba::new(0.55, 0.48, 0.3, 1.0)
      ][roll.below(3)];
      (
        [(-1.2, -0.6), (1.2, -0.6), (-1.2, 0.6), (1.2, 0.6)]
          .into_iter()
          .map(|(x, z)| {
            let tall = if z < 0.0 { 2.4 } else { 2.0 };
            (
              Stuff::Wood,
              Piece::new(block(0.12, tall, 0.12), BEAM).at_xyz(x, y + tall / 2.0, z)
            )
          })
          .chain([
            (
              Stuff::Cloth,
              Piece::new(block(2.8, 0.04, 1.6), cloth).pitched(-0.25).at_xyz(
                0.0,
                y + 2.25,
                0.0
              )
            ),
            (Stuff::Planks, slab(2.4, 0.9, 1.0, TIMBER, 1.0).at_xyz(0.0, y + 0.45, 0.2))
          ])
          .chain(barrel(Vec3::new(1.7, y, -0.5), roll))
          .collect(),
        Vec3::new(2.6, 1.0, 1.2)
      )
    }
  };
  parts.into_iter().for_each(|(stuff, piece)| works.add(stuff, piece));
  works.solid(
    Vec3::Y * (y + size.y / 2.0),
    Quat::IDENTITY,
    Collider::cuboid(size.x, size.y, size.z)
  );
  works
}

const SOD: Srgba = Srgba::new(0.36, 0.37, 0.25, 1.0);
const GRANITE: Srgba = Srgba::new(0.5, 0.5, 0.48, 1.0);

fn mound(radius: f32, ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::splat(radius));
  let rise = radius * 0.62;
  let profile: Vec<Vec2> = [Vec2::new(0.0, low - 1.0), Vec2::new(radius, low - 1.0)]
    .into_iter()
    .chain((0..=10).map(|step| {
      let angle = step as f32 / 10.0 * FRAC_PI_2;
      Vec2::new(radius * angle.cos(), high + rise * angle.sin())
    }))
    .collect();
  works.add(Stuff::Stone, Piece::new(lathe(&profile, 24), tinted(SOD, roll, 0.08)));
  let stone = tinted(STONEWORK, roll, 0.06) * 0.85;
  let front = radius * 0.9;
  works
    .add(Stuff::Masonry, slab(4.4, 3.6, 2.0, stone, 1.8).at_xyz(0.0, high + 0.8, front));
  works.add(
    Stuff::Masonry,
    slab(5.0, 0.7, 2.4, stone * 0.9, 1.8).at_xyz(0.0, high + 2.9, front + 0.2)
  );
  works.add(
    Stuff::Gloss,
    Piece::new(block(2.0, 2.4, 0.1), GLOOM).at_xyz(0.0, high + 1.2, front + 1.02)
  );
  [(0.95, 0.35), (0.7, 0.7), (0.4, 1.0)].into_iter().for_each(|(wide, tall)| {
    let top = high + rise * tall;
    works.solid(
      Vec3::Y * (top + low - 1.0) / 2.0,
      Quat::IDENTITY,
      Collider::cylinder(radius * wide, top - low + 1.0)
    )
  });
  works.solid(
    Vec3::new(0.0, high + 1.0, front),
    Quat::IDENTITY,
    Collider::cuboid(4.4, 4.0, 2.0)
  );
  works
}

fn boulder(
  works: &mut Works,
  at: Vec3,
  size: Vec3,
  turn: f32,
  color: Srgba,
  roll: &mut Roll
) {
  works.add(
    Stuff::Stone,
    Piece::new(crate::model::lump(roll.below(500) as u32, 0.25, 2), color)
      .sized(size / 2.0)
      .yawed(turn)
      .at(at)
  );
  works.solid(
    at,
    Quat::from_rotation_y(turn),
    Collider::cuboid(size.x * 0.8, size.y * 0.8, size.z * 0.8)
  );
}

fn den(ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, _) = spread(ground, Vec2::splat(5.0));
  let rock = tinted(GRANITE, roll, 0.08);
  [
    (Vec3::new(0.0, 2.0, -3.0), Vec3::new(10.0, 7.0, 7.0)),
    (Vec3::new(-3.6, 1.6, 1.0), Vec3::new(3.6, 5.0, 4.6)),
    (Vec3::new(3.6, 1.4, 0.8), Vec3::new(3.4, 4.6, 4.8)),
    (Vec3::new(0.0, 4.6, 0.8), Vec3::new(6.0, 2.0, 3.6)),
    (Vec3::new(-5.8, 0.6, -1.5), Vec3::new(3.0, 3.0, 3.4)),
    (Vec3::new(5.6, 0.4, -1.2), Vec3::new(2.6, 2.4, 3.0))
  ]
  .into_iter()
  .for_each(|(at, size)| {
    let turn = roll.spread(0.4);
    boulder(
      &mut works,
      at + Vec3::Y * low,
      size,
      turn,
      rock * roll.range(0.9, 1.08),
      roll
    )
  });
  works.add(
    Stuff::Gloss,
    Piece::new(block(3.2, 3.4, 0.2), GLOOM).at_xyz(0.0, low + 1.6, 0.9)
  );
  works
}

fn shrine(ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, high) = spread(ground, Vec2::new(1.4, 1.1));
  let stone = tinted(STONEWORK, roll, 0.06);
  works.add(
    Stuff::Masonry,
    slab(2.6, high - low + 0.8, 2.0, stone * 0.9, 1.6).at_xyz(
      0.0,
      (high + low + 0.2) / 2.0 - 0.3,
      0.0
    )
  );
  works
    .add(Stuff::Masonry, slab(1.0, 2.2, 0.7, stone, 1.6).at_xyz(0.0, high + 1.6, -0.3));
  works.add(
    Stuff::Stone,
    Piece::new(crate::model::lump(roll.below(500) as u32, 0.15, 2), GRANITE)
      .sized(Vec3::new(0.35, 0.5, 0.3))
      .at_xyz(0.0, high + 3.1, -0.3)
  );
  works.add(
    Stuff::Gold,
    Piece::new(rod(0.34, 0.08), Srgba::new(0.7, 0.58, 0.3, 1.0))
      .pitched(FRAC_PI_2)
      .at_xyz(0.0, high + 2.1, 0.08)
  );
  [-0.8, 0.0, 0.8].into_iter().for_each(|x: f32| {
    let tall = 0.25 + 0.1 * x.abs();
    works.add(
      Stuff::Bone,
      Piece::new(rod(0.05, tall), Srgba::new(0.85, 0.8, 0.66, 1.0)).at_xyz(
        x,
        high + 0.4 + tall / 2.0,
        0.6
      )
    );
    works.add(
      Stuff::Ember,
      Piece::new(block(0.04, 0.08, 0.04), Srgba::new(1.0, 0.7, 0.35, 1.0)).at_xyz(
        x,
        high + 0.44 + tall,
        0.6
      )
    );
  });
  works.solid(Vec3::Y * (high + 1.0), Quat::IDENTITY, Collider::cuboid(2.6, 3.0, 2.0));
  works
}

fn menhirs(ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let stones = 7;
  (0..stones).for_each(|index| {
    let angle = index as f32 / stones as f32 * TAU + roll.spread(0.1);
    let spot = Vec2::from_angle(angle) * 6.5;
    let tall = roll.range(2.4, 4.2);
    let y = ground(spot);
    works.add(
      Stuff::Stone,
      Piece::new(
        crate::model::lump(roll.below(500) as u32, 0.08, 2),
        tinted(GRANITE, roll, 0.06)
      )
      .sized(Vec3::new(0.9, tall / 2.0, 0.55))
      .rolled(roll.spread(0.08))
      .yawed(-angle + FRAC_PI_2)
      .at(spot.extend(y + tall / 2.0 - 0.4).xzy())
    );
    works.solid(
      spot.extend(y + tall / 2.0 - 0.4).xzy(),
      Quat::from_rotation_y(-angle + FRAC_PI_2),
      Collider::cuboid(1.5, tall, 0.9)
    );
  });
  let (low, high) = spread(ground, Vec2::splat(1.0));
  works.add(
    Stuff::Masonry,
    slab(1.8, high - low + 1.0, 1.0, tinted(STONEWORK, roll, 0.06), 1.6).at_xyz(
      0.0,
      (high + low) / 2.0 + 0.1,
      0.0
    )
  );
  works.solid(Vec3::Y * (high + 0.2), Quat::IDENTITY, Collider::cuboid(1.8, 1.0, 1.0));
  works
}

fn pillar(tall: f32, ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  let (low, _) = spread(ground, Vec2::splat(0.6));
  let lean = roll.spread(0.06);
  works.add(
    Stuff::Masonry,
    slab(1.0, tall + 0.6, 1.0, tinted(STONEWORK, roll, 0.08), 1.6).rolled(lean).at_xyz(
      0.0,
      low + tall / 2.0 - 0.3,
      0.0
    )
  );
  works.add(
    Stuff::Masonry,
    slab(1.3, 0.35, 1.3, tinted(STONEWORK, roll, 0.08) * 0.9, 1.6).at_xyz(
      0.0,
      low + 0.1,
      0.0
    )
  );
  works.solid(
    Vec3::Y * (low + tall / 2.0),
    Quat::IDENTITY,
    Collider::cuboid(1.0, tall, 1.0)
  );
  works
}

fn rubble(ground: Ground, roll: &mut Roll) -> Works {
  let mut works = Works::default();
  (0..4 + roll.below(4)).for_each(|_| {
    let spot = Vec2::new(roll.spread(2.2), roll.spread(2.2));
    let size = roll.range(0.3, 1.0);
    works.add(
      Stuff::Stone,
      Piece::new(
        crate::model::lump(roll.below(500) as u32, 0.2, 1),
        tinted(STONEWORK, roll, 0.1) * 0.9
      )
      .sized(Vec3::new(size, size * 0.6, size * roll.range(0.7, 1.2)))
      .yawed(roll.range(0.0, TAU))
      .at(spot.extend(ground(spot) + size * 0.2).xzy())
    );
  });
  works
}

fn wheel(radius: f32, roll: &mut Roll) -> Vec<(Stuff, Piece)> {
  let timber = tinted(TIMBER * 0.8, roll, 0.08);
  let rims = [-0.5, 0.5].map(|side| {
    let hoop: Vec<Vec3> = (0..=24)
      .map(|step| {
        Vec2::from_angle(step as f32 / 24.0 * TAU).extend(side).xyz()
          * Vec3::new(radius, radius, 1.0)
      })
      .collect();
    (Stuff::Wood, Piece::new(crate::model::tube(&hoop, &[0.12], 6), timber))
  });
  let spokes = (0..8).map(|spoke| {
    let angle = spoke as f32 / 8.0 * TAU;
    (Stuff::Wood, Piece::new(block(0.14, radius * 2.0, 0.14), BEAM).rolled(angle))
  });
  let paddles = (0..16).map(|paddle| {
    let angle = paddle as f32 / 16.0 * TAU;
    (
      Stuff::Planks,
      Piece::new(block(0.7, 0.08, 1.1), timber)
        .rolled(angle)
        .at(Vec2::from_angle(angle + FRAC_PI_2).extend(0.0) * radius)
    )
  });
  rims
    .into_iter()
    .chain(spokes)
    .chain(paddles)
    .chain([(Stuff::Wood, Piece::new(rod(0.3, 1.6), BEAM).pitched(FRAC_PI_2))])
    .collect()
}

fn mill(site: &Site, roll: &mut Roll) -> Works {
  let ground: Ground = &|offset| site.ground(offset);
  let plan = House {
    at: Vec2::ZERO,
    facing: 0.0,
    length: 10.0,
    depth: 7.0,
    tall: 3.4,
    roof: Roof::Shingle,
    walls: Walls::Timber
  };
  let mut works = house(&plan, ground, roll);
  let side = Vec2::new(-(plan.length / 2.0 + 1.2), -1.0);
  let world = site.origin + site.frame.transform_point(side.extend(0.0).xzy());
  let level = crate::river::reach(world.xz())
    .map_or(ground(side), |reach| reach.level - site.origin.y);
  let radius = 2.8;
  works.spinners.push((
    Transform::from_xyz(side.x, level + radius - 0.6, side.y)
      .with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
    wheel(radius, roll)
  ));
  works.add(
    Stuff::Wood,
    beam(
      Vec3::new(side.x - 0.2, level + radius + 1.2, side.y),
      Vec3::new(side.x + 2.2, level + radius + 1.2, side.y),
      0.3,
      BEAM
    )
  );
  works
}

fn built(work: &Work, site: &Site, roll: &mut Roll) -> Works {
  let ground: Ground = &|offset| site.ground(offset);
  match work {
    Work::Mill { .. } => mill(site, roll),
    Work::House(house_plan) => house(house_plan, ground, roll),
    Work::Keep { .. } => keep(ground, roll),
    Work::Windmill { .. } => windmill(ground, roll),
    Work::Well { .. } => well(ground, roll),
    Work::Fence { path } => fence(path, ground, roll),
    Work::Field { size, crop, .. } => field(*size, *crop, ground, roll),
    Work::Rampart { from, to, tall } => rampart(*from, *to, *tall, ground, roll),
    Work::Tower { radius, tall, .. } => tower(*radius, *tall, ground, roll),
    Work::Gate { tall, .. } => gate(*tall, ground, roll),
    Work::Clutter { kind, .. } => clutter(*kind, ground, roll),
    Work::Tent { .. } => {
      let mut works = Works::default();
      let y = ground(Vec2::ZERO);
      landmark::tent(roll)
        .into_iter()
        .for_each(|(stuff, piece)| works.add(stuff, piece.at_xyz(0.0, y, 0.0)));
      works
    }
    Work::Mound { radius, .. } => mound(*radius, ground, roll),
    Work::Den { .. } => den(ground, roll),
    Work::Shrine { .. } => shrine(ground, roll),
    Work::Menhirs { .. } => menhirs(ground, roll),
    Work::Pillar { tall, .. } => pillar(*tall, ground, roll),
    Work::Rubble { .. } => rubble(ground, roll),
    Work::Fire { .. } => {
      let mut works = Works::default();
      let y = ground(Vec2::ZERO);
      landmark::campfire()
        .into_iter()
        .for_each(|(stuff, piece)| works.add(stuff, piece.at_xyz(0.0, y, 0.0)));
      works.fires.push(Vec3::new(0.0, y + 0.15, 0.0));
      works
    }
  }
}

fn anchor(work: &Work) -> (Vec2, f32) {
  match work {
    Work::House(House { at, facing, .. })
    | Work::Keep { at, facing }
    | Work::Windmill { at, facing }
    | Work::Field { at, facing, .. }
    | Work::Gate { at, facing, .. }
    | Work::Clutter { at, facing, .. }
    | Work::Tent { at, facing }
    | Work::Mill { at, facing }
    | Work::Mound { at, facing, .. }
    | Work::Den { at, facing }
    | Work::Shrine { at, facing } => (*at, *facing),
    Work::Well { at }
    | Work::Tower { at, .. }
    | Work::Fire { at }
    | Work::Menhirs { at }
    | Work::Pillar { at, .. }
    | Work::Rubble { at } => (*at, 0.0),
    Work::Fence { .. } | Work::Rampart { .. } => (Vec2::ZERO, 0.0)
  }
}

struct Raised {
  origin: Vec3,
  parts: Vec<(Stuff, Mesh)>,
  solids: Vec<(Vec3, Quat, Collider)>,
  spinners: Vec<(Transform, Vec<(Stuff, Mesh)>)>,
  hearths: Vec<Vec3>
}

fn merged(parts: Vec<(Stuff, Piece)>) -> Vec<(Stuff, Mesh)> {
  parts
    .into_iter()
    .fold(Vec::<(Stuff, Vec<Piece>)>::new(), |mut groups, (stuff, piece)| {
      match groups.iter_mut().find(|(each, _)| *each == stuff) {
        Some((_, list)) => list.push(piece),
        None => groups.push((stuff, vec![piece]))
      }
      groups
    })
    .into_iter()
    .map(|(stuff, pieces)| {
      let mesh = crate::model::merge(pieces);
      let mesh = matches!(
        stuff.grain(),
        crate::stuff::Grain::Masonry | crate::stuff::Grain::Shingles
      )
      .then(|| mesh.clone().with_generated_tangents().expect("building uvs"))
      .unwrap_or(mesh);
      (stuff, mesh)
    })
    .collect()
}

fn raise(index: usize) -> Raised {
  let layout = layout(index);
  let spot = layout.place.spot();
  let origin = spot.extend(height_at(spot)).xzy();
  let mut roll = Roll::new(index as u32 * 7919 + 17);
  let all = layout.works.iter().fold(Works::default(), |mut all, work| {
    let (at, facing) = anchor(work);
    let placed = matches!(work, Work::Fence { .. } | Work::Rampart { .. });
    let offset = placed.then_some(Vec2::ZERO).unwrap_or(at - spot);
    let frame = Transform::from_translation(offset.extend(0.0).xzy())
      .with_rotation(Quat::from_rotation_y(facing));
    let site = Site { origin, frame };
    let local_work = match work {
      Work::Fence { path } => {
        Work::Fence { path: path.iter().map(|point| *point - spot).collect() }
      }
      Work::Rampart { from, to, tall } => {
        Work::Rampart { from: *from - spot, to: *to - spot, tall: *tall }
      }
      other => other.clone()
    };
    all.absorb(built(&local_work, &site, &mut roll).placed(frame));
    all
  });
  Raised {
    origin,
    parts: merged(all.parts),
    solids: all.solids,
    spinners: all.spinners.into_iter().map(|(at, parts)| (at, merged(parts))).collect(),
    hearths: all.fires
  }
}

const RAISE_REACH: f32 = 2600.0;
const RAISE_NOW: f32 = 700.0;

#[derive(Component)]
struct Spin(f32);

#[derive(Resource, Default)]
struct Raising {
  raised: HashMap<usize, Entity>,
  building: HashMap<usize, Task<Raised>>
}

fn erect(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  effects: &crate::fx::Effects,
  index: usize,
  raised: Raised
) -> Entity {
  let Raised { origin, parts, solids, spinners, hearths } = raised;
  let root = commands
    .spawn((
      Name::new(layout(index).place.name()),
      Transform::from_translation(origin),
      Visibility::Inherited,
      RigidBody::Static,
      Collider::compound(
        solids
          .into_iter()
          .map(|(at, turn, collider)| (Position(at), turn, collider))
          .collect()
      )
    ))
    .id();
  let look = |(stuff, mesh): (Stuff, Mesh), meshes: &mut Assets<Mesh>| {
    (Mesh3d(meshes.add(mesh)), MeshMaterial3d(stuffs.of(stuff)))
  };
  parts.into_iter().for_each(|part| {
    let glowing = part.0 == Stuff::Ember;
    let mut child = commands.spawn((look(part, meshes), ChildOf(root)));
    if glowing {
      child.insert(NotShadowCaster);
    }
  });
  spinners.into_iter().for_each(|(hub, parts)| {
    let hub = commands.spawn((Spin(0.5), hub, Visibility::Inherited, ChildOf(root))).id();
    parts.into_iter().for_each(|part| {
      commands.spawn((look(part, meshes), ChildOf(hub)));
    });
  });
  hearths.into_iter().for_each(|at| {
    commands.spawn((
      Flicker(0.0),
      PointLight {
        color: Color::srgb(1.0, 0.62, 0.3),
        intensity: 400_000.0,
        range: 22.0,
        ..default()
      },
      Transform::from_translation(at + Vec3::Y * 0.7),
      ChildOf(root)
    ));
    commands.spawn((
      effects.emit(&effects.campfire),
      Transform::from_translation(at),
      ChildOf(root)
    ));
  });
  root
}

fn tend_settlements(
  mut commands: Commands,
  mut raising: ResMut<Raising>,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  effects: Res<crate::fx::Effects>,
  camera: Single<&Transform, With<MainCamera>>
) {
  let eye = camera.translation.xz();
  let first = raising.raised.is_empty() && raising.building.is_empty();
  let Raising { raised, building } = &mut *raising;
  let wanted: Vec<usize> = (0..LAYOUTS.len() + SITE_LAYOUTS.len())
    .filter(|index| {
      layout(*index).place.spot().distance(eye) < RAISE_REACH
        && !raised.contains_key(index)
        && !building.contains_key(index)
    })
    .collect();
  let (urgent, later): (Vec<usize>, Vec<usize>) = wanted
    .into_iter()
    .partition(|&index| first && layout(index).place.spot().distance(eye) < RAISE_NOW);
  crate::terrain::in_parallel(&urgent, |&index| (index, raise(index)))
    .into_iter()
    .for_each(|(index, made)| {
      let root = erect(&mut commands, &mut meshes, &stuffs, &effects, index, made);
      raised.insert(index, root);
    });
  later.into_iter().for_each(|index| {
    building
      .insert(index, AsyncComputeTaskPool::get().spawn(async move { raise(index) }));
  });
  let done: Vec<(usize, Raised)> = building
    .iter_mut()
    .filter_map(|(&index, task)| check_ready(task).map(|made| (index, made)))
    .collect();
  done.into_iter().for_each(|(index, made)| {
    building.remove(&index);
    let root = erect(&mut commands, &mut meshes, &stuffs, &effects, index, made);
    raised.insert(index, root);
  });
}

fn report_raising(raising: Res<Raising>, mut pending: ResMut<Pending>) {
  pending.0.insert("settlements", raising.building.len());
}

fn spin(time: Res<Time>, mut hubs: Query<(&Spin, &mut Transform)>) {
  hubs.iter_mut().for_each(|(&Spin(rate), mut transform)| {
    transform.rotate_local_z(rate * time.delta_secs());
  });
}

pub fn plugin(app: &mut App) {
  app.init_resource::<Raising>().add_systems(
    Update,
    (tend_settlements, report_raising.after(tend_settlements), spin)
  );
}
