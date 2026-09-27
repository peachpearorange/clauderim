use {crate::{cave::{self, BURROW, CARVED, Hoard, Kit, Lining, MASONRY, OLD_WOOD, PELT,
                    Works, bones, brazier, cobweb, flagstones, moved, nest, rubble,
                    sarcophagus, sconce, tomb, urn},
             inventory::{Item, Loot},
             model::{self, Piece},
             noise::{self, Roll},
             place::Place,
             player::{self, Player, View},
             sdf::{self, Bounds, Surface},
             settlement::{self, Depths, Entrance},
             signal::{FoeKind, Pending, Prompt, Prompting},
             stuff::{Stuff, Stuffs},
             terrain::{self, Surfaces, smooth},
             work::{self, Job}},
     avian3d::prelude::*,
     bevy::{color::Mix, platform::collections::HashMap, prelude::*},
     fidget::context::Tree,
     std::f32::consts::{FRAC_PI_2, PI}};

const BELOW: f32 = 420.0;
const SHELL: f64 = 2.0;
const DOORWAY: f32 = 2.6;
const PREPARE: f32 = 90.0;
const FORGET: f32 = 260.0;
const FADING: f32 = 2.5;
const HEADINGS: [Vec2; 4] = [Vec2::NEG_Y, Vec2::X, Vec2::NEG_X, Vec2::Y];
const ROCK_DARK: LinearRgba = terrain::srgb(0.30, 0.30, 0.30);

const TOMB_HOARDS: [&[Loot]; 3] = [
  &[
    Loot::Gold(96),
    Loot::one(Item::AncientNordWarAxe),
    Loot::one(Item::PotionOfMinorHealing)
  ],
  &[Loot::Gold(154), Loot::one(Item::Amethyst), Loot::one(Item::AncientNordHelmet)],
  &[
    Loot::Gold(71),
    Loot::Goods(Item::PotionOfMinorHealing, 2),
    Loot::one(Item::IronDagger)
  ]
];
const CAVE_HOARDS: [&[Loot]; 3] = [
  &[Loot::Gold(58), Loot::Goods(Item::WolfPelt, 2), Loot::Goods(Item::Lockpick, 3)],
  &[Loot::Gold(112), Loot::one(Item::SteelWarAxe), Loot::one(Item::PotionOfMinorHealing)],
  &[Loot::Gold(83), Loot::one(Item::FurArmor), Loot::one(Item::Amethyst)]
];

struct Room {
  floor: Vec3,
  half: Vec2,
  tall: f32
}

impl Room {
  fn extent(&self, heading: Vec2) -> f32 { (self.half * heading.abs()).element_sum() }

  fn apart(&self, center: Vec2, half: Vec2, margin: f32) -> bool {
    let gap = (self.floor.xz() - center).abs() - (self.half + half + margin);
    gap.max_element() > 0.0
  }
}

struct Link {
  from: usize,
  to: usize,
  heading: Vec2
}

struct Chart {
  rooms: Vec<Room>,
  links: Vec<Link>
}

impl Chart {
  fn ends(&self, link: &Link) -> (Vec3, Vec3) {
    let (from, to) = (&self.rooms[link.from], &self.rooms[link.to]);
    let start = from.floor.xz() + link.heading * (from.extent(link.heading) - 1.0);
    let end = to.floor.xz() - link.heading * (to.extent(link.heading) - 1.0);
    (start.extend(from.floor.y).xzy(), end.extend(to.floor.y).xzy())
  }

  fn opening(&self, room: usize, spot: Vec3) -> bool {
    self.links.iter().filter(|link| link.from == room || link.to == room).any(|link| {
      let (start, end) = self.ends(link);
      let door = (link.from == room).then_some(start).unwrap_or(end);
      door.xz().distance(spot.xz()) < 3.4
    })
  }
}

