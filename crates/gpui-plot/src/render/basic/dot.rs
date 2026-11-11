//! Plot's `dot`, and the symbols it paints.

use gpui::{Hsla, Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;
use crate::style::Symbol;

use super::{Colors, DrawContext, position};

pub(super) fn draw_dots(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let radius = prepared.mark.style.radius;
    // Plot draws dots with an `r` channel largest first, so small ones sit
    // on top and stay visible.
    let mut order: Vec<usize> = (0..prepared.rows.len()).collect();
    if prepared.rows.iter().any(|row| row.r_value.is_some()) {
        order.sort_by(|a, b| {
            let r = |i: &usize| prepared.rows[*i].r.unwrap_or(0.);
            r(b).total_cmp(&r(a))
        });
    }

    ctx.clipped(|ctx| {
        for index in order {
            let row = &prepared.rows[index];
            let Some(position) = position(prepared, ctx, row) else {
                continue;
            };
            // Plot's dot defaults to a radius of 3, or 4.5 with a symbol
            // channel, so its shapes read.
            let radius = if prepared.mark.channels.symbol.is_some() && radius == 3. {
                4.5
            } else {
                radius
            };
            let r = row.r.unwrap_or(radius) * ctx.appear;
            if r <= 0. {
                continue;
            }
            let symbol = row.symbol.unwrap_or(prepared.mark.style.symbol);
            let emphasis = ctx.emphasis(index);
            // The hovered dot grows toward 1.6× its radius; the rest keep
            // theirs, so the hover reads as a lift rather than a redraw.
            let r = r * (1. + 0.6 * emphasis);
            // Plot's dot is hollow unless it is given a fill: it strokes in
            // `currentColor` (or its stroke channel) and fills nothing. A
            // constant fill symbol (`symbol: "square"`) fills instead.
            let fill_symbol = !symbol.is_line()
                && symbol != Symbol::Circle
                && prepared
                    .mark
                    .channels
                    .symbol
                    .as_ref()
                    .is_none_or(|channel| crate::render::constant_symbol(channel).is_some())
                && prepared.mark.channels.stroke.is_none();
            let hollow = prepared.mark.channels.fill.is_none() && !fill_symbol;
            let fill = if hollow {
                Hsla::transparent_black()
            } else {
                paint::alpha(colors.fill(index), ctx.row_fill_opacity(prepared, row))
            };
            // A filled dot with no stroke of its own draws no outline.
            let stroke = if prepared.mark.channels.stroke.is_none() && !hollow {
                Hsla::transparent_black()
            } else {
                paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row))
            };
            paint_symbol(
                ctx,
                position,
                r,
                symbol,
                fill,
                stroke,
                prepared.mark.style.stroke_width,
            );
        }
    });
}

/// Paint one symbol at a plot-relative position: d3's symbol of area `π r²`,
/// filled and stroked as the dot asks. A stroke symbol (`plus`, `times` …)
/// is drawn in the stroke alone.
pub fn paint_symbol(
    ctx: &mut DrawContext<'_>,
    position: Point<Pixels>,
    radius: f32,
    symbol: Symbol,
    fill: Hsla,
    stroke: Hsla,
    stroke_width: f32,
) {
    let absolute = ctx.absolute(position);
    // A circle is a fully rounded quad: one instance on the GPU, where a
    // tessellated path costs far more. Plot's scatterplots draw thousands
    // of dots, which overflow the path pipeline's buffer. SVG centers a
    // stroke on the outline, so the quad grows by half the stroke and draws
    // it as an inside border.
    if symbol == Symbol::Circle {
        let stroke_width = if stroke.a > 0. {
            stroke_width.max(0.)
        } else {
            0.
        };
        let outer = radius + stroke_width / 2.;
        let bounds = gpui::Bounds::new(
            point(absolute.x - px(outer), absolute.y - px(outer)),
            gpui::size(px(outer * 2.), px(outer * 2.)),
        );
        ctx.window.paint_quad(gpui::quad(
            bounds,
            gpui::Corners::all(px(outer)),
            fill,
            gpui::Edges::all(px(stroke_width)),
            stroke,
            gpui::BorderStyle::default(),
        ));
        return;
    }
    for (points, closed) in symbol.subpaths(absolute, radius) {
        if closed && fill.a > 0. {
            paint::paint_fill(&points, fill, ctx.window);
        }
        if stroke.a > 0. && stroke_width > 0. {
            let mut outline = points.clone();
            if closed && let Some(first) = points.first() {
                outline.push(*first);
            }
            paint::paint_stroke(&outline, stroke_width, stroke, ctx.window);
        }
    }
}
