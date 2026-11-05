//! The Delaunay marks: `delaunayLink`, `delaunayMesh`, `hull`, `voronoi` and
//! `voronoiMesh`.
//!
//! Plot builds these marks' geometry from the data's *projected* points, so the
//! shapes follow the scales: a Voronoi cell changes shape when an axis's domain
//! changes, because the pixels the points sit at move. Every shape is clipped
//! to the plot, which is also where a Voronoi cell is bounded.

use gpui::{PathBuilder, Pixels, Point, point, px};

use crate::delaunay::{self, Point2};
use crate::mark::MarkKind;
use crate::paint;
use crate::render::PreparedMark;
use crate::render::basic::{Colors, DrawContext};

/// Paint one of the Delaunay marks.
pub fn draw(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    match prepared.mark.kind {
        MarkKind::DelaunayLink => draw_links(prepared, ctx),
        MarkKind::DelaunayMesh => draw_mesh(prepared, ctx),
        MarkKind::Hull => draw_hull(prepared, ctx),
        MarkKind::Voronoi => draw_cells(prepared, ctx),
        MarkKind::VoronoiMesh => draw_voronoi_mesh(prepared, ctx),
        _ => {}
    }
}

/// The projected points of a mark's rows, with the row index each came from.
///
/// A row whose channels do not scale draws nothing, the way Plot drops an
/// undefined point before triangulating.
fn project(prepared: &PreparedMark, ctx: &DrawContext<'_>) -> Vec<(usize, Point2)> {
    prepared
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let placed = ctx.frame.point(row.x_value(), row.y_value())?;
            Some((index, [placed.x.as_f32() as f64, placed.y.as_f32() as f64]))
        })
        .collect()
}

/// A projected point as a figure pixel, ready to paint.
fn absolute(ctx: &DrawContext<'_>, point2: Point2) -> Point<Pixels> {
    ctx.absolute(point(px(point2[0] as f32), px(point2[1] as f32)))
}

/// The plot's bounds in plot-relative pixels, which is what a Voronoi cell is
/// clipped to and what `voronoiMesh` meshes within.
fn plot_bounds(ctx: &DrawContext<'_>) -> [f64; 4] {
    let plot = ctx.frame.plot;
    [
        0.,
        0.,
        plot.size.width.as_f32() as f64,
        plot.size.height.as_f32() as f64,
    ]
}

/// Draw the triangulation's edges one at a time, Plot's `delaunayLink`.
///
/// Each link keeps the style of the row it starts from, so a `stroke` or
/// `opacity` channel colors the links by their data.
fn draw_links(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let points = project(prepared, ctx);
    let coords: Vec<Point2> = points.iter().map(|(_, point)| *point).collect();
    let triangulation = delaunay::Triangulation::new(&coords);

    // `indices` maps each projected point to the distinct vertex it became;
    // invert it so a vertex finds the first projected point that supplied it,
    // and that point's row (a row that failed to project has no point, so the
    // projected index is not the row index).
    let mut vertex_row = vec![usize::MAX; triangulation.points.len()];
    for (projected, &vertex) in triangulation.indices.iter().enumerate() {
        if vertex_row[vertex] == usize::MAX {
            vertex_row[vertex] = points[projected].0;
        }
    }

    ctx.clipped(|ctx| {
        for (a, b) in triangulation.edges() {
            // The link's style comes from the row it starts at.
            let Some(&row) = vertex_row.get(a).filter(|row| **row != usize::MAX) else {
                continue;
            };
            let from = absolute(ctx, triangulation.points[a]);
            let to = absolute(ctx, triangulation.points[b]);
            let color = paint::alpha(
                colors.stroke(row),
                ctx.row_stroke_opacity(prepared, &prepared.rows[row]) * ctx.appear,
            );
            let mut builder = PathBuilder::stroke(px(width));
            builder.move_to(from);
            builder.line_to(to);
            paint::paint_path(builder.build().ok(), color, ctx.window);
        }
    });
}

