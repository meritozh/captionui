//! The bin transform, Plot's `Plot.binX` and `Plot.binY`, and its edges.

use crate::table::Dataset;
use crate::value::Value;

/// How a [`bin`] computes its bins, Plot's `thresholds` option.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Thresholds {
    /// About this many evenly spaced bins; Plot's `thresholds` number.
    Count(usize),
    /// One bin per tick of the scale's nice ticks, Plot's `thresholds` with a
    /// tick generator.
    #[default]
    Auto,
    /// Bins of this fixed width, Plot's `interval`.
    Interval(f64),
}

/// A histogram bin: the interval `[x0, x1)` and the rows that fell in it.
#[derive(Clone, Debug)]
pub struct Bin {
    pub x0: f64,
    pub x1: f64,
    pub rows: Vec<usize>,
}

impl Bin {
    pub fn count(&self) -> f64 {
        self.rows.len() as f64
    }
}

/// Bin rows by a numeric field, Plot's `Plot.binX`/`Plot.binY`.
///
/// Returns the bins and the domain the bins span, so a mark can draw them
/// without recomputing the extent.
pub fn bin(data: &Dataset, field: &str, thresholds: Thresholds) -> (Vec<Bin>, [f64; 2]) {
    let values: Vec<(usize, f64)> = data
        .rows()
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            row.get(field)
                .and_then(Value::as_number)
                // NaN and ±inf fall in no bin: NaN would floor into the
                // first one, and an infinite extent has no finite width.
                .filter(|v| v.is_finite())
                .map(|v| (index, v))
        })
        .collect();

    let numbers: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
    let Some(edges) = BinEdges::of(&numbers, thresholds) else {
        return (Vec::new(), [0., 1.]);
    };
    let mut bins: Vec<Bin> = (0..edges.count)
        .map(|i| {
            let (x0, x1) = edges.bounds(i);
            Bin {
                x0,
                x1,
                rows: Vec::new(),
            }
        })
        .collect();
    for (index, value) in values {
        bins[edges.slot(value)].rows.push(index);
    }

    let domain = [bins[0].x0, bins[bins.len() - 1].x1];
    (bins, domain)
}

/// The edges a [`bin`] lays over one axis: `count` bins of `width`, the first
/// starting at `start`.
///
/// Like d3's `bin` with tick thresholds, the edges fall on multiples of a
/// nice width, so the bins read 30–32, 32–34 … rather than starting at the
/// data's minimum; the maximum lands in the last bin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BinEdges {
    pub start: f64,
    pub width: f64,
    pub count: usize,
}

impl BinEdges {
    /// The edges for finite `values` under `thresholds`, or `None` when there
    /// are no values.
    pub fn of(values: &[f64], thresholds: Thresholds) -> Option<Self> {
        let mut values = values.iter().copied().filter(|v| v.is_finite());
        let first = values.next()?;
        let (mut lo, mut hi, mut n, mut sum, mut sum_sq) =
            (first, first, 1usize, first, first * first);
        for v in values {
            lo = lo.min(v);
            hi = hi.max(v);
            n += 1;
            sum += v;
            sum_sq += v * v;
        }
        let (lo, hi) = if lo == hi {
            (lo - 0.5, hi + 0.5)
        } else {
            (lo, hi)
        };
        /// More bins than any histogram draws; a bound against a tiny interval.
        const MAX_BINS: f64 = 10_000.;
        let span = hi - lo;
        let width = match thresholds {
            Thresholds::Interval(width)
                if width.is_finite() && width > 0. && span / width <= MAX_BINS =>
            {
                width
            }
            Thresholds::Count(count) => nice_bin_width(span / count.max(1) as f64),
            _ => {
                // Plot's default `auto` thresholds: Scott's normal reference
                // rule, capped at 200 bins.
                let mean = sum / n as f64;
                let variance =
                    (sum_sq / n as f64 - mean * mean).max(0.) * n as f64 / (n.max(2) - 1) as f64;
                let scott = 3.49 * variance.sqrt() * (n as f64).powf(-1. / 3.);
                let bins = if scott > 0. && scott.is_finite() {
                    (span / scott).ceil().clamp(1., 200.)
                } else {
                    1.
                };
                nice_bin_width(span / bins)
            }
        };
        let start = (lo / width).floor() * width;
        // A maximum exactly on an edge belongs to the bin it closes.
        let count = ((hi - start) / width).ceil().clamp(1., MAX_BINS) as usize;
        Some(Self {
            start,
            width,
            count,
        })
    }

    /// The bounds of bin `index`. Each edge is `start + i * width` rather
    /// than a running sum, so rounding does not drift across the bins.
    pub fn bounds(&self, index: usize) -> (f64, f64) {
        (
            self.start + index as f64 * self.width,
            self.start + (index + 1) as f64 * self.width,
        )
    }

    /// The bin a value falls in, clamped to the edges.
    pub fn slot(&self, value: f64) -> usize {
        let slot = ((value - self.start) / self.width).floor() as isize;
        slot.clamp(0, self.count as isize - 1) as usize
    }
}

fn nice_bin_width(rough: f64) -> f64 {
    if rough <= 0. || !rough.is_finite() {
        return 1.;
    }
    // d3's `tickIncrement`: the power of ten times 1, 2, 5 or 10 nearest the
    // rough step on a log scale, so 2.3 rounds to 2 rather than up to 5.
    let exponent = rough.log10().floor();
    let base = 10f64.powf(exponent);
    let fraction = rough / base;
    let nice = if fraction >= 50f64.sqrt() {
        10.
    } else if fraction >= 10f64.sqrt() {
        5.
    } else if fraction >= 2f64.sqrt() {
        2.
    } else {
        1.
    };
    nice * base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Table;

    #[test]
    fn bins_cover_the_extent() {
        let data = Dataset::new((0..10).map(|i| Table::empty().with("v", Value::Number(i as f64))));
        let (bins, domain) = bin(&data, "v", Thresholds::Count(5));
        assert_eq!(domain, [0., 10.]);
        assert_eq!(bins.iter().map(Bin::count).sum::<f64>(), 10.);
    }
}
