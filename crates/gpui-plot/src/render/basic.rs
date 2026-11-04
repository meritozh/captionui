//! The primitive marks: dot, line, area, bar, rect, cell, rule, text, tick,
//! vector and link.
//!
//! Each mark projects its rows through the frame's scales and paints. Shapes
//! are clipped to the plot, so a value outside the domain stops at the frame's
//! edge the way Plot's marks do.

use std::collections::HashMap;

use gpui::{
    App, Bounds, ContentMask, Hsla, PathBuilder, Pixels, Point, SharedString, TextAlign, Window,
    point, px,
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

    /// The color of one row's series, for a mark that paints per group rather
    /// than per row: a line's stroke follows its `z` value through the plot's
    /// shared ordinal scale, the same one the legend reads.
    pub fn series_color(&self, prepared: &PreparedMark, row: &PreparedRow) -> Hsla {
        if let Some(channel) = prepared.mark.channels.stroke.as_ref() {
            return channel.read(&row.row);
        }
        match row.z.as_ref().and_then(|z| self.ordinal_scale().color(z)) {
            Some(color) => color,
            None => self.default_stroke(prepared),
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
        MarkKind::Cell | MarkKind::CellX | MarkKind::CellY => draw_cells(prepared, ctx),
        MarkKind::DifferenceX | MarkKind::DifferenceY => draw_differences(prepared, ctx),
        MarkKind::WaffleX | MarkKind::WaffleY => draw_waffle_bars(prepared, ctx),
        MarkKind::Hexgrid => draw_hexgrid(prepared, ctx),
        MarkKind::Raster => draw_raster(prepared, ctx),
        MarkKind::LinearRegressionX | MarkKind::LinearRegressionY => {
            crate::render::stat::draw_linear_regression(
                prepared,
                ctx,
                prepared.mark.kind == MarkKind::LinearRegressionX,
            )
        }
        MarkKind::BollingerX | MarkKind::BollingerY => crate::render::stat::draw_bollinger(
            prepared,
            ctx,
            prepared.mark.kind == MarkKind::BollingerX,
        ),
        MarkKind::RuleX => draw_rules_x(prepared, ctx),
        MarkKind::RuleY => draw_rules_y(prepared, ctx),
        MarkKind::Text | MarkKind::TextX | MarkKind::TextY => draw_texts(prepared, ctx),
        MarkKind::TickX => draw_ticks_x(prepared, ctx),
        MarkKind::TickY => draw_ticks_y(prepared, ctx),
        MarkKind::Vector | MarkKind::VectorX | MarkKind::VectorY => draw_vectors(prepared, ctx),
        MarkKind::Spike => draw_spikes(prepared, ctx),
        MarkKind::Arrow => draw_arrows(prepared, ctx),
        MarkKind::Link => draw_links(prepared, ctx),
        MarkKind::Tree => draw_tree(prepared, ctx),
        MarkKind::Image => draw_image(prepared, ctx),
        MarkKind::Geo => draw_geo(prepared, ctx),
        MarkKind::Tip => draw_tips(prepared, ctx),
        MarkKind::AxisX => draw_axis_x(prepared, ctx),
        MarkKind::AxisY => draw_axis_y(prepared, ctx),
        MarkKind::GridX => draw_grid_x(prepared, ctx),
        MarkKind::GridY => draw_grid_y(prepared, ctx),
        MarkKind::Frame => draw_frame(prepared, ctx),
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
            // A line's color is its series': a chart drawn as one line per
            // series has a `z` value on every row, and each line takes that
            // series' color rather than the mark's one palette entry.
            let color = match group.first() {
                Some((index, row)) => colors
                    .stroke
                    .as_ref()
                    .and_then(|colors| colors.get(*index).copied())
                    .unwrap_or_else(|| ctx.series_color(prepared, row)),
                None => continue,
            };
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
            // A one-dimensional cell covers the whole frame across the axis
            // it does not read, since that axis has no bands to fill. Its
            // extent starts at the frame's edge, not at the datum's index.
            let (x0, y0, width, height) = match prepared.mark.kind {
                MarkKind::CellX => {
                    let Some(x) = ctx.frame.x.scale.position(row.x_value()) else {
                        continue;
                    };
                    (
                        x,
                        0.,
                        cell_extent(&ctx.frame.x.scale),
                        ctx.frame.plot.size.height.as_f32(),
                    )
                }
                MarkKind::CellY => {
                    let Some(y) = ctx.frame.y.scale.position(row.y_value()) else {
                        continue;
                    };
                    (
                        0.,
                        y,
                        ctx.frame.plot.size.width.as_f32(),
                        cell_extent(&ctx.frame.y.scale),
                    )
                }
                _ => {
                    let (Some(x), Some(y)) = (
                        ctx.frame.x.scale.position(row.x_value()),
                        ctx.frame.y.scale.position(row.y_value()),
                    ) else {
                        continue;
                    };
                    (
                        x,
                        y,
                        cell_extent(&ctx.frame.x.scale),
                        cell_extent(&ctx.frame.y.scale),
                    )
                }
            };
            // A band's position is its center, so the cell straddles it.
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0 - width / 2.), px(y0 - height / 2.))),
                ctx.absolute(point(px(x0 + width / 2.), px(y0 + height / 2.))),
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

