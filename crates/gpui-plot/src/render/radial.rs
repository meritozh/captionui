//! Radial marks: pie, arc, waffle, plus the box, violin and density summaries.
//!
//! A radial mark places its rows on an arc rather than on the x and y axes; its
//! domain is the sum of its values, not the frame's extents.

use gpui::{Point, point, px};

use crate::mark::MarkKind;
use crate::paint;
use crate::render::PreparedMark;
use crate::render::basic::{Colors, DrawContext, series_groups};
use crate::value::Value;

/// Paint a radial or statistical mark.
pub fn draw(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    match prepared.mark.kind {
        MarkKind::Pie => draw_pie(prepared, ctx, 0., 1.),
        MarkKind::Arc => draw_arc(prepared, ctx),
        MarkKind::Waffle => draw_waffle(prepared, ctx),
        // `vertical` here means the box's quantitative dimension runs along
        // x, so the boxes are horizontal — Plot's `boxX`, whose `x` is the
        // value and whose `y` is the category.
        MarkKind::BoxX => draw_box(prepared, ctx, true),
        MarkKind::BoxY => draw_box(prepared, ctx, false),
        MarkKind::ViolinX => draw_violin(prepared, ctx, true),
        MarkKind::ViolinY => draw_violin(prepared, ctx, false),
        MarkKind::DensityX => draw_density(prepared, ctx, true),
        MarkKind::DensityY => draw_density(prepared, ctx, false),
        MarkKind::Hexbin => draw_hexbin(prepared, ctx),
        MarkKind::Contour => draw_contour(prepared, ctx),
        _ => {}
    }
}

fn center(ctx: &DrawContext<'_>) -> Point<gpui::Pixels> {
    let plot = ctx.frame.plot;
    ctx.absolute(gpui::point(
        px(plot.size.width.as_f32() / 2.),
        px(plot.size.height.as_f32() / 2.),
    ))
}

fn radius(ctx: &DrawContext<'_>) -> f32 {
    let plot = ctx.frame.plot;
    plot.size.width.as_f32().min(plot.size.height.as_f32()) / 2. - 8.
}

fn values(prepared: &PreparedMark) -> Vec<Value> {
    prepared
        .rows
        .iter()
        .map(|row| {
            // A pie reads its length from the value channel, which the bar
            // interval carries after resolution.
            row.y2.clone().unwrap_or_else(|| row.y_value().clone())
        })
        .collect()
}

fn draw_pie(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, inner: f32, outer: f32) {
    let colors = Colors::resolve(prepared, ctx);
    let values = values(prepared);
    let total: f64 = values.iter().filter_map(Value::as_number).sum();
    if total <= 0. {
        return;
    }

    let center = center(ctx);
    let radius = radius(ctx);
    let inner_radius = radius * inner;
    let outer_radius = radius * outer;
    let mut angle = 0.0_f32;

    ctx.clipped(|ctx| {
        for (index, value) in values.iter().enumerate() {
            let Some(share) = value.as_number() else {
                continue;
            };
            let span = (share / total) as f32 * std::f32::consts::TAU;
            if span <= 0. {
                continue;
            }
            let points = paint::arc_points(center, inner_radius, outer_radius, angle, angle + span);
            let color = colors.fill(index);
            paint::paint_fill(
                &points,
                paint::alpha(color, prepared.mark.style.opacity),
                ctx.window,
            );
            if prepared.mark.channels.stroke.is_some() {
                paint::paint_stroke(
                    &points,
                    prepared.mark.style.stroke_width,
                    colors.stroke(index),
                    ctx.window,
                );
            }
            angle += span;
        }
    });
}

fn draw_arc(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    // An arc mark takes its angles from `x1`/`x2`, in turns, and its radii from
    // the style: Plot's `Plot.arc({x1: 0, x2: 1})` draws a full ring.
    let colors = Colors::resolve(prepared, ctx);
    let center = center(ctx);
    let radius = radius(ctx);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let start = row.x1.as_ref().and_then(Value::as_number).unwrap_or(0.) as f32
                * std::f32::consts::TAU;
            let end = row.x2.as_ref().and_then(Value::as_number).unwrap_or(1.) as f32
                * std::f32::consts::TAU;
            let points = paint::arc_points(center, 0., radius, start, end);
            let color = colors.fill(index);
            paint::paint_fill(
                &points,
                paint::alpha(color, prepared.mark.style.opacity),
                ctx.window,
            );
        }
    });
}

