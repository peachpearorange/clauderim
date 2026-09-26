use {crate::noise,
     bevy::{asset::RenderAssetUsages,
            mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
            prelude::*},
     std::f32::consts::TAU};

pub struct Piece(pub Mesh);

impl Piece {
  pub fn new(mesh: impl Into<Mesh>, color: Srgba) -> Self {
    let mut mesh: Mesh = mesh.into();
    mesh.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
    let long = match mesh.indices() {
      Some(Indices::U16(short)) => short.iter().map(|&index| u32::from(index)).collect(),
      Some(Indices::U32(long)) => long.clone(),
      None => (0..mesh.count_vertices() as u32).collect()
    };
    mesh.insert_indices(Indices::U32(long));
    let count = mesh.count_vertices();
    if mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_none() {
      mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; count]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![
      LinearRgba::from(color)
        .to_f32_array();
      count
    ]);
    Self(mesh)
  }

  pub fn at(self, offset: Vec3) -> Self {
    self.moved(Transform::from_translation(offset))
  }

  pub fn at_xyz(self, x: f32, y: f32, z: f32) -> Self { self.at(Vec3::new(x, y, z)) }

  pub fn sized(self, scale: Vec3) -> Self { self.moved(Transform::from_scale(scale)) }

  pub fn turned(self, rotation: Quat) -> Self {
    self.moved(Transform::from_rotation(rotation))
  }

  pub fn pitched(self, angle: f32) -> Self { self.turned(Quat::from_rotation_x(angle)) }

  pub fn yawed(self, angle: f32) -> Self { self.turned(Quat::from_rotation_y(angle)) }

  pub fn rolled(self, angle: f32) -> Self { self.turned(Quat::from_rotation_z(angle)) }

  pub fn moved(self, transform: Transform) -> Self {
    Self(self.0.transformed_by(transform))
  }

  pub fn span(self, from: Vec3, to: Vec3) -> Self {
    let gap = to - from;
    self
      .sized(Vec3::new(1.0, gap.length(), 1.0))
      .turned(Quat::from_rotation_arc(Vec3::Y, gap.normalize_or(Vec3::Y)))
      .at((from + to) / 2.0)
  }

  pub fn mirrored(&self) -> Self {
    let mut mesh =
      self.0.clone().transformed_by(Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0)));
    if let Some(Indices::U32(indices)) = mesh.indices_mut() {
      for triangle in indices.chunks_exact_mut(3) {
        triangle.swap(1, 2)
      }
    }
    Self(mesh)
  }

  pub fn shaded(mut self, shade: impl Fn(Vec3, Vec3) -> LinearRgba) -> Self {
    let positions = points(&self.0, Mesh::ATTRIBUTE_POSITION);
    let normals = points(&self.0, Mesh::ATTRIBUTE_NORMAL);
    let colors: Vec<[f32; 4]> = positions
      .iter()
      .zip(&normals)
      .map(|(&position, &normal)| shade(position, normal).to_f32_array())
      .collect();
    self.0.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    self
  }

  pub fn planar(mut self, tile: f32) -> Self {
    let positions = points(&self.0, Mesh::ATTRIBUTE_POSITION);
    let normals = points(&self.0, Mesh::ATTRIBUTE_NORMAL);
    let uvs: Vec<[f32; 2]> = positions
      .iter()
      .zip(&normals)
      .map(|(&at, &normal)| {
        let facing = normal.abs();
        let flat = match (facing.y >= facing.x.max(facing.z), facing.x > facing.z) {
          (true, _) => at.xz(),
          (false, true) => Vec2::new(at.z, -at.y),
          (false, false) => Vec2::new(at.x, -at.y)
        };
        (flat / tile).to_array()
      })
      .collect();
    self.0.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    self
  }

  pub fn tiled(mut self, repeats: Vec2) -> Self {
    if let Some(VertexAttributeValues::Float32x2(uvs)) =
      self.0.attribute_mut(Mesh::ATTRIBUTE_UV_0)
    {
      for uv in uvs.iter_mut() {
        *uv = (Vec2::from(*uv) * repeats).to_array()
      }
    }
    self
  }

  pub fn creased(mut self, depth: f32, stretch: Vec3, seed: u32) -> Self {
    let positions = points(&self.0, Mesh::ATTRIBUTE_POSITION);
    let normals = points(&self.0, Mesh::ATTRIBUTE_NORMAL);
    let folds: Vec<f32> =
      positions.iter().map(|&at| noise::fbm3(at * stretch, 3, seed)).collect();
    let moved: Vec<[f32; 3]> = positions
      .iter()
      .zip(&normals)
      .zip(&folds)
      .map(|((&at, &normal), &fold)| (at + normal * fold * depth).to_array())
      .collect();
    if let Some(VertexAttributeValues::Float32x4(colors)) =
      self.0.attribute_mut(Mesh::ATTRIBUTE_COLOR)
    {
      for (color, &fold) in colors.iter_mut().zip(&folds) {
        let shade = (1.0 + 0.8 * fold).clamp(0.6, 1.25);
        *color = [color[0] * shade, color[1] * shade, color[2] * shade, color[3]]
      }
    }
    self.0.insert_attribute(Mesh::ATTRIBUTE_POSITION, moved);
    self.0.compute_smooth_normals();
    self
  }

  pub fn grained(mut self, repeats: f32) -> Self {
    if let Some(VertexAttributeValues::Float32x2(uvs)) =
      self.0.attribute_mut(Mesh::ATTRIBUTE_UV_0)
    {
      for uv in uvs.iter_mut() {
        *uv = uv.map(|coordinate| coordinate * repeats)
      }
    }
    self
  }
}

