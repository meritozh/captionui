//! Plot's `ruleX` and `ruleY`: lines across the frame.

use gpui::{point, px};

use crate::paint;
use crate::render::PreparedMark;

use super::{Colors, DrawContext, paint_markers};

pub(super) fn draw_rules_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let height = ctx.frame.plot.size.height.as_f32();
    let width = prepared.mark.style.stroke_width;
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // A rule reads its own `x`; an `x1` stands in when only the
            // interval is set.
            // A rule without an x sits at the frame's middle, as Plot's does.
            let Some(x) = (if row.x_value().is_null() {
                Some(ctx.frame.plot.size.width.as_f32() / 2.)
            } else {
                ctx.frame.x.scale.position(row.x_value())
            }) else {
                continue;
            };
            // Plot's ruleX spans `y1`..`y2` when given them, else the frame.
            let (top, bottom) = match (&row.y1, &row.y2) {
                (Some(y1), Some(y2)) => {
                    let (Some(a), Some(b)) = (
                        ctx.frame.y.scale.position(y1),
                        ctx.frame.y.scale.position(y2),
                    ) else {
                        continue;
                    };
                    (a, b)
                }
                _ => (0., height),
            };
            // Plot draws a rule from y1 to y2: bottom to top in pixels.
            let start = ctx.absolute(point(px(x), px(bottom)));
            let end = ctx.absolute(point(px(x), px(top)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            if prepared.mark.style.dash.is_empty() {
                paint::paint_stroke(&[start, end], width, color, ctx.window);
            } else {
                paint::paint_dashed(
                    start,
                    end,
                    &prepared.mark.style.dash,
                    width,
                    color,
                    ctx.window,
                );
            }
            paint_markers(
                &[start, end],
                &prepared.mark.style.markers,
                color,
                width,
                ctx.theme.background,
                ctx.window,
            );
        }
    });
}

pub(super) fn draw_rules_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = ctx.frame.plot.size.width.as_f32();
    let stroke_width = prepared.mark.style.stroke_width;
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(y) = (if row.y_value().is_null() {
                Some(ctx.frame.plot.size.height.as_f32() / 2.)
            } else {
                ctx.frame.y.scale.position(row.y_value())
            }) else {
                continue;
            };
            let (left, right) = match (&row.x1, &row.x2) {
                (Some(x1), Some(x2)) => {
                    let (Some(a), Some(b)) = (
                        ctx.frame.x.scale.position(x1),
                        ctx.frame.x.scale.position(x2),
                    ) else {
                        continue;
                    };
                    (a, b)
                }
                _ => (0., width),
            };
            let start = ctx.absolute(point(px(left), px(y)));
            let end = ctx.absolute(point(px(right), px(y)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            if prepared.mark.style.dash.is_empty() {
                paint::paint_stroke(&[start, end], stroke_width, color, ctx.window);
            } else {
                paint::paint_dashed(
                    start,
                    end,
                    &prepared.mark.style.dash,
                    stroke_width,
                    color,
                    ctx.window,
                );
            }
            paint_markers(
                &[start, end],
                &prepared.mark.style.markers,
                color,
                stroke_width,
                ctx.theme.background,
                ctx.window,
            );
        }
    });
}
