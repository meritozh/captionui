//! Plot's `tree`: a hierarchy's links and nodes, laid out tidily.

use gpui::{Bounds, PathBuilder, Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;
use crate::style::Symbol;

use super::{Colors, DrawContext, paint_symbol};

/// Draw a tree: one parent–child link per relation in the hierarchy, a dot per
/// node, laid out tidily across the plot.
///
/// The mark reads each row's `path` (a slash-separated chain), lays the paths
/// out into levels, and maps the tidy layout onto the plot's width and height.
/// Plot's `tree` is the same, though it defaults to a fixed aspect; here the
/// tree fills the plot, padded so its deepest leaves and longest labels have
/// room.
pub(super) fn draw_tree(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    // Parse each row's path into components. A missing path draws nothing.
    let paths: Vec<Vec<&str>> = prepared
        .rows
        .iter()
        .filter_map(|row| {
            let text = row.path.as_deref()?;
            let parts: Vec<&str> = text.split('/').filter(|s| !s.is_empty()).collect();
            (!parts.is_empty()).then_some(parts)
        })
        .collect();
    if paths.is_empty() {
        return;
    }
    let tree = crate::tree::tidy(&paths);

    let colors = Colors::resolve(prepared, ctx);
    let plot = ctx.frame.plot;
    let radius = prepared.mark.style.radius;

    ctx.clipped(|ctx| {
        // Map the tidy layout onto the plot, leaving a margin for the labels.
        // The layout is in plot-local pixels, like every other mark's
        // positions: the links are lifted with `ctx.absolute` and
        // `paint_symbol` lifts the nodes, so both land on the same points.
        let margin = px(20.);
        let inner = Bounds::new(
            point(margin, margin),
            gpui::size(
                px((plot.size.width.as_f32() - margin.as_f32() * 2.).max(1.)),
                px((plot.size.height.as_f32() - margin.as_f32() * 2.).max(1.)),
            ),
        );
        let node_at = |x: f64, y: f64| -> Point<Pixels> {
            // x: leaf slots → perpendicular gap; y: level → downward gap.
            let fx = (x / tree.width).clamp(0., 1.);
            let fy = (y / tree.height).clamp(0., 1.);
            point(
                px(inner.origin.x.as_f32() + inner.size.width.as_f32() * fx as f32),
                px(inner.origin.y.as_f32() + inner.size.height.as_f32() * fy as f32),
            )
        };

        // Draw each link as a quadratic spline, the way Plot's `tree` links
        // curve from parent to child.
        let link_style = prepared.mark.style.stroke_width.max(1.);
        for (index, &(child, parent)) in tree.links.iter().enumerate() {
            let from = ctx.absolute(node_at(tree.nodes[parent].x, tree.nodes[parent].y));
            let to = ctx.absolute(node_at(tree.nodes[child].x, tree.nodes[child].y));
            let mut builder = PathBuilder::stroke(px(link_style));
            builder.move_to(from);
            builder.curve_to(
                to,
                point(px((from.x.as_f32() + to.x.as_f32()) / 2.), from.y),
            );
            let color = colors.stroke(index.min(tree.links.len().saturating_sub(1)));
            paint::paint_path(builder.build().ok(), color, ctx.window);
        }

        // A filled dot per node, in the mark's default fill.
        let fill = paint::alpha(
            colors.fill(0),
            ctx.row_fill_opacity(prepared, &prepared.rows[0]),
        );
        for node in &tree.nodes {
            let at = node_at(node.x, node.y);
            paint_symbol(
                ctx,
                at,
                radius.max(2.),
                Symbol::Circle,
                fill,
                ctx.theme.axis,
                1.,
            );
        }
    });
}