fn chart(depths: Depths, seed: u32) -> Chart {
  let mut roll = Roll::new(seed);
  let count = 5 + roll.below(3);
  let entry = Room {
    floor: Vec3::ZERO,
    half: Vec2::new(3.6, 5.0),
    tall: (depths == Depths::Tomb).then_some(4.6).unwrap_or(4.2)
  };
  (1..count).fold(Chart { rooms: vec![entry], links: Vec::new() }, |mut chart, index| {
    let last = index == count - 1;
    let placed = (0..30).find_map(|_| {
      let newest = chart.rooms.len() - 1;
      let parent = (last || roll.chance(0.65))
        .then_some(newest)
        .unwrap_or_else(|| roll.below(chart.rooms.len()));
      let heading = HEADINGS[roll.below(4)];
      let (half, tall) = match (depths, last) {
        (Depths::Tomb, false) => {
          (Vec2::new(roll.range(3.6, 6.2), roll.range(4.2, 7.5)), roll.range(4.4, 5.6))
        }
        (Depths::Tomb, true) => {
          (Vec2::new(roll.range(7.0, 8.4), roll.range(8.0, 9.5)), 6.5)
        }
        (Depths::Cave, false) => {
          (Vec2::new(roll.range(5.0, 8.5), roll.range(5.0, 8.5)), roll.range(4.2, 6.2))
        }
        (Depths::Cave, true) => {
          (Vec2::new(roll.range(8.5, 10.0), roll.range(8.5, 10.0)), 7.5)
        }
      };
      let from = &chart.rooms[parent];
      let gap = roll.range(7.0, 13.0);
      let sway = (depths == Depths::Cave).then(|| roll.spread(2.5)).unwrap_or(0.0);
      let center = from.floor.xz()
        + heading * (from.extent(heading) + gap + (half * heading.abs()).element_sum())
        + heading.perp() * sway;
      let drop = roll.range(0.0, 3.2);
      let passage = (from.floor.xz() + center) / 2.0;
      let passage_half =
        (heading.abs() * gap / 2.0 + heading.perp().abs() * 2.2).max(Vec2::ONE);
      let clear = !(parent == 0 && heading == Vec2::Y)
        && chart.rooms.iter().all(|other| other.apart(center, half, 4.0))
        && chart
          .rooms
          .iter()
          .enumerate()
          .filter(|&(other, _)| other != parent)
          .all(|(_, other)| other.apart(passage, passage_half, 1.5));
      clear.then(|| {
        (
          Room { floor: center.extend(from.floor.y - drop).xzy(), half, tall },
          parent,
          heading
        )
      })
    });
    if let Some((room, from, heading)) = placed {
      chart.links.push(Link { from, to: chart.rooms.len(), heading });
      chart.rooms.push(room);
    }
    chart
  })
}

fn scalar(value: f32) -> f64 { f64::from(value) }

fn room_shape(depths: Depths, room: &Room, roll: &mut Roll) -> Tree {
  let (x, y, z) = Tree::axes();
  let Room { floor, half, tall } = *room;
  match depths {
    Depths::Tomb => {
      let walls = sdf::at(
        sdf::cuboid(Vec3::new(half.x, tall * 0.35, half.y)),
        floor + Vec3::Y * tall * 0.35
      );
      let (span, length, along_x) = (half.x <= half.y)
        .then_some((half.x, half.y, false))
        .unwrap_or((half.y, half.x, true));
      let barrel = sdf::along_z(sdf::cylinder(span, length));
      let barrel =
        along_x.then(|| sdf::along_x(sdf::cylinder(span, length))).unwrap_or(barrel);
      let vault = barrel.remap_xyz(x, y / 0.55, z) * 0.55;
      sdf::union([walls, sdf::at(vault, floor + Vec3::Y * tall * 0.6)])
    }
    Depths::Cave => {
      let middle = floor + Vec3::Y * tall * 0.35;
      let lobes = (0..3).map(|_| {
        let offset = Vec3::new(
          roll.spread(half.x * 0.5),
          roll.range(-0.3, 0.8),
          roll.spread(half.y * 0.5)
        );
        let radii = Vec3::new(
          half.x * roll.range(0.45, 0.7),
          tall * roll.range(0.55, 0.8),
          half.y * roll.range(0.45, 0.7)
        );
        sdf::at(sdf::ellipsoid(radii), middle + offset)
      });
      let main = sdf::at(sdf::ellipsoid(Vec3::new(half.x, tall * 0.8, half.y)), middle);
      sdf::smooth_unions([main].into_iter().chain(lobes), 2.0).max(scalar(floor.y) - y)
    }
  }
}