/// Plot's `vector`: a stem of `length` pixels through the datum, rotated by
/// `rotate`, pointing up the frame's y axis.
fn draw_vectors(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let width = style.stroke_width.max(1.);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(centre) = position(prepared, ctx, row) else {
                continue;
            };
            let length = prepared
                .mark
                .length
                .as_ref()
                .map(|channel| channel.read(&row.row))
                .unwrap_or(style.length);
            let rotate = prepared
                .mark
                .rotate
                .as_ref()
                .map(|channel| channel.read(&row.row))
                .unwrap_or(style.rotate);
            let color = colors.stroke(index);
            // The stem's ends, before rotation: the vector points up, so the
            // start is below the centre.
            let (start, end) = match style.anchor {
                crate::mark::VectorAnchor::Start => (0., -length),
                crate::mark::VectorAnchor::Middle => (length / 2., -length / 2.),
                crate::mark::VectorAnchor::End => (length, 0.),
            };
            let (sin, cos) = rotate.to_radians().sin_cos();
            let turn = |x: f32, y: f32| {
                point(
                    px(centre.x.as_f32() + x * cos - y * sin),
                    px(centre.y.as_f32() + x * sin + y * cos),
                )
            };
            let from = ctx.absolute(turn(0., start));
            let to = ctx.absolute(turn(0., end));
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

/// Draw a tree: one parent–child link per relation in the hierarchy, a dot per
/// node, laid out tidily across the plot.
///
/// The mark reads each row's `path` (a slash-separated chain), lays the paths
/// out into levels, and maps the tidy layout onto the plot's width and height.
/// Plot's `tree` is the same, though it defaults to a fixed aspect; here the
/// tree fills the plot, padded so its deepest leaves and longest labels have
/// room.
fn draw_tree(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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
        let margin = px(20.);
        let inner = Bounds::new(
            point(plot.origin.x + margin, plot.origin.y + margin),
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
            let from = node_at(tree.nodes[parent].x, tree.nodes[parent].y);
            let to = node_at(tree.nodes[child].x, tree.nodes[child].y);
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

/// Draw an image mark's rows, each painting the mark's image at its position.
///
/// Plot's `image` reads an `src` and paints it with a `width`/`height`; here
/// the mark carries one [`gpui::RenderImage`] shared across its rows, and each
/// row draws it at its scaled position with the image's own size.
fn draw_image(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let Some(image) = prepared.mark.image.clone() else {
        return;
    };
    if prepared.rows.is_empty() {
        return;
    }
    // The image's pixel size, once.
    let size = image.size(0);
    let pixel_size = gpui::size(px(size.width.0 as f32), px(size.height.0 as f32));
    ctx.clipped(|ctx| {
        for row in prepared.rows.iter() {
            // The image sits at the row's scaled position, its top-left there.
            let (Some(x), Some(y)) = (
                ctx.frame.x.scale.position(row.x_value()),
                ctx.frame.y.scale.position(row.y_value()),
            ) else {
                continue;
            };
            let origin = ctx.absolute(point(px(x), px(y)));
            let bounds = Bounds::new(origin, pixel_size);
            let image_bounds = Bounds::new(origin, pixel_size);
            // `paint_image` returns a Result; a failure paints nothing.
            let _ = ctx.window.paint_image(
                bounds,
                image_bounds,
                Default::default(),
                image.clone(),
                0,
                false,
            );
        }
    });
}

/// Draw a geo mark: project each row's lon/lat into the plot and connect them
/// as a path — Plot's `geo` over a sequence of points, where the plot's own
/// scales don't need to know about geography.
fn draw_geo(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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

/// Plot's `tip`: a label at the datum the mark selects, drawn permanently so
/// a chart can point at a value without a pointer.
fn draw_tips(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let font = gpui::font(ctx.theme.font_family.clone());
    let size = ctx.theme.label_size;
    let padding = 6.;
    let gap = 10.;

    // Tips are not clipped to the plot: a tip on the topmost point sits above
    // it, and Plot draws the whole label.
    for row in prepared.rows.iter() {
        let Some(placed) = position(prepared, ctx, row) else {
            continue;
        };
        let text = prepared
            .mark
            .channels
            .text
            .as_ref()
            .map(|channel| channel.read(&row.row).label())
            .unwrap_or_else(|| row.y_value().label());
        if text.is_empty() {
            continue;
        }
        let fill = match &prepared.mark.channels.fill {
            Some(channel) => channel.read(&row.row),
            None => {
                let color = prepared.mark.style.fill;
                if color == crate::mark::MarkStyle::default().fill {
                    ctx.theme.mark_color
                } else {
                    color
                }
            }
        };
        // The box takes the mark's color, so its label reads in whichever of
        // the theme's two tooltip colors contrasts with it.
        let ink = if contrast_ratio(fill, ctx.theme.tooltip_foreground)
            >= contrast_ratio(fill, ctx.theme.tooltip_background)
        {
            ctx.theme.tooltip_foreground
        } else {
            ctx.theme.tooltip_background
        };
        let label = Label::new(
            text,
            size,
            ink,
            gpui::FontWeight::MEDIUM,
            font.clone(),
            ctx.window,
        );
        let at = ctx.absolute(placed);
        // Above the datum, or below it when the tip would run off the top.
        let above = at.y - px(gap + size.as_f32() + padding * 2.);
        let top = if above < ctx.frame.plot.origin.y {
            at.y + px(gap)
        } else {
            above
        };
        let box_bounds = Bounds::new(
            point(at.x - px(label.width.as_f32() / 2. + padding), top),
            gpui::size(label.width + px(padding * 2.), size + px(padding * 2.)),
        );
        paint::paint_rect(box_bounds, 4., fill, ctx.window);
        let text_at = point(
            box_bounds.origin.x + px(padding),
            box_bounds.origin.y + px(padding),
        );
        label.paint_at(text_at, TextAlign::Left, None, ctx.window, ctx.cx);
    }
}

/// Plot's `axisX`: an x axis drawn as a mark, so it can sit inside the frame
/// or take a color of its own rather than the plot-level axis.
fn draw_axis_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let color = furniture_color(prepared, ctx.theme.axis);
    let frame = ctx.frame;
    let plot = frame.plot;
    // The axis line along the bottom of the plot, with a tick at each x tick.
    ctx.clipped(|ctx| {
        let baseline = plot.origin.y + plot.size.height;
        paint::paint_stroke(
            &[
                point(plot.origin.x, baseline),
                point(plot.origin.x + plot.size.width, baseline),
            ],
            ctx.theme.line_width.as_f32(),
            color,
            ctx.window,
        );
        for tick in frame.x.ticks.iter() {
            let x = plot.origin.x + px(tick.position);
            paint::paint_stroke(
                &[point(x, baseline), point(x, baseline - px(4.))],
                1.,
                color,
                ctx.window,
            );
        }
    });
}

/// Plot's `axisY`: a y axis drawn as a mark.
fn draw_axis_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let color = furniture_color(prepared, ctx.theme.axis);
    let frame = ctx.frame;
    let plot = frame.plot;
    ctx.clipped(|ctx| {
        paint::paint_stroke(
            &[
                point(plot.origin.x, plot.origin.y),
                point(plot.origin.x, plot.origin.y + plot.size.height),
            ],
            ctx.theme.line_width.as_f32(),
            color,
            ctx.window,
        );
        for tick in frame.y.ticks.iter() {
            let y = plot.origin.y + px(tick.position);
            paint::paint_stroke(
                &[point(plot.origin.x, y), point(plot.origin.x + px(4.), y)],
                1.,
                color,
                ctx.window,
            );
        }
    });
}

/// Plot's `gridX`: a vertical grid line at each x tick.
fn draw_grid_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let color = grid_color(prepared, ctx);
    let frame = ctx.frame;
    let plot = frame.plot;
    ctx.clipped(|ctx| {
        for tick in frame.x.ticks.iter() {
            let x = plot.origin.x + px(tick.position);
            paint::paint_stroke(
                &[
                    point(x, plot.origin.y),
                    point(x, plot.origin.y + plot.size.height),
                ],
                ctx.theme.line_width.as_f32(),
                color,
                ctx.window,
            );
        }
    });
}

/// Plot's `gridY`: a horizontal grid line at each y tick.
fn draw_grid_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let color = grid_color(prepared, ctx);
    let frame = ctx.frame;
    let plot = frame.plot;
    ctx.clipped(|ctx| {
        for tick in frame.y.ticks.iter() {
            let y = plot.origin.y + px(tick.position);
            paint::paint_stroke(
                &[
                    point(plot.origin.x, y),
                    point(plot.origin.x + plot.size.width, y),
                ],
                ctx.theme.line_width.as_f32(),
                color,
                ctx.window,
            );
        }
    });
}

