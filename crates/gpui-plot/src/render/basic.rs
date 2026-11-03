//! The primitive marks: dot, line, area, bar, rect, cell, rule, text, tick,
//! vector and link.
//!
//! Each mark projects its rows through the frame's scales and paints. Shapes
//! are clipped to the plot, so a value outside the domain stops at the frame's
//! edge the way Plot's marks do.

use std::collections::HashMap;

use gpui::{
    App, ContentMask, Hsla, PathBuilder, Pixels, Point, SharedString, TextAlign, Window, point, px,
};

use crate::frame::Frame;
use crate::mark::{CrossAxis, Curve, MarkKind};
use crate::paint::{self, Label};
use crate::render::{PreparedMark, PreparedRow};
use crate::scale::{ColorScale, PositionScale};
use crate::style::Symbol;
use crate::theme::PlotTheme;
use crate::value::Value;

/// Everything a mark needs to paint one frame.
pub struct DrawContext<'a> {
    pub frame: &'a Frame,
    pub theme: &'a PlotTheme,
    pub mark_index: usize,
    pub appear: f32,
    /// How far the hover has faded in, from `0` to `1`.
    pub hover_progress: f32,
    /// The flattened index of the hovered datum, if any.
    pub hovered: Option<usize>,
    /// The offset a mark's rows have in the flattened row list, so a row can
    /// tell whether it is the hovered one.
    pub row_offset: usize,
    /// The plot's ordinal color scale, shared by every mark so a series keeps
    /// one color wherever it appears.
    pub ordinal: &'a ColorScale,
    pub window: &'a mut Window,
    pub cx: &'a mut App,
}

impl DrawContext<'_> {
    /// A plot-relative point in the figure's coordinates.
    pub fn absolute(&self, point: Point<Pixels>) -> Point<Pixels> {
        point + self.frame.plot.origin
    }

    /// Paint inside the plot, so shapes never spill over the axes.
    pub fn clipped<R>(&mut self, f: impl FnOnce(&mut DrawContext<'_>) -> R) -> R {
        let bounds = self.frame.plot;
        let frame = self.frame;
        let theme = self.theme;
        let mark_index = self.mark_index;
        let appear = self.appear;
        let ordinal = self.ordinal;
        let cx = &mut *self.cx;
        let window = &mut *self.window;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let mut inner = DrawContext {
                frame,
                theme,
                mark_index,
                appear,
                hover_progress: self.hover_progress,
                hovered: self.hovered,
                row_offset: self.row_offset,
                ordinal,
                window,
                cx,
            };
            f(&mut inner)
        })
    }

    /// The default fill of this mark.
    ///
    /// A mark that draws series takes a palette color; a plain mark uses the
    /// theme's one mark color, the way Plot paints every uncolored mark in
    /// `currentColor`. A line and a dot over the same data are then the same
    /// color rather than two arbitrary palette entries.
    pub fn default_fill(&self, prepared: &PreparedMark) -> Hsla {
        if prepared.mark.channels.fill.is_some() {
            return prepared.mark.style.fill;
        }
        if has_series(prepared) {
            self.theme.palette_color(self.mark_index)
        } else {
            self.theme.mark_color
        }
    }

    /// The default stroke of this mark, matching its fill.
    pub fn default_stroke(&self, prepared: &PreparedMark) -> Hsla {
        if prepared.mark.channels.stroke.is_some() {
            return prepared.mark.style.stroke;
        }
        if has_series(prepared) {
            self.theme.palette_color(self.mark_index)
        } else {
            self.theme.mark_color
        }
    }
}

impl DrawContext<'_> {
    /// How far the row at `index` is emphasized: `1` for a hovered row, fading
    /// with the hover, and `0` for the rest.
    pub fn emphasis(&self, index: usize) -> f32 {
        if self.hovered == Some(self.row_offset + index) {
            self.hover_progress
        } else {
            0.
        }
    }

    /// The plot's ordinal color scale.
    pub fn ordinal_scale(&self) -> ColorScale {
        self.ordinal.clone()
    }

    /// The opacity a row paints its fill at: the mark's `opacity`, then its
    /// `fillOpacity`, then the row's own `opacity` channel.
    pub fn row_fill_opacity(&self, prepared: &PreparedMark, row: &PreparedRow) -> f32 {
        row.opacity.unwrap_or(prepared.mark.style.opacity) * prepared.mark.style.fill_opacity
    }

    /// The opacity a row paints its stroke at.
    pub fn row_stroke_opacity(&self, prepared: &PreparedMark, row: &PreparedRow) -> f32 {
        row.opacity.unwrap_or(prepared.mark.style.opacity) * prepared.mark.style.stroke_opacity
    }
}

fn has_series(prepared: &PreparedMark) -> bool {
    prepared.rows.iter().any(|row| row.z.is_some())
}

/// Resolve fill and stroke for each row, honouring the mark's color channels
/// and a `z`-driven ordinal color scale.
pub struct Colors {
    pub fill: Option<Vec<Hsla>>,
    pub stroke: Option<Vec<Hsla>>,
    fallback_fill: Hsla,
    fallback_stroke: Hsla,
}