fn passage_shape(depths: Depths, start: Vec3, end: Vec3) -> Tree {
  let (x, y, z) = Tree::axes();
  let flat = end.xz() - start.xz();
  let (length, along) = (flat.length(), flat.normalize());
  let across = along.perp();
  let dx = x - scalar(start.x);
  let dz = z - scalar(start.z);
  let s = dx.clone() * scalar(along.x) + dz.clone() * scalar(along.y);
  let t = dx * scalar(across.x) + dz * scalar(across.y);
  let floor = (s.clone() / scalar(length)).max(0.0).min(1.0) * scalar(end.y - start.y)
    + scalar(start.y);
  let rise = y - floor.clone();
  let cross = match depths {
    Depths::Tomb => {
      let square = (t.clone().abs() - 1.6).max(-rise.clone()).max(rise.clone() - 2.4);
      let arch =
        (((t / 1.6).square() + ((rise - 2.4) / 1.3).square()).sqrt() - 1.0) * 1.3;
      square.min(arch)
    }
    Depths::Cave => {
      let bent = t - (s.clone() * 0.21).sin() * 1.1;
      let tube = (((bent / 2.1).square() + ((rise.clone() - 1.3) / 1.9).square()).sqrt()
        - 1.0)
        * 1.9;
      tube.max(-rise)
    }
  };
  cross.max(-s.clone()).max(s - scalar(length))
}

fn carving(depths: Depths, chart: &Chart, seed: u32) -> Tree {
  let mut roll = Roll::new(seed + 7);
  let rooms: Vec<Tree> =
    chart.rooms.iter().map(|room| room_shape(depths, room, &mut roll)).collect();
  let passages = chart.links.iter().map(|link| {
    let (start, end) = chart.ends(link);
    passage_shape(depths, start, end)
  });
  let joined = rooms.into_iter().chain(passages);
  match depths {
    Depths::Tomb => sdf::union(joined),
    Depths::Cave => sdf::smooth_unions(joined, 1.2)
  }
}

fn span(chart: &Chart) -> (Vec3, Vec3) {
  chart.rooms.iter().fold((Vec3::MAX, Vec3::MIN), |(low, high), room| {
    let corner = room.half.extend(0.0).xzy();
    (
      low.min(room.floor - corner),
      high.max(room.floor + corner + Vec3::Y * (room.tall + room.half.min_element()))
    )
  })
}

fn hollow(carve: Tree, (low, high): (Vec3, Vec3), lining: &Lining) -> (Mesh, Collider) {
  let center = (low + high) / 2.0;
  let half_extent = ((high - low).max_element() / 2.0 + 4.0).max(8.0);
  let depth = ((half_extent * 2.0 / 0.55).log2().ceil() as u8).clamp(6, 8);
  let shell =
    sdf::surface(sdf::difference(carve.clone() - SHELL, carve.clone()), &Bounds {
      center,
      half_extent,
      depth
    })
    .refined(1.3);
  let probed = sdf::probe(carve, &shell.vertices);
  let triangles: Vec<[usize; 3]> = shell
    .triangles
    .iter()
    .copied()
    .filter(|corners| corners.iter().any(|&corner| probed[corner].0 < 1.0))
    .collect();
  let vertices: Vec<Vec3> = shell
    .vertices
    .iter()
    .zip(&probed)
    .map(|(&at, &(_, slope))| {
      let floor = smooth(0.5, 0.8, -slope.y);
      let rough = cave::field(at, 4.0, 11) * 2.0 + cave::field(at, 1.6, 13);
      at + rough * lining.roughen * Vec3::new(1.0, 1.0 - floor, 1.0)
    })
    .collect();
  let surface = Surface { vertices, triangles };
  let mesh = surface.mesh(0.12, lining.crease, |index, normal| {
    let at = surface.vertices[index];
    let grain = noise::fbm3(at / 7.0, 3, 21) + 0.3 * noise::fbm3(at / 2.0, 2, 23);
    let base = (normal.y > 0.6).then_some(lining.floor).unwrap_or(lining.wall);
    base.mix(
      &ROCK_DARK,
      smooth(-0.2, 0.3, grain) * 0.5 + smooth(0.3, -0.8, normal.y) * 0.25
    )
  });
  let Surface { vertices, triangles } = surface;
  (
    mesh,
    Collider::trimesh(
      vertices,
      triangles.into_iter().map(|corners| corners.map(|index| index as u32)).collect()
    )
  )
}

