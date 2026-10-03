use {bevy::{asset::RenderAssetUsages,
            mesh::{Indices, PrimitiveTopology},
            prelude::*},
     fidget::{context::Tree,
              mesh::{Octree, Settings},
              shape::BoundShape,
              types::Grad,
              vm::{VmFunction, VmShape}},
     nalgebra::{Matrix4, Vector3}};

pub fn scalar(value: f32) -> f64 { f64::from(value) }

pub fn ellipsoid(radii: Vec3) -> Tree {
  let (x, y, z) = Tree::axes();
  let stretched = ((x / scalar(radii.x)).square()
    + (y / scalar(radii.y)).square()
    + (z / scalar(radii.z)).square())
  .sqrt();
  (stretched - 1.0) * scalar(radii.min_element())
}

pub fn rounded_box(half: Vec3, radius: f32) -> Tree {
  let (x, y, z) = Tree::axes();
  let r = scalar(radius);
  let q = [
    x.abs() - (scalar(half.x) - r),
    y.abs() - (scalar(half.y) - r),
    z.abs() - (scalar(half.z) - r)
  ];
  let outside =
    (q[0].max(0.0).square() + q[1].max(0.0).square() + q[2].max(0.0).square()).sqrt();
  outside + q[0].max(q[1].clone()).max(q[2].clone()).min(0.0) - r
}

pub fn cuboid(half: Vec3) -> Tree { rounded_box(half, 0.0) }

pub fn cylinder(radius: f32, half_height: f32) -> Tree {
  let (x, y, z) = Tree::axes();
  let radial = (x.square() + z.square()).sqrt() - scalar(radius);
  let axial = y.abs() - scalar(half_height);
  let outside =
    (radial.clone().max(0.0).square() + axial.clone().max(0.0).square()).sqrt();
  outside + radial.max(axial).min(0.0)
}

pub fn limb(from: Vec3, from_radius: f32, to: Vec3, to_radius: f32) -> Tree {
  let (x, y, z) = Tree::axes();
  let span = to - from;
  let [rx, ry, rz] = [x - scalar(from.x), y - scalar(from.y), z - scalar(from.z)];
  let along = ((rx.clone() * scalar(span.x)
    + ry.clone() * scalar(span.y)
    + rz.clone() * scalar(span.z))
    / scalar(span.length_squared().max(1e-6)))
  .max(0.0)
  .min(1.0);
  let away = (rx - along.clone() * scalar(span.x)).square()
    + (ry - along.clone() * scalar(span.y)).square()
    + (rz - along.clone() * scalar(span.z)).square();
  away.sqrt() - (scalar(from_radius) + along * scalar(to_radius - from_radius))
}

pub fn along_z(shape: Tree) -> Tree {
  let (x, y, z) = Tree::axes();
  shape.remap_xyz(x, z, y)
}

pub fn along_x(shape: Tree) -> Tree {
  let (x, y, z) = Tree::axes();
  shape.remap_xyz(y, x, z)
}

pub fn at(shape: Tree, offset: Vec3) -> Tree {
  let (x, y, z) = Tree::axes();
  shape.remap_xyz(x - scalar(offset.x), y - scalar(offset.y), z - scalar(offset.z))
}

pub fn union(shapes: impl IntoIterator<Item = Tree>) -> Tree {
  shapes.into_iter().reduce(|a, b| a.min(b)).expect("union of no shapes")
}

pub fn difference(shape: Tree, cutout: Tree) -> Tree { shape.max(-cutout) }

pub fn smooth_union(a: Tree, b: Tree, radius: f32) -> Tree {
  let r = scalar(radius);
  a.min(b.clone()) - 1.0 / (4.0 * r) * (r - (a - b).abs()).max(0.0).square()
}

pub fn smooth_unions(shapes: impl IntoIterator<Item = Tree>, radius: f32) -> Tree {
  shapes
    .into_iter()
    .reduce(|a, b| smooth_union(a, b, radius))
    .expect("union of no shapes")
}

pub struct Bounds {
  pub center: Vec3,
  pub half_extent: f32,
  pub depth: u8
}

pub struct Surface {
  pub vertices: Vec<Vec3>,
  pub triangles: Vec<[usize; 3]>
}

pub fn surface(shape: Tree, bounds: &Bounds) -> Surface {
  let bound: BoundShape<VmFunction, f32> =
    VmShape::from(shape).try_into().expect("sdf may only use the x, y and z axes");
  let settings = Settings {
    depth: bounds.depth,
    world_to_model: Matrix4::new_translation(&Vector3::new(
      bounds.center.x,
      bounds.center.y,
      bounds.center.z
    )) * Matrix4::new_scaling(bounds.half_extent),
    ..Default::default()
  };
  let dual = Octree::build(&bound, &settings).expect("meshing was cancelled").walk_dual();
  Surface {
    vertices: dual
      .vertices
      .iter()
      .map(|vertex| Vec3::new(vertex.x, vertex.y, vertex.z))
      .collect(),
    triangles: dual
      .triangles
      .iter()
      .map(|corners| [corners.x, corners.y, corners.z])
      .collect()
  }
}

