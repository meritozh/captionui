//! Plot's axis, grid and frame marks: `axisX`, `axisY`, `gridX`, `gridY` and
//! `frame`, drawn as marks so a chart can style or place them.
//!
//! An axis or grid mark's ticks are its data when it has some
//! (`Plot.gridX(d3.range(101))`), else its `ticks` option, else the scale's
//! own ticks. An axis mark draws where its anchor says, or at the position
//! its constant `x` (for `axisY`) or `y` (for `axisX`) names.

use gpui::{FontWeight, Hsla, Pixels, Point, TextAlign, point, px};

use crate::frame::{Axis, AxisTick};
use crate::mark::MarkKind;
use crate::options::{AxisAnchor, Ticks};
use crate::paint::{self, Label};
use crate::render::PreparedMark;
use crate::render::basic::{Colors, DrawContext};
use crate::value::Value;

/// Paint a furniture mark.
pub fn draw(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    // A furniture mark faceted by a constant (`frame({fy: "Gentoo"})`) draws
    // only in the facet it names, which holds its placeholder row.
    if prepared.rows.is_empty() {
        return;
    }
    match prepared.mark.kind {
        MarkKind::AxisX => draw_axis(prepared, ctx, true),
        MarkKind::AxisY => draw_axis(prepared, ctx, false),
        MarkKind::GridX => draw_grid(prepared, ctx, true),
        MarkKind::GridY => draw_grid(prepared, ctx, false),
        MarkKind::Frame => draw_frame(prepared, ctx),
        _ => {}
    }
}

/// The ticks a furniture mark draws along its axis.
fn ticks(prepared: &PreparedMark, axis: &Axis, x: bool) -> Vec<(AxisTick, usize)> {
    let has_data =
        prepared.mark.data.is_some() && !prepared.rows.is_empty() && !is_placeholder(prepared);
    if has_data {
        return prepared
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let value = if x { row.x_value() } else { row.y_value() }.clone();
                let position = axis.scale.position(&value)?;
                let label = axis.format(&value, &prepared.mark.axis);
                Some((
                    AxisTick {
                        value,
                        position,
                        label,
                    },
                    index,
                ))
            })
            .collect();
    }
    match &prepared.mark.axis.ticks {
        Some(ticks) => axis
            .ticks_for(ticks, &prepared.mark.axis)
            .into_iter()
            .map(|tick| (tick, 0))
            .collect(),
        None => axis
            .ticks
            .iter()
            .map(|tick| {
                let label = match &prepared.mark.axis.tick_format {
                    Some(format) => format.format(&tick.value).into(),
                    None => tick.label.clone(),
                };
                (
                    AxisTick {
                        label,
                        ..tick.clone()
                    },
                    0,
                )
            })
            .collect(),
    }
}

/// Whether the mark's only row is the placeholder a constant facet gives it.
fn is_placeholder(prepared: &PreparedMark) -> bool {
    prepared.rows.len() == 1 && prepared.rows[0].row.is_empty()
}

/// The color a furniture mark draws in: its stroke, else `currentColor`.
fn stroke_color(prepared: &PreparedMark, ctx: &DrawContext<'_>, index: usize) -> Hsla {
    let colors = Colors::resolve(prepared, ctx);
    match &prepared.mark.channels.stroke {
        Some(_) => colors.stroke(index),
        None => ctx.colors.default_stroke(prepared, ctx.mark_index),
    }
}

