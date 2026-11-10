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
        let reach = radius as usize;
        for _ in 0..3 {
            for horizontal in [true, false] {
                let source = v.clone();
                for j in 0..h as isize {
                    for i in 0..w as isize {
                        let (mut sum, mut n) = (0., 0.);
                        let at = j as usize * w + i as usize;
                        // Away from the edges every tap is in the grid: the
                        // same taps in the same order, without the checks.
                        let (along, length) = if horizontal { (i, w) } else { (j, h) };
                        let interior = along as usize >= reach
                            && (along as usize) < length - reach.min(length);
                        if interior && horizontal {
                            for x in &source[at - reach..=at + reach] {
                                if x.is_finite() {
                                    sum += x;
                                    n += 1.;
                                }
                            }
                        } else if interior {
                            let mut tap = at - reach * w;
                            for _ in 0..=2 * reach {
                                let x = source[tap];
                                if x.is_finite() {
                                    sum += x;
                                    n += 1.;
                                }
                                tap += w;
                            }
                        } else {
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
                        }
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
    /// coordinates, d3-contour's `contours`: marching squares over the grid
    /// with each value at its cell's centre, the boundary pinned to the
    /// grid's edge, and a missing value read as below every threshold.
    /// Filled with the even-odd rule, the rings make the region, holes and
    /// all.
    pub fn contour(&self, threshold: f64) -> Vec<Vec<[f64; 2]>> {
        let (w, h) = (self.width, self.height);
        // d3 pads the grid with a border below the threshold, so every ring
        // closes; a crossing into the border sits on the grid's edge.
        let (pw, ph) = (w + 2, h + 2);
        let numbers = self.numbers();
        let below = f64::NEG_INFINITY;
        let mut padded = vec![below; pw * ph];
        for j in 0..h {
            for i in 0..w {
                let v = numbers[j * w + i];
                padded[(j + 1) * pw + i + 1] = if v.is_finite() { v } else { below };
            }
        }
        let (dx, dy) = (
            (self.x2 - self.x1) / w as f64,
            (self.y2 - self.y1) / h as f64,
        );
        // Node (c, r) of the padded grid is cell (c - 1, r - 1)'s centre;
        // the border nodes sit on the grid's edge rather than a cell beyond.
        let node_x = |c: f64| -> f64 { (c - 0.5).clamp(0., w as f64) };
        let node_y = |r: f64| -> f64 { (r - 0.5).clamp(0., h as f64) };
        let value = |c: usize, r: usize| padded[r * pw + c];
        type Edge = (usize, usize, u8);
        let crossing = |edge: Edge| -> [f64; 2] {
            let (c, r, dir) = edge;
            let (a, b) = if dir == 0 {
                (value(c, r), value(c + 1, r))
            } else {
                (value(c, r), value(c, r + 1))
            };
            // d3's smooth linear interpolation, with a missing side treated
            // as the threshold's own level so the crossing hugs the edge.
            let t = if a.is_finite() && b.is_finite() && (b - a).abs() > f64::EPSILON {
                ((threshold - a) / (b - a)).clamp(0., 1.)
            } else if a.is_finite() {
                1.
            } else {
                0.
            };
            let (gx, gy) = if dir == 0 {
                let (x0, x1) = (node_x(c as f64), node_x(c as f64 + 1.));
                (x0 + (x1 - x0) * t, node_y(r as f64))
            } else {
                let (y0, y1) = (node_y(r as f64), node_y(r as f64 + 1.));
                (node_x(c as f64), y0 + (y1 - y0) * t)
            };
            [self.x1 + gx * dx, self.y1 + gy * dy]
        };
        let above = |v: f64| v >= threshold;
        let mut segments: Vec<(Edge, Edge)> = Vec::new();
        for r in 0..ph - 1 {
            for c in 0..pw - 1 {
                let tl = above(value(c, r));
                let tr = above(value(c + 1, r));
                let br = above(value(c + 1, r + 1));
                let bl = above(value(c, r + 1));
                let code = (tl as u8) << 3 | (tr as u8) << 2 | (br as u8) << 1 | bl as u8;
                let top = (c, r, 0);
                let bottom = (c, r + 1, 0);
                let left = (c, r, 1);
                let right = (c + 1, r, 1);
                match code {
                    0 | 15 => {}
                    1 | 14 => segments.push((left, bottom)),
                    2 | 13 => segments.push((bottom, right)),
                    3 | 12 => segments.push((left, right)),
                    4 | 11 => segments.push((top, right)),
                    6 | 9 => segments.push((top, bottom)),
                    7 | 8 => segments.push((left, top)),
                    5 => {
                        segments.push((left, top));
                        segments.push((bottom, right));
                    }
                    10 => {
                        segments.push((left, bottom));
                        segments.push((top, right));
                    }
                    _ => unreachable!(),
                }
            }
        }
        // Only looked up, never iterated: the hasher cannot change the rings.
        let mut at: rustc_hash::FxHashMap<Edge, Vec<usize>> = Default::default();
        for (k, (a, b)) in segments.iter().enumerate() {
            at.entry(*a).or_default().push(k);
            at.entry(*b).or_default().push(k);
        }
        let mut used = vec![false; segments.len()];
        let mut rings = Vec::new();
        for first in 0..segments.len() {
            if used[first] {
                continue;
            }
            let start = segments[first].0;
            let mut ring = vec![crossing(start)];
            let (mut segment, mut from) = (first, start);
            loop {
                used[segment] = true;
                let (a, b) = segments[segment];
                let to = if a == from { b } else { a };
                ring.push(crossing(to));
                match at
                    .get(&to)
                    .and_then(|ks| ks.iter().copied().find(|k| !used[*k]))
                {
                    Some(next) => {
                        segment = next;
                        from = to;
                    }
                    None => break,
                }
            }
            if ring.len() >= 4 {
                rings.push(ring);
            }
        }
        rings
    }
}

/// A bucketed nearest-neighbour search over points in grid space.
struct Nearest<'a> {
    points: &'a [(f64, f64)],
    buckets: Buckets,
    size: f64,
}

/// The points' indices by bucket, in point order within each.
enum Buckets {
    /// A dense grid over the occupied buckets' range: bucket `(bx, by)`'s
    /// indices are `items[starts[k]..starts[k + 1]]`, `k` its place in the
    /// range.
    Dense {
        origin: (i64, i64),
        width: u64,
        height: u64,
        starts: Vec<u32>,
        items: Vec<u32>,
    },
    /// For points spread too far for a dense grid.
    Sparse(rustc_hash::FxHashMap<(i64, i64), Vec<usize>>),
}

impl Buckets {
    /// The buckets of `keys`, each a finite point's bucket and index, in
    /// point order.
    fn new(keys: &[((i64, i64), usize)]) -> Self {
        let sparse = || {
            let mut buckets: rustc_hash::FxHashMap<(i64, i64), Vec<usize>> = Default::default();
            for &(key, k) in keys {
                buckets.entry(key).or_default().push(k);
            }
            Buckets::Sparse(buckets)
        };
        let Some(&((x0, y0), _)) = keys.first() else {
            return sparse();
        };
        let (mut lo, mut hi) = ((x0, y0), (x0, y0));
        for &((bx, by), _) in keys {
            lo = (lo.0.min(bx), lo.1.min(by));
            hi = (hi.0.max(bx), hi.1.max(by));
        }
        // A grid much larger than the points (or than an index fits) is
        // left to the map.
        let span = |a: i64, b: i64| (b as i128 - a as i128 + 1) as u128;
        let cells = span(lo.0, hi.0) * span(lo.1, hi.1);
        if cells > (4 * keys.len() as u128).max(1 << 16) || keys.len() >= u32::MAX as usize {
            return sparse();
        }
        let (width, height) = (span(lo.0, hi.0) as u64, span(lo.1, hi.1) as u64);
        let place =
            |(bx, by): (i64, i64)| ((by - lo.1) as u64 * width + (bx - lo.0) as u64) as usize;
        let mut starts = vec![0u32; (width * height) as usize + 1];
        for &(key, _) in keys {
            starts[place(key) + 1] += 1;
        }
        for k in 1..starts.len() {
            starts[k] += starts[k - 1];
        }
        let mut next = starts.clone();
        let mut items = vec![0u32; keys.len()];
        for &(key, k) in keys {
            let at = &mut next[place(key)];
            items[*at as usize] = k as u32;
            *at += 1;
        }
        Buckets::Dense {
            origin: lo,
            width,
            height,
            starts,
            items,
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Buckets::Dense { items, .. } => items.is_empty(),
            Buckets::Sparse(map) => map.is_empty(),
        }
    }

    /// Call `f` with each index in bucket `(bx, by)`, in point order.
    #[inline]
    fn each(&self, (bx, by): (i64, i64), mut f: impl FnMut(usize)) {
        match self {
            Buckets::Dense {
                origin,
                width,
                height,
                starts,
                items,
            } => {
                let (i, j) = (bx as i128 - origin.0 as i128, by as i128 - origin.1 as i128);
                if i < 0 || j < 0 || i >= *width as i128 || j >= *height as i128 {
                    return;
                }
                let k = (j as u64 * width + i as u64) as usize;
                for &index in &items[starts[k] as usize..starts[k + 1] as usize] {
                    f(index as usize);
                }
            }
            Buckets::Sparse(map) => {
                if let Some(list) = map.get(&(bx, by)) {
                    for &index in list {
                        f(index);
                    }
                }
            }
        }
    }
}

impl<'a> Nearest<'a> {
    fn new(points: &'a [(f64, f64)]) -> Self {
        let size = 4.;
        let keys: Vec<((i64, i64), usize)> = points
            .iter()
            .enumerate()
            .filter(|(_, (x, y))| x.is_finite() && y.is_finite())
            .map(|(k, (x, y))| (((x / size).floor() as i64, (y / size).floor() as i64), k))
            .collect();
        Self {
            points,
            buckets: Buckets::new(&keys),
            size,
        }
    }

    fn find(&self, x: f64, y: f64) -> Option<usize> {
        let (bx, by) = (
            (x / self.size).floor() as i64,
            (y / self.size).floor() as i64,
        );
        let mut best: Option<(usize, f64)> = None;
        let visit = |best: &mut Option<(usize, f64)>, dx: i64, dy: i64| {
            self.buckets.each((bx + dx, by + dy), |k| {
                let (px, py) = self.points[k];
                let d = (px - x).powi(2) + (py - y).powi(2);
                if best.is_none_or(|(_, b)| d < b) {
                    *best = Some((k, d));
                }
            });
        };
        for ring in 0..2048_i64 {
            // The ring's buckets in the order a scan of its square by column
            // then row meets them (so ties go the same way): the left
            // column, the top and bottom of each column between, then the
            // right column.
            if ring == 0 {
                visit(&mut best, 0, 0);
            } else {
                for dy in -ring..=ring {
                    visit(&mut best, -ring, dy);
                }
                for dx in -ring + 1..ring {
                    visit(&mut best, dx, -ring);
                    visit(&mut best, dx, ring);
                }
                for dy in -ring..=ring {
                    visit(&mut best, ring, dy);
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

    /// `Nearest` as it was: a std map of buckets, and every bucket of each
    /// ring's square tested, the inner ones skipped.
    fn find_scanning_squares(points: &[(f64, f64)], x: f64, y: f64) -> Option<usize> {
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
        let (bx, by) = ((x / size).floor() as i64, (y / size).floor() as i64);
        let mut best: Option<(usize, f64)> = None;
        for ring in 0..2048_i64 {
            for dx in -ring..=ring {
                for dy in -ring..=ring {
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    if let Some(list) = buckets.get(&(bx + dx, by + dy)) {
                        for &k in list {
                            let (px, py) = points[k];
                            let d = (px - x).powi(2) + (py - y).powi(2);
                            if best.is_none_or(|(_, b)| d < b) {
                                best = Some((k, d));
                            }
                        }
                    }
                }
            }
            if let Some((_, d)) = best
                && ((ring as f64) * size).powi(2) > d
            {
                break;
            }
            if buckets.is_empty() {
                break;
            }
        }
        best.map(|(k, _)| k)
    }

    /// `RasterGrid::blur` as it was: every tap bounds-checked.
    fn blur_checking_every_tap(grid: &mut RasterGrid, r: f64) {
        if r <= 0. || grid.values.iter().any(|v| matches!(v, Value::Text(_))) {
            return;
        }
        let (w, h) = (grid.width, grid.height);
        let mut v = grid.numbers();
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
        grid.values = v.into_iter().map(Value::number).collect();
    }

    #[test]
    fn the_interior_blur_matches_the_checked_one() {
        let value = |x: f64, y: f64| {
            let t = (x * 12.9898 + y * 78.233).sin() * 43758.5453;
            let t = t - t.floor();
            if t < 0.05 {
                Value::Null
            } else if t < 0.07 {
                Value::Number(f64::NAN)
            } else if t < 0.08 {
                Value::Number(f64::INFINITY)
            } else {
                Value::Number(t * 1e3 - 300.)
            }
        };
        for (w, h) in [(1, 1), (3, 2), (7, 5), (24, 17), (61, 40)] {
            for r in [0.4, 1., 2.6, 3., 5., 12., 40., f64::NAN] {
                let grid = RasterGrid::from_function(&value, w, h, [0., 0., w as f64, h as f64]);
                let (mut fast, mut slow) = (grid.clone(), grid);
                fast.blur(r);
                blur_checking_every_tap(&mut slow, r);
                let bits = |g: &RasterGrid| -> Vec<Option<u64>> {
                    g.values
                        .iter()
                        .map(|v| v.as_number().map(f64::to_bits))
                        .collect()
                };
                assert_eq!(bits(&fast), bits(&slow), "{w}x{h} r={r}");
            }
        }
    }

    #[test]
    fn ring_perimeters_find_what_the_square_scan_found() {
        let mut seed = 99_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut sets: Vec<Vec<(f64, f64)>> = vec![
            Vec::new(),
            vec![(f64::NAN, 1.)],
            (0..300).map(|_| (next() * 200., next() * 120.)).collect(),
            // Ties: a lattice, queried at points equidistant from several.
            (0..100)
                .map(|i| ((i % 10) as f64 * 8., (i / 10) as f64 * 8.))
                .collect(),
            // A few far-apart clusters, so queries walk many rings.
            (0..30)
                .map(|i| ((i / 10) as f64 * 300. + next(), next() * 3.))
                .collect(),
        ];
        sets[2].extend([
            (f64::INFINITY, 0.),
            (0., f64::NAN),
            (-0., 0.),
            (5., 5.),
            (5., 5.),
        ]);
        // Spread too far for a dense grid, so the buckets fall back to a map.
        sets.push(
            (0..40)
                .map(|i| {
                    (
                        next() * 300. + if i % 2 == 0 { 0. } else { 3e6 },
                        next() * 100.,
                    )
                })
                .collect(),
        );
        assert!(matches!(
            Nearest::new(&sets[2]).buckets,
            Buckets::Dense { .. }
        ));
        assert!(matches!(Nearest::new(&sets[5]).buckets, Buckets::Sparse(_)));
        for points in &sets {
            let nearest = Nearest::new(points);
            let mut queries: Vec<(f64, f64)> = (0..120)
                .map(|_| (next() * 1000. - 100., next() * 300. - 100.))
                .collect();
            queries.extend((0..100).map(|i| ((i % 10) as f64 * 4., (i / 10) as f64 * 4.)));
            queries.extend([(-0., 0.), (0., 0.), (700., -150.)]);
            for (x, y) in queries {
                assert_eq!(
                    nearest.find(x, y),
                    find_scanning_squares(points, x, y),
                    "({x}, {y}) among {} points",
                    points.len()
                );
            }
        }
    }

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