impl Colors {
    pub fn resolve(prepared: &PreparedMark, ctx: &DrawContext<'_>) -> Self {
        let fill = prepared.mark.channels.fill.as_ref().map(|channel| {
            prepared
                .rows
                .iter()
                .map(|row| channel.read(&row.row))
                .collect()
        });
        let stroke = prepared.mark.channels.stroke.as_ref().map(|channel| {
            prepared
                .rows
                .iter()
                .map(|row| channel.read(&row.row))
                .collect()
        });

        // A `z` channel with no explicit fill paints each series in a palette
        // color, which is Plot's default ordinal color scale. The scale is one
        // for the whole plot, not one per mark: a chart drawn as a line per
        // series would otherwise paint every line in the first palette color.
        let z_scale = (!has_series(prepared) || prepared.mark.channels.fill.is_some())
            .then_some(())
            .is_none()
            .then(|| ctx.ordinal_scale());

        let fill = match (fill, &z_scale) {
            (Some(colors), _) => Some(colors),
            (None, Some(scale)) => Some(
                prepared
                    .rows
                    .iter()
                    .map(|row| {
                        row.z
                            .as_ref()
                            .and_then(|z| scale.color(z))
                            .unwrap_or_else(|| ctx.default_fill(prepared))
                    })
                    .collect(),
            ),
            (None, None) => None,
        };

        let fallback_fill = ctx.default_fill(prepared);
        let fallback_stroke = ctx.default_stroke(prepared);

        Self {
            fill,
            stroke,
            fallback_fill,
            fallback_stroke,
        }
    }

    pub fn fill(&self, index: usize) -> Hsla {
        self.fill
            .as_ref()
            .and_then(|colors| colors.get(index).copied())
            .unwrap_or(self.fallback_fill)
    }

    pub fn stroke(&self, index: usize) -> Hsla {
        self.stroke
            .as_ref()
            .and_then(|colors| colors.get(index).copied())
            .unwrap_or_else(|| {
                self.fill
                    .as_ref()
                    .and_then(|colors| colors.get(index).copied())
                    .unwrap_or(self.fallback_stroke)
            })
    }
}

/// Paint one prepared mark.
pub fn draw(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    match prepared.mark.kind {
        MarkKind::Dot | MarkKind::DotX | MarkKind::DotY => draw_dots(prepared, ctx),
        MarkKind::Line | MarkKind::LineX | MarkKind::LineY => draw_lines(prepared, ctx),
        MarkKind::AreaY => draw_areas_y(prepared, ctx),
        MarkKind::AreaX => draw_areas_x(prepared, ctx),
        MarkKind::BarY => draw_bars_y(prepared, ctx),
        MarkKind::BarX => draw_bars_x(prepared, ctx),
        MarkKind::Rect | MarkKind::RectX | MarkKind::RectY => draw_rects(prepared, ctx),
        MarkKind::Cell => draw_cells(prepared, ctx),
        MarkKind::RuleX => draw_rules_x(prepared, ctx),
        MarkKind::RuleY => draw_rules_y(prepared, ctx),
        MarkKind::Text => draw_texts(prepared, ctx),
        MarkKind::TickX => draw_ticks_x(prepared, ctx),
        MarkKind::TickY => draw_ticks_y(prepared, ctx),
        MarkKind::Vector => draw_vectors(prepared, ctx),
        MarkKind::Link => draw_links(prepared, ctx),
        _ => {}
    }
}

