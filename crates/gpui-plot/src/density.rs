//! Kernel density estimation: the estimator behind Plot's `density` and
//! `violin` marks.
//!
//! Plot uses d3's `density` with an Epanechnikov kernel by default. This
//! module reproduces that: a Gaussian kernel scaled to the sample's spread,
//! evaluated over a grid.

use crate::value::Value;

/// The kernel a density estimate smooths its samples with, Plot's `kde`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kernel {
    /// The default Gaussian kernel, `exp(-½ x²)`.
    #[default]
    Gaussian,
    /// Epanechnikov: `¾ (1 - x²)` inside the unit interval.
    Epanechnikov,
    /// A triangular kernel.
    Triangular,
    /// A uniform kernel.
    Uniform,
}

impl Kernel {
    fn weight(self, x: f64) -> f64 {
        match self {
            // The standard normal density, which d3's `density` divides by
            // the bandwidth after evaluating.
            Kernel::Gaussian => (-0.5 * x * x).exp() / (std::f64::consts::TAU).sqrt(),
            Kernel::Epanechnikov => {
                if x.abs() <= 1. {
                    0.75 * (1. - x * x)
                } else {
                    0.
                }
            }
            Kernel::Triangular => {
                if x.abs() <= 1. {
                    1. - x.abs()
                } else {
                    0.
                }
            }
            Kernel::Uniform => {
                if x.abs() <= 1. {
                    0.5
                } else {
                    0.
                }
            }
        }
    }
}

/// Estimate a density over `values`, returning the density and the domain it
/// spans.
///
/// `steps` is the number of grid points; Plot's default is about 100. The
/// bandwidth is the sample's standard deviation scaled by the rule of thumb
/// `1.06 · n^(-1/5)`, which is what d3 uses.
pub fn density_of(values: &[Value], steps: usize) -> Option<(Vec<f64>, [f64; 2])> {
    let numbers: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
    if numbers.is_empty() {
        return None;
    }

    let extent = crate::scale::extent(numbers.iter().map(|n| Value::Number(*n)))?;
    let [lo, hi] = extent;
    let (lo, hi) = if lo == hi {
        (lo - 0.5, hi + 0.5)
    } else {
        (lo, hi)
    };
    let steps = steps.max(8);

    let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
    let variance = numbers.iter().map(|n| (n - mean).powi(2)).sum::<f64>() / numbers.len() as f64;
    let deviation = variance.sqrt();
    let bandwidth = if deviation == 0. {
        // A constant sample still needs a width, or every point divides by
        // zero.
        (hi - lo) / steps as f64
    } else {
        deviation * 1.06 * (numbers.len() as f64).powf(-0.2)
    }
    .max(f64::MIN_POSITIVE);

    let kernel = Kernel::default();
    let mut density = Vec::with_capacity(steps);
    for index in 0..steps {
        let position = lo + (hi - lo) * index as f64 / (steps - 1) as f64;
        let estimate: f64 = numbers
            .iter()
            .map(|sample| kernel.weight((position - sample) / bandwidth))
            .sum();
        density.push(estimate / (numbers.len() as f64 * bandwidth));
    }

    Some((density, [lo, hi]))
}

/// A two-dimensional density estimate over a pixel grid, d3's
/// `contourDensity`: the points are binned into `cell`-pixel cells and blurred
/// by a Gaussian of standard deviation `bandwidth` pixels.
#[derive(Clone, Debug)]
pub struct DensityGrid {
    /// The number of cells across.
    pub columns: usize,
    /// The number of cells down.
    pub rows: usize,
    /// The size of a cell, in pixels.
    pub cell: f32,
    /// The density per cell, row-major; its units are relative (points per
    /// cell after blurring), which is all contour thresholds need.
    pub values: Vec<f64>,
    /// Where the grid's first cell starts, in plot pixels on both axes.
    pub origin: f32,
}

impl DensityGrid {
    /// Estimate the density of `points` (pixels, from the plot's top-left)
    /// over a `width × height` plot.
    ///
    /// The grid reaches three bandwidths past the plot on every side, as
    /// d3's does, so a contour near an edge closes outside the frame rather
    /// than being cut open at it.
    pub fn estimate(
        points: &[(f32, f32)],
        width: f32,
        height: f32,
        bandwidth: f32,
        cell: f32,
    ) -> Self {
        let weighted: Vec<(f32, f32, f64)> = points.iter().map(|&(x, y)| (x, y, 1.)).collect();
        Self::estimate_weighted(&weighted, width, height, bandwidth, cell)
    }

