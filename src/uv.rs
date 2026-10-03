use {crate::model::Piece,
     bevy::{math::{DVec2, DVec3},
            mesh::{Indices, MeshVertexAttribute, PrimitiveTopology,
                   VertexAttributeValues},
            prelude::*},
     std::collections::{HashMap, VecDeque}};

const WELD: f32 = 1e-4;
const SMART: f32 = 60.0;

pub struct Edge {
  pub from: Vec3,
  pub to: Vec3,
  pub bend: f32
}

struct Welded {
  points: Vec<Vec3>,
  corners: Vec<[u32; 3]>,
  spots: Vec<[u32; 3]>,
  across: Vec<[Option<usize>; 3]>
}

impl Welded {
  fn new(mesh: &Mesh) -> Self {
    let positions = vectors(mesh, Mesh::ATTRIBUTE_POSITION);
    let mut index = HashMap::new();
    let mut points = Vec::new();
    let spot_of: Vec<u32> = positions
      .iter()
      .map(|&point| {
        *index.entry((point / WELD).round().as_ivec3()).or_insert_with(|| {
          points.push(point);
          points.len() as u32 - 1
        })
      })
      .collect();
    let indices: Vec<u32> = mesh
      .indices()
      .map(|indices| indices.iter().map(|index| index as u32).collect())
      .unwrap_or_else(|| (0..positions.len() as u32).collect());
    let corners: Vec<[u32; 3]> =
      indices.chunks_exact(3).map(|corner| [corner[0], corner[1], corner[2]]).collect();
    let spots: Vec<[u32; 3]> = corners
      .iter()
      .map(|corners| corners.map(|corner| spot_of[corner as usize]))
      .collect();
    let side = |spots: &[u32; 3], k: usize| {
      let (a, b) = (spots[k], spots[(k + 1) % 3]);
      (a.min(b), a.max(b))
    };
    let sides = spots.iter().enumerate().fold(
      HashMap::<(u32, u32), Vec<usize>>::new(),
      |mut sides, (triangle, spots)| {
        for k in 0..3 {
          sides.entry(side(spots, k)).or_default().push(triangle)
        }
        sides
      }
    );
    let across = spots
      .iter()
      .enumerate()
      .map(|(triangle, spots)| {
        std::array::from_fn(|k| {
          let beside = &sides[&side(spots, k)];
          (beside.len() == 2)
            .then(|| beside.iter().copied().find(|&other| other != triangle))
            .flatten()
        })
      })
      .collect();
    Self { points, corners, spots, across }
  }

  fn at(&self, spot: u32) -> Vec3 { self.points[spot as usize] }

  fn area_normal(&self, triangle: usize) -> Vec3 {
    let [a, b, c] = self.spots[triangle].map(|spot| self.at(spot));
    (b - a).cross(c - a) / 2.0
  }

  fn charts(&self, joins: impl Fn(Vec3, usize, usize, usize) -> bool) -> Vec<Vec<usize>> {
    let mut chart_of = vec![usize::MAX; self.spots.len()];
    let mut charts = Vec::new();
    for seed in 0..self.spots.len() {
      if chart_of[seed] == usize::MAX {
        let id = charts.len();
        chart_of[seed] = id;
        let mut members = vec![seed];
        let mut facing = self.area_normal(seed);
        let mut queue = VecDeque::from([seed]);
        while let Some(triangle) = queue.pop_front() {
          for k in 0..3 {
            if let Some(other) = self.across[triangle][k]
              && chart_of[other] == usize::MAX
              && joins(facing, triangle, other, k)
            {
              chart_of[other] = id;
              facing += self.area_normal(other);
              members.push(other);
              queue.push_back(other)
            }
          }
        }
        charts.push(members)
      }
    }
    charts
  }