fn draw_dots(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let radius = prepared.mark.style.radius;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(position) = position(prepared, ctx, row) else {
                continue;
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
            let fill = paint::alpha(colors.fill(index), ctx.row_fill_opacity(prepared, row));
            let stroke = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
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

/// Paint one symbol at a plot-relative position.
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
    let points = symbol.path(absolute, radius);
    if symbol.is_line() {
        // Stroke symbols draw their segments, not one closed path.
        let segments = symbol_segments(symbol, absolute, radius);
        for segment in segments {
            paint::paint_stroke(&segment, stroke_width, stroke, ctx.window);
        }
        if symbol == Symbol::CircleOpen {
            let circle = paint::circle_points(absolute, radius, 24);
            paint::paint_stroke(&circle, stroke_width, stroke, ctx.window);
        }
    } else {
        paint::paint_fill(&points, fill, ctx.window);
        if stroke.a > 0. && stroke_width > 0. {
            paint::paint_stroke(&points, stroke_width, stroke, ctx.window);
        }
    }
}

/// The line segments a stroke symbol draws.
fn symbol_segments(symbol: Symbol, center: Point<Pixels>, radius: f32) -> Vec<Vec<Point<Pixels>>> {
    let (x, y) = (center.x.as_f32(), center.y.as_f32());
    let p = |dx: f32, dy: f32| point(px(x + dx), px(y + dy));
    let r = radius;
    let k = std::f32::consts::FRAC_1_SQRT_2 * r;
    match symbol {
        Symbol::Cross | Symbol::Plus => vec![vec![p(-r, 0.), p(r, 0.)], vec![p(0., -r), p(0., r)]],
        Symbol::Times | Symbol::CrossRotate => {
            vec![vec![p(-k, -k), p(k, k)], vec![p(k, -k), p(-k, k)]]
        }
        Symbol::Star => {
            let mut segments = Vec::new();
            for i in 0..4 {
                let angle = std::f32::consts::PI * i as f32 / 4.;
                segments.push(vec![
                    p(-r * angle.cos(), -r * angle.sin()),
                    p(r * angle.cos(), r * angle.sin()),
                ]);
            }
            segments
        }
        Symbol::Asterisk => {
            let mut segments = Vec::new();
            for i in 0..3 {
                let angle = std::f32::consts::PI * i as f32 / 3.;
                segments.push(vec![
                    p(-r * angle.cos(), -r * angle.sin()),
                    p(r * angle.cos(), r * angle.sin()),
                ]);
            }
            segments
        }
        _ => vec![],
    }
}

fn draw_lines(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width;
    let curve = prepared.mark.style.curve;

    let groups = series_groups(prepared);

    ctx.clipped(|ctx| {
        for (group_index, group) in groups.iter().enumerate() {
            let mut points: Vec<Point<Pixels>> = group
                .iter()
                .filter_map(|(index, row)| position(prepared, ctx, row).map(|p| (index, p)))
                .map(|(_, p)| ctx.absolute(p))
                .collect();
            if points.len() < 2 {
                continue;
            }
            let color = colors
                .stroke
                .as_ref()
                .and_then(|colors| group.first().map(|(index, _)| colors[*index]))
                .unwrap_or_else(|| ctx.default_stroke(prepared));
            let _ = group_index;
            let path = curve_path(&points, curve, width, ctx.appear);
            paint::paint_path(path, color, ctx.window);
            let _ = &mut points;
        }
    });
}

fn draw_areas_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let groups = series_groups(prepared);
    let curve = prepared.mark.style.curve;

    ctx.clipped(|ctx| {
        for group in groups.iter() {
            let rows: Vec<&PreparedRow> = group.iter().map(|(_, row)| *row).collect();
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
            let polygon = paint::fill_path(
                &upper,
                paint::alpha(color, prepared.mark.style.opacity * ctx.appear),
            );
            paint::paint_path(polygon, color, ctx.window);

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

fn draw_areas_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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

fn draw_bars_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = bar_width_y(ctx.frame, prepared);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(center) = ctx.frame.x.scale.position(row.x_value()) else {
                continue;
            };
            let Some((start, end)) = row.y_interval() else {
                continue;
            };
            let (Some(y0), Some(y1)) = (
                ctx.frame.y.scale.position(&start),
                ctx.frame.y.scale.position(&end),
            ) else {
                continue;
            };
            let x0 = center - width / 2.;
            let x1 = center + width / 2.;
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x1), px(y1))),
            );
            let color = colors.fill(index);
            paint::paint_rect(
                rect,
                prepared.mark.style.corner_radius,
                paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                ctx.window,
            );
            if prepared.mark.channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    prepared.mark.style.corner_radius,
                    prepared.mark.style.stroke_width,
                    colors.stroke(index),
                    ctx.window,
                );
            }
        }
    });
}

fn draw_bars_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = bar_width_x(ctx.frame, prepared);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(center) = ctx.frame.y.scale.position(row.y_value()) else {
                continue;
            };
            let Some((start, end)) = row.x_interval() else {
                continue;
            };
            let (Some(x0), Some(x1)) = (
                ctx.frame.x.scale.position(&start),
                ctx.frame.x.scale.position(&end),
            ) else {
                continue;
            };
            let y0 = center - width / 2.;
            let y1 = center + width / 2.;
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x1), px(y1))),
            );
            let color = colors.fill(index);
            paint::paint_rect(
                rect,
                prepared.mark.style.corner_radius,
                paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                ctx.window,
            );
            if prepared.mark.channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    prepared.mark.style.corner_radius,
                    prepared.mark.style.stroke_width,
                    colors.stroke(index),
                    ctx.window,
                );
            }
        }
    });
}

fn draw_rects(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some((x_start, x_end)) = row.x_interval() else {
                continue;
            };
            let Some((y_start, y_end)) = row.y_interval() else {
                continue;
            };
            let (Some(x0), Some(x1)) = (
                ctx.frame.x.scale.position(&x_start),
                ctx.frame.x.scale.position(&x_end),
            ) else {
                continue;
            };
            let (Some(y0), Some(y1)) = (
                ctx.frame.y.scale.position(&y_start),
                ctx.frame.y.scale.position(&y_end),
            ) else {
                continue;
            };
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x1), px(y1))),
            );
            let color = colors.fill(index);
            paint::paint_rect(
                rect,
                prepared.mark.style.corner_radius,
                paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                ctx.window,
            );
            if prepared.mark.channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    prepared.mark.style.corner_radius,
                    prepared.mark.style.stroke_width,
                    colors.stroke(index),
                    ctx.window,
                );
            }
        }
    });
}