fn wall_spot(room: &Room, side: Vec2, along: f32, lift: f32, inset: f32) -> (Vec3, Vec3) {
  let inward = -side.extend(0.0).xzy();
  let reach = room.extent(side) - inset;
  let at = room.floor.xz() + side * reach + side.perp() * along;
  (at.extend(room.floor.y + lift).xzy(), inward)
}

fn facing(at: Vec3, inward: Vec3) -> Transform {
  Transform::from_translation(at).looking_to(-inward, Vec3::Y)
}

fn exit_door(works: &mut Works, depths: Depths, entry: &Room) {
  let (at, inward) = wall_spot(entry, Vec2::Y, 0.0, 0.0, 0.05);
  match depths {
    Depths::Tomb => {
      works.slab(CARVED, Vec3::new(4.6, 0.9, 0.8), facing(at + Vec3::Y * 4.4, inward));
      for side in [-1.0, 1.0] {
        works.slab(
          CARVED,
          Vec3::new(0.9, 4.4, 0.8),
          facing(at + Vec3::new(side * 1.85, 2.2, 0.0), inward)
        );
        let leaf = Piece::new(model::block(1.4, 3.9, 0.14), OLD_WOOD).at_xyz(
          side * 0.72,
          1.95,
          -0.3
        );
        works.parts.push((Stuff::Wood, leaf.moved(facing(at, inward))));
        for height in [0.8, 1.9, 3.0] {
          works.parts.push((
            Stuff::Iron,
            Piece::new(model::block(1.42, 0.12, 0.17), cave::IRON)
              .at_xyz(side * 0.72, height, -0.22)
              .moved(facing(at, inward))
          ));
        }
      }
    }
    Depths::Cave => {
      works.parts.push((
        Stuff::Leather,
        Piece::new(model::block(3.0, 3.2, 0.12), PELT)
          .at_xyz(0.0, 1.6, -0.4)
          .moved(facing(at, inward))
      ));
      works.parts.push((
        Stuff::Wood,
        Piece::new(model::rod(0.09, 3.6), OLD_WOOD)
          .rolled(FRAC_PI_2)
          .at_xyz(0.0, 3.25, -0.4)
          .moved(facing(at, inward))
      ));
      works.lamps.push((
        at + inward * 1.5 + Vec3::Y * 2.6,
        Color::srgb(0.85, 0.88, 0.95),
        160000.0,
        12.0
      ));
    }
  }
}