  fn opened(
    &self,
    charts: &[Vec<usize>],
    glued: impl Fn(usize, usize, usize) -> bool
  ) -> Self {
    let mut parent: Vec<usize> = (0..3 * self.spots.len()).collect();
    fn root(parent: &mut [usize], slot: usize) -> usize {
      let up = parent[slot];
      if up == slot {
        slot
      } else {
        let top = root(parent, up);
        parent[slot] = top;
        top
      }
    }
    let chart_of = charts.iter().enumerate().fold(
      vec![usize::MAX; self.spots.len()],
      |mut chart_of, (chart, triangles)| {
        for &triangle in triangles {
          chart_of[triangle] = chart
        }
        chart_of
      }
    );
    for triangle in 0..self.spots.len() {
      for k in 0..3 {
        if let Some(other) = self.across[triangle][k]
          && chart_of[triangle] == chart_of[other]
          && glued(triangle, other, k)
        {
          for spot in [self.spots[triangle][k], self.spots[triangle][(k + 1) % 3]] {
            if let Some(there) = self.spots[other].iter().position(|&each| each == spot)
              && let Some(here) =
                self.spots[triangle].iter().position(|&each| each == spot)
            {
              let (a, b) = (
                root(&mut parent, 3 * triangle + here),
                root(&mut parent, 3 * other + there)
              );
              parent[a] = b
            }
          }
        }
      }
    }
    let mut classes = HashMap::new();
    let mut points = Vec::new();
    let spots = (0..self.spots.len())
      .map(|triangle| {
        std::array::from_fn(|k| {
          let class = root(&mut parent, 3 * triangle + k);
          *classes.entry(class).or_insert_with(|| {
            points.push(self.at(self.spots[triangle][k]));
            points.len() as u32 - 1
          })
        })
      })
      .collect();
    Self { points, corners: self.corners.clone(), spots, across: self.across.clone() }
  }

  fn projected(&self, chart: &[usize], toward: Vec3) -> HashMap<u32, DVec2> {
    let across = toward.any_orthonormal_vector();
    let up = toward.cross(across);
    chart
      .iter()
      .flat_map(|&triangle| self.spots[triangle])
      .map(|spot| {
        (spot, DVec2::new(self.at(spot).dot(across).into(), self.at(spot).dot(up).into()))
      })
      .collect()
  }

  fn flattened(&self, chart: &[usize], conforming: bool) -> HashMap<u32, DVec2> {
    let facing = chart.iter().map(|&triangle| self.area_normal(triangle)).sum::<Vec3>();
    let toward = facing.try_normalize().unwrap_or(Vec3::Y);
    let start = self.projected(chart, toward);
    let curved = chart.iter().any(|&triangle| {
      self.area_normal(triangle).normalize_or(toward).dot(toward) < 0.995
    });
    let solved = if conforming && curved && start.len() > 3 {
      conformal(self, chart, start)
    } else {
      start
    };
    let area =
      |points: [DVec2; 3]| (points[1] - points[0]).perp_dot(points[2] - points[0]) / 2.0;
    let real: f64 =
      chart.iter().map(|&triangle| f64::from(self.area_normal(triangle).length())).sum();
    let flat: f64 = chart
      .iter()
      .map(|&triangle| area(self.spots[triangle].map(|spot| solved[&spot])).abs())
      .sum();
    let centre = solved.values().copied().sum::<DVec2>() / solved.len() as f64;
    let scale = (flat > 1e-12).then(|| (real / flat).sqrt()).unwrap_or(1.0);
    solved.into_iter().map(|(spot, at)| (spot, centre + (at - centre) * scale)).collect()
  }
}