fn draw_cells(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let (Some(x0), Some(y0)) = (
                ctx.frame.x.scale.position(row.x_value()),
                ctx.frame.y.scale.position(row.y_value()),
            ) else {
                continue;
            };
            let (width, height) = (
                cell_extent(&ctx.frame.x.scale),
                cell_extent(&ctx.frame.y.scale),
            );
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x0 + width), px(y0 + height))),
            );
            let color = colors.fill(index);
            paint::paint_rect(
                rect,
                0.,
                paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                ctx.window,
            );
        }
    });
}

/// The width a cell covers along an axis: a band's width, or a step for a
/// continuous axis.
fn cell_extent(scale: &PositionScale) -> f32 {
    match scale {
        PositionScale::Band(band) => band.band_width(),
        PositionScale::Point(point_scale) => {
            let step = point_scale.step();
            if step > 0. { step } else { 1. }
        }
        PositionScale::Continuous(_) => 1.,
    }
}

fn draw_rules_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let height = ctx.frame.plot.size.height.as_f32();
    let width = prepared.mark.style.stroke_width;
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(x) = ctx.frame.x.scale.position(row.x_value()) else {
                continue;
            };
            let start = ctx.absolute(point(px(x), px(0.)));
            let end = ctx.absolute(point(px(x), px(height)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            paint::paint_stroke(&[start, end], width, color, ctx.window);
        }
    });
}

fn draw_rules_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = ctx.frame.plot.size.width.as_f32();
    let stroke_width = prepared.mark.style.stroke_width;
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(y) = ctx.frame.y.scale.position(row.y_value()) else {
                continue;
            };
            let start = ctx.absolute(point(px(0.), px(y)));
            let end = ctx.absolute(point(px(width), px(y)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            paint::paint_stroke(&[start, end], stroke_width, color, ctx.window);
        }
    });
}

fn draw_texts(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let font = gpui::font(ctx.theme.font_family.clone());
    // Text has no outline: Plot's `text` mark paints its glyphs in `fill`,
    // and a caller that set only `stroke` (a dark label over a bar) means the
    // glyph color, not an outline.
    let colors = Colors::resolve(prepared, ctx);
    let size = px(11.);
    let (dx, dy) = (prepared.mark.style.dx, prepared.mark.style.dy);

    // Text is not clipped to the plot: a label above the first or last bar
    // overhangs the plot's edge, and Plot draws it whole. Clipping it to the
    // plot would cut the label in half.
    for (index, row) in prepared.rows.iter().enumerate() {
        let Some(position) = position(prepared, ctx, row) else {
            continue;
        };
        let color = if prepared.mark.channels.fill.is_some() {
            colors.fill(index)
        } else if prepared.mark.channels.stroke.is_some() {
            colors.stroke(index)
        } else {
            prepared.mark.style.fill
        };
        let text = prepared
            .mark
            .channels
            .text
            .as_ref()
            .map(|channel| channel.read(&row.row).label())
            .or_else(|| Some(row.y_value().label()))
            .unwrap_or_else(|| SharedString::from(""));
        if text.is_empty() {
            continue;
        }
        let label = Label::new(
            text,
            size,
            color,
            gpui::FontWeight::NORMAL,
            font.clone(),
            ctx.window,
        );
        let origin = ctx.absolute(position);
        let origin = point(px(origin.x.as_f32() + dx), px(origin.y.as_f32() + dy));
        label.paint_at(origin, TextAlign::Center, None, ctx.window, ctx.cx);
    }
}

fn draw_ticks_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let height = ctx.frame.plot.size.height.as_f32();
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(x) = ctx.frame.x.scale.position(row.x_value()) else {
                continue;
            };
            // A tick spans the full height of the frame, Plot's "barcode";
            // with a `y` channel on a band scale it spans that band instead.
            let (y0, y1) = tick_span(ctx.frame, row.y_value(), true, height);
            let start = ctx.absolute(point(px(x), px(y0)));
            let end = ctx.absolute(point(px(x), px(y1)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            paint::paint_stroke(&[start, end], width, color, ctx.window);
        }
    });
}

fn draw_ticks_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    let width_px = ctx.frame.plot.size.width.as_f32();
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(y) = ctx.frame.y.scale.position(row.y_value()) else {
                continue;
            };
            let (x0, x1) = tick_span(ctx.frame, row.x_value(), false, width_px);
            let start = ctx.absolute(point(px(x0), px(y)));
            let end = ctx.absolute(point(px(x1), px(y)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            paint::paint_stroke(&[start, end], width, color, ctx.window);
        }
    });
}

/// The extent a tick covers along the axis it spans.
///
/// `value` is the tick's secondary channel: when it lands on a band scale the
/// tick covers that band, otherwise it covers the whole frame, which is what
/// Plot's tick does without a secondary channel.
fn tick_span(frame: &Frame, value: &Value, vertical: bool, full: f32) -> (f32, f32) {
    let scale = if vertical {
        &frame.y.scale
    } else {
        &frame.x.scale
    };
    match scale {
        PositionScale::Band(band) => match band.start(&value.key()) {
            Some(start) => (start, start + band.band_width()),
            None => (0., full),
        },
        _ => (0., full),
    }
}

