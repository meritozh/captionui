//! The dodge transform, Plot's `dodgeX` and `dodgeY`: beeswarm packing.

use crate::table::{Dataset, Table};
use crate::value::Value;

/// Where a dodge places its points relative to the axis it packs along.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DodgeAnchor {
    /// Pack from the low end of the axis. (Default for `dodge_x`.)
    #[default]
    Start,
    /// Pack symmetrically around the axis's middle.
    Middle,
    /// Pack from the high end of the axis.
    End,
}

/// A dodge transform's configuration.
#[derive(Clone, Copy, Debug)]
pub struct Dodge {
    /// Where the points pack from.
    pub anchor: DodgeAnchor,
    /// The radius of each point, in the units of the packed axis. Plot's
    /// default is the mark's dot radius.
    pub radius: f32,
    /// The gap between neighbouring points, Plot's `padding`.
    pub padding: f32,
}

impl Default for Dodge {
    fn default() -> Self {
        Self {
            anchor: DodgeAnchor::Start,
            radius: 3.,
            padding: 1.,
        }
    }
}

/// Pack one-dimensional points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
///
/// Given the position `field` along the axis the data varies on, this computes
/// the *other* axis so that no two points overlap: each point is placed at the
/// nearest free offset from the baseline where a circle of `radius` fits.
/// The result is written back under `output`, in the packed axis's units.
///
/// Plot runs this in pixel space, against the resolved scale; here the caller
/// passes positions already projected, so the units are pixels throughout.
pub fn dodge(
    data: &Dataset,
    positions: &[Option<f32>],
    output: &str,
    config: Dodge,
    extent: [f32; 2],
) -> Dataset {
    let radii = vec![config.radius; positions.len()];
    let order: Vec<usize> = (0..positions.len()).collect();
    let placed = dodge_positions(positions, &radii, &order, config.anchor, config.padding, extent);
    let rows: Vec<Table> = data
        .rows()
        .iter()
        .zip(placed)
        .map(|(row, placed)| match placed {
            Some(value) => row.clone().with(output, Value::Number(value as f64)),
            None => row.clone(),
        })
        .collect();
    Dataset::new(rows)
}

/// Plot's dodge in pixels: each point, taken in `order`, sits at the
/// nearest offset from the anchor where its circle of `radii[i]` clears
/// every circle already placed by `padding`. Returns each point's position
/// across the packed axis, in the coordinates of `extent`; a point without
/// a position, or with no positive radius, gets none.
pub fn dodge_positions(
    positions: &[Option<f32>],
    radii: &[f32],
    order: &[usize],
    anchor: DodgeAnchor,
    padding: f32,
    extent: [f32; 2],
) -> Vec<Option<f32>> {
    let (start, end) = (extent[0].min(extent[1]), extent[0].max(extent[1]));
    // Plot's anchors: a direction away from the baseline, and the baseline.
    let (ky, ty) = match anchor {
        DodgeAnchor::Start => (1., start),
        DodgeAnchor::End => (-1., end),
        DodgeAnchor::Middle => (0., (start + end) / 2.),
    };
    let mut y: Vec<f32> = vec![f32::NAN; positions.len()];
    let mut out: Vec<Option<f32>> = vec![None; positions.len()];
    // The placed circles, as (low, high, index) along the varying axis.
    let mut placed: Vec<(f32, f32, usize)> = Vec::new();
    let mut intervals: Vec<f32> = Vec::new();
    let mut candidates: Vec<f32> = Vec::new();
    let mut blocked: Vec<(f32, f32)> = Vec::new();
    for &i in order {
        let Some(x) = positions.get(i).copied().flatten().filter(|x| x.is_finite()) else {
            continue;
        };
        let ri = radii.get(i).copied().unwrap_or(0.);
        if ri.is_nan() || ri <= 0. {
            continue;
        }
        // The baseline sits a radius and the padding in from the anchor.
        let y0 = if ky != 0. { ri + padding } else { 0. };
        let (l, h) = (x - ri, x + ri);
        intervals.clear();
        intervals.extend([0., 0.]);
        for &(lj, hj, j) in &placed {
            if hj < l - padding || lj > h + padding {
                continue;
            }
            let xj = positions[j].unwrap_or(f32::NAN);
            let yj = y[j] - y0;
            let dx = x - xj;
            let dr = padding + ri + radii[j];
            let dy = (dr * dr - dx * dx).max(0.).sqrt();
            intervals.push(yj - dy);
            intervals.push(yj + dy);
        }
        candidates.clear();
        candidates.extend_from_slice(&intervals);
        if ky != 0. {
            candidates.retain(|c| *c >= 0.);
            candidates.sort_by(f32::total_cmp);
        } else {
            candidates.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
        }
        // A candidate is blocked inside any interval, shrunk by a hair:
        // `lo + 1e-6 < c < hi - 1e-6`. Those open intervals' union, sorted
        // and disjoint, blocks the same candidates and answers each in a
        // search rather than a pass over every interval.
        open_union(&intervals, &mut blocked);
        if let Some(&candidate) = candidates.iter().find(|&&c| {
            let k = blocked.partition_point(|&(lo, _)| lo < c);
            // A NaN is below no start, so it is never blocked, as before.
            k == 0 || c >= blocked[k - 1].1
        }) {
            y[i] = candidate + y0;
        }
        placed.push((l, h, i));
    }
    let ky = if ky == 0. { 1. } else { ky };
    for (i, value) in y.iter().enumerate() {
        if value.is_finite() {
            out[i] = Some(value * ky + ty);
        }
    }
    out
}

