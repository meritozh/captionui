//! Delaunay geometry, for Plot's `voronoi`, `hull`, `delaunayLink` and
//! `delaunayMesh` marks.
//!
//! These marks work on the data's *projected* points rather than on its values:
//! a Voronoi cell is the region nearer one point than any other, in pixels, so
//! the geometry runs after the frame has resolved the scales. This module is
//! the geometry alone; `render::delaunay` projects the rows and paints.

/// A point in the plane, in the pixels a mark draws at.
pub type Point2 = [f64; 2];

/// The Delaunay triangulation of a set of points.
///
/// Built by Bowyer–Watson insertion: each point is inserted by deleting the
/// triangles whose circumcircle it falls inside, then joining it to the hole
/// left behind. The result is the same triangulation d3-delaunay's
/// `Delaunay.from` gives for the points of a chart.
#[derive(Clone, Debug, Default)]
pub struct Triangulation {
    /// The distinct points, in the order they were first given.
    pub points: Vec<Point2>,
    /// Which distinct point each input point became. Coincident points share
    /// one, so a mark draws one shape for them rather than a degenerate one.
    pub indices: Vec<usize>,
    /// The triangles, each three point indices counter-clockwise.
    pub triangles: Vec<[usize; 3]>,
    /// For each half-edge `3t + k`, the half-edge on the other side of the
    /// same edge, or `-1` where the edge is on the convex hull.
    pub halfedges: Vec<i32>,
}

impl Triangulation {
    /// Triangulate `input`, dropping coincident points.
    pub fn new(input: &[Point2]) -> Self {
        let mut points: Vec<Point2> = Vec::new();
        let mut indices = Vec::with_capacity(input.len());
        for point in input {
            let found = points.iter().position(|existing| *existing == *point);
            indices.push(match found {
                Some(index) => index,
                None => {
                    points.push(*point);
                    points.len() - 1
                }
            });
        }
        let n = points.len();
        if n < 3 {
            return Self {
                points,
                indices,
                ..Self::default()
            };
        }

        // A super-triangle around every point, far enough out that no real
        // circumcircle reaches its vertices: the insertion never has to
        // consider an unbounded triangle.
        let mut all = points.clone();
        let (min_x, max_x, min_y, max_y) = bounds(&points);
        let size = (max_x - min_x).max(max_y - min_y).max(1.);
        let (mid_x, mid_y) = ((min_x + max_x) / 2., (min_y + max_y) / 2.);
        let super_triangle = [
            [mid_x - 20. * size, mid_y - size],
            [mid_x + 20. * size, mid_y - size],
            [mid_x, mid_y + 20. * size],
        ];
        let base = all.len();
        all.extend(super_triangle);

        let mut triangles: Vec<[usize; 3]> = vec![[base, base + 1, base + 2]];
        for point in 0..n {
            // Which triangles' circumcircles swallow the new point.
            let bad: Vec<bool> = triangles
                .iter()
                .map(|triangle| {
                    circumcircle(&all, triangle).is_some_and(|circle| contains(circle, all[point]))
                })
                .collect();
            if !bad.iter().any(|bad| *bad) {
                continue;
            }
            let halfedges = halfedges(&triangles);
            // The hole's boundary: an edge of a deleted triangle whose other
            // side survives. It keeps the deleted triangle's direction, so the
            // new triangles built on it come out counter-clockwise.
            let mut boundary: Vec<(usize, usize)> = Vec::new();
            for (index, triangle) in triangles.iter().enumerate() {
                if !bad[index] {
                    continue;
                }
                for k in 0..3 {
                    let opposite = halfedges[3 * index + k];
                    let survives = opposite >= 0 && !bad[opposite as usize / 3];
                    if opposite < 0 || survives {
                        boundary.push((triangle[k], triangle[(k + 1) % 3]));
                    }
                }
            }
            triangles = triangles
                .into_iter()
                .enumerate()
                .filter(|(index, _)| !bad[*index])
                .map(|(_, triangle)| triangle)
                .collect();
            for (a, b) in boundary {
                // A collinear boundary edge adds no area, so it draws nothing;
                // otherwise the triangle joins the hole in whichever direction
                // keeps it counter-clockwise.
                let area = signed_area(&all[a], &all[b], &all[point]);
                if area > 0. {
                    triangles.push([a, b, point]);
                } else if area < 0. {
                    triangles.push([b, a, point]);
                }
            }
        }

        // Drop the super-triangle: every triangle that reaches one of its
        // vertices lies outside the data's hull.
        triangles.retain(|triangle| triangle.iter().all(|index| *index < n));
        let halfedges = halfedges(&triangles);
        Self {
            points,
            indices,
            triangles,
            halfedges,
        }
    }