fn draw_vectors(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let (Some(x0), Some(x1)) = (
                ctx.frame
                    .x
                    .scale
                    .position(&row.x1.clone().unwrap_or(Value::Number(0.))),
                ctx.frame.x.scale.position(row.x_value()),
            ) else {
                continue;
            };
            let (Some(y0), Some(y1)) = (
                ctx.frame
                    .y
                    .scale
                    .position(&row.y1.clone().unwrap_or(Value::Number(0.))),
                ctx.frame.y.scale.position(row.y_value()),
            ) else {
                continue;
            };
            let from = ctx.absolute(point(px(x0), px(y0)));
            let to = ctx.absolute(point(px(x1), px(y1)));
            let color = colors.stroke(index);
            paint::paint_stroke(&[from, to], width, color, ctx.window);
            paint_arrow_head(ctx, from, to, width, color);
        }
    });
}

/// A filled triangle at `to`, pointing along `from`→`to`.
fn paint_arrow_head(
    ctx: &mut DrawContext<'_>,
    from: Point<Pixels>,
    to: Point<Pixels>,
    width: f32,
    color: Hsla,
) {
    let dx = to.x.as_f32() - from.x.as_f32();
    let dy = to.y.as_f32() - from.y.as_f32();
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1. {
        return;
    }
    let size = (width * 3.).max(4.);
    let (ux, uy) = (dx / length, dy / length);
    let tip = to;
    let base = point(px(to.x.as_f32() - ux * size), px(to.y.as_f32() - uy * size));
    let left = point(
        px(base.x.as_f32() - uy * size / 2.),
        px(base.y.as_f32() + ux * size / 2.),
    );
    let right = point(
        px(base.x.as_f32() + uy * size / 2.),
        px(base.y.as_f32() - ux * size / 2.),
    );
    paint::paint_fill(&[tip, left, right], color, ctx.window);
}

fn draw_links(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = prepared.mark.style.stroke_width.max(1.);
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let (Some(x0), Some(x1)) = (
                ctx.frame
                    .x
                    .scale
                    .position(&row.x1.clone().unwrap_or(Value::Number(0.))),
                ctx.frame.x.scale.position(row.x_value()),
            ) else {
                continue;
            };
            let (Some(y0), Some(y1)) = (
                ctx.frame
                    .y
                    .scale
                    .position(&row.y1.clone().unwrap_or(Value::Number(0.))),
                ctx.frame.y.scale.position(row.y_value()),
            ) else {
                continue;
            };
            let from = ctx.absolute(point(px(x0), px(y0)));
            let to = ctx.absolute(point(px(x1), px(y1)));
            // A quadratic curve whose control point is above the midpoint
            // reads as a link, Plot's default curve for `link`.
            let control = point(
                px((from.x.as_f32() + to.x.as_f32()) / 2.),
                px((from.y.as_f32() + to.y.as_f32()) / 2. - 30.),
            );
            let mut builder = PathBuilder::stroke(px(width));
            builder.move_to(from);
            builder.curve_to(to, control);
            paint::paint_path(builder.build().ok(), colors.stroke(index), ctx.window);
        }
    });
}

/// The plot-relative position of a row.
///
/// A one-dimensional mark has no value on its cross axis, so it sits where its
/// kind says: `dotX` in the frame's middle, everything else by its index.
pub fn position(
    prepared: &PreparedMark,
    ctx: &DrawContext<'_>,
    row: &PreparedRow,
) -> Option<Point<Pixels>> {
    let frame = ctx.frame;
    let placed = if prepared.mark.kind.is_x_only()
        && prepared.mark.kind.cross_axis() == CrossAxis::Anchor
    {
        let x = frame.x.scale.position(row.x_value())?;
        Some(point(px(x), px(frame.plot.size.height.as_f32() / 2.)))
    } else if prepared.mark.kind.is_y_only() && prepared.mark.kind.cross_axis() == CrossAxis::Anchor
    {
        let y = frame.y.scale.position(row.y_value())?;
        Some(point(px(frame.plot.size.width.as_f32() / 2.), px(y)))
    } else {
        frame.point(row.x_value(), row.y_value())
    }?;

    // A dodged row carries its packed position across the axis it spreads
    // along, which replaces that coordinate rather than offsetting it.
    let Some(across) = row.dodge_offset else {
        return Some(placed);
    };
    Some(if prepared.dodge_along_x {
        gpui::point(placed.x, px(across))
    } else {
        gpui::point(px(across), placed.y)
    })
}

/// Rows grouped by their `z` series, in first-occurrence order.
pub fn series_groups(prepared: &PreparedMark) -> Vec<Vec<(usize, &PreparedRow)>> {
    let mut order: Vec<Option<Value>> = Vec::new();
    let mut groups: HashMap<crate::value::ValueKey, Vec<(usize, &PreparedRow)>> = HashMap::new();
    for (index, row) in prepared.rows.iter().enumerate() {
        let key = row.z.as_ref().map(|z| z.key());
        if !order.iter().any(|existing| existing == &row.z) {
            order.push(row.z.clone());
        }
        let entry = groups
            .entry(key.clone().unwrap_or_else(|| Value::Null.key()))
            .or_default();
        entry.push((index, row));
    }
    order
        .into_iter()
        .filter_map(|z| groups.remove(&z.clone().unwrap_or(Value::Null).key()))
        .collect()
}