fn draw_waffle(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let plot = ctx.frame.plot;
    let total: f64 = values(prepared).iter().filter_map(Value::as_number).sum();
    if total <= 0. {
        return;
    }

    let side = plot.size.width.as_f32().min(plot.size.height.as_f32());
    let origin = ctx.absolute(gpui::point(
        px((plot.size.width.as_f32() - side) / 2.),
        px((plot.size.height.as_f32() - side) / 2.),
    ));

    // Ten by ten cells, one per percent, Plot's default waffle.
    const GRID: usize = 10;
    let cell = side / GRID as f32;
    let gap = (cell * 0.08).max(0.5);
    let mut index = 0usize;

    ctx.clipped(|ctx| {
        for (row_index, value) in values(prepared).iter().enumerate() {
            let Some(share) = value.as_number() else {
                continue;
            };
            let cells = ((share / total) * (GRID * GRID) as f64).round() as usize;
            let color = colors.fill(row_index);
            for _ in 0..cells {
                if index >= GRID * GRID {
                    break;
                }
                let column = index % GRID;
                let line = index / GRID;
                let rect = paint::rect_between(
                    gpui::point(
                        px(origin.x.as_f32() + column as f32 * cell + gap / 2.),
                        px(origin.y.as_f32() + line as f32 * cell + gap / 2.),
                    ),
                    gpui::point(
                        px(origin.x.as_f32() + (column + 1) as f32 * cell - gap / 2.),
                        px(origin.y.as_f32() + (line + 1) as f32 * cell - gap / 2.),
                    ),
                );
                paint::paint_rect(
                    rect,
                    1.,
                    paint::alpha(color, prepared.mark.style.opacity),
                    ctx.window,
                );
                index += 1;
            }
        }
    });
}

/// A five-number summary of a category's values.
#[derive(Clone, Copy, Debug)]
pub struct BoxSummary {
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
}

impl BoxSummary {
    /// The summary of `values`, ignoring nulls; `None` when nothing is left.
    pub fn of(values: &[Value]) -> Option<Self> {
        let mut numbers: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
        if numbers.is_empty() {
            return None;
        }
        numbers.sort_by(f64::total_cmp);
        let quantile = |q: f64| -> f64 {
            // The same linear interpolation d3's `quantileSorted` uses.
            let position = (numbers.len() as f64 - 1.) * q;
            let lower = position.floor() as usize;
            let upper = position.ceil() as usize;
            let weight = position - lower as f64;
            numbers[lower] * (1. - weight) + numbers[upper] * weight
        };
        Some(Self {
            min: numbers[0],
            q1: quantile(0.25),
            median: quantile(0.5),
            q3: quantile(0.75),
            max: numbers[numbers.len() - 1],
        })
    }
}

