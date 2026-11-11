//! Plot's `link`: a segment or curve from one point to another.

use gpui::{Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;
use crate::value::Value;

use super::{Colors, DrawContext, curve_path, paint_markers};

pub(super) fn draw_links(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let width = style.stroke_width;
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // Plot's link runs from (x1, y1) to (x2, y2); a lone `x` (or
            // `x1`) is both ends, Plot's `maybeSameValue`.
            let x_start = row.x1.clone().unwrap_or_else(|| row.x.clone());
            let x_end = row.x2.clone().unwrap_or_else(|| {
                if row.x.is_null() {
                    x_start.clone()
                } else {
                    row.x.clone()
                }
            });
            let y_start = row.y1.clone().unwrap_or_else(|| row.y.clone());
            let y_end = row.y2.clone().unwrap_or_else(|| {
                if row.y.is_null() {
                    y_start.clone()
                } else {
                    row.y.clone()
                }
            });
            let place = |x: &Value, y: &Value| -> Option<Point<Pixels>> {
                if let Some(projection) = &ctx.frame.projection {
                    let [px_, py_] = projection.project(x.as_number()?, y.as_number()?)?;
                    return Some(point(px(px_ as f32), px(py_ as f32)));
                }
                Some(point(
                    px(ctx.frame.x.scale.position(x)?),
                    px(ctx.frame.y.scale.position(y)?),
                ))
            };
            let (Some(from), Some(to)) = (place(&x_start, &y_start), place(&x_end, &y_end)) else {
                continue;
            };
            let (from, to) = (ctx.absolute(from), ctx.absolute(to));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            // Plot's link is straight unless it names a curve.
            let points = vec![from, to];
            match style.curve {
                crate::mark::Curve::Linear => {
                    if style.dash.is_empty() {
                        paint::paint_stroke(&points, width, color, ctx.window);
                    } else {
                        paint::paint_dashed(from, to, &style.dash, width, color, ctx.window);
                    }
                }
                curve => {
                    paint::paint_path(curve_path(&points, curve, width, 1.), color, ctx.window)
                }
            }
            paint_markers(
                &points,
                &style.markers,
                color,
                width,
                ctx.theme.background,
                ctx.window,
            );
        }
    });
}