/// The width of a vertical bar: its band's width on a band scale, or a
/// fraction of the smallest gap between positions on a continuous one.
fn bar_width_y(frame: &Frame, prepared: &PreparedMark) -> f32 {
    match &frame.x.scale {
        PositionScale::Band(band) => band.band_width().max(1.),
        PositionScale::Point(point_scale) => {
            let step = point_scale.step();
            if step > 0. { step * 0.9 } else { 1. }
        }
        PositionScale::Continuous(_) => {
            let mut positions: Vec<f32> = prepared
                .rows
                .iter()
                .filter_map(|row| frame.x.scale.position(row.x_value()))
                .collect();
            positions.sort_by(f32::total_cmp);
            let gap = positions
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .filter(|gap| *gap > 0.)
                .reduce(f32::min)
                .unwrap_or_else(|| frame.plot.size.width.as_f32() / 20.);
            (gap * 0.9).max(1.)
        }
    }
}

fn bar_width_x(frame: &Frame, prepared: &PreparedMark) -> f32 {
    match &frame.y.scale {
        PositionScale::Band(band) => band.band_width().max(1.),
        PositionScale::Point(point_scale) => {
            let step = point_scale.step();
            if step > 0. { step * 0.9 } else { 1. }
        }
        PositionScale::Continuous(_) => {
            let mut positions: Vec<f32> = prepared
                .rows
                .iter()
                .filter_map(|row| frame.y.scale.position(row.y_value()))
                .collect();
            positions.sort_by(f32::total_cmp);
            let gap = positions
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .filter(|gap| *gap > 0.)
                .reduce(f32::min)
                .unwrap_or_else(|| frame.plot.size.height.as_f32() / 20.);
            (gap * 0.9).max(1.)
        }
    }
}

/// A path through `points`, connected by `curve`, revealed to `appear`.
///
/// The reveal shortens the path from its end, which is how a line draws in.
pub fn curve_path(
    points: &[Point<Pixels>],
    curve: Curve,
    width: f32,
    appear: f32,
) -> Option<gpui::Path<Pixels>> {
    if points.len() < 2 {
        return None;
    }
    let visible = ((points.len() as f32 * appear.clamp(0., 1.)) as usize)
        .max(1)
        .min(points.len());
    let points = &points[..visible];
    if points.len() < 2 {
        return None;
    }

    let mut builder = PathBuilder::stroke(px(width.max(0.1)));
    builder.move_to(points[0]);
    match curve {
        Curve::Linear => {
            for point in &points[1..] {
                builder.line_to(*point);
            }
        }
        Curve::StepAfter | Curve::StepBefore => {
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let corner = if curve == Curve::StepAfter {
                    point(b.x, a.y)
                } else {
                    point(a.x, b.y)
                };
                builder.line_to(corner);
                builder.line_to(b);
            }
        }
        Curve::Step => {
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let middle = px((a.x.as_f32() + b.x.as_f32()) / 2.);
                builder.line_to(point(middle, a.y));
                builder.line_to(point(middle, b.y));
            }
        }
        Curve::Natural => {
            let n = points.len();
            for i in 0..n - 1 {
                let p0 = if i == 0 { points[0] } else { points[i - 1] };
                let p1 = points[i];
                let p2 = points[i + 1];
                let p3 = if i + 2 < n {
                    points[i + 2]
                } else {
                    points[n - 1]
                };
                let c1 = point(
                    px(p1.x.as_f32() + (p2.x.as_f32() - p0.x.as_f32()) / 6.),
                    px(p1.y.as_f32() + (p2.y.as_f32() - p0.y.as_f32()) / 6.),
                );
                let c2 = point(
                    px(p2.x.as_f32() - (p3.x.as_f32() - p1.x.as_f32()) / 6.),
                    px(p2.y.as_f32() - (p3.y.as_f32() - p1.y.as_f32()) / 6.),
                );
                builder.cubic_bezier_to(p2, c1, c2);
            }
        }
        Curve::LinearClosed => {
            for point in &points[1..] {
                builder.line_to(*point);
            }
            builder.line_to(points[0]);
        }
        Curve::Basis | Curve::BasisOpen | Curve::BasisClosed => basis(&mut builder, points, curve),
        Curve::Cardinal | Curve::CardinalOpen | Curve::CardinalClosed => {
            cardinal(&mut builder, points, 0., curve)
        }
        Curve::CatmullRom | Curve::CatmullRomOpen | Curve::CatmullRomClosed => {
            catmull_rom(&mut builder, points, curve)
        }
        Curve::MonotoneX => monotone(&mut builder, points, true),
        Curve::MonotoneY => monotone(&mut builder, points, false),
        Curve::BumpX => bump(&mut builder, points, true),
        Curve::BumpY => bump(&mut builder, points, false),
        Curve::Bundle => bundle(&mut builder, points),
    }
    if curve.is_closed() && !matches!(curve, Curve::LinearClosed) {
        builder.line_to(points[0]);
    }
    builder.build().ok()
}

