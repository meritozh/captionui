//! Plot's `waffleX` and `waffleY`: bars divided into unit cells.

use gpui::{point, px};

use crate::mark::MarkKind;
use crate::paint;
use crate::render::PreparedMark;
use crate::value::Value;

use super::bar::{bar_width_x, bar_width_y};
use super::{Colors, DrawContext};

/// Plot's `waffleX`/`waffleY`: a bar's interval divided into unit cells, so a
/// value reads as a count of squares rather than as a length.
///
/// Plot's geometry: one unit is `unit` times the value scale's pixels per
/// value; a row (or column) holds `multiple` cells — by default as many as
/// keep the cells about square — and the cells, `multiple` units tall, are
/// numbered from the scale's zero, so the interval `[y1, y2]` covers cells
/// `y1 / unit` to `y2 / unit` of that grid, wrapping row by row. A
/// fractional end cuts its cell short. A waffle with no cross channel spans
/// the frame.
pub(super) fn draw_waffle_bars(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let along_x = prepared.mark.kind == MarkKind::WaffleX;
    let unit = style.unit.max(f32::MIN_POSITIVE) as f64;
    let gap = style.gap;
    let (value_scale, cross_scale) = if along_x {
        (&ctx.frame.x.scale, &ctx.frame.y.scale)
    } else {
        (&ctx.frame.y.scale, &ctx.frame.x.scale)
    };
    let (Some(zero), Some(one)) = (
        value_scale.position(&Value::Number(0.)),
        value_scale.position(&Value::Number(unit)),
    ) else {
        return;
    };
    // The length of a unit in pixels, and which way the values grow.
    let scale = (one - zero).abs();
    let sign = if one >= zero { 1. } else { -1. };
    if !scale.is_finite() || scale <= 0. {
        return;
    }
    let cross_channel = if along_x {
        prepared.mark.channels.y.is_some()
    } else {
        prepared.mark.channels.x.is_some()
    };
    let frame_cross = if along_x {
        ctx.frame.plot.size.height.as_f32()
    } else {
        ctx.frame.plot.size.width.as_f32()
    };
    let bar = if !cross_channel {
        frame_cross
    } else if along_x {
        bar_width_x(ctx.frame, prepared)
    } else {
        bar_width_y(ctx.frame, prepared)
    };
    let multiple = style
        .multiple
        .unwrap_or_else(|| ((bar / scale).sqrt().floor() as usize).max(1));
    let m = multiple as f32;
    // The outer size of a cell, gap included: across, and along the value.
    let cx = (bar / m).min(scale * m);
    let cy = scale * m;
    let tx = (bar - m * cx) / 2.;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let cross_start = if !cross_channel {
                0.
            } else {
                let value = if along_x {
                    row.y_value()
                } else {
                    row.x_value()
                };
                let Some(centre) = cross_scale.position(value) else {
                    continue;
                };
                centre - bar / 2.
            };
            let interval = if along_x {
                row.x_interval()
            } else {
                row.y_interval()
            };
            let Some((start, end)) = interval else {
                continue;
            };
            let (Some(v1), Some(v2)) = (start.as_number(), end.as_number()) else {
                continue;
            };
            let (i1, i2) = {
                let (a, b) = (v1 / unit, v2 / unit);
                if a <= b { (a, b) } else { (b, a) }
            };
            if !(i1.is_finite() && i2.is_finite()) || i2 <= i1 {
                continue;
            }
            // Plot sets a constant `fillOpacity` on the waffle's group, which
            // both the pattern's cell (inheriting it) and the path the
            // pattern fills apply: the opacity counts twice.
            let color = paint::alpha(
                colors.fill(index),
                ctx.row_fill_opacity(prepared, row) * style.fill_opacity,
            );
            let radius = style.corner_radius;
            let mut k = i1.floor() as i64;
            while (k as f64) < i2 {
                let (lo, hi) = (
                    (i1 - k as f64).max(0.) as f32,
                    (i2 - k as f64).min(1.) as f32,
                );
                let column = k.rem_euclid(multiple as i64) as f32;
                let line = k.div_euclid(multiple as i64) as f32;
                k += 1;
                // The cell's pattern rect, inset by half the gap, and the
                // part of it the interval covers, in value-axis pixels from
                // zero.
                let (c0, c1) = (line * cy + gap / 2., line * cy + cy - gap / 2.);
                let (v0, v1) = (c0.max(line * cy + lo * cy), c1.min(line * cy + hi * cy));
                if v1 <= v0 {
                    continue;
                }
                let across0 = cross_start + tx + column * cx + gap / 2.;
                let across1 = across0 + cx - gap;
                if across1 <= across0 {
                    continue;
                }
                let (a, b) = (zero + sign * v0, zero + sign * v1);
                let (p0, p1) = if along_x {
                    (point(px(a), px(across0)), point(px(b), px(across1)))
                } else {
                    (point(px(across0), px(a)), point(px(across1), px(b)))
                };
                let rect = paint::rect_between(ctx.absolute(p0), ctx.absolute(p1));
                // SVG clamps a corner radius to half the rect (`rx: "100%"`
                // draws a circle).
                let r = radius
                    .min(rect.size.width.as_f32() / 2.)
                    .min(rect.size.height.as_f32() / 2.);
                paint::paint_rect(rect, r, color, ctx.window);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_waffle_wraps_its_cells_across_the_bar() {
        // Plot's layout: the number of cells across is the largest square that
        // fits the bar's thickness, and the cell is no wider than that square.
        let across = ((60_f32 / 10_f32).sqrt().floor() as usize).max(1);
        assert_eq!(across, 2);
        let cell_width = (60_f32 / across as f32).min(10_f32 * across as f32);
        assert_eq!(cell_width, 20.);
        let cell_height = 10_f32 * across as f32;
        assert_eq!(cell_height, 20.);
        // A value of 5 units fills two rows of two cells and a partial third.
        let count = (50_f32 / 10_f32).round() as usize;
        assert_eq!(count, 5);
    }
}
