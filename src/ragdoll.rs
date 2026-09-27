use {crate::{combat::Dead,
             dragon::{self, Dragon},
             humanoid::{self, Rig},
             player::Player,
             walker::{Layer, Walker},
             wolf::{self, Beast}},
     avian3d::prelude::*,
     bevy::{math::Affine3A, prelude::*}};

const DENSITY: f32 = 1000.0;
const TOPPLE: f32 = 0.8;
const FROZEN_BEYOND: f32 = 220.0;
const STOP_RATE: f32 = 30.0;
const MAX_SPIN: f32 = 12.0;
const MAX_SPEED: f32 = 30.0;

#[derive(Clone, Copy)]
pub enum Link {
  Root,
  Socket { swing: f32, twist: f32 },
  Hinge { low: f32, high: f32 }
}

#[derive(Clone, Copy)]
pub struct Limb {
  pub radius: f32,
  pub from: Vec3,
  pub to: Vec3,
  pub link: Link
}

#[derive(Component)]
pub struct Tumbling;

struct Strand {
  bone: Entity,
  parent: Option<usize>,
  part: Option<Entity>,
  rest: Vec3,
  scale: Vec3
}

struct Hinge {
  upper: Entity,
  lower: Entity,
  low: f32,
  high: f32
}

#[derive(Component)]
pub struct Ragdoll {
  body: Entity,
  strands: Vec<Strand>,
  joints: Vec<Entity>,
  hinges: Vec<Hinge>
}

fn collapse(
  mut commands: Commands,
  mut fallen: Query<
    (
      Entity,
      Option<&mut Walker>,
      Option<&LinearVelocity>,
      Option<&Rig>,
      Option<&Beast>,
      Option<&Dragon>
    ),
    Added<Dead>
  >,
  bones: Query<(&GlobalTransform, &Transform, &ChildOf)>
) {
  for (owner, walker, velocity, rig, beast, dragon) in fallen.iter_mut() {
    let plan: Vec<(Entity, Option<Limb>)> = rig
      .map(|rig| rig.bones.iter().copied().zip(humanoid::limbs(&rig.frame)).collect())
      .or_else(|| {
        beast.map(|beast| beast.bones.iter().copied().zip(wolf::limbs()).collect())
      })
      .or_else(|| {
        dragon.map(|dragon| dragon.bones.iter().copied().zip(dragon::limbs()).collect())
      })
      .unwrap_or_default();
    let found: Option<Vec<_>> = plan
      .iter()
      .map(|&(bone, limb)| {
        bones.get(bone).ok().map(|(world, local, parent)| {
          let (scale, rotation, translation) = world.to_scale_rotation_translation();
          (bone, limb, scale, rotation, translation, local.translation, parent.parent())
        })
      })
      .collect();
    if let Some(found) = found
      && let Some(&(.., body)) = found.first()
    {
      let shove =
        walker.map(|mut walker| std::mem::take(&mut walker.shove)).unwrap_or_default();
      let drift = velocity.map_or(Vec3::ZERO, |velocity| velocity.0)
        + dragon.map_or(Vec3::ZERO, |dragon| dragon.velocity);
      let feet = found.iter().map(|each| each.4.y).fold(f32::MAX, f32::min);
      let mut ragdoll =
        Ragdoll { body, strands: Vec::new(), joints: Vec::new(), hinges: Vec::new() };
      for &(bone, limb, scale, rotation, translation, rest, parent) in found.iter() {
        let parent = found.iter().position(|each| each.0 == parent);
        let part = limb.map(|limb| {
          commands
            .spawn((
              Name::new("Ragdoll limb"),
              Tumbling,
              RigidBody::Dynamic,
              Collider::capsule_endpoints(
                limb.radius * scale.x,
                limb.from * scale.x,
                limb.to * scale.x
              ),
              ColliderDensity(DENSITY),
              CollisionLayers::new(Layer::Limb, Layer::World),
              Friction::new(0.8),
              LinearDamping(0.2),
              AngularDamping(0.8),
              MaxAngularSpeed(MAX_SPIN),
              MaxLinearSpeed(MAX_SPEED),
              LinearVelocity(drift + shove * (1.0 + TOPPLE * (translation.y - feet))),
              Transform::from_translation(translation).with_rotation(rotation)
            ))
            .id()
        });
        if let Some(limb) = limb
          && let Some(lower) = part
          && let Some(above) = parent.map(|index| &ragdoll.strands[index])
          && let Some(upper) = above.part
        {
          let anchor = rest * above.scale;
          let damping = JointDamping { linear: 0.5, angular: 2.5 };
          match limb.link {
            Link::Root => {}
            Link::Socket { swing, twist } => ragdoll.joints.push(
              commands
                .spawn((
                  SphericalJoint::new(upper, lower)
                    .with_local_anchor1(anchor)
                    .with_swing_limits(-swing, swing)
                    .with_twist_limits(-twist, twist),
                  damping
                ))
                .id()
            ),
            Link::Hinge { low, high } => {
              ragdoll.hinges.push(Hinge { upper, lower, low, high });
              ragdoll.joints.push(
                commands
                  .spawn((
                    RevoluteJoint::new(upper, lower)
                      .with_hinge_axis(Vec3::X)
                      .with_local_anchor1(anchor),
                    damping
                  ))
                  .id()
              )
            }
          }
        }
        ragdoll.strands.push(Strand { bone, parent, part, rest, scale });
      }
      commands.entity(owner).insert((ragdoll, CollisionLayers::NONE));
    }
  }
}