/// d3's `basis`: a cubic basis spline that passes near, but not through, the
/// data's points.
fn basis(builder: &mut PathBuilder, points: &[Point<Pixels>], curve: Curve) {
    let n = points.len();
    let open = curve.is_open();
    let (x0, y0) = (points[0].x.as_f32(), points[0].y.as_f32());
    let (x1, y1) = (points[1].x.as_f32(), points[1].y.as_f32());
    // An open spline starts on the first point; a closed one repeats it.
    if open {
        builder.line_to(points[0]);
    } else {
        builder.line_to(point(px((5. * x0 + x1) / 6.), px((5. * y0 + y1) / 6.)));
    }
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let (ax, ay) = (p0.x.as_f32(), p0.y.as_f32());
        let (bx, by) = (p1.x.as_f32(), p1.y.as_f32());
        let (cx, cy) = (p2.x.as_f32(), p2.y.as_f32());
        let c1 = point(px((2. * ax + bx) / 3.), px((2. * ay + by) / 3.));
        let c2 = point(px((ax + 2. * bx) / 3.), px((ay + 2. * by) / 3.));
        let end = point(px((ax + 4. * bx + cx) / 6.), px((ay + 4. * by + cy) / 6.));
        builder.cubic_bezier_to(end, c1, c2);
    }
    let last = points[n - 1];
    let prev = points[n - 2];
    if open {
        builder.line_to(last);
    } else {
        builder.line_to(point(
            px((5. * last.x.as_f32() + prev.x.as_f32()) / 6.),
            px((5. * last.y.as_f32() + prev.y.as_f32()) / 6.),
        ));
    }
    builder.line_to(last);
}

/// d3's `cardinal`: a cardinal spline whose tension shapes the corners.
fn cardinal(builder: &mut PathBuilder, points: &[Point<Pixels>], tension: f32, curve: Curve) {
    let n = points.len();
    let k = (1. - tension) / 6.;
    let open = curve.is_open();
    if open {
        builder.line_to(points[0]);
    }
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1]
        };
        let (ax, ay) = (p0.x.as_f32(), p0.y.as_f32());
        let (bx, by) = (p1.x.as_f32(), p1.y.as_f32());
        let (cx, cy) = (p2.x.as_f32(), p2.y.as_f32());
        let (dx, dy) = (p3.x.as_f32(), p3.y.as_f32());
        let c1 = point(px(bx + k * (cx - ax)), px(by + k * (cy - ay)));
        let c2 = point(px(cx + k * (bx - dx)), px(cy + k * (by - dy)));
        builder.cubic_bezier_to(p2, c1, c2);
    }
    if open {
        builder.line_to(points[n - 1]);
    }
}

/// d3's `catmull-rom`: a cardinal spline whose tangents come from the
/// neighbouring points' distances.
fn catmull_rom(builder: &mut PathBuilder, points: &[Point<Pixels>], curve: Curve) {
    let n = points.len();
    let open = curve.is_open();
    if open {
        builder.line_to(points[0]);
    }
    // Centripetal parameterization, d3's `alpha = 0.5`.
    const ALPHA: f32 = 0.5;
    let dist = |a: Point<Pixels>, b: Point<Pixels>| {
        let dx = b.x.as_f32() - a.x.as_f32();
        let dy = b.y.as_f32() - a.y.as_f32();
        (dx * dx + dy * dy).sqrt().powf(ALPHA)
    };
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1]
        };
        let (ax, ay) = (p0.x.as_f32(), p0.y.as_f32());
        let (bx, by) = (p1.x.as_f32(), p1.y.as_f32());
        let (cx, cy) = (p2.x.as_f32(), p2.y.as_f32());
        let (dx, dy) = (p3.x.as_f32(), p3.y.as_f32());
        let l01 = dist(p0, p1);
        let l12 = dist(p1, p2);
        let l23 = dist(p2, p3);
        let (mut c1x, mut c1y) = (bx, by);
        let (mut c2x, mut c2y) = (cx, cy);
        if l01 > f32::EPSILON {
            let a = 2. * l01 * l01 + 3. * l01 * l12 + l12 * l12;
            let den = 3. * l01 * (l01 + l12);
            c1x = (bx * a - ax * l12 * l12 + cx * l01 * l01) / den;
            c1y = (by * a - ay * l12 * l12 + cy * l01 * l01) / den;
        }
        if l23 > f32::EPSILON {
            let b = 2. * l23 * l23 + 3. * l23 * l12 + l12 * l12;
            let den = 3. * l23 * (l23 + l12);
            c2x = (cx * b + bx * l23 * l23 - dx * l12 * l12) / den;
            c2y = (cy * b + by * l23 * l23 - dy * l12 * l12) / den;
        }
        builder.cubic_bezier_to(p2, point(px(c1x), px(c1y)), point(px(c2x), px(c2y)));
    }
    if open {
        builder.line_to(points[n - 1]);
    }
}