/// The color a furniture mark draws in: the stroke the mark sets, else the
/// theme's color for that piece of furniture.
fn furniture_color(prepared: &PreparedMark, fallback: Hsla) -> Hsla {
    match &prepared.mark.channels.stroke {
        Some(channel) => channel.read(&crate::table::Table::empty()),
        None => {
            let color = prepared.mark.style.stroke;
            // A mark that never set a stroke still carries the style's default,
            // so the theme's color for this piece of furniture wins unless the
            // mark asked for one: the defaults are what "no stroke" looks like.
            if color == crate::mark::MarkStyle::default().stroke {
                fallback
            } else {
                color
            }
        }
    }
}
fn grid_color(prepared: &PreparedMark, ctx: &DrawContext<'_>) -> Hsla {
    furniture_color(prepared, ctx.theme.grid)
}

/// Plot's `frame`: the plot's outline, drawn as a mark so a chart can turn the
/// frame on without turning on the axes.
fn draw_frame(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let color = grid_color(prepared, ctx);
    let plot = ctx.frame.plot;
    let width = prepared.mark.style.stroke_width;
    ctx.clipped(|ctx| {
        let corners = [
            point(plot.origin.x, plot.origin.y),
            point(plot.origin.x + plot.size.width, plot.origin.y),
            point(
                plot.origin.x + plot.size.width,
                plot.origin.y + plot.size.height,
            ),
            point(plot.origin.x, plot.origin.y + plot.size.height),
            point(plot.origin.x, plot.origin.y),
        ];
        paint::paint_stroke(&corners, width, color, ctx.window);
    });
}

