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
  across: Vec<[Option<usize>; 3]>,
  uvs: Vec<Vec2>
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
    let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
      Some(VertexAttributeValues::Float32x2(uvs)) => {
        uvs.iter().copied().map(Vec2::from).collect()
      }
      _ => Vec::new()
    };
    Self { points, corners, spots, across, uvs }
  }

  fn uv(&self, triangle: usize, spot: u32) -> Option<Vec2> {
    self.spots[triangle]
      .iter()
      .position(|&each| each == spot)
      .and_then(|k| self.uvs.get(self.corners[triangle][k] as usize).copied())
  }

  fn original(&self, chart: &[usize]) -> Option<HashMap<u32, DVec2>> {
    let seen = chart.iter().flat_map(|&triangle| {
      self.spots[triangle].map(|spot| (spot, self.uv(triangle, spot)))
    });
    let laid = seen.fold(
      Some(HashMap::new()),
      |laid: Option<HashMap<u32, Vec2>>, (spot, uv)| {
        let (mut laid, uv) = (laid?, uv?);
        let kept = *laid.entry(spot).or_insert(uv);
        (kept.distance(uv) < 1e-4).then_some(laid)
      }
    )?;
    let (squares, targets) = chart
      .iter()
      .flat_map(|&triangle| {
        let spots = self.spots[triangle];
        (0..3).map(move |k| (spots[k], spots[(k + 1) % 3]))
      })
      .fold((DVec3::ZERO, DVec2::ZERO), |(squares, targets), (a, b)| {
        let span = (laid[&b] - laid[&a]).as_dvec2();
        let (u, v) = (span.x * span.x, span.y * span.y);
        let length = f64::from(self.at(a).distance_squared(self.at(b)));
        (
          squares + DVec3::new(u * u, u * v, v * v),
          targets + DVec2::new(length * u, length * v)
        )
      });
    let determinant = squares.x * squares.z - squares.y * squares.y;
    (determinant.abs() > 1e-18).then_some(())?;
    let stretch = DVec2::new(
      (targets.x * squares.z - targets.y * squares.y) / determinant,
      (targets.y * squares.x - targets.x * squares.y) / determinant
    );
    (stretch.min_element() > 0.0).then_some(())?;
    let stretch = DVec2::new(stretch.x.sqrt(), stretch.y.sqrt());
    let handed: f64 = chart
      .iter()
      .map(|&triangle| {
        let [a, b, c] = self.spots[triangle].map(|spot| laid[&spot].as_dvec2() * stretch);
        (b - a).perp_dot(c - a)
      })
      .sum();
    let stretch = stretch * DVec2::new(handed.signum(), 1.0);
    Some(laid.into_iter().map(|(spot, uv)| (spot, uv.as_dvec2() * stretch)).collect())
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
    Self {
      points,
      corners: self.corners.clone(),
      spots,
      across: self.across.clone(),
      uvs: self.uvs.clone()
    }
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
    let start = conforming
      .then(|| self.original(chart))
      .flatten()
      .unwrap_or_else(|| self.projected(chart, toward));
    let curved = chart.iter().any(|&triangle| {
      self.area_normal(triangle).normalize_or(toward).dot(toward) < 0.995
    });
    let solved = if conforming && curved && start.len() > 3 {
      aligned(conformal(self, chart, start.clone()), &start)
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

fn aligned(
  laid: HashMap<u32, DVec2>,
  start: &HashMap<u32, DVec2>
) -> HashMap<u32, DVec2> {
  let middle = |points: &HashMap<u32, DVec2>| {
    points.values().copied().sum::<DVec2>() / points.len().max(1) as f64
  };
  let (here, there) = (middle(&laid), middle(start));
  let (dot, cross) = laid.iter().fold((0.0, 0.0), |(dot, cross), (spot, &at)| {
    let (from, to) = (at - here, start[spot] - there);
    (dot + from.dot(to), cross + from.perp_dot(to))
  });
  let turn = DVec2::from_angle(cross.atan2(dot));
  laid.into_iter().map(|(spot, at)| (spot, here + turn.rotate(at - here))).collect()
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
  pub fn wrapped(self, from: Vec3, to: Vec3, tile: f32) -> Self {
    const BANDS: usize = 8;
    let axis = (to - from).normalize_or(Vec3::Y);
    let behind = Vec3::Z
      .reject_from_normalized(axis)
      .try_normalize()
      .unwrap_or_else(|| axis.any_orthonormal_vector());
    let side = axis.cross(behind);
    let Piece(mut mesh) = self;
    mesh.duplicate_vertices();
    let polar: Vec<(f32, f32, f32)> = vectors(&mesh, Mesh::ATTRIBUTE_POSITION)
      .into_iter()
      .map(|point| {
        let offset = point - from;
        let radial = offset.reject_from_normalized(axis);
        (radial.dot(side).atan2(-radial.dot(behind)), radial.length(), offset.dot(axis))
      })
      .collect();
    let (first, last) =
      polar.iter().fold((f32::MAX, f32::MIN), |(low, high), &(.., along)| {
        (low.min(along), high.max(along))
      });
    let band = |along: f32| {
      ((along - first) / (last - first).max(1e-4) * BANDS as f32)
        .clamp(0.0, BANDS as f32 - 1e-3)
    };
    let (sums, counts) = polar.iter().fold(
      ([0.0f32; BANDS], [0.0f32; BANDS]),
      |(mut sums, mut counts), &(_, reach, along)| {
        let index = band(along) as usize;
        sums[index] += reach;
        counts[index] += 1.0;
        (sums, counts)
      }
    );
    let girth: Vec<f32> =
      (0..BANDS).map(|index| sums[index] / counts[index].max(1.0)).collect();
    let girth_at = |along: f32| {
      let at = (band(along) - 0.5).clamp(0.0, BANDS as f32 - 1.0);
      let low = at.floor() as usize;
      let high = (low + 1).min(BANDS - 1);
      girth[low] + (girth[high] - girth[low]) * (at - low as f32)
    };
    let uvs: Vec<[f32; 2]> = polar
      .chunks_exact(3)
      .flat_map(|corners| {
        let (low, high) =
          corners.iter().fold((f32::MAX, f32::MIN), |(low, high), &(angle, ..)| {
            (low.min(angle), high.max(angle))
          });
        let split = high - low > std::f32::consts::PI;
        corners
          .iter()
          .map(|&(angle, _, along)| {
            let angle =
              if split && angle < 0.0 { angle + std::f32::consts::TAU } else { angle };
            [angle * girth_at(along) / tile, along / tile]
          })
          .collect::<Vec<_>>()
      })
      .collect();
    let count = uvs.len() as u32;
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32((0..count).collect()));
    Piece(mesh)
  }

  pub fn followed(self, tile: f32) -> Self {
    let positions = vectors(&self.0, Mesh::ATTRIBUTE_POSITION);
    let grid: Vec<Vec2> = match self.0.attribute(Mesh::ATTRIBUTE_UV_0) {
      Some(VertexAttributeValues::Float32x2(uvs)) => {
        uvs.iter().copied().map(Vec2::from).collect()
      }
      _ => Vec::new()
    };
    let key = |value: f32| (value * 1e4).round() as i64;
    let rows: Vec<Vec<usize>> = (0..grid.len())
      .fold(std::collections::BTreeMap::<i64, Vec<usize>>::new(), |mut rows, vertex| {
        rows.entry(key(grid[vertex].y)).or_default().push(vertex);
        rows
      })
      .into_values()
      .map(|mut row| {
        row.sort_by(|&a, &b| grid[a].x.total_cmp(&grid[b].x));
        row
      })
      .collect();
    let around: Vec<(usize, f32)> = rows
      .iter()
      .flat_map(|row| {
        let travelled: Vec<f32> = row
          .iter()
          .scan((row[0], 0.0), |(last, total), &vertex| {
            *total += positions[*last].distance(positions[vertex]);
            *last = vertex;
            Some(*total)
          })
          .collect();
        let middle = (0..row.len())
          .min_by(|&a, &b| {
            (grid[row[a]].x - 0.5).abs().total_cmp(&(grid[row[b]].x - 0.5).abs())
          })
          .map_or(0.0, |index| travelled[index]);
        row
          .iter()
          .zip(travelled)
          .map(move |(&vertex, gone)| (vertex, gone - middle))
          .collect::<Vec<_>>()
      })
      .collect();
    let gaps = rows.windows(2).map(|pair| {
      let columns: HashMap<i64, usize> =
        pair[0].iter().map(|&vertex| (key(grid[vertex].x), vertex)).collect();
      let matched: Vec<f32> = pair[1]
        .iter()
        .filter_map(|&vertex| {
          columns
            .get(&key(grid[vertex].x))
            .map(|&other| positions[other].distance(positions[vertex]))
        })
        .collect();
      let centre = |row: &[usize]| {
        row.iter().map(|&vertex| positions[vertex]).sum::<Vec3>() / row.len() as f32
      };
      (!matched.is_empty())
        .then(|| matched.iter().sum::<f32>() / matched.len() as f32)
        .unwrap_or_else(|| centre(&pair[0]).distance(centre(&pair[1])))
    });
    let downs: Vec<f32> = std::iter::once(0.0)
      .chain(gaps.scan(0.0, |total, gap| {
        *total += gap;
        Some(*total)
      }))
      .collect();
    let mut uvs = vec![[0.0f32; 2]; grid.len()];
    for (row, &down) in rows.iter().zip(&downs) {
      for &vertex in row {
        uvs[vertex][1] = down / tile
      }
    }
    for (vertex, across) in around {
      uvs[vertex][0] = across / tile
    }
    let Piece(mut mesh) = self;
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    Piece(mesh)
  }

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
  fn following_a_tapered_tube_keeps_cells_square_and_rows_along_it() {
    let keys: Vec<(Vec3, Vec3)> = (0..8)
      .map(|step| {
        let t = step as f32 / 7.0;
        (Vec3::new((t * 3.0).sin() * 2.0, t * 1.5, -t * 14.0), Vec3::splat(1.2 - t * 1.0))
      })
      .collect();
    let tube =
      Piece::new(crate::model::sculpt(&keys, 12, 24), Srgba::WHITE).followed(1.0);
    let (low, high, same) = distortion(&tube);
    assert!(same && low.abs() > 0.8 && high.abs() < 1.25, "area ratios {low}..{high}");
  }

  #[test]
  fn wrapping_a_cylinder_around_its_axis_keeps_its_sides_true() {
    let tube =
      Piece::new(Cylinder::new(1.0, 3.0).mesh().resolution(32).segments(4), Srgba::WHITE)
        .wrapped(Vec3::NEG_Y * 1.5, Vec3::Y * 1.5, 1.0);
    let positions = vectors(&tube.0, Mesh::ATTRIBUTE_POSITION);
    let uvs: Vec<Vec2> = match tube.0.attribute(Mesh::ATTRIBUTE_UV_0) {
      Some(VertexAttributeValues::Float32x2(uvs)) => {
        uvs.iter().copied().map(Vec2::from).collect()
      }
      _ => Vec::new()
    };
    let sides: Vec<f32> = positions
      .chunks_exact(3)
      .zip(uvs.chunks_exact(3))
      .filter_map(|(at, uv)| {
        let normal = (at[1] - at[0]).cross(at[2] - at[0]);
        (normal.y.abs() < 0.1 * normal.length())
          .then(|| ((uv[1] - uv[0]).perp_dot(uv[2] - uv[0]) / normal.length()).abs())
      })
      .collect();
    assert!(sides.iter().all(|ratio| (ratio - 1.0).abs() < 0.05), "{sides:?}");
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