/// d3's `monotone-x`/`monotone-y`: a Hermite spline whose tangents keep the
/// data's direction, so a curve never overshoots between points.
fn monotone(builder: &mut PathBuilder, points: &[Point<Pixels>], along_x: bool) {
    let n = points.len();
    let sign = |value: f32| if value < 0. { -1. } else { 1. };
    // `h` is the distance along the monotone axis, `v` the value across it.
    let deltas = |a: Point<Pixels>, b: Point<Pixels>| {
        if along_x {
            (b.x.as_f32() - a.x.as_f32(), b.y.as_f32() - a.y.as_f32())
        } else {
            (b.y.as_f32() - a.y.as_f32(), b.x.as_f32() - a.x.as_f32())
        }
    };
    let mut tangent = f32::NAN;
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1]
        };
        let (h_a, v_a) = deltas(p0, p1);
        let (h_b, v_b) = deltas(p1, p2);
        let (h_c, v_c) = deltas(p2, p3);
        let _ = (h_c, v_c);
        let s0 = if h_a != 0. { v_a / h_a } else { 0. };
        let s1 = if h_b != 0. { v_b / h_b } else { 0. };
        let p: f32 = if h_a + h_b != 0. {
            (s0 * h_b + s1 * h_a) / (h_a + h_b)
        } else {
            0.
        };
        let t1 = if s0 == 0. && s1 == 0. {
            0.
        } else {
            (sign(s0) + sign(s1)) * s0.abs().min(s1.abs()).min(0.5 * p.abs())
        };
        let t0 = if i == 0 {
            // The first segment gets a one-sided tangent.
            if h_a != 0. {
                (3. * v_a / h_a - t1) / 2.
            } else {
                t1
            }
        } else {
            tangent
        };
        let (dx, dy) = (p2.x.as_f32() - p1.x.as_f32(), p2.y.as_f32() - p1.y.as_f32());
        let (c1, c2) = if along_x {
            (
                point(
                    px(p1.x.as_f32() + dx / 3.),
                    px(p1.y.as_f32() + dx / 3. * t0),
                ),
                point(
                    px(p2.x.as_f32() - dx / 3.),
                    px(p2.y.as_f32() - dx / 3. * t1),
                ),
            )
        } else {
            (
                point(
                    px(p1.x.as_f32() + dy / 3. * t0),
                    px(p1.y.as_f32() + dy / 3.),
                ),
                point(
                    px(p2.x.as_f32() - dy / 3. * t1),
                    px(p2.y.as_f32() - dy / 3.),
                ),
            )
        };
        builder.cubic_bezier_to(p2, c1, c2);
        tangent = t1;
    }
}

/// d3's `bump-x`/`bump-y`: a Bézier whose control points share a coordinate,
/// so the curve leaves and enters each point horizontally or vertically.
fn bump(builder: &mut PathBuilder, points: &[Point<Pixels>], along_x: bool) {
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (c1, c2) = if along_x {
            let mid = (a.x.as_f32() + b.x.as_f32()) / 2.;
            (point(px(mid), a.y), point(px(mid), b.y))
        } else {
            let mid = (a.y.as_f32() + b.y.as_f32()) / 2.;
            (point(a.x, px(mid)), point(b.x, px(mid)))
        };
        builder.cubic_bezier_to(b, c1, c2);
    }
}

/// d3's `bundle`: a basis spline over points pulled toward the straight line
/// between the first and last, so the curve bends less than `basis`.
fn bundle(builder: &mut PathBuilder, points: &[Point<Pixels>]) {
    const BETA: f32 = 0.85;
    let n = points.len();
    if n < 2 {
        return;
    }
    let (x0, y0) = (points[0].x.as_f32(), points[0].y.as_f32());
    let (x1, y1) = (points[n - 1].x.as_f32(), points[n - 1].y.as_f32());
    // Each point moves `BETA` of the way from its own position toward the
    // line's, then the basis spline runs through the moved points.
    let pulled: Vec<Point<Pixels>> = points
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let t = index as f32 / (n - 1) as f32;
            point(
                px(BETA * p.x.as_f32() + (1. - BETA) * (x0 + t * (x1 - x0))),
                px(BETA * p.y.as_f32() + (1. - BETA) * (y0 + t * (y1 - y0))),
            )
        })
        .collect();
    basis(builder, &pulled, Curve::Basis);
}

/// The extent a mark contributes to the x domain.
pub fn x_values(prepared: &PreparedMark) -> Vec<Value> {
    prepared
        .rows
        .iter()
        .flat_map(|row| {
            // A row's extent is its interval when it has one: a stacked bar's
            // value has been rewritten into the interval, and the datum's own
            // channel still holds the pre-stack number.
            if row.x1.is_some() || row.x2.is_some() {
                return [row.x1.clone(), row.x2.clone()];
            }
            [Some(row.x_value().clone()), None]
        })
        .flatten()
        .filter(|value| !value.is_null())
        .collect()
}

/// The extent a mark contributes to the y domain.
pub fn y_values(prepared: &PreparedMark) -> Vec<Value> {
    prepared
        .rows
        .iter()
        .flat_map(|row| {
            if row.y1.is_some() || row.y2.is_some() {
                return [row.y1.clone(), row.y2.clone()];
            }
            [Some(row.y_value().clone()), None]
        })
        .flatten()
        .filter(|value| !value.is_null())
        .collect()
}