    /// [`DensityGrid::estimate`] with a weight per point, d3's
    /// `contourDensity().weight(…)`; weights may be negative.
    pub fn estimate_weighted(
        points: &[(f32, f32, f64)],
        width: f32,
        height: f32,
        bandwidth: f32,
        cell: f32,
    ) -> Self {
        let cell = cell.max(1.);
        let pad = (bandwidth * 3.).max(cell);
        let columns = (((width + pad * 2.) / cell).ceil() as usize).max(2);
        let rows = (((height + pad * 2.) / cell).ceil() as usize).max(2);
        let mut values = vec![0.0_f64; columns * rows];
        for &(x, y, w) in points {
            if !(x.is_finite() && y.is_finite() && w.is_finite()) {
                continue;
            }
            let column = ((x + pad) / cell).floor();
            let row = ((y + pad) / cell).floor();
            if column >= 0. && row >= 0. && (column as usize) < columns && (row as usize) < rows {
                values[row as usize * columns + column as usize] += w;
            }
        }
        let sigma = (bandwidth / cell) as f64;
        if sigma > 0. {
            let kernel = gaussian_kernel(sigma);
            blur(&mut values, columns, rows, &kernel, true);
            blur(&mut values, columns, rows, &kernel, false);
        }
        // The grid's origin sits `pad` pixels before the plot's.
        Self {
            columns,
            rows,
            cell,
            values,
            origin: -pad,
        }
    }

    /// The largest density on the grid.
    pub fn max(&self) -> f64 {
        self.values.iter().copied().fold(0., f64::max)
    }

