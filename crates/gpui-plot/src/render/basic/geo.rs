//! Plot's `geo`: geometries and lon/lat points drawn through a projection.

use gpui::{Bounds, PathBuilder, Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;
use crate::value::Value;

use super::{Colors, DrawContext};

/// Draw a geo mark: project each row's lon/lat into the plot and connect them
/// as a path — Plot's `geo` over a sequence of points, where the plot's own
/// scales don't need to know about geography.
/// Plot's `geo`: each row's geometry drawn through the plot's projection,
/// stroked in `currentColor` and unfilled unless given a fill.
fn draw_geometries(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let frame = ctx.frame;
    let fitted = frame.projection.clone().unwrap_or_else(|| {
        // Without a projection, Plot reads coordinates through the x and y
        // scales; an equirectangular fit to the data stands in for them.
        let mut all = Vec::new();
        for row in &prepared.rows {
            if let Some(g) = row.row.get("geometry").and_then(Value::as_geo) {
                all.push(g.clone());
            }
        }
        crate::geo::GeoProjection::named("equirectangular")
            .expect("named")
            .domain(crate::geo::Geometry::Collection(all))
            .fit(
                frame.plot.size.width.as_f32() as f64,
                frame.plot.size.height.as_f32() as f64,
            )
    });
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let channel = prepared.mark.channels.path.clone();
    // Plot clips a projection to the frame.
    let clip = ctx.clip;
    ctx.clip = clip || ctx.frame.projection.is_some();
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let value = match &channel {
                Some(channel) => channel.read(&row.row),
                None => row.row.value("geometry"),
            };
            let Some(geometry) = value.as_geo() else {
                continue;
            };
            let fill = prepared
                .mark
                .channels
                .fill
                .as_ref()
                .map(|_| paint::alpha(colors.fill(index), ctx.row_fill_opacity(prepared, row)));
            let stroke = match (&prepared.mark.channels.stroke, &prepared.mark.channels.fill) {
                (Some(_), _) | (None, None) => Some(paint::alpha(
                    if prepared.mark.channels.stroke.is_some() {
                        colors.stroke(index)
                    } else {
                        ctx.colors.default_stroke(prepared, ctx.mark_index)
                    },
                    ctx.row_stroke_opacity(prepared, row),
                )),
                _ => None,
            };
            let to_points = |path: &[[f64; 2]], ctx: &DrawContext<'_>| -> Vec<Point<Pixels>> {
                path.iter()
                    .map(|[x, y]| ctx.absolute(point(px(*x as f32), px(*y as f32))))
                    .collect()
            };
            // Point geometries draw as circles of the dot's radius.
            let mut points = Vec::new();
            collect_points(geometry, &mut points);
            let r = row.r.unwrap_or(style.radius.max(3.));
            for p in points {
                if let Some([x, y]) = fitted.project(p[0], p[1]) {
                    let centre = ctx.absolute(point(px(x as f32), px(y as f32)));
                    let circle = paint::circle_points(centre, r, 20);
                    match fill {
                        Some(fill) => paint::paint_fill(&circle, fill, ctx.window),
                        None => {
                            let mut ring = circle.clone();
                            ring.push(circle[0]);
                            if let Some(stroke) = stroke {
                                paint::paint_stroke(&ring, style.stroke_width, stroke, ctx.window);
                            }
                        }
                    }
                }
            }
            let paths = fitted.paths(geometry);
            if let Some(fill) = fill {
                // A polygon's rings fill together, so holes stay open.
                let rings: Vec<Vec<Point<Pixels>>> = paths
                    .iter()
                    .filter(|(_, closed)| *closed)
                    .map(|(path, _)| to_points(path, ctx))
                    .collect();
                paint::paint_fill_rings(&rings, fill, ctx.window);
            }
            if let Some(stroke) = stroke {
                for (path, closed) in &paths {
                    let mut line = to_points(path, ctx);
                    if *closed && let Some(first) = line.first().copied() {
                        line.push(first);
                    }
                    paint::paint_stroke(&line, style.stroke_width, stroke, ctx.window);
                }
            }
        }
    });
    ctx.clip = clip;
}

fn collect_points(geometry: &crate::geo::Geometry, out: &mut Vec<[f64; 2]>) {
    use crate::geo::Geometry;
    match geometry {
        Geometry::Point(p) => out.push(*p),
        Geometry::MultiPoint(ps) => out.extend(ps),
        Geometry::Collection(gs) => gs.iter().for_each(|g| collect_points(g, out)),
        _ => {}
    }
}

pub(super) fn draw_geo(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    if prepared.rows.iter().any(|row| {
        row.row
            .get("geometry")
            .is_some_and(|v| v.as_geo().is_some())
    }) || prepared.mark.channels.path.is_some()
    {
        return draw_geometries(prepared, ctx);
    }
    let projection = prepared.mark.projection.clone().unwrap_or_default();
    let plot = ctx.frame.plot;

    // Project every row's (lon, lat) onto the plot, in row order.
    let points: Vec<Option<Point<Pixels>>> = prepared
        .rows
        .iter()
        .map(|row| {
            let (Some(lon), Some(lat)) = (row.x_value().as_number(), row.y_value().as_number())
            else {
                return None;
            };
            Some(project_point(plot, &projection, lon, lat))
        })
        .collect();

    let colors = Colors::resolve(prepared, ctx);
    ctx.clipped(|ctx| {
        // Fill any GeoJSON-like rings: each ring's (lon, lat) projected and
        // traced as a closed polygon, Plot's way of drawing geographic shapes.
        if let Some(rings) = &prepared.mark.geo_geometry {
            for ring in rings {
                if ring.len() < 3 {
                    continue;
                }
                let polygon: Vec<Point<Pixels>> = ring
                    .iter()
                    .map(|(lon, lat)| project_point(plot, &projection, *lon, *lat))
                    .collect();
                paint::paint_fill(&polygon, colors.fill(0), ctx.window);
                // A stroke around the shape reads as a coastline.
                let mut outline = PathBuilder::stroke(px(1.));
                outline.move_to(polygon[0]);
                for p in &polygon[1..] {
                    outline.line_to(*p);
                }
                outline.close();
                paint::paint_path(outline.build().ok(), colors.stroke(0), ctx.window);
            }
        }
        // A rounded path joining the projected points, in row order.
        let projected: Vec<Point<Pixels>> = points.iter().flatten().cloned().collect();
        if projected.len() >= 2 {
            let mut builder = PathBuilder::stroke(px(prepared.mark.style.stroke_width.max(1.)));
            builder.move_to(projected[0]);
            for p in &projected[1..] {
                builder.line_to(*p);
            }
            paint::paint_path(builder.build().ok(), colors.stroke(0), ctx.window);
        }
        // A dot at each projected point.
        for (index, p) in points.iter().flatten().enumerate() {
            let _ = index;
            paint::paint_fill(
                &crate::paint::circle_points(*p, prepared.mark.style.radius.max(2.), 16),
                colors.fill(index),
                ctx.window,
            );
        }
    });
}

/// An absolute window point for a degree (lon, lat), projected through `p`
/// onto the plot's bounds. The frame's plot bounds are already in window
/// coordinates, so adding the projected fraction places it in window space.
fn project_point(
    plot: Bounds<Pixels>,
    p: &crate::projection::Projection,
    lon: f64,
    lat: f64,
) -> Point<Pixels> {
    let [fx, fy] = p.project(lon, lat);
    point(
        plot.origin.x + px(plot.size.width.as_f32() * fx as f32),
        plot.origin.y + px(plot.size.height.as_f32() * fy as f32),
    )
}
