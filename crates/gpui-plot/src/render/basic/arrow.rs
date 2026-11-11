//! Plot's `arrow`: a straight or bent shaft with a head.

use gpui::{Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;

use super::{Colors, DrawContext};

pub(super) fn draw_arrows(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let width = style.stroke_width.max(1.);
    let bend = style.bend.clamp(-90., 90.);
    // Plot's head: two wings of `head_length` pixels, `head_angle` degrees
    // apart, drawn from the tip.
    let wing_angle = (style.head_angle * std::f32::consts::PI / 180.) / 2.;
    let wing_scale = style.head_length / 1.5;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let (start_x, start_y) = (
                row.x1.clone().unwrap_or_else(|| row.x_value().clone()),
                row.y1.clone().unwrap_or_else(|| row.y_value().clone()),
            );
            let (end_x, end_y) = (
                row.x2.clone().unwrap_or_else(|| row.x_value().clone()),
                row.y2.clone().unwrap_or_else(|| row.y_value().clone()),
            );
            let (Some(from), Some(to)) = (
                ctx.frame.point(&start_x, &start_y),
                ctx.frame.point(&end_x, &end_y),
            ) else {
                continue;
            };
            let from = ctx.absolute(from);
            let to = ctx.absolute(to);
            let color = colors.stroke(index);
            let line = (to.x.as_f32() - from.x.as_f32()).hypot(to.y.as_f32() - from.y.as_f32());
            if line < 1. {
                continue;
            }

            let bend = bend
                * style.sweep.sign(
                    from.x.as_f32(),
                    from.y.as_f32(),
                    to.x.as_f32(),
                    to.y.as_f32(),
                );
            let points = if bend != 0. {
                arc_points(from, to, bend)
            } else {
                vec![from, to]
            };
            paint::paint_stroke(&points, width, color, ctx.window);
            // The head points along the path where it arrives, so a bent arrow
            // carries a head tangent to its arc.
            let last = &points[points.len() - 1];
            let previous = &points[points.len() - 2];
            let end_angle = (last.y.as_f32() - previous.y.as_f32())
                .atan2(last.x.as_f32() - previous.x.as_f32());

            // The wings are never longer than a third of the shaft, so a short
            // arrow keeps a proportional head.
            let head = (wing_scale * width).min(line / 3.);
            if head <= 0. {
                continue;
            }
            let tip = to;
            let wing = |side: f32| {
                let angle = end_angle + side * wing_angle;
                point(
                    px(tip.x.as_f32() - head * angle.cos()),
                    px(tip.y.as_f32() - head * angle.sin()),
                )
            };
            // One path through the tip, so the two wings meet there rather
            // than joining each other across the head.
            let wings = [wing(-1.), tip, wing(1.)];
            paint::paint_stroke(&wings, width, color, ctx.window);
        }
    });
}

/// The points of the circular arc from `from` to `to` that leaves `from` at
/// `bend` degrees to the chord.
///
/// Plot's arrow bends by sweeping a circle through both ends: the arc's radius
/// follows from the bend angle, and a positive angle curves clockwise on
/// screen.
fn arc_points(from: Point<Pixels>, to: Point<Pixels>, bend: f32) -> Vec<Point<Pixels>> {
    let (ax, ay) = (from.x.as_f32(), from.y.as_f32());
    let (bx, by) = (to.x.as_f32(), to.y.as_f32());
    let (dx, dy) = (bx - ax, by - ay);
    let chord = dx.hypot(dy);
    let bend_angle = bend.to_radians();
    // The radius of the circle whose tangent leaves `from` at `bend_angle` to
    // the chord: `hypot(chord / tan(bend_angle), chord) / 2`.
    let radius = (chord / bend_angle.tan()).hypot(chord) / 2.;
    // The circle's centre, on the side the bend curves towards.
    let sign = if bend_angle > 0. { 1. } else { -1. };
    let k = sign * (radius * radius - chord * chord / 4.).max(0.).sqrt() / chord;
    let centre = ((ax + bx) / 2. - dy * k, (ay + by) / 2. + dx * k);

    let start_angle = (ay - centre.1).atan2(ax - centre.0);
    let end_angle = (by - centre.1).atan2(bx - centre.0);
    // Sweep the short way round, in the direction the bend asks for.
    let mut delta = (end_angle - start_angle).rem_euclid(std::f32::consts::TAU);
    if sign < 0. {
        delta -= std::f32::consts::TAU;
    }

    let steps = 24;
    (0..=steps)
        .map(|step| {
            let angle = start_angle + delta * step as f32 / steps as f32;
            point(
                px(centre.0 + radius * angle.cos()),
                px(centre.1 + radius * angle.sin()),
            )
        })
        .collect()
}