    /// Every edge of the triangulation once, the hull's included, as point
    /// indices. These are the links `delaunayLink` draws and the mesh
    /// `delaunayMesh` draws.
    pub fn edges(&self) -> Vec<(usize, usize)> {
        let mut edges = Vec::new();
        for (index, triangle) in self.triangles.iter().enumerate() {
            for k in 0..3 {
                let a = triangle[k];
                let b = triangle[(k + 1) % 3];
                let opposite = self.halfedges[3 * index + k];
                // An interior edge appears twice: take it from the half-edge
                // whose opposite is later, and take a hull edge from its only
                // side.
                if opposite < 0 || opposite as usize > 3 * index + k {
                    edges.push((a, b));
                }
            }
        }
        edges
    }
}

/// The convex hull of `points`, as indices into it in counter-clockwise order.
///
/// Andrew's monotone chain: sort the points, then walk them once building the
/// lower hull and once building the upper one, dropping any point that turns
/// the wrong way.
pub fn convex_hull(points: &[Point2]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by(|a, b| {
        let (a, b) = (points[*a], points[*b]);
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    if order.len() < 3 {
        return order;
    }

    let mut hull: Vec<usize> = Vec::with_capacity(order.len() * 2);
    for forward in [true, false] {
        let start = hull.len();
        let chain: Vec<usize> = if forward {
            order.to_vec()
        } else {
            order.iter().rev().copied().collect()
        };
        for index in chain {
            while hull.len() >= start + 2 {
                let a = points[hull[hull.len() - 2]];
                let b = points[hull[hull.len() - 1]];
                if cross(a, b, points[index]) > 0. {
                    break;
                }
                hull.pop();
            }
            hull.push(index);
        }
        // The last point of each chain is the first of the next.
        hull.pop();
    }
    hull
}

/// The Voronoi cell of each point, clipped to `bounds` — `[x0, y0, x1, y1]`.
///
/// A cell is the region nearer its own point than any other, which is the
/// intersection of one half-plane per other point. Clipping the bounds
/// rectangle by those half-planes gives the same polygon d3-delaunay's
/// `voronoi.cell` gives, including the unbounded cells of the hull's points,
/// which the rectangle closes off.
pub fn voronoi_cells(points: &[Point2], bounds: [f64; 4]) -> Vec<Vec<Point2>> {
    let [x0, y0, x1, y1] = bounds;
    let rect = vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    points
        .iter()
        .enumerate()
        .map(|(site, point)| {
            let mut cell = rect.clone();
            for (other, other_point) in points.iter().enumerate() {
                if other == site || *other_point == *point {
                    continue;
                }
                // Keep the side of the bisector nearer `point`.
                let normal = [other_point[0] - point[0], other_point[1] - point[1]];
                let midpoint = [
                    (other_point[0] + point[0]) / 2.,
                    (other_point[1] + point[1]) / 2.,
                ];
                cell = clip(
                    &cell,
                    normal,
                    midpoint[0] * normal[0] + midpoint[1] * normal[1],
                );
                if cell.is_empty() {
                    break;
                }
            }
            cell
        })
        .collect()
}

/// The edges of every Voronoi cell once, as pairs of endpoints.
///
/// Adjacent cells share an edge, so a mesh drawn from the cells alone would
/// stroke each interior edge twice. Rounding the endpoints identifies the
/// shared pair, which is how the mesh reads as one lattice.
pub fn voronoi_edges(cells: &[Vec<Point2>]) -> Vec<(Point2, Point2)> {
    let key = |point: &Point2| {
        (
            (point[0] * 1000.).round() as i64,
            (point[1] * 1000.).round() as i64,
        )
    };
    let mut seen: std::collections::HashSet<((i64, i64), (i64, i64))> =
        std::collections::HashSet::new();
    let mut edges = Vec::new();
    for cell in cells {
        for index in 0..cell.len() {
            let (a, b) = (cell[index], cell[(index + 1) % cell.len()]);
            let (ka, kb) = (key(&a), key(&b));
            let ordered = if ka <= kb { (ka, kb) } else { (kb, ka) };
            if seen.insert(ordered) {
                edges.push((a, b));
            }
        }
    }
    edges
}

/// Clip a convex polygon by the half-plane `p · normal <= offset`, keeping the
/// part inside it.
fn clip(polygon: &[Point2], normal: Point2, offset: f64) -> Vec<Point2> {
    if polygon.is_empty() {
        return Vec::new();
    }
    let signed = |point: &Point2| point[0] * normal[0] + point[1] * normal[1] - offset;
    let mut output = Vec::with_capacity(polygon.len() + 1);
    for index in 0..polygon.len() {
        let current = polygon[index];
        let next = polygon[(index + 1) % polygon.len()];
        let (inside, next_inside) = (signed(&current), signed(&next));
        if inside <= 0. {
            output.push(current);
        }
        if (inside <= 0.) != (next_inside <= 0.) {
            // The edge crosses the bisector: add the crossing point.
            let t = inside / (inside - next_inside);
            output.push([
                current[0] + (next[0] - current[0]) * t,
                current[1] + (next[1] - current[1]) * t,
            ]);
        }
    }
    output
}

/// The axis-aligned bounds of the points: `(min_x, max_x, min_y, max_y)`.
fn bounds(points: &[Point2]) -> (f64, f64, f64, f64) {
    points.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), point| {
            (
                min_x.min(point[0]),
                max_x.max(point[0]),
                min_y.min(point[1]),
                max_y.max(point[1]),
            )
        },
    )
}

