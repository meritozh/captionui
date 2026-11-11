//! Plot's `differenceY` and `differenceX`: the filled gap between two series.

use gpui::{Pixels, Point, point, px};

use crate::mark::MarkKind;
use crate::paint;
use crate::render::{PreparedMark, PreparedRow};

use super::{DrawContext, series_groups};

/// Plot's `differenceX`/`differenceY`: the gap between two series as two
/// areas, one filled where the second is above the first and one where it is
/// below, with a line along the second.
pub(super) fn draw_differences(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    // `differenceY` compares two y values across x, so it runs along x; its
    // `differenceX` twin compares two x values down the y axis.
    let along_x = prepared.mark.kind == MarkKind::DifferenceY;
    // Plot's own fills: green where the second series is above the first,
    // blue where it is below.
    let opacity = prepared.mark.style.fill_opacity * ctx.appear;
    let style = &prepared.mark.style;
    let positive = style
        .positive_fill
        .unwrap_or(crate::scale::hsla_from_hex(0x3ca951));
    let negative = style
        .negative_fill
        .unwrap_or(crate::scale::hsla_from_hex(0x4269d0));
    let positive = paint::alpha(positive, opacity);
    let negative = paint::alpha(negative, opacity);
    let line = ctx.default_stroke(prepared);
    let width = prepared.mark.style.stroke_width.max(1.);

    // Each series is two curves, each placed by its own channels: the first
    // at (x1, y1) and the second at (x2, y2), so a `shiftX` compares a series
    // with itself moved along x.
    let curves: Vec<[Vec<(f32, f32)>; 2]> = series_groups(prepared)
        .into_iter()
        .map(|group| {
            [true, false].map(|first| {
                group
                    .iter()
                    .filter_map(|(_, row)| {
                        Some((
                            position_along_of(ctx, row, along_x, first)?,
                            position_across(ctx, row, along_x, first)?,
                        ))
                    })
                    .collect()
            })
        })
        .collect();

    let origin = ctx.frame.plot.origin;
    // Plot clips the difference's line to the frame (`clip: true`).
    let clip = ctx.clip;
    ctx.clip = true;
    ctx.clipped(|ctx| {
        let at = |along: f32, across: f32| {
            origin
                + if along_x {
                    point(px(along), px(across))
                } else {
                    point(px(across), px(along))
                }
        };
        for [first, second] in &curves {
            // Plot clips each curve's area by the other's, so the fills cover
            // only where both curves run, split where they cross.
            for (along, a, b, positive_run) in difference_spans(first, second, along_x) {
                let color = if positive_run { positive } else { negative };
                let mut polygon: Vec<Point<Pixels>> =
                    along.iter().zip(&a).map(|(t, v)| at(*t, *v)).collect();
                polygon.extend(along.iter().zip(&b).rev().map(|(t, v)| at(*t, *v)));
                paint::paint_fill(&polygon, color, ctx.window);
            }
            // The second series' line runs over the areas, as one path so it
            // has no breaks at the sign changes.
            let line_path: Vec<Point<Pixels>> = second.iter().map(|(t, v)| at(*t, *v)).collect();
            paint::paint_stroke(&line_path, width, line, ctx.window);
        }
    });
    ctx.clip = clip;
}

