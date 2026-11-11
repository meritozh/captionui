//! Plot's `line`: a path per series, broken where a value is missing.

use gpui::{Hsla, Pixels, Point};

use crate::paint;
use crate::render::PreparedMark;

use super::{Colors, DrawContext, curve_path, paint_markers, position, series_groups};

pub(super) fn draw_lines(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let width = style.stroke_width;
    let curve = style.curve;
    let groups = series_groups(prepared);

    ctx.clipped(|ctx| {
        for group in groups.iter() {
            // Plot breaks a line where a value is missing, so each run of
            // defined points is its own path.
            let mut runs: Vec<Vec<(usize, Point<Pixels>)>> = vec![Vec::new()];
            for (index, row) in group {
                // A scaled stroke without a value is undefined, and Plot
                // drops the point as it drops a missing position.
                let stroke_defined = match &prepared.mark.channels.stroke {
                    Some(
                        channel @ (crate::style::ColorChannel::Field(_)
                        | crate::style::ColorChannel::Value(_)),
                    ) => channel
                        .scaled_value(&row.row)
                        .is_some_and(|value| !value.is_null()),
                    _ => true,
                };
                let defined =
                    !row.x_value().is_null() && !row.y_value().is_null() && stroke_defined;
                match defined.then(|| position(prepared, ctx, row)).flatten() {
                    Some(p) => runs
                        .last_mut()
                        .expect("non-empty")
                        .push((*index, ctx.absolute(p))),
                    None => {
                        if !runs.last().expect("non-empty").is_empty() {
                            runs.push(Vec::new());
                        }
                    }
                }
            }
            for run in runs.iter().filter(|run| !run.is_empty()) {
                let color_of = |index: usize| {
                    let row = &prepared.rows[index];
                    let color = colors
                        .stroke
                        .as_ref()
                        .and_then(|colors| colors.get(index).copied())
                        .unwrap_or_else(|| ctx.series_color(prepared, row));
                    paint::alpha(color, ctx.row_stroke_opacity(prepared, row))
                };
                let (first, _) = run[0];
                let color = color_of(first);
                // Plot's `groupIndex`: a series whose stroke varies splits
                // into segments where it changes, each running on to the
                // first point of the next and painted in its own first
                // point's color.
                let mut segments: Vec<(Hsla, Vec<Point<Pixels>>)> = vec![(color, Vec::new())];
                for (index, p) in run {
                    let here = color_of(*index);
                    let (current, points) = segments.last_mut().expect("non-empty");
                    points.push(*p);
                    if here != *current {
                        segments.push((here, vec![*p]));
                    }
                }
                for (color, points) in &segments {
                    if points.len() < 2 {
                        continue;
                    }
                    if style.dash.is_empty() {
                        let path = curve_path(points, curve, width, ctx.appear);
                        paint::paint_path(path, *color, ctx.window);
                    } else {
                        paint::paint_dashed_path(points, &style.dash, width, *color, ctx.window);
                    }
                }
                let points: Vec<Point<Pixels>> = run.iter().map(|(_, p)| *p).collect();
                paint_markers(
                    &points,
                    &style.markers,
                    color,
                    width,
                    ctx.theme.background,
                    ctx.window,
                );
            }
        }
    });
}
