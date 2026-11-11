//! Plot's `areaY` and `areaX`: the band between a baseline and a series.

use gpui::{Pixels, Point};

use crate::paint;
use crate::render::{PreparedMark, PreparedRow};
use crate::value::Value;

use super::{Colors, DrawContext, curve_path, series_groups};

pub(super) fn draw_areas_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let groups = series_groups(prepared);
    let curve = prepared.mark.style.curve;

    ctx.clipped(|ctx| {
        for group in groups.iter() {
            // Plot's area is undefined where a given `y1` or `y2` is
            // missing, rather than falling back to zero.
            let channels = &prepared.mark.channels;
            let rows: Vec<&PreparedRow> = group
                .iter()
                .map(|(_, row)| *row)
                .filter(|row| {
                    (channels.y1.is_none() || row.y1.is_some())
                        && (channels.y2.is_none() || row.y2.is_some())
                })
                .collect();
            let top: Vec<Option<Point<Pixels>>> = rows
                .iter()
                .map(|row| {
                    let y = row.y2.clone().or_else(|| Some(row.y_value().clone()))?;
                    ctx.frame.point(row.x_value(), &y)
                })
                .collect();
            let bottom: Vec<Option<Point<Pixels>>> = rows
                .iter()
                .map(|row| {
                    let y = row.y1.clone().unwrap_or(Value::Number(0.));
                    ctx.frame.point(row.x_value(), &y)
                })
                .collect();

            let mut upper: Vec<Point<Pixels>> =
                top.into_iter().flatten().map(|p| ctx.absolute(p)).collect();
            let mut lower: Vec<Point<Pixels>> = bottom
                .into_iter()
                .flatten()
                .map(|p| ctx.absolute(p))
                .collect();
            if upper.len() < 2 || lower.len() < 2 {
                continue;
            }
            // The area's outline: the top edge forward, the bottom edge back.
            lower.reverse();
            upper.extend(lower);

            let color = colors
                .fill
                .as_ref()
                .and_then(|colors| group.first().map(|(index, _)| colors[*index]))
                .unwrap_or_else(|| ctx.default_fill(prepared));
            let fill = paint::alpha(
                color,
                prepared.mark.style.opacity * prepared.mark.style.fill_opacity * ctx.appear,
            );
            match paint::fill_path(&upper, fill) {
                Some(polygon) => paint::paint_path(Some(polygon), fill, ctx.window),
                // Plot draws an area in input order, so unsorted data folds
                // the outline over itself until the tessellator gives up;
                // fill it by scanlines instead, under SVG's nonzero rule.
                None => paint::paint_fill_nonzero(&upper, fill, ctx.window),
            }

            // Plot draws the area's edge too when a stroke is set.
            if prepared.mark.channels.stroke.is_some() {
                let top_edge: Vec<Point<Pixels>> = group
                    .iter()
                    .filter_map(|(_, row)| {
                        let y = row.y2.clone().or_else(|| Some(row.y_value().clone()))?;
                        ctx.frame.point(row.x_value(), &y).map(|p| ctx.absolute(p))
                    })
                    .collect();
                let path = curve_path(&top_edge, curve, prepared.mark.style.stroke_width, 1.);
                paint::paint_path(path, colors.stroke(0), ctx.window);
            }
        }
    });
}

pub(super) fn draw_areas_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let groups = series_groups(prepared);
    let curve = prepared.mark.style.curve;

    ctx.clipped(|ctx| {
        for group in groups.iter() {
            let mut right: Vec<Point<Pixels>> = group
                .iter()
                .filter_map(|(_, row)| {
                    let x = row.x2.clone().or_else(|| Some(row.x_value().clone()))?;
                    ctx.frame.point(&x, row.y_value())
                })
                .map(|p| ctx.absolute(p))
                .collect();
            let mut left: Vec<Point<Pixels>> = group
                .iter()
                .filter_map(|(_, row)| {
                    let x = row.x1.clone().unwrap_or(Value::Number(0.));
                    ctx.frame.point(&x, row.y_value())
                })
                .map(|p| ctx.absolute(p))
                .collect();
            if right.len() < 2 || left.len() < 2 {
                continue;
            }
            left.reverse();
            right.extend(left);
            let color = colors
                .fill
                .as_ref()
                .and_then(|colors| group.first().map(|(index, _)| colors[*index]))
                .unwrap_or_else(|| ctx.default_fill(prepared));
            let polygon =
                paint::fill_path(&right, paint::alpha(color, prepared.mark.style.opacity));
            paint::paint_path(polygon, color, ctx.window);
            if prepared.mark.channels.stroke.is_some() {
                let edge: Vec<Point<Pixels>> = group
                    .iter()
                    .filter_map(|(_, row)| {
                        let x = row.x2.clone().or_else(|| Some(row.x_value().clone()))?;
                        ctx.frame.point(&x, row.y_value()).map(|p| ctx.absolute(p))
                    })
                    .collect();
                let path = curve_path(&edge, curve, prepared.mark.style.stroke_width, 1.);
                paint::paint_path(path, colors.stroke(0), ctx.window);
            }
        }
    });
}