fn furnish_tomb(chart: &Chart, seed: u32) -> Works {
  let mut works = Works::default();
  let mut roll = Roll::new(seed + 101);
  let last = chart.rooms.len() - 1;
  exit_door(&mut works, Depths::Tomb, &chart.rooms[0]);
  for (index, room) in chart.rooms.iter().enumerate() {
    let low = room.floor.xz() - room.half;
    works.parts.extend(flagstones(
      seed + index as u32,
      low,
      room.floor.xz() + room.half,
      room.floor.y,
      |_| true
    ));
    for (turn, side) in [Vec2::X, Vec2::NEG_X].into_iter().enumerate() {
      let along = room.half.y * 0.55 * (turn as f32 * 2.0 - 1.0);
      let (at, inward) = wall_spot(room, side, along, 2.1, 0.02);
      (!chart.opening(index, at)).then(|| {
        works.put(sconce(), facing(at, inward));
        works.fire(at + inward * 0.34 + Vec3::Y * 0.36, 0.5, 45000.0, false);
      });
    }
    for pot in 0..3 {
      let side = HEADINGS[roll.below(4)];
      let along = roll.spread((room.half * side.perp().abs()).element_sum() - 0.8);
      let (at, _) = wall_spot(room, side, along, 0.0, 0.5);
      (!chart.opening(index, at)).then(|| {
        works
          .put(urn(seed * 13 + index as u32 * 5 + pot), Transform::from_translation(at))
      });
    }
    for pile in 0..1 + roll.below(2) {
      let at = room.floor
        + Vec3::new(roll.spread(room.half.x - 1.0), 0.0, roll.spread(room.half.y - 1.0));
      works.put(
        bones(seed * 7 + index as u32 * 3 + pile as u32),
        Transform::from_translation(at)
      );
    }
    for side in [Vec2::X, Vec2::NEG_X] {
      let (at, inward) = wall_spot(room, side, room.half.y - 0.4, room.tall * 0.62, 0.05);
      works.put(cobweb(roll.range(0.7, 1.1)), facing(at, inward));
    }
    let middle = index > 0 && index < last;
    if middle && room.half.x > 4.4 && room.half.y > 5.0 {
      works.put(
        tomb(),
        Transform::from_translation(room.floor).with_rotation(Quat::from_rotation_y(
          (room.half.x > room.half.y) as u8 as f32 * FRAC_PI_2
        ))
      );
      works.solid(
        Transform::from_translation(room.floor + Vec3::Y * 0.5).with_rotation(
          Quat::from_rotation_y((room.half.x > room.half.y) as u8 as f32 * FRAC_PI_2)
        ),
        Vec3::new(1.3, 1.0, 2.6)
      );
    }
    if middle {
      let side = roll.chance(0.5).then_some(Vec2::X).unwrap_or(Vec2::NEG_X);
      let (at, inward) = wall_spot(room, side, roll.spread(1.2), 0.0, 0.5);
      (!chart.opening(index, at)).then(|| {
        works.put(sarcophagus(), facing(at, inward));
        works.foe(at + inward * 0.15, inward, FoeKind::Draugr, true);
      });
      (roll.chance(0.7)).then(|| {
        let spot = room.floor + Vec3::new(roll.spread(1.5), 0.0, roll.spread(1.5));
        works.foe(spot, Vec3::new(roll.spread(1.0), 0.0, 1.0), FoeKind::Draugr, false);
      });
    }
  }
  let hall = &chart.rooms[last];
  let heading = chart
    .links
    .iter()
    .find(|link| link.to == last)
    .map_or(Vec2::NEG_Y, |link| link.heading);
  let dais = hall.floor + (heading * (hall.extent(heading) - 3.2)).extend(0.0).xzy();
  let toward = -heading.extend(0.0).xzy();
  works.slab(CARVED, Vec3::new(6.0, 0.6, 3.6), facing(dais + Vec3::Y * 0.3, toward));
  works.hoards.push(Hoard {
    at: facing(dais + Vec3::Y * 0.6 - toward * 0.4, toward),
    size: 1.0,
    noun: "Ancient Nord Chest",
    loot: TOMB_HOARDS[seed as usize % TOMB_HOARDS.len()]
  });
  for side in [-1.0, 1.0] {
    let foot = dais + toward * 3.0 + heading.perp().extend(0.0).xzy() * side * 3.6;
    works.put(brazier(), Transform::from_translation(foot));
    works
      .solid(Transform::from_translation(foot + Vec3::Y * 0.5), Vec3::new(0.8, 1.0, 0.8));
    works.fire(foot + Vec3::Y * 1.0, 1.1, 140000.0, side > 0.0);
  }
  works.foe(dais + toward * 5.0, toward, FoeKind::DraugrOverlord, true);
  works.foe(
    dais + toward * 6.5 + heading.perp().extend(0.0).xzy() * 2.5,
    toward,
    FoeKind::Draugr,
    false
  );
  works
}