fn stop_hinges(
  ragdolls: Query<&Ragdoll>,
  mut parts: Query<(&Rotation, &mut AngularVelocity), With<Tumbling>>
) {
  for hinge in ragdolls.iter().flat_map(|ragdoll| &ragdoll.hinges) {
    if let Ok([(upper, mut upper_spin), (lower, mut lower_spin)]) =
      parts.get_many_mut([hinge.upper, hinge.lower])
    {
      let relative = upper.0.inverse() * lower.0;
      let relative = (relative.w < 0.0).then(|| -relative).unwrap_or(relative);
      let angle = 2.0 * relative.x.atan2(relative.w);
      let excess = angle - angle.clamp(hinge.low, hinge.high);
      let axis = upper.0 * Vec3::X;
      let spin = (lower_spin.0 - upper_spin.0).dot(axis);
      let wanted = -excess * STOP_RATE;
      if (excess > 0.0 && spin > wanted) || (excess < 0.0 && spin < wanted) {
        let fix = axis * (wanted - spin) * 0.5;
        lower_spin.0 += fix;
        upper_spin.0 -= fix;
      }
    }
  }
}

fn dangle(
  mut ragdolls: Query<
    (&Ragdoll, &mut Transform, Option<&mut Walker>, Option<&mut LinearVelocity>),
    Without<Tumbling>
  >,
  parts: Query<(&Position, &Rotation), With<Tumbling>>,
  mut bones: Query<&mut Transform, (Without<Ragdoll>, Without<Tumbling>)>
) {
  for (ragdoll, mut place, walker, velocity) in ragdolls.iter_mut() {
    let posed = |strand: &Strand| {
      strand.part.and_then(|part| parts.get(part).ok()).map(|(position, rotation)| {
        Affine3A::from_scale_rotation_translation(strand.scale, rotation.0, position.0)
      })
    };
    if let Some(pelvis) = ragdoll.strands.first().and_then(posed) {
      place.translation = pelvis.translation.into();
    }
    if let Some(mut walker) = walker {
      walker.grounded = true;
      walker.shove = Vec3::ZERO;
    }
    if let Some(mut velocity) = velocity {
      velocity.0 = Vec3::ZERO;
    }
    if let Ok(body) = bones.get(ragdoll.body) {
      let body = place.compute_affine() * body.compute_affine();
      ragdoll.strands.iter().fold(Vec::<Affine3A>::new(), |mut worlds, strand| {
        let above = strand.parent.map_or(body, |index| worlds[index]);
        let world = match (posed(strand), bones.get_mut(strand.bone)) {
          (Some(world), Ok(mut local)) => {
            *local = Transform::from_matrix((above.inverse() * world).into());
            world
          }
          (None, Ok(local)) => above * local.compute_affine(),
          (_, Err(_)) => above
        };
        worlds.push(world);
        worlds
      });
    }
  }
}

fn settle(
  mut commands: Commands,
  hero: Single<&GlobalTransform, With<Player>>,
  ragdolls: Query<(Entity, &GlobalTransform), (With<Ragdoll>, Without<Player>)>
) {
  for (entity, _) in ragdolls
    .iter()
    .filter(|(_, at)| at.translation().distance(hero.translation()) > FROZEN_BEYOND)
  {
    commands.entity(entity).remove::<Ragdoll>();
  }
}

fn stiffen(
  mut commands: Commands,
  risen: Query<(Entity, &Ragdoll), Without<Dead>>,
  mut bones: Query<&mut Transform, Without<Ragdoll>>
) {
  for (entity, ragdoll) in risen.iter() {
    for strand in ragdoll.strands.iter() {
      if let Ok(mut bone) = bones.get_mut(strand.bone) {
        bone.translation = strand.rest;
      }
    }
    commands
      .entity(entity)
      .remove::<Ragdoll>()
      .insert(CollisionLayers::new(Layer::Walker, LayerMask::ALL));
  }
}