/// The WCAG contrast ratio between two colors' relative luminances, so a
/// label can pick the ink that reads against the color behind it.
fn contrast_ratio(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn luminance(color: Hsla) -> f32 {
    let rgba = gpui::Rgba::from(color);
    let channel = |value: f32| {
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgba.r) + 0.7152 * channel(rgba.g) + 0.0722 * channel(rgba.b)
}

/// Plot's `spike`: a triangle whose point sits at the datum, `length` tall.
/// Plot's `spike`: a vector preset that draws a triangle at the datum rather
/// than a stem with a head.
fn draw_spikes(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(centre) = position(prepared, ctx, row) else {
                continue;
            };
            let length = prepared
                .mark
                .length
                .as_ref()
                .map(|channel| channel.read(&row.row))
                .unwrap_or(style.length);
            let color = colors.stroke(index);
            // Plot's spike shape: an apex `length` above the datum, with a base
            // half as wide as a dot's radius each side.
            let half = style.radius;
            let apex = ctx.absolute(point(px(centre.x.as_f32()), px(centre.y.as_f32() - length)));
            let left = ctx.absolute(point(px(centre.x.as_f32() - half), px(centre.y.as_f32())));
            let right = ctx.absolute(point(px(centre.x.as_f32() + half), px(centre.y.as_f32())));
            paint::paint_fill(
                &[apex, left, right],
                paint::alpha(color, style.fill_opacity * ctx.appear),
                ctx.window,
            );
            paint::paint_stroke(&[left, apex, right], style.stroke_width, color, ctx.window);
        }
    });
}

