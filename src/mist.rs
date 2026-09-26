use {crate::{cloud, noise,
             signal::Pending,
             sky::Clock,
             terrain::{self, smooth},
             work::{self, Job}},
     bevy::{asset::{RenderAssetUsages, embedded_asset},
            light::{NotShadowCaster, NotShadowReceiver},
            mesh::{Indices, PrimitiveTopology},
            pbr::{Material, MaterialPlugin},
            prelude::*,
            render::render_resource::AsBindGroup,
            shader::ShaderRef}};

const REACH: f32 = 7000.0;
const SPACING: f32 = 130.0;
const FOOT: f32 = 300.0;

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
struct Mist {
  #[uniform(0)]
  toward_light: Vec4,
  #[uniform(1)]
  light: Vec4,
  #[uniform(2)]
  ambient: Vec4,
  #[uniform(3)]
  drift: Vec4
}

impl Material for Mist {
  fn vertex_shader() -> ShaderRef { "embedded://skyrim2/mist.wgsl".into() }

  fn fragment_shader() -> ShaderRef { "embedded://skyrim2/mist.wgsl".into() }

  fn alpha_mode(&self) -> AlphaMode { AlphaMode::Premultiplied }

  fn enable_prepass() -> bool { false }

  fn enable_shadows() -> bool { false }
}

struct Bank {
  center: Vec3,
  size: f32,
  seed: f32,
  ground: Vec3
}

fn banks_in_row(row: usize) -> Vec<Bank> {
  let across = (2.0 * REACH / SPACING) as usize;
  (0..across)
    .filter_map(|column| {
      let cell = Vec2::new(column as f32, row as f32);
      let wobble = Vec2::new(
        noise::hash(column as i32, row as i32, 901),
        noise::hash(column as i32, row as i32, 903)
      );
      let at = (cell + wobble) * SPACING - REACH;
      let height = terrain::wild_height(at);
      let belt = 330.0 + 260.0 * (noise::fbm(at / 2600.0, 2, 905) * 0.5 + 0.5);
      let gathering =
        noise::fbm(at / 700.0, 3, 907) + 0.35 * noise::fbm(at / 2400.0, 2, 909);
      let chosen = height > FOOT
        && ((height - belt).abs() < 210.0 || (height - belt * 2.6).abs() < 260.0)
        && noise::hash(column as i32, row as i32, 911) < smooth(-0.25, 0.25, gathering);
      chosen.then(|| {
        let slope = |offset: Vec2| {
          (terrain::wild_height(at + offset * 30.0)
            - terrain::wild_height(at - offset * 30.0))
            / 60.0
        };
        let size = (90.0 + 170.0 * noise::hash(column as i32, row as i32, 913))
          * smooth(500.0, 1200.0, height).mul_add(0.6, 1.0);
        Bank {
          center: at.extend(height + size * 0.3 + 15.0).xzy(),
          size,
          seed: noise::hash(column as i32, row as i32, 915) * 100.0,
          ground: Vec3::new(height, slope(Vec2::X), slope(Vec2::Y))
        }
      })
    })
    .collect()
}

fn gather(banks: Vec<Bank>) -> Mesh {
  let corners = [
    Vec2::new(-1.0, -1.0),
    Vec2::new(1.0, -1.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(-1.0, 1.0)
  ];
  let positions: Vec<Vec3> = banks.iter().flat_map(|bank| [bank.center; 4]).collect();
  let grounds: Vec<Vec3> = banks.iter().flat_map(|bank| [bank.ground; 4]).collect();
  let shapes: Vec<Vec2> =
    banks.iter().flat_map(|bank| [Vec2::new(bank.size, bank.seed); 4]).collect();
  let uvs: Vec<Vec2> = banks.iter().flat_map(|_| corners).collect();
  let indices = (0..banks.len() as u32)
    .flat_map(|bank| [0, 1, 2, 0, 2, 3].map(|corner| bank * 4 + corner))
    .collect();
  Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, grounds)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, shapes)
    .with_inserted_indices(Indices::U32(indices))
}

#[derive(Resource)]
struct Gathering(Option<Job<Vec<Vec<Bank>>>>);

#[derive(Resource)]
struct Veil(Handle<Mist>);

fn start(mut commands: Commands, mut mists: ResMut<Assets<Mist>>) {
  commands.insert_resource(Veil(mists.add(Mist::default())));
  commands.insert_resource(Gathering(Some(work::spawn(work::rows(
    (2.0 * REACH / SPACING) as usize,
    banks_in_row
  )))));
}

fn settle(
  mut commands: Commands,
  mut gathering: ResMut<Gathering>,
  mut pending: ResMut<Pending>,
  mut meshes: ResMut<Assets<Mesh>>,
  veil: Res<Veil>
) {
  let finished = gathering.0.as_mut().and_then(Job::done);
  if let Some(rows) = finished {
    gathering.0 = None;
    let mesh = gather(rows.into_iter().flatten().collect());
    commands.spawn((
      Name::new("Mist"),
      Mesh3d(meshes.add(mesh)),
      MeshMaterial3d(veil.0.clone()),
      Transform::IDENTITY,
      bevy::camera::visibility::NoFrustumCulling,
      NotShadowCaster,
      NotShadowReceiver
    ));
  }
  pending.0.insert("mist", usize::from(gathering.0.is_some()));
}

fn light_mist(
  time: Res<Time>,
  clock: Res<Clock>,
  veil: Res<Veil>,
  mut mists: ResMut<Assets<Mist>>
) {
  let (toward_light, light, ambient) = cloud::lighting(clock.hour);
  if let Some(mut mist) = mists.get_mut(&veil.0) {
    *mist = Mist {
      toward_light: toward_light.extend(0.0),
      light: light.extend(0.0),
      ambient: ambient.extend(0.0),
      drift: Vec4::new(time.elapsed_secs(), 0.0, 0.0, 0.0)
    };
  }
}

pub fn plugin(app: &mut App) {
  embedded_asset!(app, "mist.wgsl");
  app
    .add_plugins(MaterialPlugin::<Mist>::default())
    .add_systems(Startup, start)
    .add_systems(Update, (settle, light_mist));
}

#[cfg(test)]
mod tests {
  #[test]
  #[ignore]
  fn banks() {
    let count: usize = (0..(2.0 * super::REACH / super::SPACING) as usize)
      .map(|row| super::banks_in_row(row).len())
      .sum();
    println!("{count} mist banks");
  }
}
