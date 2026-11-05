//! The raster grid behind Plot's `raster` and `contour` marks.
//!
//! A raster is a grid of values over a rectangle `[x1, x2] × [y1, y2]` in data
//! coordinates. Plot fills it three ways: from a dense array of values with a
//! `width` and `height`; by sampling a function `(x, y) → value`; or by
//! interpolating scattered samples (`nearest`, `barycentric`, or Plot's
//! `random-walk`). It may then be blurred, and a contour mark traces the
//! regions at or above each of its thresholds as closed rings, d3-contour's
//! `contours`.

use std::rc::Rc;

use crate::value::Value;

/// How scattered samples fill the grid, Plot's `interpolate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolate {
    /// Each sample fills only the cell it lands in.
    None,
    /// Each cell takes its nearest sample's value (a Voronoi diagram).
    Nearest,
    /// Each cell inside the samples' triangulation blends its triangle's
    /// corners; cells outside take their nearest sample.
    Barycentric,
    /// Plot's random walk, approximated by nearest-sample interpolation
    /// smoothed by a blur.
    RandomWalk,
}

impl Interpolate {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "none" => Self::None,
            "nearest" => Self::Nearest,
            "barycentric" => Self::Barycentric,
            "random-walk" => Self::RandomWalk,
            _ => return Option::None,
        })
    }
}

/// A contour mark's thresholds, Plot's `thresholds` and `interval`.
#[derive(Clone, Debug)]
pub enum ContourThresholds {
    /// About this many, at nice values.
    Count(usize),
    /// Exactly these.
    Values(Vec<f64>),
    /// Every multiple of this step.
    Interval(f64),
}

/// A raster or contour mark's options.
#[derive(Clone, Default)]
pub struct RasterSpec {
    /// Plot's `width` and `height`: the grid a dense array of values fills.
    pub size: Option<(usize, usize)>,
    /// Plot's `fill: (x, y) => …` (or `value`): a function the grid samples.
    pub function: Option<Rc<dyn Fn(f64, f64) -> Value>>,
    pub interpolate: Option<Interpolate>,
    /// Plot's `blur`, in grid cells.
    pub blur: f64,
    pub thresholds: Option<ContourThresholds>,
}

impl std::fmt::Debug for RasterSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RasterSpec")
            .field("size", &self.size)
            .field("interpolate", &self.interpolate)
            .field("blur", &self.blur)
            .finish_non_exhaustive()
    }
}

/// A filled raster grid in data coordinates. Cell `(i, j)` (row-major, `i`
/// across) spans `x1 + i·dx … x1 + (i+1)·dx` and likewise in y.
#[derive(Clone, Debug)]
pub struct RasterGrid {
    pub width: usize,
    pub height: usize,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub values: Vec<Value>,
}

impl RasterGrid {
    /// The data-space centre of cell `(i, j)`.
    pub fn center(&self, i: usize, j: usize) -> (f64, f64) {
        let dx = (self.x2 - self.x1) / self.width as f64;
        let dy = (self.y2 - self.y1) / self.height as f64;
        (
            self.x1 + (i as f64 + 0.5) * dx,
            self.y1 + (j as f64 + 0.5) * dy,
        )
    }

    /// The numbers of the grid, NaN where missing or not numeric.
    pub fn numbers(&self) -> Vec<f64> {
        self.values
            .iter()
            .map(|v| v.as_number().unwrap_or(f64::NAN))
            .collect()
    }

    /// A grid from a dense array of values.
    pub fn from_values(values: Vec<Value>, width: usize, height: usize, extent: [f64; 4]) -> Self {
        let [x1, y1, x2, y2] = extent;
        Self {
            width,
            height,
            x1,
            y1,
            x2,
            y2,
            values,
        }
    }

    /// A grid sampling `f` at each cell's centre.
    pub fn from_function(
        f: &dyn Fn(f64, f64) -> Value,
        width: usize,
        height: usize,
        extent: [f64; 4],
    ) -> Self {
        let [x1, y1, x2, y2] = extent;
        let mut grid = Self {
            width,
            height,
            x1,
            y1,
            x2,
            y2,
            values: Vec::with_capacity(width * height),
        };
        for j in 0..height {
            for i in 0..width {
                let (x, y) = grid.center(i, j);
                grid.values.push(f(x, y));
            }
        }
        grid
    }

