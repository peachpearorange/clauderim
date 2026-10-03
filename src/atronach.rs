use {crate::{combat::{Dead, Fighting},
             humanoid::Motion,
             model::{self, Piece},
             noise::Roll,
             place,
             ragdoll::{Limb, Link, Ragdoll},
             signal::{FoeKind, FoeSpawn},
             stuff::{Stuff, Stuffs},
             terrain::{self, height_at}},
     bevy::{mesh::VertexAttributeValues, prelude::*},
     std::{f32::consts::TAU, sync::LazyLock}};

const TILE: f32 = 0.6;

const ICE: Srgba = Srgba::rgb(0.5, 0.82, 0.9);

fn shard(seed: u32, from: Vec3, to: Vec3, girth: impl Fn(f32) -> Vec2) -> Piece {
  let mut mesh = model::hewn(seed, 11, 0.0, 2);
  let raw: Vec<Vec3> = mesh
    .attribute(Mesh::ATTRIBUTE_POSITION)
    .and_then(VertexAttributeValues::as_float3)
    .map(|values| values.iter().copied().map(Vec3::from).collect())
    .unwrap_or_default();
  let (low, high) = raw.iter().fold((Vec3::MAX, Vec3::MIN), |(low, high), &point| {
    (low.min(point), high.max(point))
  });
  let (centre, half) = ((low + high) / 2.0, (high - low) / 2.0);
  let turn = Quat::from_rotation_arc(Vec3::Y, (to - from).normalize());
  let length = from.distance(to);
  let positions: Vec<Vec3> = raw
    .into_iter()
    .map(|point| {
      let unit = (point - centre) / half;
      let along = (unit.y + 1.0) / 2.0;
      let Vec2 { x: wide, y: deep } = girth(along) / 2.0;
      let placed = from + turn * Vec3::new(unit.x * wide, along * length, unit.z * deep);
      placed.with_y(placed.y.max(0.0))
    })
    .collect();
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh.compute_flat_normals();
  Piece::new(mesh, ICE).wrapped(from, to, TILE)
}

fn tapered(from: Vec2, to: Vec2) -> impl Fn(f32) -> Vec2 {
  move |along| from.lerp(to, along)
}