fn draw_arrows(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let width = style.stroke_width.max(1.);
    let bend = style.bend.clamp(-90., 90.);
    // Plot's head: two wings of `head_length` pixels, `head_angle` degrees
    // apart, drawn from the tip.
    let wing_angle = (style.head_angle * std::f32::consts::PI / 180.) / 2.;
    let wing_scale = style.head_length / 1.5;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let (start_x, start_y) = (
                row.x1.clone().unwrap_or_else(|| row.x_value().clone()),
                row.y1.clone().unwrap_or_else(|| row.y_value().clone()),
            );
            let (end_x, end_y) = (
                row.x2.clone().unwrap_or_else(|| row.x_value().clone()),
                row.y2.clone().unwrap_or_else(|| row.y_value().clone()),
            );
            let (Some(from), Some(to)) = (
                ctx.frame.point(&start_x, &start_y),
                ctx.frame.point(&end_x, &end_y),
            ) else {
                continue;
            };
            let from = ctx.absolute(from);
            let to = ctx.absolute(to);
            let color = colors.stroke(index);
            let line = (to.x.as_f32() - from.x.as_f32()).hypot(to.y.as_f32() - from.y.as_f32());
            if line < 1. {
                continue;
            }

            let points = if bend != 0. {
                arc_points(from, to, bend)
            } else {
                vec![from, to]
            };
            paint::paint_stroke(&points, width, color, ctx.window);
            // The head points along the path where it arrives, so a bent arrow
            // carries a head tangent to its arc.
            let last = &points[points.len() - 1];
            let previous = &points[points.len() - 2];
            let end_angle = (last.y.as_f32() - previous.y.as_f32())
                .atan2(last.x.as_f32() - previous.x.as_f32());

            // The wings are never longer than a third of the shaft, so a short
            // arrow keeps a proportional head.
            let head = (wing_scale * width).min(line / 3.);
            if head <= 0. {
                continue;
            }
            let tip = to;
            let wing = |side: f32| {
                let angle = end_angle + side * wing_angle;
                point(
                    px(tip.x.as_f32() - head * angle.cos()),
                    px(tip.y.as_f32() - head * angle.sin()),
                )
            };
            // One path through the tip, so the two wings meet there rather
            // than joining each other across the head.
            let wings = [wing(-1.), tip, wing(1.)];
            paint::paint_stroke(&wings, width, color, ctx.window);
        }
    });
}