/// The circumcircle of a triangle: its centre and squared radius, or `None`
/// when the triangle has no area and its circle is unbounded.
fn circumcircle(points: &[Point2], triangle: &[usize; 3]) -> Option<(f64, f64, f64)> {
    let [a, b, c] = [
        points[triangle[0]],
        points[triangle[1]],
        points[triangle[2]],
    ];
    let d = 2. * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    if d == 0. || !d.is_finite() {
        return None;
    }
    let a2 = a[0] * a[0] + a[1] * a[1];
    let b2 = b[0] * b[0] + b[1] * b[1];
    let c2 = c[0] * c[0] + c[1] * c[1];
    let ux = (a2 * (b[1] - c[1]) + b2 * (c[1] - a[1]) + c2 * (a[1] - b[1])) / d;
    let uy = (a2 * (c[0] - b[0]) + b2 * (a[0] - c[0]) + c2 * (b[0] - a[0])) / d;
    let r2 = (a[0] - ux).powi(2) + (a[1] - uy).powi(2);
    Some((ux, uy, r2))
}

/// Whether a point falls inside a circumcircle. The tolerance is relative so
/// a circle that passes exactly through the point still counts, which keeps
/// four points on one circle from leaving a hole in the triangulation.
fn contains(circle: (f64, f64, f64), point: Point2) -> bool {
    let (cx, cy, r2) = circle;
    let d2 = (point[0] - cx).powi(2) + (point[1] - cy).powi(2);
    d2 <= r2 * (1. + 1e-9)
}

/// The cross product of `b - a` and `c - a`: positive when `a`, `b`, `c` turn
/// counter-clockwise.
fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// The signed area of a triangle: positive when its points turn
/// counter-clockwise.
fn signed_area(a: &Point2, b: &Point2, c: &Point2) -> f64 {
    cross(*a, *b, *c) / 2.
}