fn points(mesh: &Mesh, attribute: bevy::mesh::MeshVertexAttribute) -> Vec<Vec3> {
  mesh
    .attribute(attribute)
    .and_then(VertexAttributeValues::as_float3)
    .map(|values| values.iter().copied().map(Vec3::from).collect())
    .unwrap_or_default()
}

pub fn merge(pieces: impl IntoIterator<Item = Piece>) -> Mesh {
  pieces
    .into_iter()
    .map(|Piece(mesh)| mesh)
    .reduce(|mut all, mesh| {
      all.merge(&mesh).expect("pieces share attributes");
      all
    })
    .expect("merged at least one piece")
}

fn assemble(
  positions: Vec<Vec3>,
  normals: Vec<Vec3>,
  uvs: Vec<Vec2>,
  indices: Vec<u32>
) -> Mesh {
  Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

fn grid_indices(rows: u32, columns: u32) -> Vec<u32> {
  (0..rows)
    .flat_map(|row| {
      (0..columns).flat_map(move |column| {
        let first = row * (columns + 1) + column;
        let next = first + columns + 1;
        [first, first + 1, next, first + 1, next + 1, next]
      })
    })
    .collect()
}

pub fn lathe(profile: &[Vec2], sides: u32) -> Mesh {
  let length: f32 =
    profile.windows(2).map(|pair| pair[0].distance(pair[1])).sum::<f32>().max(0.001);
  let travelled: Vec<f32> = profile
    .iter()
    .scan((0.0, profile[0]), |(sum, last), &point| {
      *sum += last.distance(point);
      *last = point;
      Some(*sum / length)
    })
    .collect();
  let downward = profile[profile.len() - 1].y < profile[0].y;
  let flip = downward.then_some(-1.0).unwrap_or(1.0);
  let slope = |index: usize| {
    let (before, after) =
      (profile[index.saturating_sub(1)], profile[(index + 1).min(profile.len() - 1)]);
    let along = (after - before).normalize_or(Vec2::Y);
    Vec2::new(along.y, -along.x) * flip
  };
  let travelled = &travelled;
  let rings = profile.iter().enumerate().flat_map(|(index, &point)| {
    let outward = slope(index);
    (0..=sides).map(move |side| {
      let angle = side as f32 / sides as f32 * TAU;
      let (sin, cos) = angle.sin_cos();
      (
        Vec3::new(point.x * cos, point.y, point.x * sin),
        Vec3::new(outward.x * cos, outward.y, outward.x * sin).normalize_or(Vec3::Y),
        Vec2::new(side as f32 / sides as f32, travelled[index])
      )
    })
  });
  let (positions, normals, uvs) = rings.fold(
    (Vec::new(), Vec::new(), Vec::new()),
    |(mut positions, mut normals, mut uvs), (position, normal, uv)| {
      positions.push(position);
      normals.push(normal);
      uvs.push(uv);
      (positions, normals, uvs)
    }
  );
  let mut indices = grid_indices(profile.len() as u32 - 1, sides);
  if !downward {
    for triangle in indices.chunks_exact_mut(3) {
      triangle.swap(1, 2)
    }
  }
  assemble(positions, normals, uvs, indices)
}

pub fn tube(path: &[Vec3], radii: &[f32], sides: u32) -> Mesh {
  let count = path.len();
  let tangent = |index: usize| {
    (path[(index + 1).min(count - 1)] - path[index.saturating_sub(1)])
      .normalize_or(Vec3::Y)
  };
  let frames: Vec<(Vec3, Vec3)> = (0..count)
    .scan(tangent(0).any_orthonormal_vector(), |normal, index| {
      let along = tangent(index);
      let side = along.cross(*normal).normalize_or(along.any_orthonormal_vector());
      *normal = side.cross(along).normalize();
      Some((*normal, side))
    })
    .collect();
  let (positions, normals, uvs) = (0..count)
    .flat_map(|index| {
      let (normal, side) = frames[index];
      (0..=sides).map(move |step| {
        let angle = step as f32 / sides as f32 * TAU;
        let outward = normal * angle.cos() + side * angle.sin();
        (
          path[index] + outward * radii[index.min(radii.len() - 1)],
          outward,
          Vec2::new(step as f32 / sides as f32, index as f32 / (count - 1) as f32)
        )
      })
    })
    .fold(
      (Vec::new(), Vec::new(), Vec::new()),
      |(mut positions, mut normals, mut uvs), (position, normal, uv)| {
        positions.push(position);
        normals.push(normal);
        uvs.push(uv);
        (positions, normals, uvs)
      }
    );
  assemble(positions, normals, uvs, grid_indices(count as u32 - 1, sides))
}

pub fn curve(from: Vec3, bend: Vec3, to: Vec3, steps: usize) -> Vec<Vec3> {
  (0..=steps)
    .map(|step| {
      let t = step as f32 / steps as f32;
      from * (1.0 - t) * (1.0 - t) + bend * 2.0 * t * (1.0 - t) + to * t * t
    })
    .collect()
}

pub fn taper(steps: usize, from: f32, to: f32) -> Vec<f32> {
  (0..=steps).map(|step| from.lerp(to, step as f32 / steps as f32)).collect()
}

fn bulged(seed: u32, roughness: f32, detail: u32) -> Mesh {
  let mut mesh = Sphere::new(1.0).mesh().ico(detail).expect("ico sphere");
  let bulge = |point: Vec3| {
    let lumpy = noise::fbm3(point * 1.3 + Vec3::splat(seed as f32 * 7.31), 4, seed);
    let facets = noise::fbm3(point * 3.1, 2, seed + 11);
    point * (1.0 + roughness * (lumpy * 1.6 + facets * 0.5))
  };
  let positions: Vec<Vec3> =
    points(&mesh, Mesh::ATTRIBUTE_POSITION).into_iter().map(bulge).collect();
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh
}

pub fn lump(seed: u32, roughness: f32, detail: u32) -> Mesh {
  let mut mesh = bulged(seed, roughness, detail);
  mesh.duplicate_vertices();
  mesh.compute_flat_normals();
  mesh
}

pub fn hewn(seed: u32, cuts: u32, ledges: f32, detail: u32) -> Mesh {
  let mut roll = noise::Roll::new(seed);
  let planes: Vec<(Vec3, f32)> = (0..cuts)
    .map(|cut| {
      let rise = 1.0 - 2.0 * (cut as f32 + 0.5) / cuts as f32;
      let around = cut as f32 * 2.399_963 + roll.spread(0.5);
      let even = Vec3::new(
        around.cos() * (1.0 - rise * rise).sqrt(),
        rise,
        around.sin() * (1.0 - rise * rise).sqrt()
      );
      let jitter = Vec3::new(roll.spread(0.4), roll.spread(0.3), roll.spread(0.4));
      ((even + jitter).normalize_or(even), roll.range(0.6, 1.0))
    })
    .collect();
  let (layers, tilt) =
    (roll.range(1.6, 2.4), Vec2::new(roll.spread(0.2), roll.spread(0.2)));
  let carve = |direction: Vec3| {
    let reach = planes
      .iter()
      .filter(|&&(normal, _)| normal.dot(direction) > 0.05)
      .map(|&(normal, offset)| offset / normal.dot(direction))
      .fold(1.4, f32::min);
    let point = direction * reach;
    let bed = (point.y + tilt.dot(point.xz())) * layers
      + 0.25 * noise::fbm3(point * 1.3 + Vec3::splat(seed as f32), 2, seed + 3);
    let shelf = ledges * ((bed.fract() - 0.72) / 0.08).clamp(0.0, 1.0);
    point * (1.0 - shelf)
  };
  let mut mesh = Sphere::new(1.0).mesh().ico(detail).expect("ico sphere");
  let positions: Vec<Vec3> = points(&mesh, Mesh::ATTRIBUTE_POSITION)
    .into_iter()
    .map(|point| carve(point.normalize()))
    .collect();
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh.duplicate_vertices();
  mesh.compute_flat_normals();
  mesh
}

pub fn blade(length: f32, width: f32, thickness: f32, tip: f32) -> Mesh {
  let rows: Vec<(f32, f32)> = [0.0, 0.3, 0.6, 1.0 - tip, 1.0]
    .into_iter()
    .map(|t| {
      let narrowing = ((1.0 - t) / tip).min(1.0);
      (t * length, width * 0.5 * narrowing * (1.0 - 0.12 * t))
    })
    .collect();
  let ring = |y: f32, half: f32| {
    [
      Vec3::new(half, y, 0.0),
      Vec3::new(0.0, y, thickness * 0.5),
      Vec3::new(-half, y, 0.0),
      Vec3::new(0.0, y, -thickness * 0.5)
    ]
  };
  let faces: Vec<[Vec3; 3]> = rows
    .windows(2)
    .flat_map(|pair| {
      let (low, high) = (ring(pair[0].0, pair[0].1), ring(pair[1].0, pair[1].1));
      (0..4).flat_map(move |side| {
        let next = (side + 1) % 4;
        [[low[side], high[side], low[next]], [low[next], high[side], high[next]]]
      })
    })
    .collect();
  let positions: Vec<Vec3> = faces.iter().flatten().copied().collect();
  let normals: Vec<Vec3> = faces
    .iter()
    .flat_map(|&[a, b, c]| [(b - a).cross(c - a).normalize_or(Vec3::Z); 3])
    .collect();
  let uvs =
    positions.iter().map(|position| Vec2::new(position.x * 4.0, position.y)).collect();
  let indices = (0..positions.len() as u32).collect();
  assemble(positions, normals, uvs, indices)
}

pub fn fan(points: &[Vec2], thickness: f32) -> Mesh {
  let outline: Vec<Vec3> = points.iter().map(|point| point.extend(0.0)).collect();
  let center = outline.iter().sum::<Vec3>() / outline.len() as f32;
  let faces: Vec<[Vec3; 3]> = outline
    .iter()
    .zip(outline.iter().cycle().skip(1))
    .flat_map(|(&a, &b)| {
      let (front, back) = (Vec3::Z * thickness * 0.5, Vec3::NEG_Z * thickness * 0.5);
      [
        [center + front, a + front, b + front],
        [center + back, b + back, a + back],
        [a + front, a + back, b + back],
        [a + front, b + back, b + front]
      ]
    })
    .collect();
  let positions: Vec<Vec3> = faces.iter().flatten().copied().collect();
  let normals: Vec<Vec3> = faces
    .iter()
    .flat_map(|&[a, b, c]| [(b - a).cross(c - a).normalize_or(Vec3::Z); 3])
    .collect();
  let uvs = positions.iter().map(|position| position.xy()).collect();
  let indices = (0..positions.len() as u32).collect();
  assemble(positions, normals, uvs, indices)
}

pub fn ball(radius: f32) -> Mesh { Sphere::new(radius).mesh().uv(16, 12) }

pub fn block(x: f32, y: f32, z: f32) -> Mesh { Cuboid::new(x, y, z).into() }

pub fn rod(radius: f32, length: f32) -> Mesh {
  Cylinder::new(radius, length).mesh().resolution(12).into()
}

pub fn cone(radius: f32, height: f32) -> Mesh {
  Cone { radius, height }.mesh().resolution(14).into()
}

#[derive(Clone, Copy)]
pub struct Hoop {
  pub at: Vec3,
  pub wide: f32,
  pub front: f32,
  pub back: f32
}

impl Hoop {
  pub const fn new(y: f32, wide: f32, front: f32, back: f32) -> Self {
    Self { at: Vec3::new(0.0, y, 0.0), wide, front, back }
  }

  pub const fn pole(y: f32) -> Self { Self::new(y, 0.0, 0.0, 0.0) }

  pub const fn shifted(self, x: f32, z: f32) -> Self {
    Self { at: Vec3::new(self.at.x + x, self.at.y, self.at.z + z), ..self }
  }

  pub fn scaled(self, scale: f32) -> Self {
    Self {
      at: self.at,
      wide: self.wide * scale,
      front: self.front * scale,
      back: self.back * scale
    }
  }

  pub fn lerp(self, other: Hoop, amount: f32) -> Self {
    Self {
      at: self.at.lerp(other.at, amount),
      wide: self.wide.lerp(other.wide, amount),
      front: self.front.lerp(other.front, amount),
      back: self.back.lerp(other.back, amount)
    }
  }

  fn point(&self, angle: f32) -> Vec3 {
    let (sin, cos) = angle.sin_cos();
    let depth = (sin < 0.0).then_some(self.front).unwrap_or(self.back);
    self.at + Vec3::new(self.wide * cos, 0.0, depth * sin)
  }
}

pub fn loft(hoops: &[Hoop], sides: u32) -> Mesh {
  let rows = hoops.len();
  let last = rows - 1;
  let upward = hoops[last].at.y > hoops[0].at.y;
  let flip = upward.then_some(1.0).unwrap_or(-1.0);
  let angle = |side: u32| side as f32 / sides as f32 * TAU;
  let grid: Vec<Vec<Vec3>> = hoops
    .iter()
    .map(|hoop| (0..=sides).map(|side| hoop.point(angle(side))).collect())
    .collect();
  let (positions, normals, uvs) = (0..rows)
    .flat_map(|row| {
      let grid = &grid;
      let (below, above) = (row.saturating_sub(1), (row + 1).min(last));
      let pole =
        (hoops[row].at - hoops[row.checked_sub(1).unwrap_or(1)].at).normalize_or(Vec3::Y);
      (0..=sides).map(move |side| {
        let (before, after) = ((side + sides - 1) % sides, (side + 1) % sides);
        let around = grid[row][after as usize] - grid[row][before as usize];
        let along = grid[above][side as usize] - grid[below][side as usize];
        (
          grid[row][side as usize],
          (along.cross(around) * flip).normalize_or(pole),
          Vec2::new(side as f32 / sides as f32, row as f32 / last as f32)
        )
      })
    })
    .fold(
      (Vec::new(), Vec::new(), Vec::new()),
      |(mut positions, mut normals, mut uvs), (position, normal, uv)| {
        positions.push(position);
        normals.push(normal);
        uvs.push(uv);
        (positions, normals, uvs)
      }
    );
  let mut indices = grid_indices(last as u32, sides);
  if upward {
    for triangle in indices.chunks_exact_mut(3) {
      triangle.swap(1, 2)
    }
  }
  assemble(positions, normals, uvs, indices)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn outwardness(mesh: &Mesh) -> f32 {
    let positions = points(mesh, Mesh::ATTRIBUTE_POSITION);
    let center = positions.iter().sum::<Vec3>() / positions.len() as f32;
    let Some(Indices::U32(indices)) = mesh.indices() else { panic!("u32 indices") };
    indices
      .chunks_exact(3)
      .map(|triangle| {
        let [a, b, c] = [0, 1, 2].map(|corner| positions[triangle[corner] as usize]);
        (b - a)
          .cross(c - a)
          .normalize_or_zero()
          .dot(((a + b + c) / 3.0 - center).normalize_or_zero())
      })
      .sum::<f32>()
      / (indices.len() / 3) as f32
  }

  #[test]
  fn windings_face_outward() {
    let up = lathe(
      &[
        Vec2::new(0.0, -0.02),
        Vec2::new(0.066, 0.0),
        Vec2::new(0.036, 0.18),
        Vec2::new(0.0, 0.215)
      ],
      10
    );
    let pipe = tube(
      &curve(Vec3::ZERO, Vec3::new(0.2, 0.3, 0.0), Vec3::new(0.3, 0.6, -0.1), 8),
      &[0.05],
      8
    );
    let edge = blade(0.8, 0.06, 0.014, 0.14);
    let plate = fan(
      &[
        Vec2::new(0.0, 0.0),
        Vec2::new(0.2, 0.0),
        Vec2::new(0.2, 0.1),
        Vec2::new(0.0, 0.1)
      ],
      0.02
    );
    for (name, mesh) in
      [("up", up), ("tube", pipe), ("blade", edge), ("fan", plate)].into_iter()
    {
      println!("{name} {}", outwardness(&mesh))
    }
  }
}

pub fn spline(keys: &[Vec3], steps: usize) -> Vec<Vec3> {
  let key = |index: isize| keys[index.clamp(0, keys.len() as isize - 1) as usize];
  (0..keys.len() as isize - 1)
    .flat_map(|segment| {
      let [before, from, to, after] =
        [key(segment - 1), key(segment), key(segment + 1), key(segment + 2)];
      (0..steps).map(move |step| {
        let t = step as f32 / steps as f32;
        0.5
          * (from * 2.0
            + (to - before) * t
            + (before * 2.0 - from * 5.0 + to * 4.0 - after) * t * t
            + (from * 3.0 - before - to * 3.0 + after) * t * t * t)
      })
    })
    .chain(keys.last().copied())
    .collect()
}

pub fn sweep(spine: &[Vec3], girth: &[Vec3], sides: u32) -> Mesh {
  let count = spine.len();
  let tangent = |index: usize| {
    (spine[(index + 1).min(count - 1)] - spine[index.saturating_sub(1)])
      .normalize_or(Vec3::NEG_Z)
  };
  let first = tangent(0);
  let hint = (first.y.abs() > 0.9).then_some(Vec3::Z).unwrap_or(Vec3::Y);
  let ups: Vec<Vec3> = (0..count)
    .scan(hint, |up, index| {
      *up = up.reject_from_normalized(tangent(index)).normalize_or(*up);
      Some(*up)
    })
    .collect();
  let (positions, normals, uvs) = (0..count)
    .flat_map(|index| {
      let (along, up) = (tangent(index), ups[index]);
      let across = up.cross(along);
      let Vec3 { x: wide, y: high, z: deep } = girth[index.min(girth.len() - 1)];
      (0..=sides).map(move |step| {
        let (sin, cos) = (step as f32 / sides as f32 * TAU).sin_cos();
        let tall = (sin >= 0.0).then_some(high).unwrap_or(deep);
        (
          spine[index] + across * cos * wide + up * sin * tall,
          (across * cos / wide.max(1e-4) + up * sin / tall.max(1e-4)).normalize_or(up),
          Vec2::new(step as f32 / sides as f32, index as f32 / (count - 1) as f32)
        )
      })
    })
    .fold(
      (Vec::new(), Vec::new(), Vec::new()),
      |(mut positions, mut normals, mut uvs), (position, normal, uv)| {
        positions.push(position);
        normals.push(normal);
        uvs.push(uv);
        (positions, normals, uvs)
      }
    );
  assemble(positions, normals, uvs, grid_indices(count as u32 - 1, sides))
}

pub fn sculpt(keys: &[(Vec3, Vec3)], steps: usize, sides: u32) -> Mesh {
  let (spine, girth): (Vec<Vec3>, Vec<Vec3>) = keys.iter().copied().unzip();
  let girth: Vec<Vec3> = spline(&girth, steps).into_iter().map(Vec3::abs).collect();
  sweep(&spline(&spine, steps), &girth, sides)
}

pub fn ruffled(mut mesh: Mesh, depth: f32, stretch: Vec3, seed: u32) -> Mesh {
  let normals = points(&mesh, Mesh::ATTRIBUTE_NORMAL);
  let positions: Vec<Vec3> = points(&mesh, Mesh::ATTRIBUTE_POSITION)
    .into_iter()
    .zip(normals)
    .map(|(position, normal)| {
      position + normal * noise::fbm3(position * stretch, 3, seed) * depth
    })
    .collect();
  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh
}

pub fn sheet(grid: &[Vec<Vec3>]) -> Mesh {
  let (rows, columns) = (grid.len(), grid[0].len());
  let at = |row: usize, column: usize| grid[row.min(rows - 1)][column.min(columns - 1)];
  let (positions, normals, uvs) = (0..rows)
    .flat_map(|row| {
      (0..columns).map(move |column| {
        let across = at(row, column + 1) - at(row, column.saturating_sub(1));
        let along = at(row + 1, column) - at(row.saturating_sub(1), column);
        (
          at(row, column),
          across.cross(along).normalize_or(Vec3::Y),
          Vec2::new(column as f32 / (columns - 1) as f32, row as f32 / (rows - 1) as f32)
        )
      })
    })
    .fold(
      (Vec::new(), Vec::new(), Vec::new()),
      |(mut positions, mut normals, mut uvs), (position, normal, uv)| {
        positions.push(position);
        normals.push(normal);
        uvs.push(uv);
        (positions, normals, uvs)
      }
    );
  assemble(positions, normals, uvs, grid_indices(rows as u32 - 1, columns as u32 - 1))
}