fn swelling(ends: Vec2, middle: Vec2, at: f32) -> impl Fn(f32) -> Vec2 {
  move |along| {
    let reach = (1.0 - ((along - at) / at.max(1.0 - at)).abs()).clamp(0.0, 1.0);
    ends.lerp(middle, reach.sqrt())
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
  Left,
  Right
}

impl Side {
  const BOTH: [Side; 2] = [Side::Left, Side::Right];

  fn sign(self) -> f32 {
    match self {
      Side::Left => -1.0,
      Side::Right => 1.0
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bone {
  Hips,
  Chest,
  Head,
  Arm(Side),
  Forearm(Side),
  Thigh(Side),
  Shin(Side)
}

impl Bone {
  const ALL: [Bone; 11] = [
    Bone::Hips,
    Bone::Chest,
    Bone::Head,
    Bone::Arm(Side::Left),
    Bone::Arm(Side::Right),
    Bone::Forearm(Side::Left),
    Bone::Forearm(Side::Right),
    Bone::Thigh(Side::Left),
    Bone::Thigh(Side::Right),
    Bone::Shin(Side::Left),
    Bone::Shin(Side::Right)
  ];

  fn parent(self) -> Option<Bone> {
    match self {
      Bone::Hips => None,
      Bone::Chest | Bone::Thigh(_) => Some(Bone::Hips),
      Bone::Head | Bone::Arm(_) => Some(Bone::Chest),
      Bone::Forearm(side) => Some(Bone::Arm(side)),
      Bone::Shin(side) => Some(Bone::Thigh(side))
    }
  }

  fn pivot(self) -> Vec3 {
    let at = |side: Side, x: f32, y: f32, z: f32| Vec3::new(side.sign() * x, y, z);
    match self {
      Bone::Hips => Vec3::new(0.0, 1.15, 0.0),
      Bone::Chest => Vec3::new(0.0, 1.4, 0.0),
      Bone::Head => Vec3::new(0.0, 2.4, 0.04),
      Bone::Arm(side) => at(side, 0.82, 2.48, 0.0),
      Bone::Forearm(side) => at(side, 0.97, 1.72, 0.08),
      Bone::Thigh(side) => at(side, 0.18, 1.2, 0.0),
      Bone::Shin(side) => at(side, 0.35, 0.86, 0.02)
    }
  }

  fn rest(self) -> Vec3 { self.pivot() - self.parent().map_or(Vec3::ZERO, Bone::pivot) }
}

pub fn body() -> Vec<(Bone, Mesh)> {
  let core = [
    (
      Bone::Head,
      shard(1, Vec3::new(0.0, 2.3, 0.02), Vec3::new(0.0, 3.25, 0.12), |along| {
        Vec2::new(0.78, 0.5).lerp(Vec2::new(0.02, 0.02), along.powf(0.75))
      })
    ),
    (
      Bone::Chest,
      shard(
        2,
        Vec3::new(0.0, 1.45, 0.0),
        Vec3::new(0.0, 2.62, 0.02),
        tapered(Vec2::new(0.36, 0.32), Vec2::new(1.3, 0.62))
      )
    ),
    (
      Bone::Chest,
      shard(
        3,
        Vec3::new(0.0, 1.1, 0.0),
        Vec3::new(0.0, 1.65, 0.0),
        swelling(Vec2::new(0.36, 0.3), Vec2::new(0.44, 0.36), 0.4)
      )
    ),
    (
      Bone::Hips,
      shard(
        4,
        Vec3::new(0.0, 0.98, -0.02),
        Vec3::new(0.0, 1.3, 0.0),
        swelling(Vec2::new(0.5, 0.36), Vec2::new(0.62, 0.42), 0.5)
      )
    )
  ];
  let limbs = |side: Side| {
    let at = |x: f32, y: f32, z: f32| Vec3::new(side.sign() * x, y, z);
    let seed = (side == Side::Right) as u32 * 20;
    [
      (
        Bone::Chest,
        shard(seed + 5, at(0.1, 2.3, 0.0), at(1.0, 2.62, 0.0), |along| {
          Vec2::new(0.62, 0.62).lerp(Vec2::new(0.12, 0.2), along.powf(1.6))
        })
      ),
      (
        Bone::Arm(side),
        shard(
          seed + 6,
          at(0.72, 2.6, 0.0),
          at(0.96, 1.55, 0.08),
          swelling(Vec2::new(0.4, 0.4), Vec2::new(0.44, 0.42), 0.5)
        )
      ),
      (
        Bone::Forearm(side),
        shard(seed + 7, at(0.95, 1.85, 0.08), at(1.14, 0.1, 0.22), |along| {
          swelling(Vec2::new(0.44, 0.4), Vec2::new(0.72, 0.52), 0.35)(along)
            * (1.0 - along.powf(2.2)).max(0.05)
        })
      ),
      (
        Bone::Thigh(side),
        shard(
          seed + 8,
          at(0.15, 1.25, 0.0),
          at(0.33, 0.82, 0.02),
          swelling(Vec2::new(0.36, 0.36), Vec2::new(0.42, 0.42), 0.45)
        )
      ),
      (
        Bone::Shin(side),
        shard(seed + 9, at(0.34, 1.0, 0.02), at(0.44, -0.12, 0.06), |along| {
          swelling(Vec2::new(0.42, 0.44), Vec2::new(0.48, 0.5), 0.3)(along)
            .lerp(Vec2::new(0.64, 0.76), along.powf(2.0))
        })
      )
    ]
  };
  let shards: Vec<(Bone, Piece)> =
    core.into_iter().chain(Side::BOTH.into_iter().flat_map(limbs)).collect();
  Bone::ALL
    .into_iter()
    .map(|bone| {
      let pieces = shards
        .iter()
        .filter(|(owner, _)| *owner == bone)
        .map(|(_, Piece(mesh))| Piece(mesh.clone()).at(-bone.pivot()));
      (bone, Stuff::Ice.fitted(model::merge(pieces)))
    })
    .collect()
}

pub fn limbs() -> [Option<Limb>; 11] {
  let limb =
    |radius: f32, to: Vec3, link: Link| Some(Limb { radius, from: Vec3::ZERO, to, link });
  let socket = Link::Socket { swing: 1.0, twist: 0.4 };
  let hinge = Link::Hinge { low: -1.6, high: 0.3 };
  Bone::ALL.map(|bone| match bone {
    Bone::Hips => Some(Limb {
      radius: 0.32,
      from: Vec3::new(-0.15, 0.0, 0.0),
      to: Vec3::new(0.15, 0.0, 0.0),
      link: Link::Root
    }),
    Bone::Chest => limb(0.48, Vec3::Y * 1.0, socket),
    Bone::Head => limb(0.24, Vec3::Y * 0.6, socket),
    Bone::Arm(side) => limb(0.21, Vec3::new(side.sign() * 0.14, -0.85, 0.08), socket),
    Bone::Forearm(side) => limb(0.26, Vec3::new(side.sign() * 0.17, -1.5, 0.14), hinge),
    Bone::Thigh(side) => limb(0.2, Vec3::new(side.sign() * 0.15, -0.3, 0.0), socket),
    Bone::Shin(side) => limb(0.3, Vec3::new(side.sign() * 0.09, -0.75, 0.04), hinge)
  })
}

#[derive(Component)]
pub struct Lumbering {
  pub bones: Vec<(Bone, Entity)>,
  stride: f32,
  offset: f32,
  shift: f32
}

fn rig(
  commands: &mut Commands,
  parts: &[(Bone, Handle<Mesh>)],
  ice: &Handle<StandardMaterial>,
  (root, owner): (Entity, Entity),
  (offset, shift): (f32, f32)
) {
  let bones =
    parts.iter().fold(Vec::new(), |mut bones: Vec<(Bone, Entity)>, (bone, mesh)| {
      let parent = bone
        .parent()
        .and_then(|parent| bones.iter().find(|(each, _)| *each == parent))
        .map_or(root, |&(_, entity)| entity);
      let entity = commands
        .spawn((
          Transform::from_translation(bone.rest()),
          Visibility::Inherited,
          Mesh3d(mesh.clone()),
          MeshMaterial3d(ice.clone()),
          ChildOf(parent)
        ))
        .id();
      bones.push((*bone, entity));
      bones
    });
  commands.entity(owner).insert(Lumbering { bones, stride: 0.0, offset, shift });
}

pub fn spawn(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  stuffs: &Stuffs,
  holder: Entity,
  offset: f32
) {
  let parts: Vec<(Bone, Handle<Mesh>)> =
    body().into_iter().map(|(bone, mesh)| (bone, meshes.add(mesh))).collect();
  rig(commands, &parts, &stuffs.of(Stuff::Ice), (holder, holder), (offset, offset))
}

#[derive(Component)]
pub struct Forming(pub Entity);

fn form(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  stuffs: Res<Stuffs>,
  mut parts: Local<Vec<(Bone, Handle<Mesh>)>>,
  forming: Query<(Entity, &Forming)>
) {
  if parts.is_empty() && !forming.is_empty() {
    *parts = body().into_iter().map(|(bone, mesh)| (bone, meshes.add(mesh))).collect()
  }
  let ice = stuffs.of(Stuff::Ice);
  for (entity, &Forming(body)) in &forming {
    let offset = Roll::new(entity.index().index()).range(0.0, 10.0);
    rig(&mut commands, &parts, &ice, (body, entity), (offset, 0.0));
    commands.entity(entity).remove::<Forming>();
  }
}

fn rise(from: f32, to: f32, at: f32) -> f32 {
  let t = ((at - from) / (to - from)).clamp(0.0, 1.0);
  t * t * (3.0 - 2.0 * t)
}

fn posed(
  bone: Bone,
  motion: &Motion,
  time: f32,
  stride: f32,
  shift: f32
) -> (Vec3, Quat) {
  let walk = (motion.speed / 2.5).clamp(0.0, 1.0);
  let still = 1.0 - walk;
  let (step, lift) = stride.sin_cos();
  let breath = (time * 1.3).sin();
  let attack = motion.swing.map_or(0.0, |progress| (progress + shift).fract());
  let swinging = motion.swing.is_some() as u8 as f32;
  let (raise, slam, settle) =
    (rise(0.0, 0.45, attack), rise(0.45, 0.62, attack), rise(0.75, 1.0, attack));
  let euler = |x: f32, y: f32, z: f32| Quat::from_euler(EulerRot::YXZ, y, x, z);
  match bone {
    Bone::Hips => (
      Vec3::Y
        * (0.02 * breath * still
          - 0.1 * walk * step.abs()
          - 0.08 * swinging * slam * (1.0 - settle)),
      euler(0.0, 0.08 * walk * step, 0.05 * walk * step)
    ),
    Bone::Chest => (
      Vec3::ZERO,
      euler(
        0.03 * breath * still
          + 0.12 * walk
          + swinging * (-0.18 * raise + 0.45 * slam - 0.27 * settle),
        -0.12 * walk * step + swinging * (0.25 * raise - 0.5 * slam + 0.25 * settle),
        0.0
      )
    ),
    Bone::Head => (
      Vec3::ZERO,
      euler(
        -0.06 * walk - 0.02 * breath * still
          + swinging * (0.1 * raise - 0.2 * slam + 0.1 * settle),
        0.2 * (time * 0.4).sin() * still + 0.06 * walk * step,
        0.0
      )
    ),
    Bone::Arm(side) => {
      let k = side.sign();
      let smash = (side == Side::Right) as u8 as f32 * swinging;
      (
        Vec3::ZERO,
        euler(
          -0.32 * walk * step * k
            + 0.05 * (time * 0.9 + k).sin() * still
            + smash * (3.0 * raise - 2.1 * slam - 0.9 * settle),
          0.0,
          k * (0.04 + 0.02 * breath * still) + smash * k * 0.25 * raise * (1.0 - settle)
        )
      )
    }
    Bone::Forearm(side) => {
      let k = side.sign();
      let smash = (side == Side::Right) as u8 as f32 * swinging;
      (
        Vec3::ZERO,
        euler(
          0.08 + 0.18 * walk * (-step * k).max(0.0) + smash * (0.9 * raise - 0.9 * slam),
          0.0,
          0.0
        )
      )
    }
    Bone::Thigh(side) => {
      let k = side.sign();
      (
        Vec3::ZERO,
        euler(0.42 * walk * step * k + 0.12 * swinging * slam * (1.0 - settle), 0.0, 0.0)
      )
    }
    Bone::Shin(side) => {
      let k = side.sign();
      (
        Vec3::ZERO,
        euler(
          -0.75 * walk * (lift * k).max(0.0) - 0.2 * swinging * slam * (1.0 - settle),
          0.0,
          0.0
        )
      )
    }
  }
}

fn pace(speed: f32) -> f32 { TAU * (0.6 + 0.15 * speed) }

pub fn animate(
  time: Res<Time>,
  mut frames: Query<(&mut Lumbering, &Motion), (Without<Dead>, Without<Ragdoll>)>,
  mut transforms: Query<&mut Transform, Without<Lumbering>>
) {
  for (mut lumbering, motion) in &mut frames {
    lumbering.stride += time.delta_secs() * pace(motion.speed);
    let &Lumbering { stride, offset, shift, .. } = &*lumbering;
    for &(bone, entity) in &lumbering.bones {
      if let Ok(mut transform) = transforms.get_mut(entity) {
        let (lift, turn) = posed(
          bone,
          motion,
          time.elapsed_secs() + offset,
          stride + offset * pace(motion.speed),
          shift
        );
        *transform = Transform::from_translation(bone.rest() + lift).with_rotation(turn)
      }
    }
  }
}

const HAUNTS: usize = 9;

fn snowfield(at: Vec2) -> bool {
  let height = height_at(at);
  let slope = Vec2::new(
    height_at(at + Vec2::X * 6.0) - height,
    height_at(at + Vec2::Y * 6.0) - height
  ) / 6.0;
  height > 240.0 && slope.length() < 0.35 && place::route_distance(at) > 12.0
}

static HOMES: LazyLock<Vec<Vec2>> = LazyLock::new(|| {
  let reach = terrain::WORLD - 150.0;
  let mut spots: Vec<Vec2> = (-60..=60)
    .flat_map(|row| {
      (-60..=60)
        .map(move |column| place::START + Vec2::new(column as f32, row as f32) * 45.0)
    })
    .filter(|spot| {
      spot.abs().max_element() < reach
        && (350.0..2900.0).contains(&spot.distance(place::START))
    })
    .filter(|&spot| snowfield(spot))
    .collect();
  spots.sort_by(|a, b| a.distance(place::START).total_cmp(&b.distance(place::START)));
  spots.into_iter().fold(Vec::new(), |mut homes, spot| {
    if homes.len() < HAUNTS && homes.iter().all(|home: &Vec2| home.distance(spot) > 550.0)
    {
      homes.push(spot)
    }
    homes
  })
});

fn haunt(mut commands: Commands) {
  for (index, &home) in HOMES.iter().enumerate() {
    commands.spawn((
      FoeSpawn { kind: FoeKind::FrostAtronach, dormant: false },
      Transform::from_translation(home.extend(height_at(home)).xzy()).with_rotation(
        Quat::from_rotation_y(Roll::new(index as u32 * 13 + 5).range(0.0, TAU))
      )
    ));
  }
}

pub fn plugin(app: &mut App) {
  app.add_systems(Startup, haunt).add_systems(Update, (form, animate.after(Fighting)));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn atronachs_haunt_snowfields_around_the_start() {
    println!(
      "{:?}",
      HOMES
        .iter()
        .map(|home| (home.round(), height_at(*home).round()))
        .collect::<Vec<_>>()
    );
    assert_eq!(HOMES.len(), HAUNTS);
    assert!(HOMES.iter().all(|&home| snowfield(home)));
  }
}