    /// The contour lines at `threshold`, as polylines in plot pixels.
    ///
    /// Marching squares over the cell centres, with each crossing linearly
    /// interpolated, and the segments stitched into polylines; a closed
    /// contour ends where it began.
    pub fn contours(&self, threshold: f64) -> Vec<Vec<(f32, f32)>> {
        let (columns, rows) = (self.columns, self.rows);
        let value = |c: usize, r: usize| self.values[r * columns + c];
        // An edge between two adjacent grid nodes, named so both of the
        // squares that share it name it the same: horizontal edges are
        // `(c, r, 0)` from node (c, r) to (c+1, r); vertical ones `(c, r, 1)`
        // from (c, r) to (c, r+1).
        type Edge = (usize, usize, u8);
        let crossing = |edge: Edge| -> (f32, f32) {
            let (c, r, dir) = edge;
            let (a, b) = if dir == 0 {
                (value(c, r), value(c + 1, r))
            } else {
                (value(c, r), value(c, r + 1))
            };
            let t = if (b - a).abs() > f64::EPSILON {
                ((threshold - a) / (b - a)).clamp(0., 1.) as f32
            } else {
                0.5
            };
            let (x, y) = if dir == 0 {
                (c as f32 + t, r as f32)
            } else {
                (c as f32, r as f32 + t)
            };
            // Node (c, r) is the centre of cell (c, r).
            (
                self.origin + (x + 0.5) * self.cell,
                self.origin + (y + 0.5) * self.cell,
            )
        };

        let mut segments: Vec<(Edge, Edge)> = Vec::new();
        for r in 0..rows - 1 {
            for c in 0..columns - 1 {
                let above = |v: f64| v >= threshold;
                let tl = above(value(c, r));
                let tr = above(value(c + 1, r));
                let br = above(value(c + 1, r + 1));
                let bl = above(value(c, r + 1));
                let code = (tl as u8) << 3 | (tr as u8) << 2 | (br as u8) << 1 | bl as u8;
                let top = (c, r, 0);
                let bottom = (c, r + 1, 0);
                let left = (c, r, 1);
                let right = (c + 1, r, 1);
                let centre_above =
                    (value(c, r) + value(c + 1, r) + value(c + 1, r + 1) + value(c, r + 1)) / 4.
                        >= threshold;
                match code {
                    0 | 15 => {}
                    1 | 14 => segments.push((left, bottom)),
                    2 | 13 => segments.push((bottom, right)),
                    3 | 12 => segments.push((left, right)),
                    4 | 11 => segments.push((top, right)),
                    6 | 9 => segments.push((top, bottom)),
                    7 | 8 => segments.push((left, top)),
                    // The saddles: the centre decides which corners connect.
                    5 => {
                        if centre_above {
                            segments.push((left, top));
                            segments.push((bottom, right));
                        } else {
                            segments.push((left, bottom));
                            segments.push((top, right));
                        }
                    }
                    10 => {
                        if centre_above {
                            segments.push((left, bottom));
                            segments.push((top, right));
                        } else {
                            segments.push((left, top));
                            segments.push((bottom, right));
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }

        // Stitch: every crossing is shared by at most two segments.
        let mut at: std::collections::HashMap<Edge, Vec<usize>> = std::collections::HashMap::new();
        for (index, (a, b)) in segments.iter().enumerate() {
            at.entry(*a).or_default().push(index);
            at.entry(*b).or_default().push(index);
        }
        let mut used = vec![false; segments.len()];
        let mut lines = Vec::new();
        // Open chains first, from their ends, then the closed loops.
        let mut starts: Vec<(usize, Edge)> = Vec::new();
        for (edge, indices) in &at {
            if indices.len() == 1 {
                starts.push((indices[0], *edge));
            }
        }
        starts.sort();
        let loops = (0..segments.len()).map(|index| (index, segments[index].0));
        for (first, start) in starts.into_iter().chain(loops) {
            if used[first] {
                continue;
            }
            let mut line = vec![crossing(start)];
            let (mut segment, mut from) = (first, start);
            loop {
                used[segment] = true;
                let (a, b) = segments[segment];
                let to = if a == from { b } else { a };
                line.push(crossing(to));
                let next = at
                    .get(&to)
                    .and_then(|indices| indices.iter().copied().find(|index| !used[*index]));
                match next {
                    Some(next) => {
                        segment = next;
                        from = to;
                    }
                    None => break,
                }
            }
            if line.len() >= 2 {
                lines.push(line);
            }
        }
        lines
    }
}

/// A normalized Gaussian kernel of standard deviation `sigma` cells, reaching
/// three deviations either side.
fn gaussian_kernel(sigma: f64) -> Vec<f64> {
    let radius = (sigma * 3.).ceil() as isize;
    let mut kernel: Vec<f64> = (-radius..=radius)
        .map(|i| (-(i * i) as f64 / (2. * sigma * sigma)).exp())
        .collect();
    let total: f64 = kernel.iter().sum();
    for weight in kernel.iter_mut() {
        *weight /= total;
    }
    kernel
}

/// Convolve the grid with `kernel` along its rows (`horizontal`) or columns.
fn blur(values: &mut [f64], columns: usize, rows: usize, kernel: &[f64], horizontal: bool) {
    let radius = (kernel.len() / 2) as isize;
    let source = values.to_vec();
    for r in 0..rows {
        for c in 0..columns {
            let mut sum = 0.;
            for (k, weight) in kernel.iter().enumerate() {
                let offset = k as isize - radius;
                let (cc, rr) = if horizontal {
                    (c as isize + offset, r as isize)
                } else {
                    (c as isize, r as isize + offset)
                };
                if cc < 0 || rr < 0 || cc >= columns as isize || rr >= rows as isize {
                    continue;
                }
                sum += weight * source[rr as usize * columns + cc as usize];
            }
            values[r * columns + c] = sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_is_centered_on_the_data() {
        let values: Vec<Value> = (0..100).map(|i| Value::Number(i as f64)).collect();
        let (density, domain) = density_of(&values, 400).unwrap();
        let step = (domain[1] - domain[0]) / (density.len() - 1) as f64;

        // The grid spans the data only, so the Gaussian's tails are cut off and
        // the integral falls short of one; what must hold is that the estimate
        // is centered where the data is.
        let area: f64 = density.windows(2).map(|w| (w[0] + w[1]) / 2. * step).sum();
        assert!(area > 0.8 && area <= 1.0, "area was {area}");

        let mass: f64 = density.iter().sum::<f64>() * step;
        let mean = density
            .iter()
            .enumerate()
            .map(|(index, value)| (domain[0] + step * index as f64) * value)
            .sum::<f64>()
            * step
            / mass;
        assert!((mean - 49.5).abs() < 1.5, "mean was {mean}");
    }

    #[test]
    fn density_peaks_around_the_mode() {
        // Two clusters, the larger one on the right.
        let mut values: Vec<Value> = (0..20).map(|_| Value::Number(0.)).collect();
        values.extend((0..60).map(|_| Value::Number(10.)));
        let (density, _) = density_of(&values, 50).unwrap();
        let peak = density
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!(peak > density.len() / 2);
    }

    #[test]
    fn constant_samples_do_not_divide_by_zero() {
        let values = vec![Value::Number(5.); 10];
        let (density, _) = density_of(&values, 20).unwrap();
        assert!(density.iter().all(|d| d.is_finite()));
    }

    #[test]
    fn density_grid_contours_ring_a_cluster() {
        // A tight cluster at (50, 50): its contours are closed rings around it.
        let points: Vec<(f32, f32)> = (0..50)
            .map(|i| (50. + (i % 5) as f32, 50. + (i / 10) as f32))
            .collect();
        let grid = DensityGrid::estimate(&points, 100., 100., 10., 4.);
        let max = grid.max();
        assert!(max > 0.);
        let rings = grid.contours(max / 2.);
        assert_eq!(rings.len(), 1, "one ring around one cluster");
        let ring = &rings[0];
        assert_eq!(ring.first(), ring.last(), "the ring closes");
        for (x, y) in ring {
            let distance = ((x - 52.).powi(2) + (y - 52.).powi(2)).sqrt();
            assert!(
                distance > 2. && distance < 25.,
                "ring point at {distance}px"
            );
        }
        assert!(grid.contours(max * 2.).is_empty());
    }

    #[test]
    fn kernels_peak_at_zero() {
        for kernel in [Kernel::Gaussian, Kernel::Epanechnikov, Kernel::Triangular] {
            assert!(kernel.weight(0.) > kernel.weight(0.5));
        }
        // A uniform kernel is flat inside its support, which is the point.
        assert_eq!(Kernel::Uniform.weight(0.), Kernel::Uniform.weight(0.5));
    }
}
