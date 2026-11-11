//! Plot's `tickX` and `tickY`: strokes across a band or the whole frame.

use gpui::{point, px};

use crate::frame::Frame;
use crate::paint;
use crate::render::PreparedMark;
use crate::scale::PositionScale;
use crate::value::Value;

use super::{Colors, DrawContext};

pub(super) fn draw_ticks_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let height = ctx.frame.plot.size.height.as_f32();
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(x) = ctx.frame.x.scale.position(row.x_value()) else {
                continue;
            };
            // A tick spans the full height of the frame, Plot's "barcode";
            // with a `y` channel on a band scale it spans that band instead.
            let (y0, y1) = tick_span(ctx.frame, row.y_value(), true, height);
            let start = ctx.absolute(point(px(x), px(y0)));
            let end = ctx.absolute(point(px(x), px(y1)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            paint::paint_stroke(&[start, end], width, color, ctx.window);
        }
    });
}

pub(super) fn draw_ticks_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let width_px = ctx.frame.plot.size.width.as_f32();
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(y) = ctx.frame.y.scale.position(row.y_value()) else {
                continue;
            };
            let (x0, x1) = tick_span(ctx.frame, row.x_value(), false, width_px);
            let start = ctx.absolute(point(px(x0), px(y)));
            let end = ctx.absolute(point(px(x1), px(y)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            paint::paint_stroke(&[start, end], width, color, ctx.window);
        }
    });
}

/// The extent a tick covers along the axis it spans.
///
/// `value` is the tick's secondary channel: when it lands on a band scale the
/// tick covers that band, otherwise it covers the whole frame, which is what
/// Plot's tick does without a secondary channel.
fn tick_span(frame: &Frame, value: &Value, vertical: bool, full: f32) -> (f32, f32) {
    let scale = if vertical {
        &frame.y.scale
    } else {
        &frame.x.scale
    };
    match scale {
        PositionScale::Band(band) => match band.start(&value.key()) {
            Some(start) => (start, start + band.band_width()),
            None => (0., full),
        },
        _ => (0., full),
    }
}
