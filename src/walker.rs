use {crate::{humanoid::Motion,
             place::{LAKE, LAKE_LEVEL, LAKE_RADIUS},
             terrain::{BOUND, Ground}},
     avian3d::{math::AdjustPrecision, prelude::*},
     bevy::prelude::*};

const GROUND_PROBE: f32 = 0.2;
const STEP_DOWN: f32 = 0.6;
const TURN_RATE: f32 = 10.0;
const LANDING_SPEED: f32 = 1.0;
const SWIM_DEPTH: f32 = 1.3;
const FLOAT_DEPTH: f32 = 1.2;
const SWIM_PACE: f32 = 0.45;
const SURGE: f32 = 2.6;

#[derive(Component, Default)]
#[require(
  RigidBody::Kinematic,
  CustomPositionIntegration,
  SpeculativeMargin(0.0),
  LinearVelocity,
  Visibility
)]
pub struct Walker {
  pub wish: Vec3,
  pub facing: Option<Vec3>,
  pub leap: Option<f32>,
  pub grounded: bool,
  pub swimming: bool,
  pub shove: Vec3
}

fn walk(
  time: Res<Time>,
  gravity: Res<Gravity>,
  ground: Res<Ground>,
  mut walkers: Query<(
    Entity,
    &Collider,
    &mut Transform,
    &mut LinearVelocity,
    &mut Walker,
    Option<&mut Motion>
  )>,
  move_and_slide: MoveAndSlide
) {
  let delta = time.delta_secs().min(0.05);
  walkers.iter_mut().for_each(
    |(entity, collider, mut transform, mut velocity, mut walker, motion)| {
      let resting = walker.grounded
        && walker.leap.is_none()
        && walker.facing.is_none()
        && walker.wish == Vec3::ZERO
        && walker.shove.length_squared() < 1e-4
        && velocity.0.length_squared() < 1e-4;
      let (grounded, swimming) = if resting {
        velocity.0 = Vec3::ZERO;
        (true, false)
      } else {
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let &Transform { translation, rotation, .. } = &*transform;
        let feet = collider.aabb(translation.adjust_precision(), rotation).min.y;
        let float_line = LAKE_LEVEL - FLOAT_DEPTH;
        let swimming = translation.xz().distance(LAKE) < LAKE_RADIUS * 2.0
          && LAKE_LEVEL - ground.height(translation.xz()) > SWIM_DEPTH
          && feet < float_line + 0.3;
        let grounded = !swimming
          && (walker.grounded || velocity.y < LANDING_SPEED)
          && move_and_slide
            .spatial_query
            .cast_shape(
              collider,
              translation.adjust_precision(),
              rotation.adjust_precision(),
              Dir3::NEG_Y,
              &ShapeCastConfig::from_max_distance(GROUND_PROBE),
              &filter
            )
            .is_some_and(|hit| hit.normal1.y > 0.45);
        let leaping = walker.leap.take().filter(|_| grounded || swimming);
        let footed = grounded && leaping.is_none();
        walker.grounded = footed;
        walker.swimming = swimming;
        let buoyancy = ((float_line - feet) * 3.0).clamp(-2.0, 2.0);
        let rise = match leaping {
          Some(_) if swimming => SURGE,
          Some(height) => (-2.0 * gravity.0.y * height).sqrt(),
          None if swimming => velocity.y.lerp(buoyancy, 1.0 - (-3.0 * delta).exp()),
          None if grounded => 0.0,
          None => velocity.y + gravity.0.y * delta
        };
        let flat = velocity.0.with_y(0.0);
        let drive = if swimming {
          flat.lerp(walker.wish * SWIM_PACE, 1.0 - (-3.0 * delta).exp())
        } else if grounded {
          walker.wish
        } else {
          flat.lerp(walker.wish, 1.0 - (-1.5 * delta).exp())
        };
        let shove = walker.shove;
        walker.shove = shove * (-4.0 * delta).exp();
        velocity.0 = drive.with_y(rise) + shove;

        if let Some(facing) =
          walker.facing.or((walker.wish.length_squared() > 0.1).then_some(walker.wish))
          && facing.with_y(0.0).length_squared() > 0.0001
        {
          let target = Quat::from_rotation_y(f32::atan2(-facing.x, -facing.z));
          transform.rotation = transform
            .rotation
            .slerp(target, 1.0 - (-TURN_RATE * delta).exp())
            .normalize();
        }

        let MoveAndSlideOutput { position, projected_velocity } = move_and_slide
          .move_and_slide(
            collider,
            transform.translation.adjust_precision(),
            transform.rotation.adjust_precision(),
            velocity.0,
            std::time::Duration::from_secs_f32(delta),
            &MoveAndSlideConfig::default(),
            &filter,
            |_| MoveAndSlideHitResponse::Accept
          );
        let settled = footed
          .then(|| {
            move_and_slide.spatial_query.cast_shape(
              collider,
              position,
              transform.rotation.adjust_precision(),
              Dir3::NEG_Y,
              &ShapeCastConfig::from_max_distance(STEP_DOWN),
              &filter
            )
          })
          .flatten()
          .filter(|hit| hit.normal1.y > 0.45)
          .map_or(position, |hit| position - Vec3::Y * (hit.distance - 0.01).max(0.0));
        transform.translation = settled
          .clamp(Vec3::new(-BOUND, -500.0, -BOUND), Vec3::new(BOUND, 4000.0, BOUND));
        velocity.0 = projected_velocity;
        (grounded, swimming)
      };

      if let Some(mut motion) = motion {
        let speed = velocity.0.with_y(0.0).length();
        let paddling = swimming as u8 as f32;
        motion.speed = motion
          .speed
          .lerp(speed * (grounded || swimming) as u8 as f32, 1.0 - (-10.0 * delta).exp());
        motion.stride += (speed * 1.35 + paddling * 1.6) * delta;
        let aloft = (!grounded && !swimming && velocity.y.abs() > 1.0) as u8 as f32;
        motion.airborne = motion.airborne.lerp(aloft, 1.0 - (-8.0 * delta).exp());
        motion.swim = motion.swim.lerp(paddling, 1.0 - (-4.0 * delta).exp());
      }
    }
  );
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Walking;

pub fn plugin(app: &mut App) { app.add_systems(Update, walk.in_set(Walking)); }