/// The half-edge opposite each half-edge of `triangles`, or `-1` on the hull.
fn halfedges(triangles: &[[usize; 3]]) -> Vec<i32> {
    let mut map: std::collections::HashMap<(usize, usize), usize> =
        std::collections::HashMap::with_capacity(triangles.len() * 3);
    let mut result = vec![-1; triangles.len() * 3];
    for (index, triangle) in triangles.iter().enumerate() {
        for k in 0..3 {
            map.insert((triangle[k], triangle[(k + 1) % 3]), 3 * index + k);
        }
    }
    for (index, triangle) in triangles.iter().enumerate() {
        for k in 0..3 {
            let a = triangle[k];
            let b = triangle[(k + 1) % 3];
            if let Some(&opposite) = map.get(&(b, a)) {
                result[3 * index + k] = opposite as i32;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Point2> {
        vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]]
    }

    #[test]
    fn a_square_triangulates_into_two_triangles() {
        let triangulation = Triangulation::new(&square());
        assert_eq!(triangulation.triangles.len(), 2);
        assert_eq!(triangulation.edges().len(), 5);
    }

    #[test]
    fn coincident_points_share_a_vertex() {
        let triangulation = Triangulation::new(&[[0., 0.], [0., 0.], [10., 0.], [5., 8.]]);
        assert_eq!(triangulation.points.len(), 3);
        assert_eq!(triangulation.indices, vec![0, 0, 1, 2]);
        assert_eq!(triangulation.triangles.len(), 1);
    }

    #[test]
    fn collinear_points_have_no_triangles() {
        let triangulation = Triangulation::new(&[[0., 0.], [1., 1.], [2., 2.], [3., 3.]]);
        assert!(triangulation.triangles.is_empty());
        assert!(triangulation.edges().is_empty());
    }

    #[test]
    fn every_circumcircle_is_empty() {
        // A cloud of points with no four on one circle: the defining property
        // of a Delaunay triangulation is that no point lies inside another
        // triangle's circumcircle.
        let mut seed = 7_u64;
        let points: Vec<Point2> = (0..60)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let x = (seed % 1000) as f64 / 100.;
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let y = (seed % 1000) as f64 / 100.;
                [x, y]
            })
            .collect();
        let triangulation = Triangulation::new(&points);
        for triangle in &triangulation.triangles {
            let circle = circumcircle(&triangulation.points, triangle).unwrap();
            for (index, point) in triangulation.points.iter().enumerate() {
                if triangle.contains(&index) {
                    continue;
                }
                assert!(
                    !contains(circle, *point),
                    "point {index} falls inside {triangle:?}'s circle"
                );
            }
        }
    }

    #[test]
    fn the_hull_wraps_every_point() {
        let points = vec![
            [0., 0.],
            [10., 0.],
            [10., 10.],
            [0., 10.],
            [5., 5.],
            [3., 7.],
        ];
        let hull = convex_hull(&points);
        assert_eq!(hull.len(), 4);
        // Counter-clockwise, starting at the lower left.
        assert_eq!(hull, vec![0, 1, 2, 3]);
    }

    #[test]
    fn voronoi_cells_tile_the_bounds() {
        let points = vec![[100., 100.], [300., 100.], [200., 300.]];
        let bounds = [0., 0., 400., 400.];
        let cells = voronoi_cells(&points, bounds);
        assert_eq!(cells.len(), 3);
        for cell in &cells {
            assert!(!cell.is_empty(), "every point owns some of the bounds");
            // Every cell stays inside the bounds rectangle.
            for point in cell {
                assert!(point[0] >= -1e-9 && point[0] <= 400. + 1e-9);
                assert!(point[1] >= -1e-9 && point[1] <= 400. + 1e-9);
            }
        }
        // The shared bisector of the first two points splits the bounds.
        assert!(cells[0].iter().all(|point| point[0] <= 200. + 1e-9));
        assert!(cells[1].iter().all(|point| point[0] >= 200. - 1e-9));
    }

    #[test]
    fn a_site_outside_the_bounds_keeps_its_cell() {
        // The nearer site still owns the half of the rectangle it faces, so a
        // point outside the bounds clips down to its share of them rather than
        // vanishing.
        let cells = voronoi_cells(&[[-10., 200.], [500., 200.]], [0., 0., 400., 400.]);
        assert_eq!(cells.len(), 2);
        // The bisector falls at the midpoint, x = 245.
        assert!(cells[0].iter().all(|point| point[0] <= 245. + 1e-9));
        assert!(cells[1].iter().all(|point| point[0] >= 245. - 1e-9));
        assert!(!cells[0].is_empty() && !cells[1].is_empty());
    }

    #[test]
    fn a_voronoi_mesh_draws_each_edge_once() {
        let cells = voronoi_cells(&square(), [0., 0., 10., 10.]);
        let edges = voronoi_edges(&cells);
        // Two interior edges, four outer corners and two edge midpoints
        // against the bounds: the mesh is a lattice, not a pile of cell
        // outlines.
        assert!(edges.len() < cells.iter().map(|cell| cell.len()).sum::<usize>());
    }
}