fn conformal(
  welded: &Welded,
  chart: &[usize],
  start: HashMap<u32, DVec2>
) -> HashMap<u32, DVec2> {
  let spots: Vec<u32> = start.keys().copied().collect();
  let local: HashMap<u32, usize> =
    spots.iter().enumerate().map(|(index, &spot)| (spot, index)).collect();
  let rows: Vec<[(usize, f64); 6]> = chart
    .iter()
    .flat_map(|&triangle| {
      let [p0, p1, p2] = welded.spots[triangle]
        .map(|spot| DVec3::from(welded.at(spot).to_array().map(f64::from)));
      let (e1, e2) = (p1 - p0, p2 - p0);
      let normal = e1.cross(e2);
      let doubled = normal.length();
      (doubled > 1e-14).then(|| {
        let x = e1.normalize();
        let y = normal.normalize().cross(x);
        let q =
          [DVec2::ZERO, DVec2::new(e1.length(), 0.0), DVec2::new(e2.dot(x), e2.dot(y))];
        let weight = 1.0 / doubled.sqrt();
        let columns = welded.spots[triangle].map(|spot| local[&spot]);
        let w: [DVec2; 3] =
          std::array::from_fn(|j| (q[(j + 2) % 3] - q[(j + 1) % 3]) * weight);
        let real = std::array::from_fn(|entry| {
          let (j, v) = (entry / 2, entry % 2);
          (2 * columns[j] + v, if v == 0 { w[j].x } else { -w[j].y })
        });
        let imaginary = std::array::from_fn(|entry| {
          let (j, v) = (entry / 2, entry % 2);
          (2 * columns[j] + v, if v == 0 { w[j].y } else { w[j].x })
        });
        [real, imaginary]
      })
    })
    .flatten()
    .collect();
  let count = 2 * spots.len();
  let by_x = |a: &&u32, b: &&u32| start[a].x.total_cmp(&start[b].x);
  let pins =
    [spots.iter().min_by(by_x), spots.iter().max_by(by_x)].map(|pin| local[pin.unwrap()]);
  let free = |column: usize| !pins.contains(&(column / 2));
  let normal = |x: &[f64]| {
    let residual: Vec<f64> =
      rows.iter().map(|row| row.iter().map(|&(column, c)| c * x[column]).sum()).collect();
    rows.iter().zip(&residual).fold(vec![0.0; count], |mut out, (row, &r)| {
      for &(column, c) in row {
        if free(column) {
          out[column] += c * r
        }
      }
      out
    })
  };
  let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
  let mut x: Vec<f64> =
    spots.iter().flat_map(|spot| [start[spot].x, start[spot].y]).collect();
  let mut residual: Vec<f64> = normal(&x).into_iter().map(|g| -g).collect();
  let mut direction = residual.clone();
  let first = dot(&residual, &residual);
  let mut size = first;
  let mut round = 0;
  while round < (count * 6).max(4000) && size > first * 1e-12 && size > 1e-24 {
    let pushed = normal(&direction);
    let step = size / dot(&direction, &pushed).max(1e-30);
    for column in 0..count {
      x[column] += step * direction[column];
      residual[column] -= step * pushed[column]
    }
    let next = dot(&residual, &residual);
    for column in 0..count {
      direction[column] = residual[column] + next / size * direction[column]
    }
    size = next;
    round += 1
  }
  spots
    .iter()
    .enumerate()
    .map(|(index, &spot)| (spot, DVec2::new(x[2 * index], x[2 * index + 1])))
    .collect()
}

fn vectors(mesh: &Mesh, attribute: MeshVertexAttribute) -> Vec<Vec3> {
  mesh
    .attribute(attribute)
    .and_then(VertexAttributeValues::as_float3)
    .map(|values| values.iter().copied().map(Vec3::from).collect())
    .unwrap_or_else(|| vec![Vec3::ZERO; mesh.count_vertices()])
}

fn colors(mesh: &Mesh) -> Vec<[f32; 4]> {
  match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
    Some(VertexAttributeValues::Float32x4(colors)) => colors.clone(),
    _ => vec![[1.0; 4]; mesh.count_vertices()]
  }
}

fn laid(
  mesh: &Mesh,
  welded: &Welded,
  charts: &[Vec<usize>],
  uv: impl Fn(usize, &[usize]) -> HashMap<u32, DVec2>
) -> Mesh {
  let (positions, normals, tints) = (
    vectors(mesh, Mesh::ATTRIBUTE_POSITION),
    vectors(mesh, Mesh::ATTRIBUTE_NORMAL),
    colors(mesh)
  );
  let mut made = HashMap::new();
  let (mut at, mut facing, mut tint, mut uvs, mut indices) =
    (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
  for (chart, triangles) in charts.iter().enumerate() {
    let laid = uv(chart, triangles);
    for &triangle in triangles {
      for k in 0..3 {
        let (corner, spot) = (welded.corners[triangle][k], welded.spots[triangle][k]);
        let index = *made.entry((corner, chart, spot)).or_insert_with(|| {
          let vertex = corner as usize;
          at.push(positions[vertex]);
          facing.push(normals[vertex]);
          tint.push(tints[vertex]);
          uvs.push(laid[&spot].as_vec2().to_array());
          at.len() as u32 - 1
        });
        indices.push(index)
      }
    }
  }
  Mesh::new(PrimitiveTopology::TriangleList, mesh.asset_usage)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, at)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, facing)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, tint)
    .with_inserted_indices(Indices::U32(indices))
}

fn unfolded(
  mesh: &Mesh,
  welded: &Welded,
  charts: &[Vec<usize>],
  tile: f32,
  conforming: bool
) -> Mesh {
  laid(mesh, welded, charts, |_, triangles| {
    welded
      .flattened(triangles, conforming)
      .into_iter()
      .map(|(spot, at)| (spot, at / f64::from(tile)))
      .collect()
  })
}

