use {crate::{humanoid::Motion, terrain::BOUND},
     avian3d::{math::AdjustPrecision, prelude::*},
     bevy::prelude::*};

const GROUND_PROBE: f32 = 0.2;
const TURN_RATE: f32 = 10.0;

#[derive(Component, Default)]
#[require(RigidBody::Kinematic, CustomPositionIntegration, SpeculativeMargin(0.0), LinearVelocity, Visibility)]
pub struct Walker {
  pub wish: Vec3,
  pub facing: Option<Vec3>,
  pub leap: Option<f32>,
  pub grounded: bool,
  pub shove: Vec3
}

fn walk(
  time: Res<Time>,
  gravity: Res<Gravity>,
  mut walkers: Query<(Entity, &Collider, &mut Transform, &mut LinearVelocity, &mut Walker, Option<&mut Motion>)>,
  move_and_slide: MoveAndSlide
) {
  let delta = time.delta_secs().min(0.05);
  walkers.iter_mut().for_each(|(entity, collider, mut transform, mut velocity, mut walker, motion)| {
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let grounded = move_and_slide
      .spatial_query
      .cast_shape(
        collider,
        transform.translation.adjust_precision(),
        transform.rotation.adjust_precision(),
        Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(GROUND_PROBE),
        &filter
      )
      .is_some_and(|hit| hit.normal1.y < -0.45 || hit.normal2.y > 0.45);
    walker.grounded = grounded;
    let pull = gravity.0.y * delta;
    let rise = match walker.leap.take() {
      Some(height) if grounded => (-2.0 * gravity.0.y * height).sqrt(),
      _ if grounded => (velocity.y + pull).max(pull * 4.0),
      _ => velocity.y + pull
    };
    let drive = grounded
      .then_some(walker.wish)
      .unwrap_or(velocity.0.with_y(0.0).lerp(walker.wish, 1.0 - (-1.5 * delta).exp()));
    let shove = walker.shove;
    walker.shove = shove * (-4.0 * delta).exp();
    velocity.0 = drive.with_y(rise) + shove;

    if let Some(facing) = walker.facing.or((walker.wish.length_squared() > 0.1).then_some(walker.wish))
      && facing.with_y(0.0).length_squared() > 0.0001
    {
      let target = Quat::from_rotation_y(f32::atan2(-facing.x, -facing.z));
      transform.rotation = transform.rotation.slerp(target, 1.0 - (-TURN_RATE * delta).exp());
    }

    let MoveAndSlideOutput { position, projected_velocity } = move_and_slide.move_and_slide(
      collider,
      transform.translation.adjust_precision(),
      transform.rotation.adjust_precision(),
      velocity.0,
      std::time::Duration::from_secs_f32(delta),
      &MoveAndSlideConfig::default(),
      &filter,
      |_| MoveAndSlideHitResponse::Accept
    );
    transform.translation = position.clamp(Vec3::new(-BOUND, -500.0, -BOUND), Vec3::new(BOUND, 4000.0, BOUND));
    velocity.0 = projected_velocity;

    if let Some(mut motion) = motion {
      let speed = velocity.0.with_y(0.0).length();
      motion.speed = motion.speed.lerp(speed * grounded as u8 as f32, 1.0 - (-10.0 * delta).exp());
      motion.stride += speed * delta * 1.35;
      let aloft = (!grounded && velocity.y.abs() > 1.0) as u8 as f32;
      motion.airborne = motion.airborne.lerp(aloft, 1.0 - (-8.0 * delta).exp());
    }
  });
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Walking;

pub fn plugin(app: &mut App) { app.add_systems(Update, walk.in_set(Walking)); }