pub fn probe(shape: Tree, points: &[Vec3]) -> Vec<(f32, Vec3)> {
  let shape = VmShape::from(shape);
  let tape = shape.grad_slice_tape(Default::default());
  let mut eval = VmShape::new_grad_slice_eval();
  let seeded = |axis: fn(Vec3) -> f32, slope: Vec3| {
    points
      .iter()
      .map(|&at| Grad::new(axis(at), slope.x, slope.y, slope.z))
      .collect::<Vec<_>>()
  };
  eval
    .eval(
      &tape,
      &seeded(|at| at.x, Vec3::X),
      &seeded(|at| at.y, Vec3::Y),
      &seeded(|at| at.z, Vec3::Z)
    )
    .expect("sdf may only use the x, y and z axes")
    .iter()
    .map(|&Grad { v, dx, dy, dz }| (v, Vec3::new(dx, dy, dz).normalize_or_zero()))
    .collect()
}

const SLIVER: f32 = 1e-7;

fn box_axes(normal: Vec3) -> (Vec3, Vec3) {
  let axis = normal.abs();
  if axis.x > axis.y && axis.x > axis.z {
    (Vec3::Z, Vec3::NEG_Y)
  } else if axis.y >= axis.z {
    (Vec3::X, Vec3::Z)
  } else {
    (Vec3::X, Vec3::NEG_Y)
  }
}

impl Surface {
  pub fn refined(self, longest: f32) -> Self {
    (0..4).fold(self, |Surface { mut vertices, triangles }, _| {
      let mut middles = std::collections::HashMap::<(usize, usize), usize>::new();
      let mut middle = |a: usize, b: usize, vertices: &mut Vec<Vec3>| {
        (vertices[a].distance(vertices[b]) > longest).then(|| {
          *middles.entry((a.min(b), a.max(b))).or_insert_with(|| {
            vertices.push((vertices[a] + vertices[b]) / 2.0);
            vertices.len() - 1
          })
        })
      };
      let triangles = triangles
        .into_iter()
        .flat_map(|[a, b, c]| {
          let splits = [
            middle(a, b, &mut vertices),
            middle(b, c, &mut vertices),
            middle(c, a, &mut vertices)
          ];
          let count = splits.iter().flatten().count();
          let turn = (count == 1)
            .then(|| splits.iter().position(Option::is_some))
            .unwrap_or_else(|| {
              splits.iter().position(Option::is_none).map(|open| (open + 1) % 3)
            })
            .unwrap_or(0);
          let corners = [a, b, c];
          let [a, b, c] = [0, 1, 2].map(|index| corners[(index + turn) % 3]);
          let [ab, bc, ca] = [0, 1, 2].map(|index| splits[(index + turn) % 3]);
          match (ab, bc, ca) {
            (Some(ab), Some(bc), Some(ca)) => {
              vec![[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
            }
            (Some(ab), Some(bc), None) => vec![[ab, b, bc], [a, ab, bc], [a, bc, c]],
            (Some(ab), None, None) => vec![[a, ab, c], [ab, b, c]],
            _ => vec![[a, b, c]]
          }
        })
        .collect();
      Surface { vertices, triangles }
    })
  }

  pub fn mesh(
    &self,
    uv_scale: f32,
    crease: f32,
    mut paint: impl FnMut(usize, Vec3) -> LinearRgba
  ) -> Mesh {
    let faces: Vec<([usize; 3], Vec3)> = self
      .triangles
      .iter()
      .map(|&corners| {
        let [a, b, c] = corners.map(|index| self.vertices[index]);
        (corners, (b - a).cross(c - a))
      })
      .filter(|(_, weighted)| weighted.length_squared() > SLIVER)
      .collect();
    let adjacency = faces.iter().enumerate().fold(
      vec![Vec::new(); self.vertices.len()],
      |mut adjacency: Vec<Vec<usize>>, (index, (corners, _))| {
        for &corner in corners.iter() {
          adjacency[corner].push(index)
        }
        adjacency
      }
    );
    let smoothed = |corner: usize, flat: Vec3| {
      adjacency[corner]
        .iter()
        .map(|&neighbour| faces[neighbour].1)
        .filter(|weighted| weighted.normalize().dot(flat) > crease)
        .sum::<Vec3>()
        .try_normalize()
        .unwrap_or(flat)
    };
    let corners: Vec<(usize, Vec3, [f32; 2], [f32; 4])> = faces
      .iter()
      .flat_map(|&(corners, weighted)| {
        let flat = weighted.normalize();
        let (across, down) = box_axes(flat);
        corners.map(|corner| {
          let at = self.vertices[corner] * uv_scale;
          let normal = smoothed(corner, flat);
          let tangent = (across - normal * normal.dot(across)).normalize_or(across);
          let handedness = tangent.cross(normal).dot(down).signum();
          (
            corner,
            normal,
            [at.dot(across), at.dot(down)],
            tangent.extend(handedness).to_array()
          )
        })
      })
      .collect();
    let colors: Vec<[f32; 4]> = corners
      .iter()
      .map(|&(corner, normal, ..)| paint(corner, normal).to_f32_array())
      .collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
      .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        corners.iter().map(|&(corner, ..)| self.vertices[corner]).collect::<Vec<_>>()
      )
      .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        corners.iter().map(|&(_, normal, ..)| normal).collect::<Vec<_>>()
      )
      .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        corners.iter().map(|&(.., uv, _)| uv).collect::<Vec<_>>()
      )
      .with_inserted_attribute(
        Mesh::ATTRIBUTE_TANGENT,
        corners.iter().map(|&(.., tangent)| tangent).collect::<Vec<_>>()
      )
      .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
      .with_inserted_indices(Indices::U32((0..corners.len() as u32).collect()))
  }
}