fn furnish_cave(chart: &Chart, seed: u32) -> Works {
  let mut works = Works::default();
  let mut roll = Roll::new(seed + 211);
  let last = chart.rooms.len() - 1;
  let bandits = roll.chance(0.5);
  exit_door(&mut works, Depths::Cave, &chart.rooms[0]);
  for (index, room) in chart.rooms.iter().enumerate() {
    let glow = bandits
      .then_some(Color::srgb(1.0, 0.75, 0.5))
      .unwrap_or(Color::srgb(0.6, 0.75, 1.0));
    works.lamps.push((
      room.floor + Vec3::Y * room.tall * 0.7,
      glow,
      70000.0 + 12000.0 * room.half.max_element(),
      room.half.max_element() * 2.4
    ));
    for pile in 0..1 + roll.below(2) {
      let side = HEADINGS[roll.below(4)];
      let (at, _) = wall_spot(room, side, roll.spread(2.5), 0.0, 1.8);
      works.parts.extend(moved(
        rubble(seed * 3 + index as u32 * 11 + pile as u32, 12, 2.2, 1.5),
        Transform::from_translation(at)
      ));
    }
    for pile in 0..2 + roll.below(3) {
      let at = room.floor
        + Vec3::new(roll.spread(room.half.x * 0.6), 0.02, roll.spread(room.half.y * 0.6));
      works.put(
        bones(seed * 5 + index as u32 * 7 + pile as u32),
        Transform::from_translation(at)
      );
    }
    let middle = index > 0 && index < last;
    match (bandits, middle) {
      (false, true) => {
        let at = room.floor + Vec3::new(roll.spread(2.0), 0.03, roll.spread(2.0));
        works.put(nest(seed + index as u32), Transform::from_translation(at));
        for pack in 0..1 + roll.below(2) {
          works.foe(
            at + Vec3::new(pack as f32 * 1.4 - 0.6, 0.0, 0.8),
            Vec3::new(roll.spread(1.0), 0.0, 1.0),
            FoeKind::Wolf,
            false
          );
        }
      }
      (true, true) => {
        let hearth = room.floor + Vec3::new(roll.spread(1.5), 0.0, roll.spread(1.5));
        works.parts.extend(moved(
          rubble(seed + index as u32 * 17, 8, 0.9, 0.5),
          Transform::from_translation(hearth)
        ));
        works.fire(hearth + Vec3::Y * 0.3, 0.9, 90000.0, false);
        for guard in 0..1 + roll.below(2) {
          let angle = roll.range(0.0, PI * 2.0);
          let at =
            hearth + Vec3::new(angle.cos(), 0.0, angle.sin()) * (2.4 + guard as f32);
          works.foe(at, hearth - at, FoeKind::Bandit, false);
        }
      }
      _ => {}
    }
  }
  let den = &chart.rooms[last];
  let heading = chart
    .links
    .iter()
    .find(|link| link.to == last)
    .map_or(Vec2::NEG_Y, |link| link.heading);
  let back = den.floor + (heading * (den.extent(heading) - 2.6)).extend(0.02).xzy();
  let toward = -heading.extend(0.0).xzy();
  works.hoards.push(Hoard {
    at: facing(back, toward),
    size: 0.8,
    noun: bandits.then_some("Bandit Chest").unwrap_or("Weathered Chest"),
    loot: CAVE_HOARDS[seed as usize % CAVE_HOARDS.len()]
  });
  match bandits {
    true => {
      let hearth = den.floor;
      works.parts.extend(moved(
        rubble(seed + 999, 9, 1.0, 0.55),
        Transform::from_translation(hearth)
      ));
      works.fire(hearth + Vec3::Y * 0.3, 1.1, 120000.0, true);
      works.foe(back + toward * 3.5, toward, FoeKind::BanditChief, false);
      works.foe(hearth + Vec3::new(2.5, 0.0, 0.5), toward, FoeKind::Bandit, false);
    }
    false => {
      works.put(nest(seed + 77), Transform::from_translation(den.floor));
      for pack in 0..3 {
        works.foe(
          den.floor + Vec3::new(pack as f32 * 1.6 - 1.6, 0.0, 1.2),
          toward,
          FoeKind::Wolf,
          false
        );
      }
    }
  }
  works
}

struct Hewn {
  rock: (Mesh, Collider),
  works: Works,
  exit: Vec3
}

fn hew(depths: Depths, seed: u32) -> Hewn {
  let chart = chart(depths, seed);
  let lining = match depths {
    Depths::Tomb => &MASONRY,
    Depths::Cave => &BURROW
  };
  let rock = hollow(carving(depths, &chart, seed), span(&chart), lining);
  let (low, high) = span(&chart);
  let mut works = match depths {
    Depths::Tomb => furnish_tomb(&chart, seed),
    Depths::Cave => furnish_cave(&chart, seed)
  };
  works.rooms.push((low - Vec3::splat(3.0), high + Vec3::splat(3.0)));
  let entry = &chart.rooms[0];
  Hewn { rock, works, exit: entry.floor + Vec3::Z * (entry.half.y - 1.6) }
}

