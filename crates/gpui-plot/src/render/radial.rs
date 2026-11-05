//! Radial marks: pie, arc, waffle, plus the box, violin and density summaries.
//!
//! A radial mark places its rows on an arc rather than on the x and y axes; its
//! domain is the sum of its values, not the frame's extents.

use std::collections::HashMap;

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
        MarkKind::Contour => crate::render::basic::draw_contour_grid(prepared, ctx),
        MarkKind::Density => draw_density_2d(prepared, ctx),
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
    (plot.size.width.as_f32().min(plot.size.height.as_f32()) / 2. - 8.).max(0.)
}

fn values(prepared: &PreparedMark) -> Vec<Value> {
    prepared
        .rows
        .iter()
        .map(|row| {
            // A pie reads its length from the value channel, which the bar
            // interval carries after resolution. A share has to be a finite,
            // non-negative number: a NaN would poison the total and a
            // negative one would push the other slices past a full turn.
            let value = row.y2.clone().unwrap_or_else(|| row.y_value().clone());
            match value.as_number() {
                Some(share) if share.is_finite() && share >= 0. => value,
                _ => Value::Null,
            }
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

/// Plot's `hexbin`: points binned into hexagons on the same lattice the
/// `hexgrid` mark draws, so the two align.
///
/// The binning is Plot's `hbin`: the point is rounded onto the nearest hex
/// center, and when it falls near a hexagon's corner the neighbouring center
/// wins if it is closer. The lattice's origin is offset by half a pixel, which
/// keeps the frame's rightmost values out of a floating bin beyond the edge.
fn draw_hexbin(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let bin_width = prepared.mark.style.bin_width.max(2.);
    // The lattice's spacing: `dx` across, and rows `dy` apart.
    let dx = bin_width;
    let dy = dx * (1.5 / 3f32.sqrt());
    let (ox, oy) = (0.5_f32, 0.0_f32);

    // Key by the lattice cell, and keep its center in pixels.
    let mut cells: HashMap<(i32, i32), (f32, f32, usize)> = HashMap::new();
    for row in prepared.rows.iter() {
        let Some(placed) = ctx.frame.point(row.x_value(), row.y_value()) else {
            continue;
        };
        let (px, py) = (placed.x.as_f32(), placed.y.as_f32());
        let mut line = ((py - oy) / dy).round();
        let mut column = ((px - ox) / dx - (line as i32 & 1) as f32 / 2.).round();
        let fy = (py - oy) / dy - line;
        // Near a corner, take the neighbouring hexagon when it is nearer.
        if fy.abs() * 3. > 1. {
            let fx = (px - ox) / dx - (line as i32 & 1) as f32 / 2. - column;
            let column2 = column
                + if (px - ox) / dx - (line as i32 & 1) as f32 / 2. < column {
                    -0.5
                } else {
                    0.5
                };
            let line2 = line + if (py - oy) / dy < line { -1. } else { 1. };
            let fx2 = (px - ox) / dx - (line2 as i32 & 1) as f32 / 2. - column2;
            let fy2 = (py - oy) / dy - line2;
            if fx * fx + fy * fy > fx2 * fx2 + fy2 * fy2 {
                column = column2 + if line as i32 & 1 == 1 { 0.5 } else { -0.5 };
                line = line2;
            }
        }
        let key = (column as i32, line as i32);
        let centre = (
            (column + (line as i32 & 1) as f32 / 2.) * dx + ox,
            line * dy + oy,
        );
        let entry = cells.entry(key).or_insert((centre.0, centre.1, 0));
        entry.2 += 1;
    }

    let max = cells
        .values()
        .map(|(.., count)| *count)
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let scale = crate::scale::ColorScale::continuous([0., max], ctx.theme.sequential_range());

    ctx.clipped(|ctx| {
        for (x, y, count) in cells.values() {
            let center = ctx.absolute(gpui::point(px(*x), px(*y)));
            // Plot's hexagon symbol, pointy-top: `binWidth` is its full
            // width, and its corners reach `ry` above and below the centre.
            let rx = bin_width / 2.;
            let ry = rx * (2. / 3f32.sqrt());
            let points = vec![
                point(px(center.x.as_f32()), px(center.y.as_f32() - ry)),
                point(px(center.x.as_f32() + rx), px(center.y.as_f32() - ry / 2.)),
                point(px(center.x.as_f32() + rx), px(center.y.as_f32() + ry / 2.)),
                point(px(center.x.as_f32()), px(center.y.as_f32() + ry)),
                point(px(center.x.as_f32() - rx), px(center.y.as_f32() + ry / 2.)),
                point(px(center.x.as_f32() - rx), px(center.y.as_f32() - ry / 2.)),
            ];
            let color = scale
                .color(&Value::Number(*count as f64))
                .unwrap_or_else(|| colors.fill(0));
            paint::paint_fill(
                &points,
                paint::alpha(color, ctx.row_fill_opacity(prepared, &prepared.rows[0])),
                ctx.window,
            );
        }
    });
}

/// Plot's `density` defaults: a 20-pixel Gaussian bandwidth over 4-pixel
/// cells (d3's `contourDensity`), and 20 thresholds.
const DENSITY_BANDWIDTH: f32 = 20.;
const DENSITY_CELL: f32 = 4.;
const DENSITY_THRESHOLDS: usize = 20;

/// Draw Plot's `density`: each series' kernel density over the frame's
/// pixels, d3's `contourDensity` (20-pixel bandwidth, 4-pixel cells), cut at
/// thresholds shared by every series — `thresholds` evenly spaced up to the
/// densest series' peak, or the ones given (in Plot's units, density × 100).
/// `fill: "density"` or `stroke: "density"` colors each threshold's band
/// through the color scale; another fill fills the bands, and without one the
/// lines stroke in their series' color.
fn draw_density_2d(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let plot = ctx.frame.plot;
    let (width, height) = (plot.size.width.as_f32(), plot.size.height.as_f32());
    let spec = prepared.mark.density.clone().unwrap_or_default();
    let bandwidth = spec.bandwidth.unwrap_or(DENSITY_BANDWIDTH);
    let groups = series_groups(prepared);
    let place = |row: &crate::render::PreparedRow| -> Option<(f32, f32)> {
        let p = crate::render::basic::position(prepared, ctx, row)?;
        Some((p.x.as_f32(), p.y.as_f32()))
    };
    let grids: Vec<(usize, crate::density::DensityGrid)> = groups
        .iter()
        .filter_map(|group| {
            let points: Vec<(f32, f32, f64)> = group
                .iter()
                .filter_map(|(_, row)| {
                    let (x, y) = place(row)?;
                    let w = spec.weight.as_ref().map_or(1., |f| f(&row.row));
                    Some((x, y, w))
                })
                .collect();
            let first = group.first()?.0;
            (!points.is_empty()).then(|| {
                (
                    first,
                    crate::density::DensityGrid::estimate_weighted(
                        &points,
                        width,
                        height,
                        bandwidth,
                        DENSITY_CELL,
                    ),
                )
            })
        })
        .collect();
    // d3 normalizes the density to points per square pixel; our grid holds
    // weight per cell after blurring, so divide by the cell's area. Plot's
    // thresholds are in units of density × 100.
    let k = 100. / (DENSITY_CELL as f64 * DENSITY_CELL as f64);
    let thresholds: Vec<f64> = match &spec.values {
        Some(values) => values.iter().map(|t| t / k).collect(),
        None => {
            let max = grids.iter().map(|(_, g)| g.max()).fold(0., f64::max);
            let n = spec.count.unwrap_or(DENSITY_THRESHOLDS).max(1);
            (1..n).map(|i| max * i as f64 / n as f64).collect()
        }
    };
    let is_density = |c: &Option<crate::style::ColorChannel>| matches!(c, Some(crate::style::ColorChannel::Field(f)) if f.eq_ignore_ascii_case("density"));
    let (fill_density, stroke_density) = (
        is_density(&prepared.mark.channels.fill),
        is_density(&prepared.mark.channels.stroke),
    );
    let style = &prepared.mark.style;
    let density_color = |t: f64, ctx: &DrawContext<'_>| {
        ctx.colors
            .scale
            .as_ref()
            .and_then(|s| s.color(&crate::value::Value::Number(t * k)))
    };
    ctx.clipped(|ctx| {
        for (first, grid) in &grids {
            let row = &prepared.rows[*first];
            let fill = if fill_density {
                None
            } else {
                prepared
                    .mark
                    .channels
                    .fill
                    .as_ref()
                    .map(|c| ctx.colors.channel(c, &row.row))
            };
            let stroke = if stroke_density || fill_density || fill.is_some() {
                prepared
                    .mark
                    .channels
                    .stroke
                    .as_ref()
                    .filter(|_| !stroke_density)
                    .map(|c| ctx.colors.channel(c, &row.row))
            } else {
                Some(ctx.colors.series_color(prepared, ctx.mark_index, row))
            };
            for &t in &thresholds {
                let rings: Vec<Vec<Point<gpui::Pixels>>> = grid
                    .contours(t)
                    .into_iter()
                    .map(|line| {
                        line.iter()
                            .map(|(x, y)| ctx.absolute(point(px(*x), px(*y))))
                            .collect()
                    })
                    .collect();
                let band_fill = if fill_density {
                    density_color(t, ctx)
                } else {
                    fill
                };
                if let Some(color) = band_fill {
                    let color =
                        paint::alpha(color, style.opacity * style.fill_opacity * ctx.appear);
                    paint::paint_fill_rings(&rings, color, ctx.window);
                }
                let band_stroke = if stroke_density {
                    density_color(t, ctx)
                } else {
                    stroke
                };
                if let Some(color) = band_stroke {
                    let color =
                        paint::alpha(color, ctx.row_stroke_opacity(prepared, row) * ctx.appear);
                    for ring in &rings {
                        paint::paint_stroke(ring, style.stroke_width, color, ctx.window);
                    }
                }
            }
        }
    });
}

/// The peak of a density mark's estimate in Plot's units (density × 100),
/// over the frame, for its "density" color scale.
pub fn density_peak(prepared: &PreparedMark, frame: &crate::frame::Frame) -> Option<f64> {
    if prepared.mark.kind != MarkKind::Density {
        return None;
    }
    let spec = prepared.mark.density.clone().unwrap_or_default();
    let bandwidth = spec.bandwidth.unwrap_or(DENSITY_BANDWIDTH);
    let (width, height) = (
        frame.plot.size.width.as_f32(),
        frame.plot.size.height.as_f32(),
    );
    let mut max = 0.0_f64;
    for group in series_groups(prepared) {
        let points: Vec<(f32, f32, f64)> = group
            .iter()
            .filter_map(|(_, row)| {
                let p = crate::render::basic::position_in(prepared, frame, row)?;
                Some((
                    p.x.as_f32(),
                    p.y.as_f32(),
                    spec.weight.as_ref().map_or(1., |f| f(&row.row)),
                ))
            })
            .collect();
        let grid = crate::density::DensityGrid::estimate_weighted(
            &points,
            width,
            height,
            bandwidth,
            DENSITY_CELL,
        );
        max = max.max(grid.max());
    }
    let n = spec.count.unwrap_or(DENSITY_THRESHOLDS).max(1) as f64;
    // The highest threshold, as Plot's legend runs to it.
    Some(max * (n - 1.) / n * 100. / (DENSITY_CELL as f64 * DENSITY_CELL as f64))
}