/// The points of the circular arc from `from` to `to` that leaves `from` at
/// `bend` degrees to the chord.
///
/// Plot's arrow bends by sweeping a circle through both ends: the arc's radius
/// follows from the bend angle, and a positive angle curves clockwise on
/// screen.
fn arc_points(from: Point<Pixels>, to: Point<Pixels>, bend: f32) -> Vec<Point<Pixels>> {
    let (ax, ay) = (from.x.as_f32(), from.y.as_f32());
    let (bx, by) = (to.x.as_f32(), to.y.as_f32());
    let (dx, dy) = (bx - ax, by - ay);
    let chord = dx.hypot(dy);
    let bend_angle = bend.to_radians();
    // The radius of the circle whose tangent leaves `from` at `bend_angle` to
    // the chord: `hypot(chord / tan(bend_angle), chord) / 2`.
    let radius = (chord / bend_angle.tan()).hypot(chord) / 2.;
    // The circle's centre, on the side the bend curves towards.
    let sign = if bend_angle > 0. { 1. } else { -1. };
    let k = sign * (radius * radius - chord * chord / 4.).max(0.).sqrt() / chord;
    let centre = ((ax + bx) / 2. - dy * k, (ay + by) / 2. + dx * k);

    let start_angle = (ay - centre.1).atan2(ax - centre.0);
    let end_angle = (by - centre.1).atan2(bx - centre.0);
    // Sweep the short way round, in the direction the bend asks for.
    let mut delta = (end_angle - start_angle).rem_euclid(std::f32::consts::TAU);
    if sign < 0. {
        delta -= std::f32::consts::TAU;
    }

    let steps = 24;
    (0..=steps)
        .map(|step| {
            let angle = start_angle + delta * step as f32 / steps as f32;
            point(
                px(centre.0 + radius * angle.cos()),
                px(centre.1 + radius * angle.sin()),
            )
        })
        .collect()
}

/// Plot's `differenceX`/`differenceY`: the gap between two series as two
/// areas, one filled where the second is above the first and one where it is
/// below, with a line along the second.
fn draw_differences(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    // `differenceY` compares two y values across x, so it runs along x; its
    // `differenceX` twin compares two x values down the y axis.
    let along_x = prepared.mark.kind == MarkKind::DifferenceY;
    // Plot's own fills: green where the second series is above the first,
    // blue where it is below.
    let opacity = prepared.mark.style.fill_opacity * ctx.appear;
    let positive = paint::alpha(crate::scale::hsla_from_hex(0x3ca951), opacity);
    let negative = paint::alpha(crate::scale::hsla_from_hex(0x4269d0), opacity);
    let line = ctx.default_stroke(prepared);
    let width = prepared.mark.style.stroke_width.max(1.);

    // Each row is one position with two values: the point on the axis the
    // mark runs along, and the two series' positions across it.
    let placed: Vec<(f32, f32, f32)> = prepared
        .rows
        .iter()
        .filter_map(|row| {
            Some((
                position_along(ctx, row, along_x)?,
                position_across(ctx, row, along_x, true)?,
                position_across(ctx, row, along_x, false)?,
            ))
        })
        .collect();

    // A span is positive where the second series is the larger one. The two
    // axes invert differently on screen: a larger value sits *higher* on y and
    // further right on x, so the comparison flips with the axis.
    let sign =
        |(_, first, second): &(f32, f32, f32)| difference_is_positive(along_x, *first, *second);

    let origin = ctx.frame.plot.origin;
    ctx.clipped(|ctx| {
        let at = |along: f32, across: f32| {
            origin
                + if along_x {
                    point(px(along), px(across))
                } else {
                    point(px(across), px(along))
                }
        };

        // Consecutive spans of one sign are one area, so a run of them fills as
        // a single path: filling each span on its own leaves a seam where two
        // quads meet.
        let mut start = 0;
        while start + 1 < placed.len() {
            let positive_run = sign(&placed[start + 1]);
            let mut end = start + 1;
            while end + 1 < placed.len() && sign(&placed[end + 1]) == positive_run {
                end += 1;
            }
            let color = if positive_run { positive } else { negative };
            let mut polygon: Vec<Point<Pixels>> = (start..=end)
                .map(|index| at(placed[index].0, placed[index].1))
                .collect();
            polygon.extend(
                (start..=end)
                    .rev()
                    .map(|index| at(placed[index].0, placed[index].2)),
            );
            paint::paint_fill(&polygon, color, ctx.window);
            start = end;
        }

        // The second series' line runs over the areas, as one path so it has no
        // breaks at the sign changes.
        let line_path: Vec<Point<Pixels>> = placed
            .iter()
            .map(|(along, _, second)| at(*along, *second))
            .collect();
        paint::paint_stroke(&line_path, width, line, ctx.window);
    });
}