fn draw_box(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, value_on_x: bool) {
    let colors = Colors::resolve(prepared, ctx);
    let width = box_width(ctx, value_on_x);
    let groups = crate::render::group_rows(&prepared.rows, &|row| {
        if value_on_x {
            row.y_value().clone()
        } else {
            row.x_value().clone()
        }
    });

    ctx.clipped(|ctx| {
        for (group_index, (category, members)) in groups.iter().enumerate() {
            let values: Vec<Value> = members
                .iter()
                .map(|row| {
                    if value_on_x {
                        row.x_value().clone()
                    } else {
                        row.y_value().clone()
                    }
                })
                .collect();
            let Some(summary) = BoxSummary::of(&values) else {
                continue;
            };
            let Some(anchor) = (if value_on_x {
                ctx.frame.y.scale.position(category)
            } else {
                ctx.frame.x.scale.position(category)
            }) else {
                continue;
            };
            let color = colors.fill(group_index);

            // Whiskers span the full extent; the box spans the quartiles.
            let (a0, a1, b0, b1) = if value_on_x {
                (
                    ctx.frame.x.scale.position(&Value::Number(summary.min)),
                    ctx.frame.x.scale.position(&Value::Number(summary.q1)),
                    ctx.frame.x.scale.position(&Value::Number(summary.q3)),
                    ctx.frame.x.scale.position(&Value::Number(summary.max)),
                )
            } else {
                (
                    ctx.frame.y.scale.position(&Value::Number(summary.min)),
                    ctx.frame.y.scale.position(&Value::Number(summary.q1)),
                    ctx.frame.y.scale.position(&Value::Number(summary.q3)),
                    ctx.frame.y.scale.position(&Value::Number(summary.max)),
                )
            };
            let (Some(a0), Some(a1), Some(b0), Some(b1)) = (a0, a1, b0, b1) else {
                continue;
            };

            let thin = width / 4.;
            if value_on_x {
                // The whisker runs from the box's ends to the extremes, not
                // through the box, where a line would read as a gridline.
                for (from, to) in [(a0, a1), (b0, b1)] {
                    paint::paint_stroke(
                        &[
                            ctx.absolute(gpui::point(px(from), px(anchor))),
                            ctx.absolute(gpui::point(px(to), px(anchor))),
                        ],
                        1.5,
                        color,
                        ctx.window,
                    );
                }
                let box_rect = paint::rect_between(
                    ctx.absolute(gpui::point(px(a1), px(anchor - width / 2.))),
                    ctx.absolute(gpui::point(px(b0), px(anchor + width / 2.))),
                );
                paint::paint_rect(box_rect, 0., paint::alpha(color, 0.35), ctx.window);
                paint::paint_rect_outline(box_rect, 0., 1.5, color, ctx.window);
                // The median line sits inside the box.
                if let Some(median) = ctx.frame.x.scale.position(&Value::Number(summary.median)) {
                    paint::paint_stroke(
                        &[
                            ctx.absolute(gpui::point(px(median), px(anchor - width / 2.))),
                            ctx.absolute(gpui::point(px(median), px(anchor + width / 2.))),
                        ],
                        2.,
                        color,
                        ctx.window,
                    );
                }
                // Whisker caps.
                for cap in [a0, b1] {
                    paint::paint_stroke(
                        &[
                            ctx.absolute(gpui::point(px(cap), px(anchor - thin / 2.))),
                            ctx.absolute(gpui::point(px(cap), px(anchor + thin / 2.))),
                        ],
                        1.5,
                        color,
                        ctx.window,
                    );
                }
            } else {
                for (from, to) in [(a0, a1), (b0, b1)] {
                    paint::paint_stroke(
                        &[
                            ctx.absolute(gpui::point(px(anchor), px(from))),
                            ctx.absolute(gpui::point(px(anchor), px(to))),
                        ],
                        1.5,
                        color,
                        ctx.window,
                    );
                }
                let box_rect = paint::rect_between(
                    ctx.absolute(gpui::point(px(anchor - width / 2.), px(a1))),
                    ctx.absolute(gpui::point(px(anchor + width / 2.), px(b0))),
                );
                paint::paint_rect(box_rect, 0., paint::alpha(color, 0.35), ctx.window);
                paint::paint_rect_outline(box_rect, 0., 1.5, color, ctx.window);
                if let Some(median) = ctx.frame.y.scale.position(&Value::Number(summary.median)) {
                    paint::paint_stroke(
                        &[
                            ctx.absolute(gpui::point(px(anchor - width / 2.), px(median))),
                            ctx.absolute(gpui::point(px(anchor + width / 2.), px(median))),
                        ],
                        2.,
                        color,
                        ctx.window,
                    );
                }
                for cap in [a0, b1] {
                    paint::paint_stroke(
                        &[
                            ctx.absolute(gpui::point(px(anchor - thin / 2.), px(cap))),
                            ctx.absolute(gpui::point(px(anchor + thin / 2.), px(cap))),
                        ],
                        1.5,
                        color,
                        ctx.window,
                    );
                }
            }
        }
    });
}

fn box_width(ctx: &DrawContext<'_>, value_on_x: bool) -> f32 {
    let scale = if value_on_x {
        &ctx.frame.y.scale
    } else {
        &ctx.frame.x.scale
    };
    match scale {
        crate::scale::PositionScale::Band(band) => (band.band_width() * 0.8).max(4.),
        crate::scale::PositionScale::Point(point) => (point.step() * 0.8).max(4.),
        crate::scale::PositionScale::Continuous(_) => 20.,
    }
}