struct Delved {
  root: Entity,
  foes: Vec<Entity>,
  origin: Vec3,
  exit: Vec3
}

#[derive(Clone, Copy)]
enum Going {
  Down(Place),
  Up(Place)
}

#[derive(Resource, Default)]
struct Delving {
  veil: f32,
  going: Option<Going>,
  delved: HashMap<Place, Delved>,
  hewing: HashMap<Place, Job<Hewn>>
}

#[derive(Component)]
struct Veil;

fn entrance(place: Place) -> Option<Entrance> {
  settlement::layouts()
    .find(|layout| layout.place == place)
    .and_then(|layout| layout.entrance)
}

fn origin(entrance: Entrance) -> Vec3 {
  entrance.at.extend(terrain::height_at(entrance.at) - BELOW).xzy()
}

fn seed_of(place: Place) -> u32 {
  let spot = place.spot();
  ((spot.x as i32 as u32).wrapping_mul(73_856_093)
    ^ (spot.y as i32 as u32).wrapping_mul(19_349_663))
    % 65_536
}

fn spawn_veil(mut commands: Commands) {
  commands.spawn((
    Veil,
    Node {
      position_type: PositionType::Absolute,
      width: percent(100.0),
      height: percent(100.0),
      ..default()
    },
    BackgroundColor(Color::BLACK.with_alpha(0.0)),
    GlobalZIndex(90),
    Pickable::IGNORE
  ));
}

fn plunge(mut delving: ResMut<Delving>) {
  let wanted = crate::opts::opts()
    .delve
    .as_deref()
    .map(|kind| (kind == "cave").then_some(Depths::Cave).unwrap_or(Depths::Tomb));
  let nearest = wanted.and_then(|depths| {
    settlement::layouts()
      .filter(|layout| layout.entrance.is_some_and(|entrance| entrance.depths == depths))
      .min_by(|a, b| {
        let gap = |place: Place| place.spot().distance(crate::place::START);
        gap(a.place).total_cmp(&gap(b.place))
      })
      .map(|layout| layout.place)
  });
  if let Some(place) = nearest {
    delving.going = Some(Going::Down(place));
    delving.veil = 1.0;
  }
}

fn prepare(
  mut delving: ResMut<Delving>,
  mut pending: ResMut<Pending>,
  player: Single<&Transform, With<Player>>
) {
  let at = player.translation.xz();
  let wanted: Vec<(Place, Depths)> = settlement::layouts()
    .filter_map(|layout| layout.entrance.map(|entrance| (layout.place, entrance)))
    .filter(|(_, entrance)| entrance.at.distance(at) < PREPARE)
    .map(|(place, entrance)| (place, entrance.depths))
    .collect();
  let Delving { delved, hewing, going, .. } = &mut *delving;
  let delving_going = going.is_some();
  for (place, depths) in wanted.into_iter().filter(|_| !cfg!(target_arch = "wasm32")) {
    if !delved.contains_key(&place) && !hewing.contains_key(&place) {
      hewing.insert(place, work::task(move || hew(depths, seed_of(place))));
    }
  }
  pending.0.insert("depths", hewing.len() + usize::from(delving_going));
}