fn unravel(
  removed: On<Remove, Ragdoll>,
  ragdolls: Query<&Ragdoll>,
  mut commands: Commands
) {
  if let Ok(ragdoll) = ragdolls.get(removed.entity) {
    for entity in
      ragdoll.joints.iter().chain(ragdoll.strands.iter().flat_map(|strand| &strand.part))
    {
      commands.entity(*entity).try_despawn();
    }
  }
}

pub fn plugin(app: &mut App) {
  app
    .add_observer(unravel)
    .add_systems(FixedUpdate, stop_hinges)
    .add_systems(
      Update,
      (collapse, stiffen, settle).chain().after(crate::combat::Fighting)
    )
    .add_systems(PostUpdate, dangle.before(TransformSystems::Propagate));
}

#[cfg(test)]
mod tests {
  use {super::*,
       crate::{humanoid::{Grip, MAN},
               terrain::{self, Ground}},
       bevy::time::TimeUpdateStrategy,
       std::time::Duration};

  fn fall(ground: &Ground, spot: Vec2, shove: Vec3, frame: f32) -> (f32, f32) {
    let mut app = App::new();
    app.add_plugins((
      MinimalPlugins,
      PhysicsPlugins::default(),
      TransformPlugin,
      bevy::asset::AssetPlugin::default(),
      bevy::mesh::MeshPlugin,
      bevy::diagnostic::DiagnosticsPlugin
    ));
    app
      .add_observer(unravel)
      .add_systems(FixedUpdate, stop_hinges)
      .add_systems(Update, collapse)
      .add_systems(PostUpdate, dangle.before(TransformSystems::Propagate));
    app.finish();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
      frame
    )));
    let chunk = terrain::chunk_of(spot);
    app.world_mut().spawn((
      RigidBody::Static,
      terrain::chunk_collider(&ground.chunk(chunk)),
      Friction::new(0.8),
      Transform::from_translation(
        (chunk.as_vec2() * terrain::CHUNK_SIZE + terrain::CHUNK_SIZE / 2.0)
          .extend(0.0)
          .xzy()
      )
    ));
    let floor = ground.height(spot);
    let owner = app
      .world_mut()
      .spawn((
        Transform::from_translation(spot.extend(floor + MAN.hip).xzy()),
        Visibility::Inherited,
        Walker { shove, ..default() },
        Collider::capsule(0.34, 1.16)
      ))
      .id();
    let body = app
      .world_mut()
      .spawn((
        Transform::from_xyz(0.0, -MAN.hip, 0.0),
        Visibility::Inherited,
        ChildOf(owner)
      ))
      .id();
    let mut commands = app.world_mut().commands();
    let bones = humanoid::skeleton(&mut commands, body, MAN);
    commands.entity(owner).insert(Rig {
      bones,
      frame: MAN,
      grip: Grip::Blade,
      hunch: 0.0
    });
    app.update();
    app.update();
    app.world_mut().entity_mut(owner).insert(Dead);
    (0..(6.0 / frame) as usize).for_each(|_| app.update());
    let world = app.world_mut();
    let fastest = world
      .query_filtered::<&LinearVelocity, With<Tumbling>>()
      .iter(world)
      .map(|velocity| velocity.0.length())
      .fold(0.0, f32::max);
    let head = world.get::<GlobalTransform>(bones[2]).unwrap().translation();
    (fastest, head.y - ground.height(head.xz()))
  }

  #[test]
  fn ragdolls_come_to_rest_on_terrain() {
    let ground = Ground::default();
    for (spot, shove, frame) in [
      (Vec2::new(69.2, 286.9), Vec3::X * 3.0, 1.0 / 60.0),
      (Vec2::new(69.2, 286.9), Vec3::X * -4.0, 1.0 / 60.0),
      (Vec2::new(69.2, 286.9), Vec3::X * 8.0, 0.23),
      (Vec2::new(69.2, 286.9), Vec3::X * -2.0, 0.1),
      (Vec2::new(-250.0, -170.0), Vec3::Z * 5.0, 0.15)
    ] {
      let (fastest, head) = fall(&ground, spot, shove, frame);
      assert!(
        fastest < 1.5 && head < 0.8,
        "{spot} {shove} {frame}: fastest limb {fastest} m/s, head {head} m above ground"
      );
    }
  }
}