    /// A grid interpolating scattered `(x, y, value)` samples.
    pub fn from_samples(
        samples: &[(f64, f64, Value)],
        width: usize,
        height: usize,
        extent: [f64; 4],
        how: Interpolate,
    ) -> Self {
        let [x1, y1, x2, y2] = extent;
        let mut grid = Self {
            width,
            height,
            x1,
            y1,
            x2,
            y2,
            values: vec![Value::Null; width * height],
        };
        if samples.is_empty() {
            return grid;
        }
        let to_grid = |x: f64, y: f64| {
            (
                (x - x1) / (x2 - x1) * width as f64,
                (y - y1) / (y2 - y1) * height as f64,
            )
        };
        let points: Vec<(f64, f64)> = samples.iter().map(|(x, y, _)| to_grid(*x, *y)).collect();
        match how {
            Interpolate::None => {
                for ((gx, gy), (_, _, v)) in points.iter().zip(samples) {
                    let (i, j) = (gx.floor() as isize, gy.floor() as isize);
                    if i >= 0 && j >= 0 && (i as usize) < width && (j as usize) < height {
                        grid.values[j as usize * width + i as usize] = v.clone();
                    }
                }
            }
            Interpolate::Nearest | Interpolate::RandomWalk => {
                let nearest = Nearest::new(&points);
                for j in 0..height {
                    for i in 0..width {
                        if let Some(k) = nearest.find(i as f64 + 0.5, j as f64 + 0.5) {
                            grid.values[j * width + i] = samples[k].2.clone();
                        }
                    }
                }
            }
            Interpolate::Barycentric => {
                let triangulation = crate::delaunay::Triangulation::new(
                    &points.iter().map(|p| [p.0, p.1]).collect::<Vec<_>>(),
                );
                // Each distinct point's first sample value.
                let mut value_of: Vec<f64> = vec![f64::NAN; triangulation.points.len()];
                for (k, &d) in triangulation.indices.iter().enumerate() {
                    if value_of[d].is_nan() {
                        value_of[d] = samples[k].2.as_number().unwrap_or(f64::NAN);
                    }
                }
                let mut filled = vec![false; width * height];
                for t in &triangulation.triangles {
                    let [a, b, c] = t.map(|k| triangulation.points[k]);
                    let [va, vb, vc] = t.map(|k| value_of[k]);
                    let (min_x, max_x) = (a[0].min(b[0]).min(c[0]), a[0].max(b[0]).max(c[0]));
                    let (min_y, max_y) = (a[1].min(b[1]).min(c[1]), a[1].max(b[1]).max(c[1]));
                    let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
                    if det.abs() < 1e-12 {
                        continue;
                    }
                    let i0 = (min_x - 0.5).ceil().max(0.) as usize;
                    let i1 = ((max_x - 0.5).floor() as isize).min(width as isize - 1);
                    let j0 = (min_y - 0.5).ceil().max(0.) as usize;
                    let j1 = ((max_y - 0.5).floor() as isize).min(height as isize - 1);
                    if i1 < 0 || j1 < 0 {
                        continue;
                    }
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            let (px, py) = (i as f64 + 0.5, j as f64 + 0.5);
                            let l1 =
                                ((b[1] - c[1]) * (px - c[0]) + (c[0] - b[0]) * (py - c[1])) / det;
                            let l2 =
                                ((c[1] - a[1]) * (px - c[0]) + (a[0] - c[0]) * (py - c[1])) / det;
                            let l3 = 1. - l1 - l2;
                            if l1 >= -1e-9 && l2 >= -1e-9 && l3 >= -1e-9 {
                                grid.values[j * width + i] =
                                    Value::number(l1 * va + l2 * vb + l3 * vc);
                                filled[j * width + i] = true;
                            }
                        }
                    }
                }
                // Plot extrapolates beyond the hull from the nearest sample.
                let nearest = Nearest::new(&points);
                for j in 0..height {
                    for i in 0..width {
                        if !filled[j * width + i]
                            && let Some(k) = nearest.find(i as f64 + 0.5, j as f64 + 0.5)
                        {
                            grid.values[j * width + i] = samples[k].2.clone();
                        }
                    }
                }
            }
        }
        if how == Interpolate::RandomWalk {
            grid.blur(2.);
        }
        grid
    }

    /// d3's `blur2` on the grid's numbers: three box-blur passes of radius
    /// `r` each way, ignoring missing values.
    pub fn blur(&mut self, r: f64) {
        if r <= 0. || self.values.iter().any(|v| matches!(v, Value::Text(_))) {
            return;
        }
        let (w, h) = (self.width, self.height);
        let mut v = self.numbers();
        let radius = r.round().max(1.) as isize;
        for _ in 0..3 {
            for horizontal in [true, false] {
                let source = v.clone();
                for j in 0..h as isize {
                    for i in 0..w as isize {
                        let (mut sum, mut n) = (0., 0.);
                        for k in -radius..=radius {
                            let (ii, jj) = if horizontal { (i + k, j) } else { (i, j + k) };
                            if ii < 0 || jj < 0 || ii >= w as isize || jj >= h as isize {
                                continue;
                            }
                            let x = source[jj as usize * w + ii as usize];
                            if x.is_finite() {
                                sum += x;
                                n += 1.;
                            }
                        }
                        let at = j as usize * w + i as usize;
                        if source[at].is_finite() && n > 0. {
                            v[at] = sum / n;
                        }
                    }
                }
            }
        }
        self.values = v.into_iter().map(Value::number).collect();
    }

    /// The finite extent of the grid's numbers.
    pub fn extent(&self) -> Option<[f64; 2]> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for v in self.numbers() {
            if v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        (lo <= hi).then_some([lo, hi])
    }

    /// Plot's contour thresholds over the grid: about Sturges' count of nice
    /// values (or the given ones), trimmed to the values' extent.
    pub fn thresholds(&self, spec: Option<&ContourThresholds>) -> Vec<f64> {
        let Some([lo, hi]) = self.extent() else {
            return Vec::new();
        };
        match spec {
            Some(ContourThresholds::Values(values)) => values.clone(),
            Some(ContourThresholds::Interval(step)) if *step > 0. => {
                let mut t = (lo / step).floor() * step;
                let mut out = Vec::new();
                while t < hi && out.len() < 10_000 {
                    out.push(t);
                    t += step;
                }
                out
            }
            other => {
                let count = match other {
                    Some(ContourThresholds::Count(n)) => *n,
                    _ => {
                        let n = self
                            .numbers()
                            .iter()
                            .filter(|v| v.is_finite())
                            .count()
                            .max(1);
                        ((n as f64).log2().ceil() as usize) + 1
                    }
                };
                let step = crate::ticks::nice_step((hi - lo) / count.max(1) as f64);
                let (nlo, nhi) = ((lo / step).floor() * step, (hi / step).ceil() * step);
                let mut ticks: Vec<f64> =
                    crate::ticks::linear_ticks([nlo, nhi], count, crate::scale::Transform::Linear);
                while ticks.last().is_some_and(|t| *t >= hi) {
                    ticks.pop();
                }
                while ticks.len() > 1 && ticks[1] < lo {
                    ticks.remove(0);
                }
                ticks
            }
        }
    }

    /// The closed rings bounding the region at or above `threshold`, in data
    /// coordinates; filled with the even-odd rule they make the region,
    /// holes and all.
    pub fn contour(&self, threshold: f64) -> Vec<Vec<[f64; 2]>> {
        let (w, h) = (self.width, self.height);
        // Pad with a border below every threshold so every ring closes.
        let (pw, ph) = (w + 2, h + 2);
        let numbers = self.numbers();
        let mut padded = vec![f64::NEG_INFINITY; pw * ph];
        for j in 0..h {
            for i in 0..w {
                let v = numbers[j * w + i];
                padded[(j + 1) * pw + i + 1] = if v.is_finite() { v } else { f64::NEG_INFINITY };
            }
        }
        // A finite stand-in for "below", so crossings interpolate sensibly.
        let floor = self
            .extent()
            .map_or(threshold - 1., |[lo, _]| lo.min(threshold) - 1.);
        for v in padded.iter_mut() {
            if !v.is_finite() {
                *v = floor;
            }
        }
        let grid = crate::density::DensityGrid {
            columns: pw,
            rows: ph,
            cell: 1.,
            values: padded,
            origin: -1.5,
        };
        let (dx, dy) = (
            (self.x2 - self.x1) / w as f64,
            (self.y2 - self.y1) / h as f64,
        );
        grid.contours(threshold)
            .into_iter()
            .filter(|ring| ring.len() >= 3)
            .map(|ring| {
                ring.into_iter()
                    .map(|(gx, gy)| {
                        [
                            self.x1 + (gx as f64 + 0.5) * dx,
                            self.y1 + (gy as f64 + 0.5) * dy,
                        ]
                    })
                    .collect()
            })
            .collect()
    }
}

