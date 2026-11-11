//! Plot's `hexgrid`: the hexagon lattice under a hexbin chart.

use gpui::px;

use crate::paint;
use crate::render::PreparedMark;

use super::DrawContext;

/// Plot's `hexgrid`: a static lattice of hexagons under the data, so a hexbin
/// chart's bins sit on a visible grid. The mark reads no data.
pub(super) fn draw_hexgrid(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let style = &prepared.mark.style;
    let plot = ctx.frame.plot;
    let color = style.stroke;
    // A pointy-top hexagon: `binWidth` is its horizontal width, twice its
    // circumradius.
    let rx = style.bin_width.max(1.) / 2.;
    let ry = rx * (2. / 3f32.sqrt());
    let wx = rx * 2.;
    let wy = ry * 1.5;
    // The lattice's origin, half a pixel across: the same offset `hexbin` bins
    // on, so the two marks' hexagons coincide.
    let (ox, oy) = (0.5_f32, 0.0_f32);
    let origin = ctx.absolute(gpui::point(px(0.), px(0.)));
    let width = plot.size.width.as_f32();
    let height = plot.size.height.as_f32();
    let columns = (width / wx).ceil() as i32 + 2;
    let rows = (height / wy).ceil() as i32 + 2;

    ctx.clipped(|ctx| {
        for row in -1..rows {
            for column in -1..columns {
                let cx = column as f32 * wx + (row & 1) as f32 * rx + ox;
                let cy = row as f32 * wy + oy;
                // Plot strokes the hexagon's top, right and bottom edges, not
                // the left: the neighbour to the left draws that edge, so every
                // edge is painted exactly once and the lattice reads evenly.
                let points = [
                    gpui::point(origin.x + px(cx), origin.y + px(cy - ry)),
                    gpui::point(origin.x + px(cx + rx), origin.y + px(cy - ry / 2.)),
                    gpui::point(origin.x + px(cx + rx), origin.y + px(cy + ry / 2.)),
                    gpui::point(origin.x + px(cx), origin.y + px(cy + ry)),
                ];
                paint::paint_stroke(
                    &points,
                    style.stroke_width,
                    paint::alpha(color, style.stroke_opacity),
                    ctx.window,
                );
            }
        }
    });
}