/// Draw the triangulation as one path, Plot's `delaunayMesh`.
///
/// Every triangle's three edges go into one path, so the mesh strokes once
/// rather than once per edge and its overlaps do not darken.
fn draw_mesh(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let points = project(prepared, ctx);
    let coords: Vec<Point2> = points.iter().map(|(_, point)| *point).collect();
    let triangulation = delaunay::Triangulation::new(&coords);
    if triangulation.triangles.is_empty() {
        return;
    }
    let color = paint::alpha(
        colors.stroke(0),
        ctx.row_stroke_opacity(prepared, &prepared.rows[0]) * ctx.appear,
    );
    let vertices = &triangulation.points;

    ctx.clipped(|ctx| {
        let mut builder = PathBuilder::stroke(px(width));
        for triangle in &triangulation.triangles {
            for k in 0..3 {
                let a = absolute(ctx, vertices[triangle[k]]);
                let b = absolute(ctx, vertices[triangle[(k + 1) % 3]]);
                builder.move_to(a);
                builder.line_to(b);
            }
        }
        paint::paint_path(builder.build().ok(), color, ctx.window);
    });
}

/// Draw the convex hull as one closed outline, Plot's `hull`.
///
/// The hull is an outline, not an area: Plot's default fill is `none`, so it
/// only fills when the mark gives it a `fill` (directly or through its series).
fn draw_hull(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let points = project(prepared, ctx);
    let coords: Vec<Point2> = points.iter().map(|(_, point)| *point).collect();
    let hull = delaunay::convex_hull(&coords);
    if hull.len() < 3 {
        return;
    }
    let stroke = paint::alpha(
        colors.stroke(0),
        ctx.row_stroke_opacity(prepared, &prepared.rows[0]) * ctx.appear,
    );

    ctx.clipped(|ctx| {
        let outline: Vec<Point<Pixels>> = hull
            .iter()
            .map(|index| absolute(ctx, coords[*index]))
            .collect();
        if prepared.mark.channels.fill.is_some() {
            let fill = paint::alpha(
                colors.fill(0),
                ctx.row_fill_opacity(prepared, &prepared.rows[0]) * ctx.appear,
            );
            paint::paint_fill(&outline, fill, ctx.window);
        }
        paint::paint_stroke(&outline, width, stroke, ctx.window);
    });
}

/// Draw one Voronoi cell per datum, Plot's `voronoi`.
///
/// A cell keeps its own row's style, so a `fill` channel shades each point's
/// region by its data. Like Plot, an unshaded `voronoi` is outlines only: the
/// mark fills nothing unless it is given a fill.
fn draw_cells(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let points = project(prepared, ctx);
    let coords: Vec<Point2> = points.iter().map(|(_, point)| *point).collect();
    let cells = delaunay::voronoi_cells(&coords, plot_bounds(ctx));

    ctx.clipped(|ctx| {
        for (site, cell) in cells.iter().enumerate() {
            if cell.len() < 3 {
                continue;
            }
            let Some((index, _)) = points.get(site) else {
                continue;
            };
            let outline: Vec<Point<Pixels>> =
                cell.iter().map(|point2| absolute(ctx, *point2)).collect();
            if prepared.mark.channels.fill.is_some() {
                let fill = paint::alpha(
                    colors.fill(*index),
                    ctx.row_fill_opacity(prepared, &prepared.rows[*index]) * ctx.appear,
                );
                paint::paint_fill(&outline, fill, ctx.window);
            }
            if prepared.mark.channels.stroke.is_some() {
                let stroke = paint::alpha(
                    colors.stroke(*index),
                    ctx.row_stroke_opacity(prepared, &prepared.rows[*index]) * ctx.appear,
                );
                paint::paint_stroke(&outline, width, stroke, ctx.window);
            }
        }
    });
}

/// Draw the Voronoi lattice once, Plot's `voronoiMesh`.
///
/// The mesh is the cells' shared edges: an interior edge belongs to two
/// cells, so drawing every cell's outline would stroke it twice.
fn draw_voronoi_mesh(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let points = project(prepared, ctx);
    let coords: Vec<Point2> = points.iter().map(|(_, point)| *point).collect();
    let cells = delaunay::voronoi_cells(&coords, plot_bounds(ctx));
    let edges = delaunay::voronoi_edges(&cells);
    if edges.is_empty() {
        return;
    }
    let color = paint::alpha(
        colors.stroke(0),
        ctx.row_stroke_opacity(prepared, &prepared.rows[0]) * ctx.appear,
    );

    ctx.clipped(|ctx| {
        let mut builder = PathBuilder::stroke(px(width));
        for (a, b) in edges {
            builder.move_to(absolute(ctx, a));
            builder.line_to(absolute(ctx, b));
        }
        paint::paint_path(builder.build().ok(), color, ctx.window);
    });
}