/// The union of the open intervals `(lo + 1e-6, hi - 1e-6)` over the pairs
/// `[lo, hi]` of `pairs`, as sorted, disjoint open intervals in `union`.
///
/// An empty or NaN interval contains nothing and is left out. Sorted by
/// start, an interval joins the one before when it starts strictly inside
/// it; one starting at the other's end stays apart, since that end is in
/// neither. A value then lies in some interval exactly when it lies in the
/// last union member starting below it, which compares only the values the
/// intervals were shrunk to.
fn open_union(pairs: &[f32], union: &mut Vec<(f32, f32)>) {
    union.clear();
    union.extend(
        pairs
            .chunks(2)
            .map(|pair| (pair[0] + 1e-6, pair[1] - 1e-6))
            .filter(|(lo, hi)| lo < hi),
    );
    union.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged = 0;
    for k in 0..union.len() {
        let (lo, hi) = union[k];
        if merged > 0 && lo < union[merged - 1].1 {
            let end = &mut union[merged - 1].1;
            if hi > *end {
                *end = hi;
            }
        } else {
            union[merged] = (lo, hi);
            merged += 1;
        }
    }
    union.truncate(merged);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `dodge_positions` as it was: each candidate tested against every
    /// interval.
    fn dodge_positions_scanning(
        positions: &[Option<f32>],
        radii: &[f32],
        order: &[usize],
        anchor: DodgeAnchor,
        padding: f32,
        extent: [f32; 2],
    ) -> Vec<Option<f32>> {
        let (start, end) = (extent[0].min(extent[1]), extent[0].max(extent[1]));
        let (ky, ty) = match anchor {
            DodgeAnchor::Start => (1., start),
            DodgeAnchor::End => (-1., end),
            DodgeAnchor::Middle => (0., (start + end) / 2.),
        };
        let mut y: Vec<f32> = vec![f32::NAN; positions.len()];
        let mut out: Vec<Option<f32>> = vec![None; positions.len()];
        let mut placed: Vec<(f32, f32, usize)> = Vec::new();
        let mut intervals: Vec<f32> = Vec::new();
        for &i in order {
            let Some(x) = positions
                .get(i)
                .copied()
                .flatten()
                .filter(|x| x.is_finite())
            else {
                continue;
            };
            let ri = radii.get(i).copied().unwrap_or(0.);
            if ri.is_nan() || ri <= 0. {
                continue;
            }
            let y0 = if ky != 0. { ri + padding } else { 0. };
            let (l, h) = (x - ri, x + ri);
            intervals.clear();
            intervals.extend([0., 0.]);
            for &(lj, hj, j) in &placed {
                if hj < l - padding || lj > h + padding {
                    continue;
                }
                let xj = positions[j].unwrap_or(f32::NAN);
                let yj = y[j] - y0;
                let dx = x - xj;
                let dr = padding + ri + radii[j];
                let dy = (dr * dr - dx * dx).max(0.).sqrt();
                intervals.push(yj - dy);
                intervals.push(yj + dy);
            }
            let mut candidates: Vec<f32> = intervals.clone();
            if ky != 0. {
                candidates.retain(|c| *c >= 0.);
                candidates.sort_by(f32::total_cmp);
            } else {
                candidates.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
            }
            'candidates: for candidate in candidates {
                for pair in intervals.chunks(2) {
                    if pair[0] + 1e-6 < candidate && candidate < pair[1] - 1e-6 {
                        continue 'candidates;
                    }
                }
                y[i] = candidate + y0;
                break;
            }
            placed.push((l, h, i));
        }
        let ky = if ky == 0. { 1. } else { ky };
        for (i, value) in y.iter().enumerate() {
            if value.is_finite() {
                out[i] = Some(value * ky + ty);
            }
        }
        out
    }

    #[test]
    fn the_interval_union_dodges_as_the_scan_did() {
        let mut seed = 31_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let bits = |v: &[Option<f32>]| -> Vec<Option<u32>> {
            v.iter().map(|p| p.map(f32::to_bits)).collect()
        };
        for n in [0, 1, 2, 5, 40, 400] {
            for case in 0..6 {
                let positions: Vec<Option<f32>> = (0..n)
                    .map(|_| match case {
                        // Snapped: many dots at the same x.
                        0 => Some((next() * 10.).floor() * 8.),
                        1 if next() < 0.1 => None,
                        1 if next() < 0.1 => Some(f32::NAN),
                        1 if next() < 0.05 => Some(f32::INFINITY),
                        _ => Some(next() * 600.),
                    })
                    .collect();
                let radii: Vec<f32> = (0..n)
                    .map(|_| match case {
                        2 => 3.,
                        3 if next() < 0.1 => 0.,
                        3 if next() < 0.1 => f32::NAN,
                        3 if next() < 0.1 => -2.,
                        _ => 0.5 + next() * 12.,
                    })
                    .collect();
                let mut order: Vec<usize> = (0..n).collect();
                if case % 2 == 1 {
                    order.reverse();
                }
                if case == 4 {
                    order.sort_by(|a, b| radii[*b].total_cmp(&radii[*a]));
                }
                for anchor in [DodgeAnchor::Start, DodgeAnchor::Middle, DodgeAnchor::End] {
                    for padding in [0., 1., 2.5] {
                        for extent in [[0., 400.], [300., 20.]] {
                            assert_eq!(
                                bits(&dodge_positions(
                                    &positions, &radii, &order, anchor, padding, extent
                                )),
                                bits(&dodge_positions_scanning(
                                    &positions, &radii, &order, anchor, padding, extent
                                )),
                                "n={n} case={case} {anchor:?} padding={padding} {extent:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn dodge_packs_overlapping_points_apart() {
        // Three points at the same x, with a radius of 4: they cannot all sit
        // on the baseline, so the dodge stacks them.
        let data = Dataset::new((0..3).map(|_| Table::empty().with("v", Value::Number(1.))));
        let packed = dodge(
            &data,
            &[Some(50.), Some(50.), Some(50.)],
            "dodge",
            Dodge {
                anchor: DodgeAnchor::Start,
                radius: 4.,
                padding: 1.,
            },
            [0., 100.],
        );
        let offsets: Vec<f64> = packed
            .values("dodge")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(offsets.len(), 3);
        // The dodge writes the cross-axis coordinate, not the along-axis one:
        // the first point sits a radius and the padding off the anchor, as
        // Plot's does, and the others are pushed off it by at least a
        // diameter so that no two overlap.
        assert_eq!(offsets[0], 5.);
        for pair in offsets.windows(2) {
            assert!((pair[1] - pair[0]).abs() >= 8., "points {pair:?} overlap");
        }
    }

    #[test]
    fn dodge_leaves_separated_points_on_the_baseline() {
        let data = Dataset::new((0..3).map(|_| Table::empty()));
        let packed = dodge(
            &data,
            &[Some(10.), Some(50.), Some(90.)],
            "dodge",
            Dodge::default(),
            [0., 100.],
        );
        let offsets: Vec<f64> = packed
            .values("dodge")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        // Far enough apart that every point fits where it already is, so each
        // keeps the baseline, a radius and the padding off the anchor.
        assert_eq!(offsets, vec![4., 4., 4.]);
    }
}
