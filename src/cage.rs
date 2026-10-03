use {crate::sdf::Surface,
     bevy::{asset::RenderAssetUsages,
            mesh::{Indices, PrimitiveTopology},
            prelude::*},
     std::{collections::{HashMap, HashSet},
           f32::consts::TAU}};

const PLANE: f32 = 1e-4;

#[derive(Clone, Default)]
pub struct Cage {
  pub points: Vec<Vec3>,
  pub faces: Vec<Vec<u32>>,
  pub creases: HashMap<(u32, u32), f32>
}

fn edge(a: u32, b: u32) -> (u32, u32) { (a.min(b), a.max(b)) }

fn rim(face: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
  face.iter().zip(face.iter().cycle().skip(1)).map(|(&a, &b)| (a, b))
}

struct Edges {
  keys: Vec<(u32, u32)>,
  index: HashMap<(u32, u32), usize>,
  faces: Vec<Vec<usize>>
}

impl From<Surface> for Cage {
  fn from(Surface { vertices, triangles }: Surface) -> Self {
    let faces = triangles
      .into_iter()
      .map(|corners| corners.map(|corner| corner as u32).to_vec())
      .collect();
    Self { points: vertices, faces, ..default() }
  }
}

impl Cage {
  fn outward(points: Vec<Vec3>, faces: Vec<Vec<u32>>) -> Self {
    let centre = points.iter().sum::<Vec3>() / points.len() as f32;
    let cage = Self { points, faces, ..default() };
    let faces = (0..cage.faces.len())
      .map(|face| {
        let mut corners = cage.faces[face].clone();
        if cage.normal(face).dot(cage.centre(face) - centre) < 0.0 {
          corners.reverse()
        }
        corners
      })
      .collect();
    Self { faces, ..cage }
  }