/// Whether a difference span's second series is the larger one, given both
/// series' positions across the axis the mark runs along.
///
/// The screen inverts the axes differently: a larger value sits higher on y but
/// further right on x, so the same comparison reads opposite on each.
fn difference_is_positive(along_x: bool, first: f32, second: f32) -> bool {
    if along_x {
        second < first
    } else {
        second > first
    }
}

/// A row's position along the axis a difference mark runs along.
fn position_along(ctx: &DrawContext<'_>, row: &PreparedRow, along_x: bool) -> Option<f32> {
    if along_x {
        ctx.frame.x.scale.position(row.x_value())
    } else {
        ctx.frame.y.scale.position(row.y_value())
    }
}

/// A row's first or second value, on the axis across the difference.
fn position_across(
    ctx: &DrawContext<'_>,
    row: &PreparedRow,
    along_x: bool,
    first: bool,
) -> Option<f32> {
    let value = if along_x {
        if first {
            row.y1.clone().or_else(|| Some(row.y_value().clone()))
        } else {
            row.y2.clone().or_else(|| Some(row.y_value().clone()))
        }
    } else if first {
        row.x1.clone().or_else(|| Some(row.x_value().clone()))
    } else {
        row.x2.clone().or_else(|| Some(row.x_value().clone()))
    };
    let scale = if along_x {
        &ctx.frame.y.scale
    } else {
        &ctx.frame.x.scale
    };
    scale.position(value.as_ref()?)
}

/// Plot's `waffleX`/`waffleY`: a bar's interval divided into unit cells, so a
/// value reads as a count of squares rather than as a length.
///
/// The cells wrap: the bar's thickness sets how many fit across it, and the
/// interval's length fills rows (or columns) of that many cells. A bar whose
/// value is not a whole number of cells ends with a partial row.
fn draw_waffle_bars(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let along_x = prepared.mark.kind == MarkKind::WaffleX;
    let unit = style.unit.max(f32::MIN_POSITIVE);
    let gap = style.gap;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(centre) = (if along_x {
                ctx.frame.y.scale.position(row.y_value())
            } else {
                ctx.frame.x.scale.position(row.x_value())
            }) else {
                continue;
            };
            let interval = if along_x {
                row.x_interval()
            } else {
                row.y_interval()
            };
            let Some((start, end)) = interval else {
                continue;
            };
            let (Some(a), Some(b)) = (
                if along_x {
                    ctx.frame.x.scale.position(&start)
                } else {
                    ctx.frame.y.scale.position(&start)
                },
                if along_x {
                    ctx.frame.x.scale.position(&end)
                } else {
                    ctx.frame.y.scale.position(&end)
                },
            ) else {
                continue;
            };

            // One unit along the value axis, in pixels: the interval's
            // pixel length divided by how many units it spans.
            let span = (end.as_number().unwrap_or(0.) - start.as_number().unwrap_or(0.)).abs();
            let scale = (b - a).abs() / (span / unit as f64) as f32;
            if !scale.is_finite() || scale <= 0. {
                continue;
            }
            let bar = if along_x {
                bar_width_x(ctx.frame, prepared)
            } else {
                bar_width_y(ctx.frame, prepared)
            };
            // How many cells fit across the bar, Plot's `multiple`: the bar
            // stays roughly as wide as its cells are tall.
            let across = ((bar / scale).sqrt().floor() as usize).max(1);
            // A cell is as wide as the bar allows and one row tall, where a
            // row holds `across` units.
            let cell_width = (bar / across as f32).min(scale * across as f32);
            let cell_height = scale * across as f32;
            let low = a.min(b);
            let count = ((b - a).abs() / scale).round().max(1.) as usize;
            let color = colors.fill(index);

            for step in 0..count {
                let line = step / across;
                let column = step % across;
                let (x, y, w, h) = if along_x {
                    (
                        low + line as f32 * cell_height,
                        centre - bar / 2. + column as f32 * cell_width,
                        cell_width - gap,
                        cell_height - gap,
                    )
                } else {
                    (
                        centre - bar / 2. + column as f32 * cell_width,
                        low + line as f32 * cell_height,
                        cell_width - gap,
                        cell_height - gap,
                    )
                };
                let rect = paint::rect_between(
                    ctx.absolute(point(px(x), px(y))),
                    ctx.absolute(point(px(x + w), px(y + h))),
                );
                paint::paint_rect(
                    rect,
                    style.corner_radius,
                    paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                    ctx.window,
                );
            }
        }
    });
}