fn draw_violin(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, value_on_x: bool) {
    let colors = Colors::resolve(prepared, ctx);
    let width = box_width(ctx, value_on_x) / 2.;
    let groups = crate::render::group_rows(&prepared.rows, &|row| {
        if value_on_x {
            row.y_value().clone()
        } else {
            row.x_value().clone()
        }
    });

    ctx.clipped(|ctx| {
        for (group_index, (category, members)) in groups.iter().enumerate() {
            let values: Vec<Value> = members
                .iter()
                .map(|row| {
                    if value_on_x {
                        row.x_value().clone()
                    } else {
                        row.y_value().clone()
                    }
                })
                .collect();
            let Some((density, domain)) = crate::density::density_of(&values, 40) else {
                continue;
            };
            let Some(anchor) = (if value_on_x {
                ctx.frame.y.scale.position(category)
            } else {
                ctx.frame.x.scale.position(category)
            }) else {
                continue;
            };
            let peak = density.iter().cloned().fold(0.0_f64, f64::max);
            if peak <= 0. {
                continue;
            }

            // The violin's outline: one side out, the mirrored side back.
            let mut points = Vec::with_capacity(density.len() * 2);
            for (index, value) in density.iter().enumerate() {
                let position =
                    domain[0] + (domain[1] - domain[0]) * index as f64 / (density.len() - 1) as f64;
                let offset = (value / peak) as f32 * width;
                let scale_position = if value_on_x {
                    ctx.frame.x.scale.position(&Value::Number(position))
                } else {
                    ctx.frame.y.scale.position(&Value::Number(position))
                };
                let Some(scale_position) = scale_position else {
                    continue;
                };
                if value_on_x {
                    points.push(ctx.absolute(gpui::point(px(scale_position), px(anchor - offset))));
                } else {
                    points.push(ctx.absolute(gpui::point(px(anchor + offset), px(scale_position))));
                }
            }
            for (index, value) in density.iter().enumerate().rev() {
                let position =
                    domain[0] + (domain[1] - domain[0]) * index as f64 / (density.len() - 1) as f64;
                let offset = (value / peak) as f32 * width;
                let scale_position = if value_on_x {
                    ctx.frame.x.scale.position(&Value::Number(position))
                } else {
                    ctx.frame.y.scale.position(&Value::Number(position))
                };
                let Some(scale_position) = scale_position else {
                    continue;
                };
                if value_on_x {
                    points.push(ctx.absolute(gpui::point(px(scale_position), px(anchor + offset))));
                } else {
                    points.push(ctx.absolute(gpui::point(px(anchor - offset), px(scale_position))));
                }
            }
            let color = colors.fill(group_index);
            paint::paint_fill(&points, paint::alpha(color, 0.5), ctx.window);
            paint::paint_stroke(&points, 1., color, ctx.window);
        }
    });
}

/// A density estimate along one axis.
///
/// `along_x` is true for `densityX`, whose quantitative channel is `x`; the
/// estimate rises from the plot's bottom edge. `densityY` reads `y` and rises
/// from the left edge.
fn draw_density(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, along_x: bool) {
    let colors = Colors::resolve(prepared, ctx);
    let groups = series_groups(prepared);

    ctx.clipped(|ctx| {
        for (group_index, group) in groups.iter().enumerate() {
            let values: Vec<Value> = group
                .iter()
                .map(|(_, row)| {
                    if along_x {
                        row.x_value().clone()
                    } else {
                        row.y_value().clone()
                    }
                })
                .collect();
            let Some((density, domain)) = crate::density::density_of(&values, 80) else {
                continue;
            };
            let peak = density.iter().cloned().fold(0.0_f64, f64::max);
            if peak <= 0. {
                continue;
            }
            // The estimate's length runs along the axis the mark does *not*
            // read: a density along x grows up the plot's height, one along y
            // grows across its width.
            let plot = ctx.frame.plot;
            let across = if along_x {
                plot.size.height.as_f32()
            } else {
                plot.size.width.as_f32()
            };

            let points: Vec<Point<gpui::Pixels>> = density
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    let position = domain[0]
                        + (domain[1] - domain[0]) * index as f64 / (density.len() - 1) as f64;
                    let offset = (value / peak) as f32 * across;
                    if along_x {
                        // A density along x grows upward from the bottom.
                        let x = ctx.frame.x.scale.position(&Value::Number(position))?;
                        Some(ctx.absolute(gpui::point(px(x), px(across - offset))))
                    } else {
                        let y = ctx.frame.y.scale.position(&Value::Number(position))?;
                        Some(ctx.absolute(gpui::point(px(offset), px(y))))
                    }
                })
                .collect();

            if points.len() < 2 {
                continue;
            }
            let color = colors
                .fill
                .as_ref()
                .and_then(|colors| group.first().map(|(index, _)| colors[*index]))
                .unwrap_or_else(|| ctx.default_fill(prepared));

            // Close the curve down to the edge it grows from: the plot's
            // bottom for a density along x, its left edge for one along y.
            let mut outline = points.clone();
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                let baseline = |p: &Point<gpui::Pixels>| {
                    if along_x {
                        point(p.x, plot.origin.y + plot.size.height)
                    } else {
                        point(plot.origin.x, p.y)
                    }
                };
                outline.push(baseline(last));
                outline.push(baseline(first));
            }
            paint::paint_fill(&outline, paint::alpha(color, 0.3), ctx.window);
            paint::paint_stroke(&points, 1.5, color, ctx.window);
            let _ = group_index;
        }
    });
}