impl Piece {
  pub fn unwrapped(self, tile: f32) -> Self {
    let welded = Welded::new(&self.0);
    let joining = SMART.to_radians().cos();
    let charts = welded.charts(|facing, _, other, _| {
      welded.area_normal(other).normalize_or_zero().dot(facing.normalize_or_zero())
        > joining
    });
    Self(unfolded(&self.0, &welded, &charts, tile, false))
  }

  pub fn seamed(self, tile: f32, seam: impl Fn(Edge) -> bool) -> Self {
    let welded = Welded::new(&self.0);
    let glued = |triangle: usize, other: usize, k: usize| {
      let spots = welded.spots[triangle];
      let [here, there] =
        [triangle, other].map(|face| welded.area_normal(face).normalize_or_zero());
      !seam(Edge {
        from: welded.at(spots[k]),
        to: welded.at(spots[(k + 1) % 3]),
        bend: here.dot(there).clamp(-1.0, 1.0).acos()
      })
    };
    let charts = welded.charts(|_, triangle, other, k| glued(triangle, other, k));
    let opened = welded.opened(&charts, glued);
    Self(unfolded(&self.0, &opened, &charts, tile, true))
  }

  pub fn boxed(self, tile: f32) -> Self {
    let welded = Welded::new(&self.0);
    let side = |triangle: usize| {
      let facing = welded.area_normal(triangle).abs();
      if facing.y >= facing.x.max(facing.z) {
        1
      } else if facing.x > facing.z {
        0
      } else {
        2
      }
    };
    let charts = welded.charts(|_, triangle, other, _| side(triangle) == side(other));
    Self(laid(&self.0, &welded, &charts, |_, triangles| {
      let axis = side(triangles[0]);
      triangles
        .iter()
        .flat_map(|&triangle| welded.spots[triangle])
        .map(|spot| {
          let at = welded.at(spot);
          let flat = match axis {
            1 => at.xz(),
            0 => Vec2::new(at.z, -at.y),
            _ => Vec2::new(at.x, -at.y)
          };
          (spot, (flat / tile).as_dvec2())
        })
        .collect()
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn distortion(piece: &Piece) -> (f32, f32, bool) {
    let positions = vectors(&piece.0, Mesh::ATTRIBUTE_POSITION);
    let uvs: Vec<Vec2> = match piece.0.attribute(Mesh::ATTRIBUTE_UV_0) {
      Some(VertexAttributeValues::Float32x2(uvs)) => {
        uvs.iter().copied().map(Vec2::from).collect()
      }
      _ => Vec::new()
    };
    let indices: Vec<usize> = piece.0.indices().unwrap().iter().collect();
    let ratios: Vec<f32> = indices
      .chunks_exact(3)
      .filter_map(|corners| {
        let [a, b, c] = [0, 1, 2].map(|k| positions[corners[k]]);
        let [p, q, r] = [0, 1, 2].map(|k| uvs[corners[k]]);
        let real = (b - a).cross(c - a).length();
        (real > 1e-6).then(|| (q - p).perp_dot(r - p) / real)
      })
      .collect();
    let low = ratios.iter().copied().fold(f32::MAX, f32::min);
    let high = ratios.iter().copied().fold(f32::MIN, f32::max);
    (
      low,
      high,
      ratios.iter().all(|&ratio| ratio > 0.0) || ratios.iter().all(|&ratio| ratio < 0.0)
    )
  }

  #[test]
  fn a_cut_cylinder_unrolls_without_stretch() {
    let tube =
      Piece::new(Cylinder::new(1.0, 3.0).mesh().resolution(32).segments(6), Srgba::WHITE)
        .seamed(1.0, |edge| {
          let cut = |point: Vec3| point.x > 0.0 && point.z.abs() < 1e-3;
          edge.bend > 0.6 || (cut(edge.from) && cut(edge.to))
        });
    let (low, high, _) = distortion(&tube);
    assert!(low.abs() > 0.85 && high.abs() < 1.15, "area ratios {low}..{high}");
  }

  #[test]
  fn smart_unwrapping_keeps_scale_on_a_sphere() {
    let ball =
      Piece::new(Sphere::new(1.0).mesh().ico(3).unwrap(), Srgba::WHITE).unwrapped(1.0);
    let (low, high, _) = distortion(&ball);
    assert!(
      low.abs().min(high.abs()) > 0.25 && low.abs().max(high.abs()) < 2.0,
      "{low}..{high}"
    );
  }
}