#[allow(clippy::too_many_arguments)]
fn delve(
  time: Res<Time>,
  keys: Res<ButtonInput<KeyCode>>,
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  stuffs: Res<Stuffs>,
  surfaces: Res<Surfaces>,
  mut delving: ResMut<Delving>,
  mut prompt: ResMut<Prompt>,
  mut view: ResMut<View>,
  mut daylight: ResMut<crate::sky::Daylight>,
  mut veil: Single<&mut BackgroundColor, With<Veil>>,
  player: Single<(&mut Transform, &mut LinearVelocity), With<Player>>
) {
  let (mut body, mut velocity) = player.into_inner();
  let at = body.translation;
  let delving = &mut *delving;
  let finished: Vec<(Place, Hewn)> = delving
    .hewing
    .iter_mut()
    .filter_map(|(&place, job)| job.done().map(|hewn| (place, hewn)))
    .collect();
  for (place, Hewn { rock, works, exit }) in finished {
    delving.hewing.remove(&place);
    if let Some(entrance) = entrance(place) {
      let origin = origin(entrance);
      let mut kit = Kit {
        commands: &mut commands,
        meshes: &mut meshes,
        materials: &mut materials,
        stuffs: &stuffs,
        surfaces: &surfaces
      };
      let (root, foes) =
        kit.raise(place.name(), Transform::from_translation(origin), rock, works);
      delving.delved.insert(place, Delved { root, foes, origin, exit: origin + exit });
    }
  }
  let outside = settlement::layouts()
    .filter_map(|layout| layout.entrance.map(|entrance| (layout.place, entrance)))
    .find(|(_, entrance)| {
      entrance.at.distance(at.xz()) < DOORWAY
        && at.y > terrain::height_at(entrance.at) - 6.0
    });
  let inside = delving
    .delved
    .iter()
    .find(|(_, delved)| delved.exit.distance(at - Vec3::Y * 0.9) < DOORWAY)
    .map(|(&place, _)| place);
  if delving.going.is_none() {
    if let Some((place, _)) = outside {
      prompt.0 = Some(Prompting { verb: "Enter".into(), noun: place.name().into() });
      if keys.just_pressed(KeyCode::KeyE) {
        delving.going = Some(Going::Down(place));
      }
    } else if let Some(place) = inside {
      prompt.0 = Some(Prompting { verb: "Leave".into(), noun: place.name().into() });
      if keys.just_pressed(KeyCode::KeyE) {
        delving.going = Some(Going::Up(place));
      }
    }
  }
  let arrived = match delving.going {
    Some(Going::Down(place)) if delving.veil >= 1.0 => {
      if !delving.delved.contains_key(&place)
        && !delving.hewing.contains_key(&place)
        && let Some(entrance) = entrance(place)
      {
        let depths = entrance.depths;
        delving.hewing.insert(place, work::task(move || hew(depths, seed_of(place))));
      }
      delving.delved.get(&place).map(|delved| {
        view.yaw = 0.0;
        delved.exit + Vec3::new(0.0, player::capsule_offset() + 0.3, -3.2)
      })
    }
    Some(Going::Up(place)) if delving.veil >= 1.0 => entrance(place).map(|entrance| {
      let spot = entrance.at + entrance.outward * 2.5;
      view.yaw = f32::atan2(-entrance.outward.x, -entrance.outward.y);
      spot.extend(terrain::height_at(spot) + player::capsule_offset() + 0.4).xzy()
    }),
    _ => None
  };
  if let Some(spot) = arrived {
    body.translation = spot;
    velocity.0 = Vec3::ZERO;
    delving.going = None;
    daylight.snap = 3;
  }
  let dark = delving.going.is_some();
  delving.veil = (delving.veil
    + time.delta_secs() * FADING * dark.then_some(1.0).unwrap_or(-0.8))
  .clamp(0.0, 1.0);
  veil.0 = Color::BLACK.with_alpha(smooth(0.0, 1.0, delving.veil));
  let forgotten: Vec<Place> = delving
    .delved
    .iter()
    .filter(|(_, delved)| {
      at.y > delved.origin.y + BELOW * 0.5
        && delved.origin.xz().distance(at.xz()) > FORGET
    })
    .map(|(&place, _)| place)
    .collect();
  for place in forgotten {
    if let Some(delved) = delving.delved.remove(&place) {
      commands.entity(delved.root).despawn();
      for foe in delved.foes {
        commands.entity(foe).try_despawn();
      }
    }
  }
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Delving>()
    .add_systems(Startup, (spawn_veil, plunge))
    .add_systems(Update, (prepare, delve.before(crate::walker::Walking)));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[ignore]
  fn charts() {
    for depths in [Depths::Tomb, Depths::Cave] {
      for seed in 0..4 {
        let chart = chart(depths, seed * 977);
        let start = web_time::Instant::now();
        let hewn = hew(depths, seed * 977);
        println!(
          "{depths:?} {seed}: {} rooms, {} links, {} foes? exit {} hewn in {:?}",
          chart.rooms.len(),
          chart.links.len(),
          hewn.works.hoards.len(),
          hewn.exit,
          start.elapsed()
        );
      }
    }
  }
}