fn draw_axis(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, x: bool) {
    let frame = ctx.frame;
    let plot = frame.plot;
    let options = &prepared.mark.axis;
    let axis = if x { &frame.x } else { &frame.y };
    let anchor = options.axis.flatten().unwrap_or(if x {
        AxisAnchor::Bottom
    } else {
        AxisAnchor::Left
    });
    // A constant `y` (for axisX) or `x` (for axisY) places the axis there.
    let placed = prepared.rows.first().and_then(|row| {
        let channel = if x {
            &prepared.mark.channels.y
        } else {
            &prepared.mark.channels.x
        };
        let value = channel.as_ref()?.read(&row.row);
        if x {
            frame.y.scale.position(&value)
        } else {
            frame.x.scale.position(&value)
        }
    });
    let tick_size = options.tick_size.unwrap_or(6.);
    let tick_padding = options.tick_padding.unwrap_or(3.);
    let size = px(prepared.mark.style.font_size.unwrap_or(10.));
    let font = gpui::font(ctx.theme.font_family.clone());
    let opacity = prepared.mark.style.opacity;
    let fill = match &prepared.mark.channels.fill {
        Some(channel) => ctx.colors.channel(channel, &gpui_plot_empty()),
        None => ctx.colors.default_fill(prepared, ctx.mark_index),
    };
    let fill = paint::alpha(fill, opacity * prepared.mark.style.fill_opacity);
    for (tick, index) in ticks(prepared, axis, x) {
        let stroke = paint::alpha(
            stroke_color(prepared, ctx, index),
            opacity * prepared.mark.style.stroke_opacity,
        );
        let label = Label::new(
            tick.label.clone(),
            size,
            fill,
            FontWeight::NORMAL,
            font.clone(),
            ctx.window,
        );
        if x {
            let top = anchor == AxisAnchor::Top;
            let edge = match placed {
                Some(y) => plot.origin.y + px(y),
                None if top => plot.origin.y,
                None => plot.origin.y + plot.size.height,
            };
            let direction = if top { -1. } else { 1. };
            let tx = plot.origin.x + px(tick.position);
            if tick_size > 0. && stroke.a > 0. {
                paint::paint_stroke(
                    &[point(tx, edge), point(tx, edge + px(tick_size * direction))],
                    1.,
                    stroke,
                    ctx.window,
                );
            }
            if !tick.label.is_empty() && fill.a > 0. {
                let offset = tick_size.max(0.) + tick_padding;
                let ty = if top {
                    edge - px(offset) - label.height
                } else {
                    edge + px(offset)
                };
                paint_rotated(
                    &label,
                    point(tx, ty),
                    TextAlign::Center,
                    options.tick_rotate,
                    ctx,
                );
            }
        } else {
            let right = anchor == AxisAnchor::Right;
            let edge = match placed {
                Some(x) => plot.origin.x + px(x),
                None if right => plot.origin.x + plot.size.width,
                None => plot.origin.x,
            };
            let direction = if right { 1. } else { -1. };
            let ty = plot.origin.y + px(tick.position);
            if tick_size > 0. && stroke.a > 0. {
                paint::paint_stroke(
                    &[point(edge, ty), point(edge + px(tick_size * direction), ty)],
                    1.,
                    stroke,
                    ctx.window,
                );
            }
            if !tick.label.is_empty() && fill.a > 0. {
                let offset = tick_size.max(0.) + tick_padding;
                let origin = point(edge + px(offset * direction), ty - label.height / 2.);
                let align = if right {
                    TextAlign::Left
                } else {
                    TextAlign::Right
                };
                paint_rotated(&label, origin, align, options.tick_rotate, ctx);
            }
        }
    }
}

/// The empty record a constant channel reads.
fn gpui_plot_empty() -> crate::table::Table {
    crate::table::Table::empty()
}

/// Paint a tick label; a rotated one pivots on its anchor.
fn paint_rotated(
    label: &Label,
    origin: Point<Pixels>,
    align: TextAlign,
    rotate: Option<f32>,
    ctx: &mut DrawContext<'_>,
) {
    // GPUI cannot rotate shaped text, so a rotated label stands its glyphs
    // off the axis by its rotation's vertical reach instead.
    let origin = match rotate {
        Some(degrees) if degrees.abs() > 1. => {
            let reach = label.width.as_f32() * degrees.to_radians().sin().abs() / 2.;
            point(origin.x, origin.y + px(reach))
        }
        _ => origin,
    };
    label.paint_at(origin, align, None, ctx.window, ctx.cx);
}

fn draw_grid(prepared: &PreparedMark, ctx: &mut DrawContext<'_>, x: bool) {
    let frame = ctx.frame;
    let plot = frame.plot;
    let axis = if x { &frame.x } else { &frame.y };
    let width = prepared.mark.style.stroke_width;
    let dash = prepared.mark.style.dash.clone();
    for (tick, index) in ticks(prepared, axis, x) {
        let row = prepared.rows.get(index);
        let opacity = row.map_or(prepared.mark.style.opacity, |row| {
            row.opacity.unwrap_or(prepared.mark.style.opacity)
        }) * prepared.mark.style.stroke_opacity;
        let color = paint::alpha(stroke_color(prepared, ctx, index), opacity);
        let (start, end) = if x {
            let gx = plot.origin.x + px(tick.position);
            (
                point(gx, plot.origin.y),
                point(gx, plot.origin.y + plot.size.height),
            )
        } else {
            let gy = plot.origin.y + px(tick.position);
            (
                point(plot.origin.x, gy),
                point(plot.origin.x + plot.size.width, gy),
            )
        };
        if dash.is_empty() {
            paint::paint_stroke(&[start, end], width, color, ctx.window);
        } else {
            paint::paint_dashed(start, end, &dash, width, color, ctx.window);
        }
    }
}