/// The runs of one sign between two piecewise-linear curves, each a list of
/// `(along, across)` pixels: for each run, its along positions, the first and
/// second curves' across positions there, and whether the second is the
/// larger value. Both curves are sampled at every vertex of either within
/// the span they share, and a crossing splits a run.
#[allow(clippy::type_complexity)]
fn difference_spans(
    first: &[(f32, f32)],
    second: &[(f32, f32)],
    along_x: bool,
) -> Vec<(Vec<f32>, Vec<f32>, Vec<f32>, bool)> {
    let sorted = |curve: &[(f32, f32)]| {
        let mut curve = curve.to_vec();
        curve.sort_by(|a, b| a.0.total_cmp(&b.0));
        curve
    };
    let (first, second) = (sorted(first), sorted(second));
    let (Some(f0), Some(s0), Some(f1), Some(s1)) =
        (first.first(), second.first(), first.last(), second.last())
    else {
        return Vec::new();
    };
    let (lo, hi) = (f0.0.max(s0.0), f1.0.min(s1.0));
    if hi <= lo {
        return Vec::new();
    }
    let sample = |curve: &[(f32, f32)], t: f32| -> f32 {
        let i = curve.partition_point(|p| p.0 < t);
        if i == 0 {
            return curve[0].1;
        }
        if i >= curve.len() {
            return curve[curve.len() - 1].1;
        }
        let (a, b) = (curve[i - 1], curve[i]);
        if b.0 == a.0 {
            b.1
        } else {
            a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0)
        }
    };
    let mut ts: Vec<f32> = first
        .iter()
        .chain(&second)
        .map(|p| p.0)
        .filter(|t| *t >= lo && *t <= hi)
        .chain([lo, hi])
        .collect();
    ts.sort_by(f32::total_cmp);
    ts.dedup();
    let sign = |a: f32, b: f32| difference_is_positive(along_x, a, b);
    type Run = (Vec<f32>, Vec<f32>, Vec<f32>, bool);
    let mut runs: Vec<Run> = Vec::new();
    let push = |t: f32, a: f32, b: f32, positive: bool, runs: &mut Vec<Run>| {
        match runs.last_mut() {
            Some((along, va, vb, p)) if *p == positive => {
                along.push(t);
                va.push(a);
                vb.push(b);
            }
            _ => runs.push((vec![t], vec![a], vec![b], positive)),
        }
    };
    let mut previous: Option<(f32, f32, f32)> = None;
    for &t in &ts {
        let (a, b) = (sample(&first, t), sample(&second, t));
        if let Some((pt, pa, pb)) = previous {
            let (d0, d1) = (pb - pa, b - a);
            if d0 * d1 < 0. {
                // The curves cross: both runs meet at the crossing.
                let u = d0 / (d0 - d1);
                let (ct, cv) = (pt + (t - pt) * u, pa + (a - pa) * u);
                let before = sign(pa, pb);
                push(ct, cv, cv, before, &mut runs);
                push(ct, cv, cv, !before, &mut runs);
            }
        }
        // A tie takes the sign of the run it continues.
        let positive = if a == b {
            runs.last().is_some_and(|r| r.3)
        } else {
            sign(a, b)
        };
        push(t, a, b, positive, &mut runs);
        previous = Some((t, a, b));
    }
    runs.retain(|run| run.0.len() >= 2);
    runs
}

/// A row's first or second position along the axis a difference runs along:
/// its `x1` or `x2` (for a `differenceY`), else its `x`.
fn position_along_of(
    ctx: &DrawContext<'_>,
    row: &PreparedRow,
    along_x: bool,
    first: bool,
) -> Option<f32> {
    let (one, two, value, scale) = if along_x {
        (&row.x1, &row.x2, row.x_value(), &ctx.frame.x.scale)
    } else {
        (&row.y1, &row.y2, row.y_value(), &ctx.frame.y.scale)
    };
    let chosen = if first { one } else { two };
    scale.position(chosen.as_ref().unwrap_or(value))
}

/// Whether a difference span's second series is the larger one, given both
/// series' positions across the axis the mark runs along.
///
/// The screen inverts the axes differently: a larger value sits higher on y but
/// further right on x, so the same comparison reads opposite on each.
fn difference_is_positive(along_x: bool, first: f32, second: f32) -> bool {
    if along_x {
        second < first
    } else {
        second > first
    }
}

/// A row's first or second value, on the axis across the difference.
fn position_across(
    ctx: &DrawContext<'_>,
    row: &PreparedRow,
    along_x: bool,
    first: bool,
) -> Option<f32> {
    let value = if along_x {
        if first {
            row.y1.clone().or_else(|| Some(row.y_value().clone()))
        } else {
            row.y2.clone().or_else(|| Some(row.y_value().clone()))
        }
    } else if first {
        row.x1.clone().or_else(|| Some(row.x_value().clone()))
    } else {
        row.x2.clone().or_else(|| Some(row.x_value().clone()))
    };
    let scale = if along_x {
        &ctx.frame.y.scale
    } else {
        &ctx.frame.x.scale
    };
    scale.position(value.as_ref()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_difference_fills_where_the_second_series_leads() {
        // A larger value is higher on y and further right on x, so "the second
        // series is larger" is the smaller screen position on y and the larger
        // one on x.
        assert!(difference_is_positive(true, 300., 200.));
        assert!(!difference_is_positive(true, 200., 300.));
        assert!(difference_is_positive(false, 100., 200.));
        assert!(!difference_is_positive(false, 200., 100.));
    }
}