  pub fn cuboid(size: Vec3) -> Self {
    let half = size / 2.0;
    let points = (0..8)
      .map(|corner| {
        half
          * Vec3::new(
            [-1.0, 1.0][corner & 1],
            [-1.0, 1.0][corner >> 1 & 1],
            [-1.0, 1.0][corner >> 2 & 1]
          )
      })
      .collect();
    let faces = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [
      1, 5, 7, 3
    ]]
    .map(Vec::from)
    .into();
    Self::outward(points, faces)
  }

  pub fn prism(sides: u32, radius: f32, height: f32) -> Self {
    let ring = |y: f32| {
      (0..sides).map(move |side| {
        let angle = side as f32 / sides as f32 * TAU;
        Vec3::new(angle.cos() * radius, y, angle.sin() * radius)
      })
    };
    let points = ring(-height / 2.0).chain(ring(height / 2.0)).collect();
    let faces = (0..sides)
      .map(|side| {
        let next = (side + 1) % sides;
        vec![side, next, next + sides, side + sides]
      })
      .chain([(0..sides).collect(), (sides..2 * sides).collect()])
      .collect();
    Self::outward(points, faces)
  }

  pub fn centre(&self, face: usize) -> Vec3 {
    let corners = &self.faces[face];
    corners.iter().map(|&corner| self.points[corner as usize]).sum::<Vec3>()
      / corners.len() as f32
  }

  fn area_normal(&self, face: usize) -> Vec3 {
    rim(&self.faces[face])
      .map(|(a, b)| self.points[a as usize].cross(self.points[b as usize]))
      .sum::<Vec3>()
      / 2.0
  }

  pub fn normal(&self, face: usize) -> Vec3 { self.area_normal(face).normalize_or_zero() }

  pub fn faces_where(&self, pick: impl Fn(Vec3, Vec3) -> bool) -> Vec<usize> {
    (0..self.faces.len())
      .filter(|&face| pick(self.centre(face), self.normal(face)))
      .collect()
  }

  pub fn facing(&self, toward: Vec3) -> Vec<usize> {
    self.faces_where(|_, normal| normal.dot(toward.normalize()) > 0.7)
  }

  pub fn shaped(self, shape: impl Fn(Vec3) -> Vec3) -> Self {
    Self { points: self.points.into_iter().map(shape).collect(), ..self }
  }

  pub fn without(self, faces: &[usize]) -> Self {
    let gone: HashSet<usize> = faces.iter().copied().collect();
    let kept = self
      .faces
      .into_iter()
      .enumerate()
      .filter(|(face, _)| !gone.contains(face))
      .map(|(_, face)| face);
    Self { faces: kept.collect(), ..self }
  }

  pub fn extruded(mut self, faces: &[usize], shape: impl Fn(Vec3) -> Vec3) -> Self {
    let directed: HashSet<(u32, u32)> =
      faces.iter().flat_map(|&face| rim(&self.faces[face])).collect();
    let border: Vec<(u32, u32)> = faces
      .iter()
      .flat_map(|&face| rim(&self.faces[face]).collect::<Vec<_>>())
      .filter(|&(a, b)| !directed.contains(&(b, a)))
      .collect();
    let mut copies = HashMap::new();
    for &face in faces {
      for corner in self.faces[face].iter_mut() {
        *corner = *copies.entry(*corner).or_insert_with(|| {
          self.points.push(shape(self.points[*corner as usize]));
          self.points.len() as u32 - 1
        })
      }
    }
    self
      .faces
      .extend(border.into_iter().map(|(a, b)| vec![a, b, copies[&b], copies[&a]]));
    self
  }

  pub fn pushed(self, faces: &[usize], distance: f32) -> Self {
    let toward =
      faces.iter().map(|&face| self.area_normal(face)).sum::<Vec3>().normalize_or_zero()
        * distance;
    self.extruded(faces, |point| point + toward)
  }

  pub fn inset(mut self, faces: &[usize], width: f32) -> Self {
    for &face in faces {
      let normal = self.normal(face);
      let outer = self.faces[face].clone();
      let count = outer.len();
      let inner: Vec<u32> = (0..count)
        .map(|corner| {
          let [before, here, after] = [count - 1, 0, 1]
            .map(|step| self.points[outer[(corner + step) % count] as usize]);
          let [into_before, into_after] = [here - before, after - here]
            .map(|side| normal.cross(side.normalize_or_zero()));
          let miter =
            (into_before + into_after) / (1.0 + into_before.dot(into_after)).max(0.2);
          self.points.push(here + miter * width);
          self.points.len() as u32 - 1
        })
        .collect();
      self.faces.extend((0..count).map(|corner| {
        let next = (corner + 1) % count;
        vec![outer[corner], outer[next], inner[next], inner[corner]]
      }));
      self.faces[face] = inner
    }
    self
  }

  pub fn beveled(self, width: f32) -> Self {
    let faces: Vec<usize> = (0..self.faces.len()).collect();
    self.inset(&faces, width)
  }

  pub fn creased(mut self, pick: impl Fn(Vec3, Vec3) -> bool, sharpness: f32) -> Self {
    let picked: Vec<(u32, u32)> = self
      .faces
      .iter()
      .flat_map(|face| rim(face).collect::<Vec<_>>())
      .map(|(a, b)| edge(a, b))
      .filter(|&(a, b)| pick(self.points[a as usize], self.points[b as usize]))
      .collect();
    self.creases.extend(picked.into_iter().map(|key| (key, sharpness)));
    self
  }

  pub fn sharpened(mut self, degrees: f32, sharpness: f32) -> Self {
    let Edges { keys, faces, .. } = self.edges();
    let bent = degrees.to_radians().cos();
    let picked: Vec<(u32, u32)> = keys
      .into_iter()
      .zip(faces)
      .filter(|(_, faces)| {
        faces.len() == 2 && self.normal(faces[0]).dot(self.normal(faces[1])) < bent
      })
      .map(|(key, _)| key)
      .collect();
    self.creases.extend(picked.into_iter().map(|key| (key, sharpness)));
    self
  }

  pub fn mirrored(self) -> Self {
    let count = self.points.len() as u32;
    let flat = |point: Vec3| point.x.abs() < PLANE;
    let points: Vec<Vec3> = self
      .points
      .iter()
      .map(|&point| if flat(point) { point.with_x(0.0) } else { point })
      .collect();
    let across =
      |corner: u32| if flat(points[corner as usize]) { corner } else { corner + count };
    let faces = self
      .faces
      .iter()
      .filter(|face| !face.iter().all(|&corner| flat(points[corner as usize])))
      .flat_map(|face| {
        [face.clone(), face.iter().rev().map(|&corner| across(corner)).collect()]
      })
      .collect();
    let creases = self
      .creases
      .iter()
      .flat_map(|(&(a, b), &sharpness)| {
        [((a, b), sharpness), (edge(across(a), across(b)), sharpness)]
      })
      .collect();
    let mirrored: Vec<Vec3> =
      points.iter().map(|&point| point * Vec3::new(-1.0, 1.0, 1.0)).collect();
    Self { points: points.into_iter().chain(mirrored).collect(), faces, creases }
  }

  fn vertex_normals(&self) -> Vec<Vec3> {
    self
      .faces
      .iter()
      .enumerate()
      .fold(vec![Vec3::ZERO; self.points.len()], |mut normals, (face, corners)| {
        let normal = self.area_normal(face);
        for &corner in corners {
          normals[corner as usize] += normal
        }
        normals
      })
      .into_iter()
      .map(|normal| normal.normalize_or_zero())
      .collect()
  }

  pub fn displaced(self, by: impl Fn(Vec3, Vec3) -> f32) -> Self {
    let normals = self.vertex_normals();
    let points = self
      .points
      .iter()
      .zip(normals)
      .map(|(&point, normal)| point + normal * by(point, normal));
    Self { points: points.collect(), ..self }
  }

  pub fn pulled(self, at: Vec3, radius: f32, by: Vec3) -> Self {
    self.shaped(|point| {
      let near = (1.0 - point.distance(at) / radius).clamp(0.0, 1.0);
      point + by * near * near * (3.0 - 2.0 * near)
    })
  }

  pub fn relaxed(self, rounds: u32, amount: f32) -> Self {
    let Edges { keys, .. } = self.edges();
    (0..rounds).fold(self, |cage, _| {
      let (sums, counts) = keys.iter().fold(
        (vec![Vec3::ZERO; cage.points.len()], vec![0.0f32; cage.points.len()]),
        |(mut sums, mut counts), &(a, b)| {
          sums[a as usize] += cage.points[b as usize];
          sums[b as usize] += cage.points[a as usize];
          counts[a as usize] += 1.0;
          counts[b as usize] += 1.0;
          (sums, counts)
        }
      );
      let points = cage
        .points
        .iter()
        .zip(sums.iter().zip(&counts))
        .map(|(&point, (&sum, &count))| {
          (count > 0.0).then(|| point.lerp(sum / count, amount)).unwrap_or(point)
        })
        .collect();
      Self { points, ..cage }
    })
  }

  pub fn solidified(self, thickness: f32) -> Self {
    let count = self.points.len() as u32;
    let normals = self.vertex_normals();
    let directed: HashSet<(u32, u32)> =
      self.faces.iter().flat_map(|face| rim(face)).collect();
    let walls: Vec<Vec<u32>> = self
      .faces
      .iter()
      .flat_map(|face| rim(face).collect::<Vec<_>>())
      .filter(|&(a, b)| !directed.contains(&(b, a)))
      .map(|(a, b)| vec![b, a, a + count, b + count])
      .collect();
    let inner: Vec<Vec3> = self
      .points
      .iter()
      .zip(&normals)
      .map(|(&point, &normal)| point - normal * thickness)
      .collect();
    let backs: Vec<Vec<u32>> = self
      .faces
      .iter()
      .map(|face| face.iter().rev().map(|&corner| corner + count).collect())
      .collect();
    let creases = self
      .creases
      .iter()
      .flat_map(|(&(a, b), &sharpness)| {
        [((a, b), sharpness), ((a + count, b + count), sharpness)]
      })
      .collect();
    Self {
      points: self.points.into_iter().chain(inner).collect(),
      faces: self.faces.into_iter().chain(backs).chain(walls).collect(),
      creases
    }
  }

  pub fn cut(self, near: Vec3, amount: f32) -> Self {
    let Edges { keys, index, faces: beside } = self.edges();
    let middle =
      |(a, b): (u32, u32)| (self.points[a as usize] + self.points[b as usize]) / 2.0;
    let start = keys
      .iter()
      .copied()
      .min_by(|&one, &other| {
        middle(one).distance(near).total_cmp(&middle(other).distance(near))
      })
      .expect("cut a cage with edges");
    let across = |face: usize, (from, to): (u32, u32)| {
      let corners = &self.faces[face];
      let at = |corner: u32| corners.iter().position(|&other| other == corner);
      at(from).zip(at(to)).map(|(from_at, to_at)| {
        let step = if (from_at + 1) % 4 == to_at { 3 } else { 1 };
        (corners[(from_at + step) % 4], corners[(to_at + 4 - step) % 4])
      })
    };
    let quad = |face: &usize| self.faces[*face].len() == 4;
    let crossings: HashMap<usize, ((u32, u32), (u32, u32))> = beside[index[&start]]
      .iter()
      .filter(|face| quad(face))
      .flat_map(|&face| {
        std::iter::successors(Some((start, face)), |&(entry, face)| {
          let exit = across(face, entry)?;
          beside[index[&edge(exit.0, exit.1)]]
            .iter()
            .copied()
            .find(|&other| other != face)
            .filter(quad)
            .map(|other| (exit, other))
        })
        .take(self.faces.len())
        .filter_map(|(entry, face)| across(face, entry).map(|exit| (face, (entry, exit))))
        .collect::<Vec<_>>()
      })
      .collect();
    let mut points = self.points.clone();
    let mut made = HashMap::new();
    let mut midpoint = |(from, to): (u32, u32)| {
      *made.entry(edge(from, to)).or_insert_with(|| {
        points.push(points[from as usize].lerp(points[to as usize], amount));
        points.len() as u32 - 1
      })
    };
    let halves: HashMap<usize, [Vec<u32>; 2]> = crossings
      .iter()
      .map(|(&face, &((from, to), (exit_from, exit_to)))| {
        let (entered, exited) = (midpoint((from, to)), midpoint((exit_from, exit_to)));
        let forward = rim(&self.faces[face]).any(|pair| pair == (from, to));
        let halves = if forward {
          [vec![from, entered, exited, exit_from], vec![entered, to, exit_to, exited]]
        } else {
          [vec![entered, from, exit_from, exited], vec![to, entered, exited, exit_to]]
        };
        (face, halves)
      })
      .collect();
    let faces = self
      .faces
      .iter()
      .enumerate()
      .flat_map(|(face, corners)| {
        halves.get(&face).cloned().map(Vec::from).unwrap_or_else(|| {
          vec![
            rim(corners)
              .flat_map(|(a, b)| std::iter::once(a).chain(made.get(&edge(a, b)).copied()))
              .collect(),
          ]
        })
      })
      .collect();
    let creases = self
      .creases
      .iter()
      .flat_map(|(&(a, b), &sharpness)| {
        made
          .get(&edge(a, b))
          .map(|&middle| vec![(edge(a, middle), sharpness), (edge(middle, b), sharpness)])
          .unwrap_or_else(|| vec![((a, b), sharpness)])
      })
      .collect();
    Self { points, faces, creases }
  }

  fn edges(&self) -> Edges {
    self.faces.iter().enumerate().fold(
      Edges { keys: Vec::new(), index: HashMap::new(), faces: Vec::new() },
      |mut edges, (face, corners)| {
        for (a, b) in rim(corners) {
          let key = edge(a, b);
          let index = *edges.index.entry(key).or_insert_with(|| {
            edges.keys.push(key);
            edges.faces.push(Vec::new());
            edges.keys.len() - 1
          });
          edges.faces[index].push(face)
        }
        edges
      }
    )
  }

  fn subdivided_once(&self) -> Self {
    let Edges { keys, index, faces: beside } = self.edges();
    let count = self.points.len();
    let face_points: Vec<Vec3> =
      (0..self.faces.len()).map(|face| self.centre(face)).collect();
    let sharpness: Vec<f32> = keys
      .iter()
      .zip(&beside)
      .map(|(key, faces)| {
        (faces.len() == 2)
          .then(|| self.creases.get(key).copied().unwrap_or(0.0))
          .unwrap_or(f32::INFINITY)
      })
      .collect();
    let middle =
      |(a, b): (u32, u32)| (self.points[a as usize] + self.points[b as usize]) / 2.0;
    let edge_points =
      keys.iter().zip(&beside).zip(&sharpness).map(|((&key, faces), &sharp)| {
        let smooth = (faces.len() == 2)
          .then(|| {
            (middle(key) + (face_points[faces[0]] + face_points[faces[1]]) / 2.0) / 2.0
          })
          .unwrap_or(middle(key));
        smooth.lerp(middle(key), sharp.min(1.0))
      });
    let (around, touching) = keys.iter().enumerate().fold(
      (vec![Vec::new(); count], vec![HashSet::new(); count]),
      |(mut around, mut touching): (Vec<Vec<usize>>, Vec<HashSet<usize>>),
       (index, &(a, b))| {
        for corner in [a, b] {
          around[corner as usize].push(index);
          touching[corner as usize].extend(beside[index].iter().copied())
        }
        (around, touching)
      }
    );
    let vertex_points = (0..count).map(|corner| {
      let here = self.points[corner];
      let edges = &around[corner];
      let valence = edges.len() as f32;
      let sharp: Vec<usize> =
        edges.iter().copied().filter(|&index| sharpness[index] > 0.0).collect();
      let smooth = (!edges.is_empty() && !touching[corner].is_empty())
        .then(|| {
          let faces =
            touching[corner].iter().map(|&face| face_points[face]).sum::<Vec3>()
              / touching[corner].len() as f32;
          let edges =
            edges.iter().map(|&index| middle(keys[index])).sum::<Vec3>() / valence;
          (faces + 2.0 * edges + (valence - 3.0) * here) / valence
        })
        .unwrap_or(here);
      let strength = sharp.iter().map(|&index| sharpness[index]).sum::<f32>()
        / sharp.len().max(1) as f32;
      let other = |index: usize| {
        let (a, b) = keys[index];
        self.points[if a as usize == corner { b } else { a } as usize]
      };
      let sharpened = match sharp.len() {
        0 | 1 => smooth,
        2 => (6.0 * here + other(sharp[0]) + other(sharp[1])) / 8.0,
        _ => here
      };
      smooth.lerp(sharpened, strength.min(1.0))
    });
    let edge_at = |a: u32, b: u32| (count + index[&edge(a, b)]) as u32;
    let face_at = |face: usize| (count + keys.len() + face) as u32;
    let faces = self
      .faces
      .iter()
      .enumerate()
      .flat_map(|(face, corners)| {
        let sides = corners.len();
        (0..sides).map(move |side| {
          let [before, here, after] =
            [sides - 1, 0, 1].map(|step| corners[(side + step) % sides]);
          vec![here, edge_at(here, after), face_at(face), edge_at(before, here)]
        })
      })
      .collect();
    let creases = keys
      .iter()
      .zip(&sharpness)
      .filter(|&(_, &sharp)| sharp.is_finite() && sharp > 1.0)
      .flat_map(|(&(a, b), &sharp)| {
        let middle = edge_at(a, b);
        [(edge(a, middle), sharp - 1.0), (edge(middle, b), sharp - 1.0)]
      })
      .collect();
    Self {
      points: vertex_points
        .chain(edge_points)
        .chain(face_points.iter().copied())
        .collect(),
      faces,
      creases
    }
  }

  pub fn subdivided(self, levels: u32) -> Self {
    (0..levels).fold(self, |cage, _| cage.subdivided_once())
  }

  pub fn mesh(&self, smooth_degrees: f32) -> Mesh {
    let normals: Vec<Vec3> =
      (0..self.faces.len()).map(|face| self.area_normal(face)).collect();
    let touching = self.faces.iter().enumerate().fold(
      vec![Vec::new(); self.points.len()],
      |mut touching: Vec<Vec<usize>>, (face, corners)| {
        for &corner in corners {
          touching[corner as usize].push(face)
        }
        touching
      }
    );
    let smooth = smooth_degrees.to_radians().cos();
    let (positions, shading, corners, _) = self.faces.iter().enumerate().fold(
      (Vec::new(), Vec::new(), Vec::new(), HashMap::new()),
      |(mut positions, mut shading, mut corners, mut made): (
        Vec<Vec3>,
        Vec<Vec3>,
        Vec<u32>,
        HashMap<(u32, IVec3), u32>
      ),
       (face, ring)| {
        let flat = normals[face].normalize_or_zero();
        let vertices: Vec<u32> = ring
          .iter()
          .map(|&corner| {
            let normal = touching[corner as usize]
              .iter()
              .map(|&other| normals[other])
              .filter(|normal| normal.normalize_or_zero().dot(flat) >= smooth)
              .sum::<Vec3>()
              .normalize_or(flat);
            *made.entry((corner, (normal * 1000.0).round().as_ivec3())).or_insert_with(
              || {
                positions.push(self.points[corner as usize]);
                shading.push(normal);
                positions.len() as u32 - 1
              }
            )
          })
          .collect();
        let at = |index: usize| positions[vertices[index] as usize];
        let triangles: Vec<[usize; 3]> = match vertices.len() {
          3 => vec![[0, 1, 2]],
          4 if at(0).distance(at(2)) <= at(1).distance(at(3)) => {
            vec![[0, 1, 2], [0, 2, 3]]
          }
          4 => vec![[0, 1, 3], [1, 2, 3]],
          sides => (1..sides - 1).map(|corner| [0, corner, corner + 1]).collect()
        };
        corners.extend(triangles.into_iter().flatten().map(|index| vertices[index]));
        (positions, shading, corners, made)
      }
    );
    let count = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
      .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
      .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, shading)
      .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; count])
      .with_inserted_indices(Indices::U32(corners))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn closed(cage: &Cage) -> bool {
    cage.edges().faces.iter().all(|faces| faces.len() == 2)
  }

  fn extent(cage: &Cage) -> Vec3 {
    cage.points.iter().fold(Vec3::ZERO, |most, point| most.max(point.abs()))
  }

  #[test]
  fn subdivision_keeps_a_closed_cube_closed_and_rounds_it() {
    let cube = Cage::cuboid(Vec3::splat(2.0));
    let smooth = cube.clone().subdivided(3);
    assert!(closed(&smooth));
    assert_eq!(smooth.faces.len(), 6 * 64);
    assert!(extent(&smooth).x < 0.9);
    let hard = cube.sharpened(30.0, 10.0).subdivided(3);
    assert!((extent(&hard).x - 1.0).abs() < 1e-4);
    assert!(hard.points.iter().any(|point| (point.abs() - Vec3::ONE).length() < 1e-4));
  }

  #[test]
  fn beveling_keeps_flat_faces_and_rounds_edges() {
    let block = Cage::cuboid(Vec3::splat(2.0)).beveled(0.1).subdivided(3);
    assert!(closed(&block));
    assert!((extent(&block).x - 1.0).abs() < 0.01);
    assert!(block.points.iter().all(|point| point.length() < 3f32.sqrt() - 0.05));
  }

  #[test]
  fn extruding_and_mirroring_stay_closed() {
    let cube = Cage::cuboid(Vec3::ONE).shaped(|point| point + Vec3::X * 0.5);
    let top = cube.facing(Vec3::Y);
    let grown = cube.pushed(&top, 1.0);
    let side = grown.faces_where(|centre, normal| normal.x > 0.7 && centre.y > 0.5);
    let armed = grown.pushed(&side, 0.6).inset(&[0], 0.1);
    assert!(closed(&armed));
    let both = armed.mirrored();
    assert!(closed(&both.clone().subdivided(1)));
    assert!(both.points.iter().any(|point| point.x < -1.4));
  }

  #[test]
  fn a_loop_cut_rings_a_prism_and_solidify_closes_a_sheet() {
    let prism = Cage::prism(6, 1.0, 2.0);
    let cut = prism.clone().cut(Vec3::new(1.0, 0.0, 0.0), 0.25);
    assert!(closed(&cut));
    assert_eq!(cut.points.len(), 18);
    assert_eq!(cut.faces.len(), 14);
    assert!(cut.points[12..].iter().all(|point| (point.y.abs() - 0.5).abs() < 1e-4
      && (point.y - cut.points[12].y).abs() < 1e-4));
    let tile = Cage::cuboid(Vec3::ONE);
    let top = tile.facing(Vec3::Y)[0];
    let sheet = Cage { faces: vec![tile.faces[top].clone()], ..tile };
    let slab = sheet.solidified(0.2);
    assert!(closed(&slab));
    assert_eq!(slab.faces.len(), 6);
  }
}
