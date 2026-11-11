//! Plot's `vector` and its `spike` preset: a stem or a triangle at each datum.

use gpui::{Hsla, Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;

use super::{Colors, DrawContext, position};

/// Plot's `vector`: a stem of `length` pixels through the datum, rotated by
/// `rotate`, pointing up the frame's y axis.
pub(super) fn draw_vectors(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let width = style.stroke_width.max(1.);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(centre) = position(prepared, ctx, row) else {
                continue;
            };
            let length = prepared
                .mark
                .length
                .as_ref()
                .map(|channel| channel.read(&row.row))
                .unwrap_or(style.length);
            let rotate = prepared
                .mark
                .rotate
                .as_ref()
                .map(|channel| channel.read(&row.row))
                .unwrap_or(style.rotate);
            let color = colors.stroke(index);
            // The stem's ends, before rotation: the vector points up, so the
            // start is below the centre.
            let (start, end) = match style.anchor {
                crate::mark::VectorAnchor::Start => (0., -length),
                crate::mark::VectorAnchor::Middle => (length / 2., -length / 2.),
                crate::mark::VectorAnchor::End => (length, 0.),
            };
            let (sin, cos) = rotate.to_radians().sin_cos();
            let turn = |x: f32, y: f32| {
                point(
                    px(centre.x.as_f32() + x * cos - y * sin),
                    px(centre.y.as_f32() + x * sin + y * cos),
                )
            };
            let from = ctx.absolute(turn(0., start));
            let to = ctx.absolute(turn(0., end));
            paint::paint_stroke(&[from, to], width, color, ctx.window);
            paint_arrow_head(ctx, from, to, width, color);
        }
    });
}

/// A filled triangle at `to`, pointing along `from`→`to`.
fn paint_arrow_head(
    ctx: &mut DrawContext<'_>,
    from: Point<Pixels>,
    to: Point<Pixels>,
    width: f32,
    color: Hsla,
) {
    let dx = to.x.as_f32() - from.x.as_f32();
    let dy = to.y.as_f32() - from.y.as_f32();
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1. {
        return;
    }
    let size = (width * 3.).max(4.);
    let (ux, uy) = (dx / length, dy / length);
    let tip = to;
    let base = point(px(to.x.as_f32() - ux * size), px(to.y.as_f32() - uy * size));
    let left = point(
        px(base.x.as_f32() - uy * size / 2.),
        px(base.y.as_f32() + ux * size / 2.),
    );
    let right = point(
        px(base.x.as_f32() + uy * size / 2.),
        px(base.y.as_f32() - ux * size / 2.),
    );
    paint::paint_fill(&[tip, left, right], color, ctx.window);
}

/// Plot's `spike`: a triangle whose point sits at the datum, `length` tall.
/// Plot's `spike`: a vector preset that draws a triangle at the datum rather
/// than a stem with a head.
pub(super) fn draw_spikes(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(centre) = position(prepared, ctx, row) else {
                continue;
            };
            let length = prepared
                .mark
                .length
                .as_ref()
                .map(|channel| channel.read(&row.row))
                .unwrap_or(style.length);
            let color = colors.stroke(index);
            // Plot's spike shape: an apex `length` above the datum, with a base
            // half as wide as a dot's radius each side.
            let half = style.radius;
            let apex = ctx.absolute(point(px(centre.x.as_f32()), px(centre.y.as_f32() - length)));
            let left = ctx.absolute(point(px(centre.x.as_f32() - half), px(centre.y.as_f32())));
            let right = ctx.absolute(point(px(centre.x.as_f32() + half), px(centre.y.as_f32())));
            paint::paint_fill(
                &[apex, left, right],
                paint::alpha(color, style.fill_opacity * ctx.appear),
                ctx.window,
            );
            paint::paint_stroke(&[left, apex, right], style.stroke_width, color, ctx.window);
        }
    });
}