/// Plot's `hexgrid`: a static lattice of hexagons under the data, so a hexbin
/// chart's bins sit on a visible grid. The mark reads no data.
fn draw_hexgrid(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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

/// Plot's `raster`: a dense pixel grid, one rectangle per datum, colored from
/// its fill. The grid comes from each row's position and the nearest gap
/// between positions, so a row's rectangle covers the cell around it.
fn draw_raster(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    if prepared.rows.is_empty() {
        return;
    }

    // The raster's cells are the gaps between adjacent positions on each
    // axis; a single row spans the frame.
    let cell = |values: Vec<Option<f32>>, extent: f32| -> f32 {
        let mut positions: Vec<f32> = values.into_iter().flatten().collect();
        positions.sort_by(f32::total_cmp);
        let gap = positions
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .filter(|gap| *gap > 0.)
            .reduce(f32::min)
            .unwrap_or(extent);
        gap.max(1.)
    };
    let width = cell(
        prepared
            .rows
            .iter()
            .map(|row| ctx.frame.x.scale.position(row.x_value()))
            .collect(),
        ctx.frame.plot.size.width.as_f32(),
    );
    let height = cell(
        prepared
            .rows
            .iter()
            .map(|row| ctx.frame.y.scale.position(row.y_value()))
            .collect(),
        ctx.frame.plot.size.height.as_f32(),
    );

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let (Some(x), Some(y)) = (
                ctx.frame.x.scale.position(row.x_value()),
                ctx.frame.y.scale.position(row.y_value()),
            ) else {
                continue;
            };
            let rect = paint::rect_between(
                ctx.absolute(point(px(x - width / 2.), px(y - height / 2.))),
                ctx.absolute(point(px(x + width / 2.), px(y + height / 2.))),
            );
            paint::paint_rect(
                rect,
                0.,
                paint::alpha(colors.fill(index), ctx.row_fill_opacity(prepared, row)),
                ctx.window,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_difference_fills_where_the_second_series_leads() {
        // A larger value is higher on y and further right on x, so "the second
        // series is larger" is the smaller screen position on y and the larger
        // one on x.
        assert!(difference_is_positive(true, 300., 200.));
        assert!(!difference_is_positive(true, 200., 300.));
        assert!(difference_is_positive(false, 100., 200.));
        assert!(!difference_is_positive(false, 200., 100.));
    }

    #[test]
    fn a_waffle_wraps_its_cells_across_the_bar() {
        // Plot's layout: the number of cells across is the largest square that
        // fits the bar's thickness, and the cell is no wider than that square.
        let across = ((60_f32 / 10_f32).sqrt().floor() as usize).max(1);
        assert_eq!(across, 2);
        let cell_width = (60_f32 / across as f32).min(10_f32 * across as f32);
        assert_eq!(cell_width, 20.);
        let cell_height = 10_f32 * across as f32;
        assert_eq!(cell_height, 20.);
        // A value of 5 units fills two rows of two cells and a partial third.
        let count = (50_f32 / 10_f32).round() as usize;
        assert_eq!(count, 5);
    }
}