fn draw_frame(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let plot = ctx.frame.plot;
    let style = &prepared.mark.style;
    let inset = style.inset.unwrap_or(0.);
    let (left, top) = (plot.origin.x + px(inset), plot.origin.y + px(inset));
    let (right, bottom) = (
        plot.origin.x + plot.size.width - px(inset),
        plot.origin.y + plot.size.height - px(inset),
    );
    let row = prepared
        .rows
        .first()
        .map(|row| row.row.clone())
        .unwrap_or_default();
    // Plot's frame fills nothing and strokes `currentColor`; given a fill and
    // no stroke it only fills.
    if let Some(channel) = &prepared.mark.channels.fill {
        let fill = paint::alpha(
            ctx.colors.channel(channel, &row),
            style.opacity * style.fill_opacity,
        );
        paint::paint_rect(
            gpui::Bounds::from_corners(point(left, top), point(right, bottom)),
            style.corner_radius,
            fill,
            ctx.window,
        );
        if prepared.mark.channels.stroke.is_none() {
            return;
        }
    }
    let stroke = match &prepared.mark.channels.stroke {
        Some(channel) => ctx.colors.channel(channel, &row),
        None => ctx.colors.default_stroke(prepared, ctx.mark_index),
    };
    let stroke = paint::alpha(stroke, style.opacity * style.stroke_opacity);
    let width = style.stroke_width;
    let side = |a: Point<Pixels>, b: Point<Pixels>, ctx: &mut DrawContext<'_>| {
        paint::paint_stroke(&[a, b], width, stroke, ctx.window);
    };
    match prepared.mark.axis.axis.flatten() {
        Some(AxisAnchor::Top) => side(point(left, top), point(right, top), ctx),
        Some(AxisAnchor::Bottom) => side(point(left, bottom), point(right, bottom), ctx),
        Some(AxisAnchor::Left) => side(point(left, top), point(left, bottom), ctx),
        Some(AxisAnchor::Right) => side(point(right, top), point(right, bottom), ctx),
        _ => {
            if style.corner_radius > 0. {
                paint::paint_rect_outline(
                    gpui::Bounds::from_corners(point(left, top), point(right, bottom)),
                    style.corner_radius,
                    width,
                    stroke,
                    ctx.window,
                );
            } else {
                paint::paint_stroke(
                    &[
                        point(left, top),
                        point(right, top),
                        point(right, bottom),
                        point(left, bottom),
                        point(left, top),
                    ],
                    width,
                    stroke,
                    ctx.window,
                );
            }
        }
    }
}

impl Axis {
    /// A tick label as this axis formats it, under a mark's own options.
    pub fn format(
        &self,
        value: &Value,
        options: &crate::options::ScaleOptions,
    ) -> gpui::SharedString {
        match &options.tick_format {
            Some(format) => format.format(value).into(),
            None => match self.ticks.iter().find(|t| t.value.key() == value.key()) {
                Some(tick) => tick.label.clone(),
                None => match value {
                    Value::Number(n) => crate::format::format_number(",", *n).into(),
                    other => other.label(),
                },
            },
        }
    }

    /// The ticks a mark's `ticks` option asks this axis for.
    pub fn ticks_for(
        &self,
        ticks: &Ticks,
        options: &crate::options::ScaleOptions,
    ) -> Vec<AxisTick> {
        let values: Vec<Value> = match ticks {
            Ticks::Values(values) => values.clone(),
            Ticks::Count(count) => match &self.scale {
                crate::scale::PositionScale::Continuous(scale) => {
                    if self.temporal {
                        scale
                            .time_ticks(*count)
                            .into_iter()
                            .map(|m| Value::date_from_millis(m as i64))
                            .collect()
                    } else {
                        scale.ticks(*count).into_iter().map(Value::Number).collect()
                    }
                }
                _ => self.ticks.iter().map(|t| t.value.clone()).collect(),
            },
            Ticks::Interval(interval) => match &self.scale {
                crate::scale::PositionScale::Continuous(scale) => {
                    crate::frame::interval_ticks(*interval, scale.domain(), self.temporal)
                        .into_iter()
                        .map(|n| {
                            if self.temporal {
                                Value::date_from_millis(n as i64)
                            } else {
                                Value::Number(n)
                            }
                        })
                        .collect()
                }
                _ => self.ticks.iter().map(|t| t.value.clone()).collect(),
            },
        };
        let count = match ticks {
            Ticks::Count(count) => *count,
            _ => values.len().max(1),
        };
        let numbers: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
        let (lo, hi) = numbers
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        let format = crate::format::tick_format(lo, hi, count, None);
        values
            .into_iter()
            .filter_map(|value| {
                let position = self.scale.position(&value)?;
                let label = match (&options.tick_format, &value) {
                    (Some(f), v) => f.format(v).into(),
                    (None, Value::Number(n)) if !self.temporal => format.format(*n).into(),
                    (None, Value::Date(_)) | (None, Value::Number(_)) => {
                        self.format(&value, options)
                    }
                    (None, v) => v.label(),
                };
                Some(AxisTick {
                    value,
                    position,
                    label,
                })
            })
            .collect()
    }
}
