//! Plot's `barY` and `barX`, and the width a band or a gap gives a bar.

use gpui::{point, px};

use crate::frame::Frame;
use crate::paint;
use crate::render::PreparedMark;
use crate::scale::PositionScale;

use super::{Colors, DrawContext};

pub(super) fn draw_bars_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = bar_width_y(ctx.frame, prepared);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // A bar without an x spans the frame, Plot's `barY` with only y.
            let full = row.x_value().is_null() && row.x1.is_none();
            let Some(center) = (if full {
                Some(ctx.frame.plot.size.width.as_f32() / 2.)
            } else {
                ctx.frame.x.scale.position(row.x_value())
            }) else {
                continue;
            };
            let width = if full {
                ctx.frame.plot.size.width.as_f32()
            } else {
                width
            };
            let Some((start, end)) = row.y_interval() else {
                continue;
            };
            let (Some(y0), Some(y1)) = (
                ctx.frame.y.scale.position(&start),
                ctx.frame.y.scale.position(&end),
            ) else {
                continue;
            };
            let inset = prepared.mark.style.inset.unwrap_or(0.);
            let x0 = center - width / 2. + inset;
            let x1 = center + width / 2. - inset;
            let (y0, y1) = {
                let (lo, hi) = (y0.min(y1), y0.max(y1));
                let inset = inset.min((hi - lo) / 2.);
                (lo + inset, hi - inset)
            };
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x1), px(y1))),
            );
            let color = colors.fill(index);
            paint::paint_rect(
                rect,
                prepared.mark.style.corner_radius,
                paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                ctx.window,
            );
            if prepared.mark.channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    prepared.mark.style.corner_radius,
                    prepared.mark.style.stroke_width,
                    colors.stroke(index),
                    ctx.window,
                );
            }
        }
    });
}

pub(super) fn draw_bars_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = bar_width_x(ctx.frame, prepared);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // A bar without a y spans the frame, Plot's `barX` with only x.
            let full = row.y_value().is_null() && row.y1.is_none();
            let Some(center) = (if full {
                Some(ctx.frame.plot.size.height.as_f32() / 2.)
            } else {
                ctx.frame.y.scale.position(row.y_value())
            }) else {
                continue;
            };
            let width = if full {
                ctx.frame.plot.size.height.as_f32()
            } else {
                width
            };
            let Some((start, end)) = row.x_interval() else {
                continue;
            };
            let (Some(x0), Some(x1)) = (
                ctx.frame.x.scale.position(&start),
                ctx.frame.x.scale.position(&end),
            ) else {
                continue;
            };
            let inset = prepared.mark.style.inset.unwrap_or(0.);
            let y0 = center - width / 2. + inset;
            let y1 = center + width / 2. - inset;
            let (x0, x1) = {
                let (lo, hi) = (x0.min(x1), x0.max(x1));
                let inset = inset.min((hi - lo) / 2.);
                (lo + inset, hi - inset)
            };
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x1), px(y1))),
            );
            let color = colors.fill(index);
            paint::paint_rect(
                rect,
                prepared.mark.style.corner_radius,
                paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                ctx.window,
            );
            if prepared.mark.channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    prepared.mark.style.corner_radius,
                    prepared.mark.style.stroke_width,
                    colors.stroke(index),
                    ctx.window,
                );
            }
        }
    });
}

/// The width of a vertical bar: its band's width on a band scale, or a
/// fraction of the smallest gap between positions on a continuous one.
pub(super) fn bar_width_y(frame: &Frame, prepared: &PreparedMark) -> f32 {
    match &frame.x.scale {
        PositionScale::Band(band) => band.band_width().max(1.),
        PositionScale::Point(point_scale) => {
            let step = point_scale.step();
            if step > 0. { step * 0.9 } else { 1. }
        }
        PositionScale::Continuous(_) => {
            let mut positions: Vec<f32> = prepared
                .rows
                .iter()
                .filter_map(|row| frame.x.scale.position(row.x_value()))
                .collect();
            positions.sort_by(f32::total_cmp);
            let gap = positions
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .filter(|gap| *gap > 0.)
                .reduce(f32::min)
                .unwrap_or_else(|| frame.plot.size.width.as_f32() / 20.);
            (gap * 0.9).max(1.)
        }
    }
}

pub(super) fn bar_width_x(frame: &Frame, prepared: &PreparedMark) -> f32 {
    match &frame.y.scale {
        PositionScale::Band(band) => band.band_width().max(1.),
        PositionScale::Point(point_scale) => {
            let step = point_scale.step();
            if step > 0. { step * 0.9 } else { 1. }
        }
        PositionScale::Continuous(_) => {
            let mut positions: Vec<f32> = prepared
                .rows
                .iter()
                .filter_map(|row| frame.y.scale.position(row.y_value()))
                .collect();
            positions.sort_by(f32::total_cmp);
            let gap = positions
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .filter(|gap| *gap > 0.)
                .reduce(f32::min)
                .unwrap_or_else(|| frame.plot.size.height.as_f32() / 20.);
            (gap * 0.9).max(1.)
        }
    }
}