fn draw_hexbin(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let plot = ctx.frame.plot;
    let side = plot.size.width.as_f32().min(plot.size.height.as_f32());
    let radius = (side / 18.).max(4.);

    // Bin points into hexagon cells keyed by their row and column, then draw
    // each cell with the count's color.
    let mut cells: std::collections::HashMap<(i32, i32), usize> = std::collections::HashMap::new();
    for row in prepared.rows.iter() {
        let Some(position) = ctx.frame.point(row.x_value(), row.y_value()) else {
            continue;
        };
        let (x, y) = (position.x.as_f32(), position.y.as_f32());
        let column = (x / (radius * 1.5)).floor() as i32;
        let line = ((y - if column % 2 == 0 { 0. } else { radius * 0.866 }) / (radius * 1.732))
            .floor() as i32;
        *cells.entry((column, line)).or_insert(0) += 1;
    }

    let max = cells.values().copied().max().unwrap_or(1).max(1) as f64;
    let scale = crate::scale::ColorScale::continuous([0., max], ctx.theme.sequential_range());

    ctx.clipped(|ctx| {
        for ((column, line), count) in cells.iter() {
            let x = *column as f32 * radius * 1.5 + radius;
            let y = *line as f32 * radius * 1.732
                + if column % 2 == 0 {
                    radius
                } else {
                    radius * 1.866
                };
            let center = ctx.absolute(gpui::point(px(x), px(y)));
            let points = paint::circle_points(center, radius * 0.95, 6);
            let color = scale
                .color(&Value::Number(*count as f64))
                .unwrap_or_else(|| colors.fill(0));
            paint::paint_fill(&points, paint::alpha(color, 0.85), ctx.window);
        }
    });
}

fn draw_contour(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    // Contours need a raster; without one this mark draws the density of its
    // two channels as nested rings, which is the same shape a contour
    // threshold traces.
    let colors = Colors::resolve(prepared, ctx);
    let plot = ctx.frame.plot;
    let (width, height) = (
        plot.size.width.as_f32() as usize,
        plot.size.height.as_f32() as usize,
    );
    let (width, height) = (width.clamp(8, 120), height.clamp(8, 120));

    // A coarse raster of counts, one cell per datum.
    let mut raster = vec![0.0_f64; width * height];
    for row in prepared.rows.iter() {
        let Some(position) = ctx.frame.point(row.x_value(), row.y_value()) else {
            continue;
        };
        let column =
            ((position.x.as_f32() / plot.size.width.as_f32()) * width as f32).floor() as usize;
        let line =
            ((position.y.as_f32() / plot.size.height.as_f32()) * height as f32).floor() as usize;
        if column < width && line < height {
            raster[line * width + column] += 1.;
        }
    }

    let max = raster.iter().cloned().fold(0.0_f64, f64::max);
    if max <= 0. {
        return;
    }
    let color = colors.fill(0);

    ctx.clipped(|ctx| {
        for (index, count) in raster.iter().enumerate() {
            if *count <= 0. {
                continue;
            }
            let column = (index % width) as f32;
            let line = (index / width) as f32;
            let cell_width = plot.size.width.as_f32() / width as f32;
            let cell_height = plot.size.height.as_f32() / height as f32;
            let rect = paint::rect_between(
                ctx.absolute(gpui::point(px(column * cell_width), px(line * cell_height))),
                ctx.absolute(gpui::point(
                    px((column + 1.) * cell_width),
                    px((line + 1.) * cell_height),
                )),
            );
            paint::paint_rect(
                rect,
                0.,
                paint::alpha(color, (count / max) as f32 * 0.8),
                ctx.window,
            );
        }
    });
}