/// A bucketed nearest-neighbour search over points in grid space.
struct Nearest<'a> {
    points: &'a [(f64, f64)],
    buckets: std::collections::HashMap<(i64, i64), Vec<usize>>,
    size: f64,
}

impl<'a> Nearest<'a> {
    fn new(points: &'a [(f64, f64)]) -> Self {
        let size = 4.;
        let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> =
            std::collections::HashMap::new();
        for (k, (x, y)) in points.iter().enumerate() {
            if x.is_finite() && y.is_finite() {
                buckets
                    .entry(((x / size).floor() as i64, (y / size).floor() as i64))
                    .or_default()
                    .push(k);
            }
        }
        Self {
            points,
            buckets,
            size,
        }
    }

    fn find(&self, x: f64, y: f64) -> Option<usize> {
        let (bx, by) = (
            (x / self.size).floor() as i64,
            (y / self.size).floor() as i64,
        );
        let mut best: Option<(usize, f64)> = None;
        for ring in 0..2048_i64 {
            for dx in -ring..=ring {
                for dy in -ring..=ring {
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    if let Some(list) = self.buckets.get(&(bx + dx, by + dy)) {
                        for &k in list {
                            let (px, py) = self.points[k];
                            let d = (px - x).powi(2) + (py - y).powi(2);
                            if best.is_none_or(|(_, b)| d < b) {
                                best = Some((k, d));
                            }
                        }
                    }
                }
            }
            // A ring further out can't hold anything nearer than `best`
            // once its inner edge is beyond it.
            if let Some((_, d)) = best
                && ((ring as f64) * self.size).powi(2) > d
            {
                break;
            }
            if self.buckets.is_empty() {
                break;
            }
        }
        best.map(|(k, _)| k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contours_close_around_a_peak() {
        let grid = RasterGrid::from_function(
            &|x, y| Value::Number(-((x - 5.).powi(2) + (y - 5.).powi(2))),
            20,
            20,
            [0., 0., 10., 10.],
        );
        let rings = grid.contour(-4.);
        assert_eq!(rings.len(), 1);
        let ring = &rings[0];
        assert_eq!(ring.first(), ring.last());
        for p in ring {
            let r = ((p[0] - 5.).powi(2) + (p[1] - 5.).powi(2)).sqrt();
            assert!((r - 2.).abs() < 0.3, "radius {r}");
        }
        assert!(!grid.thresholds(None).is_empty());
    }

    #[test]
    fn nearest_fills_every_cell() {
        let samples = vec![(0., 0., Value::Number(1.)), (10., 10., Value::Number(2.))];
        let grid =
            RasterGrid::from_samples(&samples, 10, 10, [0., 0., 10., 10.], Interpolate::Nearest);
        assert_eq!(grid.values[0], Value::Number(1.));
        assert_eq!(grid.values[99], Value::Number(2.));
    }
}
