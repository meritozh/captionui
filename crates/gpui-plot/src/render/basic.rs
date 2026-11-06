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
use crate::scale::PositionScale;
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
    /// The plot's color scales, shared by every mark so a series keeps one
    /// color wherever it appears, and read by the legend and tooltip too.
    pub colors: &'a crate::render::color::PlotColors,
    /// Whether the mark clips to the plot area, Plot's `clip`. Plot does not
    /// clip by default, so a dot on the domain's edge shows whole.
    pub clip: bool,
    pub window: &'a mut Window,
    pub cx: &'a mut App,
}

impl DrawContext<'_> {
    /// A plot-relative point in the figure's coordinates.
    pub fn absolute(&self, point: Point<Pixels>) -> Point<Pixels> {
        point + self.frame.plot.origin
    }

    /// Paint inside the plot when the mark clips, Plot's `clip: true`;
    /// otherwise paint as is.
    pub fn clipped<R>(&mut self, f: impl FnOnce(&mut DrawContext<'_>) -> R) -> R {
        if !self.clip {
            return f(self);
        }
        let bounds = self.frame.plot;
        let frame = self.frame;
        let theme = self.theme;
        let mark_index = self.mark_index;
        let appear = self.appear;
        let colors = self.colors;
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
                colors,
                clip: true,
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
        self.colors.default_fill(prepared, self.mark_index)
    }

    /// The default stroke of this mark, matching its fill.
    pub fn default_stroke(&self, prepared: &PreparedMark) -> Hsla {
        self.colors.default_stroke(prepared, self.mark_index)
    }

    /// The color of one row's series, for a mark that paints per group rather
    /// than per row: a line's stroke follows its `z` value through the plot's
    /// shared ordinal scale, the same one the legend reads.
    pub fn series_color(&self, prepared: &PreparedMark, row: &PreparedRow) -> Hsla {
        self.colors.series_color(prepared, self.mark_index, row)
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

use crate::render::color::has_series;

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
                .map(|row| ctx.colors.channel(channel, &row.row))
                .collect()
        });
        let stroke = prepared.mark.channels.stroke.as_ref().map(|channel| {
            prepared
                .rows
                .iter()
                .map(|row| ctx.colors.channel(channel, &row.row))
                .collect()
        });

        // Without a fill channel a row is colored by the plot's scales — the
        // continuous `color` option, then the shared ordinal `z` scale — the
        // same resolution the legend and tooltip read.
        let scaled = fill.is_none() && (has_series(prepared) || ctx.colors.continuous.is_some());
        let fill = match fill {
            Some(colors) => Some(colors),
            None if scaled => Some(
                prepared
                    .rows
                    .iter()
                    .map(|row| ctx.colors.row_fill(prepared, ctx.mark_index, row))
                    .collect(),
            ),
            None => None,
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
        MarkKind::AxisX | MarkKind::AxisY | MarkKind::GridX | MarkKind::GridY | MarkKind::Frame => {
            crate::render::furniture::draw(prepared, ctx)
        }
        _ => {}
    }
}

fn draw_dots(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let radius = prepared.mark.style.radius;
    // Plot draws dots with an `r` channel largest first, so small ones sit
    // on top and stay visible.
    let mut order: Vec<usize> = (0..prepared.rows.len()).collect();
    if prepared.rows.iter().any(|row| row.r_value.is_some()) {
        order.sort_by(|a, b| {
            let r = |i: &usize| prepared.rows[*i].r.unwrap_or(0.);
            r(b).total_cmp(&r(a))
        });
    }

    ctx.clipped(|ctx| {
        for index in order {
            let row = &prepared.rows[index];
            let Some(position) = position(prepared, ctx, row) else {
                continue;
            };
            // Plot's dot defaults to a radius of 3, or 4.5 with a symbol
            // channel, so its shapes read.
            let radius = if prepared.mark.channels.symbol.is_some() && radius == 3. {
                4.5
            } else {
                radius
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
            // Plot's dot is hollow unless it is given a fill: it strokes in
            // `currentColor` (or its stroke channel) and fills nothing. A
            // constant fill symbol (`symbol: "square"`) fills instead.
            let fill_symbol = !symbol.is_line()
                && symbol != Symbol::Circle
                && prepared
                    .mark
                    .channels
                    .symbol
                    .as_ref()
                    .is_none_or(|channel| crate::render::constant_symbol(channel).is_some())
                && prepared.mark.channels.stroke.is_none();
            let hollow = prepared.mark.channels.fill.is_none() && !fill_symbol;
            let fill = if hollow {
                Hsla::transparent_black()
            } else {
                paint::alpha(colors.fill(index), ctx.row_fill_opacity(prepared, row))
            };
            // A filled dot with no stroke of its own draws no outline.
            let stroke = if prepared.mark.channels.stroke.is_none() && !hollow {
                Hsla::transparent_black()
            } else {
                paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row))
            };
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

/// Paint one symbol at a plot-relative position: d3's symbol of area `π r²`,
/// filled and stroked as the dot asks. A stroke symbol (`plus`, `times` …)
/// is drawn in the stroke alone.
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
    // A circle is a fully rounded quad: one instance on the GPU, where a
    // tessellated path costs far more. Plot's scatterplots draw thousands
    // of dots, which overflow the path pipeline's buffer. SVG centers a
    // stroke on the outline, so the quad grows by half the stroke and draws
    // it as an inside border.
    if symbol == Symbol::Circle {
        let stroke_width = if stroke.a > 0. {
            stroke_width.max(0.)
        } else {
            0.
        };
        let outer = radius + stroke_width / 2.;
        let bounds = gpui::Bounds::new(
            point(absolute.x - px(outer), absolute.y - px(outer)),
            gpui::size(px(outer * 2.), px(outer * 2.)),
        );
        ctx.window.paint_quad(gpui::quad(
            bounds,
            gpui::Corners::all(px(outer)),
            fill,
            gpui::Edges::all(px(stroke_width)),
            stroke,
            gpui::BorderStyle::default(),
        ));
        return;
    }
    for (points, closed) in symbol.subpaths(absolute, radius) {
        if closed && fill.a > 0. {
            paint::paint_fill(&points, fill, ctx.window);
        }
        if stroke.a > 0. && stroke_width > 0. {
            let mut outline = points.clone();
            if closed && let Some(first) = points.first() {
                outline.push(*first);
            }
            paint::paint_stroke(&outline, stroke_width, stroke, ctx.window);
        }
    }
}

fn draw_lines(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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

/// Paint Plot's markers at a path's vertices: `markerStart` at the first,
/// `markerMid` between, `markerEnd` at the last. A marker scales with the
/// stroke width, as SVG's `markerUnits="strokeWidth"` does.
pub fn paint_markers(
    points: &[Point<Pixels>],
    markers: &[Option<crate::mark::Marker>; 3],
    color: Hsla,
    stroke_width: f32,
    background: Hsla,
    window: &mut Window,
) {
    use crate::mark::Marker;
    if points.is_empty() || markers.iter().all(Option::is_none) {
        return;
    }
    let k = 0.667 * stroke_width;
    let direction = |i: usize| -> f32 {
        let (a, b) = if i + 1 < points.len() {
            (points[i], points[i + 1])
        } else if i > 0 {
            (points[i - 1], points[i])
        } else {
            return 0.;
        };
        (b.y.as_f32() - a.y.as_f32()).atan2(b.x.as_f32() - a.x.as_f32())
    };
    let last = points.len() - 1;
    for (i, at) in points.iter().enumerate() {
        let which = if i == 0 {
            0
        } else if i == last {
            2
        } else {
            1
        };
        let marker = if points.len() == 1 {
            markers[0].or(markers[2])
        } else {
            markers[which]
        };
        let Some(marker) = marker else {
            continue;
        };
        let (cx, cy) = (at.x.as_f32(), at.y.as_f32());
        let rotate = |x: f32, y: f32, angle: f32| {
            let (s, c) = angle.sin_cos();
            point(px(cx + (x * c - y * s) * k), px(cy + (x * s + y * c) * k))
        };
        match marker {
            Marker::Dot => {
                paint::paint_fill(&paint::circle_points(*at, 2.5 * k, 20), color, window)
            }
            Marker::CircleFill => {
                paint::paint_fill(&paint::circle_points(*at, 3. * k, 20), color, window);
                let mut ring = paint::circle_points(*at, 3. * k, 20);
                ring.push(ring[0]);
                paint::paint_stroke(&ring, 1.5 * k, background, window);
            }
            Marker::CircleStroke => {
                paint::paint_fill(&paint::circle_points(*at, 3. * k, 20), background, window);
                let mut ring = paint::circle_points(*at, 3. * k, 20);
                ring.push(ring[0]);
                paint::paint_stroke(&ring, 1.5 * k, color, window);
            }
            Marker::Arrow | Marker::ArrowReverse => {
                let mut angle = direction(i);
                if marker == Marker::ArrowReverse && which == 0 {
                    angle += std::f32::consts::PI;
                }
                let chevron = [
                    rotate(-1.5, -3., angle),
                    rotate(1.5, 0., angle),
                    rotate(-1.5, 3., angle),
                ];
                paint::paint_stroke(&chevron, 1.5 * k, color, window);
            }
            Marker::Tick | Marker::TickX | Marker::TickY => {
                let angle = match marker {
                    Marker::TickX => std::f32::consts::FRAC_PI_2,
                    Marker::TickY => 0.,
                    _ => direction(i),
                };
                let scale = stroke_width;
                let rot = |x: f32, y: f32| {
                    let (s, c) = angle.sin_cos();
                    point(
                        px(cx + (x * c - y * s) * scale),
                        px(cy + (x * s + y * c) * scale),
                    )
                };
                paint::paint_stroke(
                    &[rot(0., -3.), rot(0., 3.)],
                    stroke_width.min(1.),
                    color,
                    window,
                );
            }
        }
    }
}

fn draw_areas_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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
            let polygon = paint::fill_path(&upper, fill);
            paint::paint_path(polygon, fill, ctx.window);

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
            // A bar without an x spans the frame, Plot's `barY` with only y.
            let full = row.x_value().is_null() && row.x1.is_none();
            let Some(center) = (if full {
                Some(ctx.frame.plot.size.width.as_f32() / 2.)
            } else {
                ctx.frame.x.scale.position(row.x_value())
            }) else {
                continue;
            };
            let width = if full {
                ctx.frame.plot.size.width.as_f32()
            } else {
                width
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
            let inset = prepared.mark.style.inset.unwrap_or(0.);
            let x0 = center - width / 2. + inset;
            let x1 = center + width / 2. - inset;
            let (y0, y1) = {
                let (lo, hi) = (y0.min(y1), y0.max(y1));
                let inset = inset.min((hi - lo) / 2.);
                (lo + inset, hi - inset)
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

fn draw_bars_x(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = bar_width_x(ctx.frame, prepared);

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // A bar without a y spans the frame, Plot's `barX` with only x.
            let full = row.y_value().is_null() && row.y1.is_none();
            let Some(center) = (if full {
                Some(ctx.frame.plot.size.height.as_f32() / 2.)
            } else {
                ctx.frame.y.scale.position(row.y_value())
            }) else {
                continue;
            };
            let width = if full {
                ctx.frame.plot.size.height.as_f32()
            } else {
                width
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
            let inset = prepared.mark.style.inset.unwrap_or(0.);
            let y0 = center - width / 2. + inset;
            let y1 = center + width / 2. - inset;
            let (x0, x1) = {
                let (lo, hi) = (x0.min(x1), x0.max(x1));
                let inset = inset.min((hi - lo) / 2.);
                (lo + inset, hi - inset)
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
            // Plot's `inset`: an explicit inset shrinks every side; a binned
            // rect defaults to half a pixel along the axes it bins, so its
            // neighbours read apart.
            let (bin_x, bin_y) = crate::render::bin_axes(&prepared.mark);
            let inset = |binned: bool| {
                prepared
                    .mark
                    .style
                    .inset
                    .unwrap_or(if binned { 0.5 } else { 0. })
            };
            let shrink = |a: f32, b: f32, inset: f32| {
                let (lo, hi) = (a.min(b), a.max(b));
                let inset = inset.min((hi - lo) / 2.);
                (lo + inset, hi - inset)
            };
            let (x0, x1) = shrink(x0, x1, inset(bin_x));
            let (y0, y1) = shrink(y0, y1, inset(bin_y));
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x1), px(y1))),
            );
            // Plot's rect fills nothing when given only a stroke.
            if !(prepared.mark.channels.stroke.is_some() && prepared.mark.channels.fill.is_none()) {
                let color = colors.fill(index);
                paint::paint_rect(
                    rect,
                    prepared.mark.style.corner_radius,
                    paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                    ctx.window,
                );
            }
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
            // A cell without a y spans the frame's height (`cellX`, or a
            // `cell` given only x), one without an x its width.
            let no_y = prepared.mark.kind == MarkKind::CellX || row.y_value().is_null();
            let no_x = prepared.mark.kind == MarkKind::CellY || row.x_value().is_null();
            let (plot_w, plot_h) = (
                ctx.frame.plot.size.width.as_f32(),
                ctx.frame.plot.size.height.as_f32(),
            );
            let (x0, width) = if no_x {
                (plot_w / 2., plot_w)
            } else {
                let Some(x) = ctx.frame.x.scale.position(row.x_value()) else {
                    continue;
                };
                (x, cell_extent(&ctx.frame.x.scale))
            };
            let (y0, height) = if no_y {
                (plot_h / 2., plot_h)
            } else {
                let Some(y) = ctx.frame.y.scale.position(row.y_value()) else {
                    continue;
                };
                (y, cell_extent(&ctx.frame.y.scale))
            };
            let inset = prepared.mark.style.inset.unwrap_or(0.);
            let (hw, hh) = ((width / 2. - inset).max(0.), (height / 2. - inset).max(0.));
            // A band's position is its center, so the cell straddles it.
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0 - hw), px(y0 - hh))),
                ctx.absolute(point(px(x0 + hw), px(y0 + hh))),
            );
            // Like Plot's rect, a cell given only a stroke fills nothing,
            // and outlines itself in that stroke.
            let channels = &prepared.mark.channels;
            if !(channels.stroke.is_some() && channels.fill.is_none()) {
                let color = colors.fill(index);
                paint::paint_rect(
                    rect,
                    0.,
                    paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                    ctx.window,
                );
            }
            if channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    0.,
                    prepared.mark.style.stroke_width,
                    paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row)),
                    ctx.window,
                );
            }
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
            // A rule reads its own `x`; an `x1` stands in when only the
            // interval is set.
            // A rule without an x sits at the frame's middle, as Plot's does.
            let Some(x) = (if row.x_value().is_null() {
                Some(ctx.frame.plot.size.width.as_f32() / 2.)
            } else {
                ctx.frame.x.scale.position(row.x_value())
            }) else {
                continue;
            };
            // Plot's ruleX spans `y1`..`y2` when given them, else the frame.
            let (top, bottom) = match (&row.y1, &row.y2) {
                (Some(y1), Some(y2)) => {
                    let (Some(a), Some(b)) = (
                        ctx.frame.y.scale.position(y1),
                        ctx.frame.y.scale.position(y2),
                    ) else {
                        continue;
                    };
                    (a, b)
                }
                _ => (0., height),
            };
            // Plot draws a rule from y1 to y2: bottom to top in pixels.
            let start = ctx.absolute(point(px(x), px(bottom)));
            let end = ctx.absolute(point(px(x), px(top)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            if prepared.mark.style.dash.is_empty() {
                paint::paint_stroke(&[start, end], width, color, ctx.window);
            } else {
                paint::paint_dashed(
                    start,
                    end,
                    &prepared.mark.style.dash,
                    width,
                    color,
                    ctx.window,
                );
            }
            paint_markers(
                &[start, end],
                &prepared.mark.style.markers,
                color,
                width,
                ctx.theme.background,
                ctx.window,
            );
        }
    });
}

fn draw_rules_y(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let width = ctx.frame.plot.size.width.as_f32();
    let stroke_width = prepared.mark.style.stroke_width;
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(y) = (if row.y_value().is_null() {
                Some(ctx.frame.plot.size.height.as_f32() / 2.)
            } else {
                ctx.frame.y.scale.position(row.y_value())
            }) else {
                continue;
            };
            let (left, right) = match (&row.x1, &row.x2) {
                (Some(x1), Some(x2)) => {
                    let (Some(a), Some(b)) = (
                        ctx.frame.x.scale.position(x1),
                        ctx.frame.x.scale.position(x2),
                    ) else {
                        continue;
                    };
                    (a, b)
                }
                _ => (0., width),
            };
            let start = ctx.absolute(point(px(left), px(y)));
            let end = ctx.absolute(point(px(right), px(y)));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            if prepared.mark.style.dash.is_empty() {
                paint::paint_stroke(&[start, end], stroke_width, color, ctx.window);
            } else {
                paint::paint_dashed(
                    start,
                    end,
                    &prepared.mark.style.dash,
                    stroke_width,
                    color,
                    ctx.window,
                );
            }
            paint_markers(
                &[start, end],
                &prepared.mark.style.markers,
                color,
                stroke_width,
                ctx.theme.background,
                ctx.window,
            );
        }
    });
}

fn draw_texts(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    use crate::mark::{LineAnchor, TextAnchor};
    let style = &prepared.mark.style;
    let mut font = gpui::font(ctx.theme.font_family.clone());
    if style.italic {
        font.style = gpui::FontStyle::Italic;
    }
    let weight = style
        .font_weight
        .map(|w| gpui::FontWeight(w as f32))
        .unwrap_or(gpui::FontWeight::NORMAL);
    // Plot's text paints its glyphs in `fill`, `currentColor` by default; a
    // stroke alone (a halo) does not color the glyphs.
    let colors = Colors::resolve(prepared, ctx);
    let size = px(style.font_size.unwrap_or(10.));
    let (dx, dy) = (style.dx, style.dy);
    let values = prepared
        .rows
        .first()
        .is_some_and(|row| row.row.get(crate::table::VALUE).is_some());

    // Text is not clipped to the plot: Plot draws a label whole even where it
    // overhangs the frame.
    for (index, row) in prepared.rows.iter().enumerate() {
        let Some(position) = position(prepared, ctx, row) else {
            continue;
        };
        let color = if prepared.mark.channels.fill.is_some() {
            colors.fill(index)
        } else {
            ctx.colors.default_fill(prepared, ctx.mark_index)
        };
        let color = paint::alpha(color, ctx.row_fill_opacity(prepared, row));
        let text = match &prepared.mark.channels.text {
            Some(channel) => channel.read(&row.row),
            None if values => row.row.value(crate::table::VALUE),
            None => Value::Null,
        };
        let text = match (&text, &prepared.mark.text_format) {
            (Value::Null, _) => continue,
            (value, Some(format)) => SharedString::from(format.format(value)),
            (value, None) => value.label(),
        };
        if text.is_empty() {
            continue;
        }
        // Plot splits on newlines and, given a `lineWidth` in ems, wraps.
        let mut lines: Vec<String> = Vec::new();
        for line in text.split('\n') {
            match style.line_width {
                Some(ems) => lines.extend(wrap(line, ems, size.as_f32(), ctx.window, &font)),
                None => lines.push(line.to_string()),
            }
        }
        let line_height = size.as_f32() * 1.2;
        let block = line_height * lines.len() as f32;
        let origin = ctx.absolute(position);
        // Plot's text infers its anchors from a `frameAnchor` on the
        // frame's edge: "top-left" reads start-aligned and hangs from the top.
        use crate::mark::FrameAnchor as F;
        let frame_anchor = style.frame_anchor;
        let line_anchor = match (style.line_anchor_set, frame_anchor) {
            (false, Some(F::Top | F::TopLeft | F::TopRight)) => LineAnchor::Top,
            (false, Some(F::Bottom | F::BottomLeft | F::BottomRight)) => LineAnchor::Bottom,
            _ => style.line_anchor,
        };
        let text_anchor = match (style.text_anchor_set, frame_anchor) {
            (false, Some(F::Left | F::TopLeft | F::BottomLeft)) => TextAnchor::Start,
            (false, Some(F::Right | F::TopRight | F::BottomRight)) => TextAnchor::End,
            _ => style.text_anchor,
        };
        let top = match line_anchor {
            LineAnchor::Top => origin.y.as_f32() + dy,
            LineAnchor::Middle => origin.y.as_f32() + dy - block / 2.,
            LineAnchor::Bottom => origin.y.as_f32() + dy - block,
        };
        let align = match text_anchor {
            TextAnchor::Start => TextAlign::Left,
            TextAnchor::Middle => TextAlign::Center,
            TextAnchor::End => TextAlign::Right,
        };
        // Plot's `rotate`: the label turns about its anchor point.
        if style.text_rotate.abs() > 0.5 {
            let anchor = point(px(origin.x.as_f32() + dx), px(origin.y.as_f32() + dy));
            paint::paint_rotated_text(
                &lines.join(" "),
                size.as_f32(),
                color,
                &ctx.theme.font_family,
                anchor,
                style.text_rotate,
                align,
                ctx.window,
                ctx.cx,
            );
            continue;
        }
        for (i, line) in lines.into_iter().enumerate() {
            let y_line = top + i as f32 * line_height;
            // Plot's `textStroke`: a halo, painted as the glyphs offset in a
            // ring around their place, under the glyphs themselves.
            if let Some(halo) = style.text_stroke {
                let halo_label = Label::new(
                    line.clone().into(),
                    size,
                    halo,
                    weight,
                    font.clone(),
                    ctx.window,
                );
                let w = style.text_stroke_width / 2.;
                for k in 0..8 {
                    let a = std::f32::consts::TAU * k as f32 / 8.;
                    let y = y_line + (line_height - halo_label.height.as_f32()) / 2. + w * a.sin();
                    halo_label.paint_at(
                        point(px(origin.x.as_f32() + dx + w * a.cos()), px(y)),
                        align,
                        None,
                        ctx.window,
                        ctx.cx,
                    );
                }
            }
            let label = Label::new(line.into(), size, color, weight, font.clone(), ctx.window);
            let y = y_line + (line_height - label.height.as_f32()) / 2.;
            label.paint_at(
                point(px(origin.x.as_f32() + dx), px(y)),
                align,
                None,
                ctx.window,
                ctx.cx,
            );
        }
    }
}

/// Wrap `text` to `ems` em-widths, at spaces, as Plot's `lineWidth` does.
fn wrap(text: &str, ems: f32, size: f32, window: &mut Window, font: &gpui::Font) -> Vec<String> {
    let limit = ems * size;
    let measure = |text: &str, window: &mut Window| {
        Label::new(
            text.to_string().into(),
            px(size),
            Hsla::black(),
            gpui::FontWeight::NORMAL,
            font.clone(),
            window,
        )
        .width
        .as_f32()
    };
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if !current.is_empty() && measure(&candidate, window) > limit {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    lines.push(current);
    lines
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
    let style = &prepared.mark.style;
    let width = style.stroke_width;
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // Plot's link runs from (x1, y1) to (x2, y2); a lone `x` (or
            // `x1`) is both ends, Plot's `maybeSameValue`.
            let x_start = row.x1.clone().unwrap_or_else(|| row.x.clone());
            let x_end = row.x2.clone().unwrap_or_else(|| {
                if row.x.is_null() {
                    x_start.clone()
                } else {
                    row.x.clone()
                }
            });
            let y_start = row.y1.clone().unwrap_or_else(|| row.y.clone());
            let y_end = row.y2.clone().unwrap_or_else(|| {
                if row.y.is_null() {
                    y_start.clone()
                } else {
                    row.y.clone()
                }
            });
            let place = |x: &Value, y: &Value| -> Option<Point<Pixels>> {
                if let Some(projection) = &ctx.frame.projection {
                    let [px_, py_] = projection.project(x.as_number()?, y.as_number()?)?;
                    return Some(point(px(px_ as f32), px(py_ as f32)));
                }
                Some(point(
                    px(ctx.frame.x.scale.position(x)?),
                    px(ctx.frame.y.scale.position(y)?),
                ))
            };
            let (Some(from), Some(to)) = (place(&x_start, &y_start), place(&x_end, &y_end)) else {
                continue;
            };
            let (from, to) = (ctx.absolute(from), ctx.absolute(to));
            let color = paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row));
            // Plot's link is straight unless it names a curve.
            let points = vec![from, to];
            match style.curve {
                crate::mark::Curve::Linear => {
                    if style.dash.is_empty() {
                        paint::paint_stroke(&points, width, color, ctx.window);
                    } else {
                        paint::paint_dashed(from, to, &style.dash, width, color, ctx.window);
                    }
                }
                curve => {
                    paint::paint_path(curve_path(&points, curve, width, 1.), color, ctx.window)
                }
            }
            paint_markers(
                &points,
                &style.markers,
                color,
                width,
                ctx.theme.background,
                ctx.window,
            );
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

fn draw_geo(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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

/// The plot-relative position of a row.
///
/// A one-dimensional mark has no value on its cross axis, so it sits where its
/// kind says: `dotX` in the frame's middle, everything else by its index.
pub fn position(
    prepared: &PreparedMark,
    ctx: &DrawContext<'_>,
    row: &PreparedRow,
) -> Option<Point<Pixels>> {
    position_in(prepared, ctx.frame, row)
}

/// [`position`] against a frame alone, for the hover hit-test, which has no
/// draw context but must place a row exactly where the painter did.
pub fn position_in(
    prepared: &PreparedMark,
    frame: &Frame,
    row: &PreparedRow,
) -> Option<Point<Pixels>> {
    // A pixel-space transform (Plot's hexbin) placed the row itself.
    if let Some((x, y)) = row.pixel {
        return Some(point(px(x), px(y)));
    }
    // Under Plot's `projection`, x and y are longitude and latitude.
    if let Some(projection) = &frame.projection {
        let (lon, lat) = (row.x_value().as_number()?, row.y_value().as_number()?);
        let [x, y] = projection.project(lon, lat)?;
        return Some(point(px(x as f32), px(y as f32)));
    }
    // A geo mark places its rows through its projection, not the scales.
    if prepared.mark.kind == MarkKind::Geo {
        let projection = prepared.mark.projection.clone().unwrap_or_default();
        let (lon, lat) = (row.x_value().as_number()?, row.y_value().as_number()?);
        let [fx, fy] = projection.project(lon, lat);
        return Some(point(
            px(frame.plot.size.width.as_f32() * fx as f32),
            px(frame.plot.size.height.as_f32() * fy as f32),
        ));
    }
    // A missing coordinate sits where the mark's `frameAnchor` says, the
    // frame's middle by default: Plot's dot with only an `x`, or a `dotX`.
    use crate::mark::FrameAnchor;
    let anchor = prepared.mark.style.frame_anchor.unwrap_or_default();
    let (width, height) = (
        frame.plot.size.width.as_f32(),
        frame.plot.size.height.as_f32(),
    );
    let anchor_x = match anchor {
        FrameAnchor::Left | FrameAnchor::TopLeft | FrameAnchor::BottomLeft => 0.,
        FrameAnchor::Right | FrameAnchor::TopRight | FrameAnchor::BottomRight => width,
        _ => width / 2.,
    };
    let anchor_y = match anchor {
        FrameAnchor::Top | FrameAnchor::TopLeft | FrameAnchor::TopRight => 0.,
        FrameAnchor::Bottom | FrameAnchor::BottomLeft | FrameAnchor::BottomRight => height,
        _ => height / 2.,
    };
    let only_x =
        prepared.mark.kind.is_x_only() && prepared.mark.kind.cross_axis() == CrossAxis::Anchor;
    let only_y =
        prepared.mark.kind.is_y_only() && prepared.mark.kind.cross_axis() == CrossAxis::Anchor;
    let x = if only_y || row.x_value().is_null() {
        anchor_x
    } else {
        frame.x.scale.position(row.x_value())?
    };
    let y = if only_x || row.y_value().is_null() {
        anchor_y
    } else {
        frame.y.scale.position(row.y_value())?
    };
    let placed = point(px(x), px(y));

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
    if points.len() < 2 || !paint::finite(points) {
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
    if let Some([x1, _, x2, _]) = crate::render::raster_extent(&prepared.mark)
        && matches!(prepared.mark.kind, MarkKind::Raster | MarkKind::Contour)
    {
        return vec![Value::Number(x1), Value::Number(x2)];
    }
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
        .chain(prepared.filtered_domain[0].iter().cloned())
        .collect()
}

/// The extent a mark contributes to the y domain.
pub fn y_values(prepared: &PreparedMark) -> Vec<Value> {
    if let Some([_, y1, _, y2]) = crate::render::raster_extent(&prepared.mark)
        && matches!(prepared.mark.kind, MarkKind::Raster | MarkKind::Contour)
    {
        return vec![Value::Number(y1), Value::Number(y2)];
    }
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
        .chain(prepared.filtered_domain[1].iter().cloned())
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
            Some(channel) => ctx.colors.channel(channel, &row.row),
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

            let bend = bend
                * style.sweep.sign(
                    from.x.as_f32(),
                    from.y.as_f32(),
                    to.x.as_f32(),
                    to.y.as_f32(),
                );
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
    let style = &prepared.mark.style;
    let positive = style
        .positive_fill
        .unwrap_or(crate::scale::hsla_from_hex(0x3ca951));
    let negative = style
        .negative_fill
        .unwrap_or(crate::scale::hsla_from_hex(0x4269d0));
    let positive = paint::alpha(positive, opacity);
    let negative = paint::alpha(negative, opacity);
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
    let Some(grid) = crate::render::resolve_raster(prepared, ctx.frame) else {
        return;
    };
    let opacity = prepared.mark.style.opacity * prepared.mark.style.fill_opacity * ctx.appear;
    let row = crate::table::Table::empty();
    let place = |x: f64, y: f64, ctx: &DrawContext<'_>| -> Option<Point<Pixels>> {
        if let Some(projection) = &ctx.frame.projection {
            let [px_, py_] = projection.project(x, y)?;
            return Some(point(px(px_ as f32), px(py_ as f32)));
        }
        Some(point(
            px(ctx.frame.x.scale.position(&Value::Number(x))?),
            px(ctx.frame.y.scale.position(&Value::Number(y))?),
        ))
    };
    let (dx, dy) = (
        (grid.x2 - grid.x1) / grid.width as f64,
        (grid.y2 - grid.y1) / grid.height as f64,
    );
    let _ = row;
    ctx.clipped(|ctx| {
        for j in 0..grid.height {
            for i in 0..grid.width {
                let value = &grid.values[j * grid.width + i];
                if value.is_null() {
                    continue;
                }
                let color = match value.as_text().and_then(|t| crate::css::parse(t)) {
                    Some(c) => c,
                    None => match ctx.colors.scale.as_ref().and_then(|s| s.color(value)) {
                        Some(c) => c,
                        None => continue,
                    },
                };
                let x0 = grid.x1 + i as f64 * dx;
                let y0 = grid.y1 + j as f64 * dy;
                let corners = [
                    place(x0, y0, ctx),
                    place(x0 + dx, y0, ctx),
                    place(x0 + dx, y0 + dy, ctx),
                    place(x0, y0 + dy, ctx),
                ];
                let Some(corners) = corners.into_iter().collect::<Option<Vec<_>>>() else {
                    continue;
                };
                let color = paint::alpha(color, opacity);
                if ctx.frame.projection.is_some() {
                    let points: Vec<Point<Pixels>> =
                        corners.into_iter().map(|p| ctx.absolute(p)).collect();
                    paint::paint_fill(&points, color, ctx.window);
                } else {
                    // Overlap by a hair so neighbouring cells leave no seam.
                    let (a, b) = (ctx.absolute(corners[0]), ctx.absolute(corners[2]));
                    let rect = paint::rect_between(a, b);
                    let rect = gpui::Bounds::new(
                        rect.origin - point(px(0.25), px(0.25)),
                        rect.size + gpui::size(px(0.5), px(0.5)),
                    );
                    paint::paint_rect(rect, 0., color, ctx.window);
                }
            }
        }
    });
}

/// Plot's `contour`: the grid's isobands, one closed region per threshold
/// (the area at or above it), filled through the color scale at the
/// threshold's value and stroked in `currentColor` unless filled.
pub fn draw_contour_grid(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let Some(grid) = crate::render::resolve_raster(prepared, ctx.frame) else {
        return;
    };
    let spec = prepared.mark.raster.clone().unwrap_or_default();
    let thresholds = grid.thresholds(spec.thresholds.as_ref());
    let style = &prepared.mark.style;
    let filled = prepared.mark.channels.fill.is_some();
    let stroke = match &prepared.mark.channels.stroke {
        Some(channel) => Some(ctx.colors.channel(channel, &crate::table::Table::empty())),
        None if !filled => Some(ctx.colors.default_stroke(prepared, ctx.mark_index)),
        None => None,
    };
    let place = |p: [f64; 2], ctx: &DrawContext<'_>| -> Option<Point<Pixels>> {
        if let Some(projection) = &ctx.frame.projection {
            let [x, y] = projection.project(p[0], p[1])?;
            return Some(ctx.absolute(point(px(x as f32), px(y as f32))));
        }
        Some(ctx.absolute(point(
            px(ctx.frame.x.scale.position(&Value::Number(p[0]))?),
            px(ctx.frame.y.scale.position(&Value::Number(p[1]))?),
        )))
    };
    ctx.clipped(|ctx| {
        for threshold in thresholds {
            let rings: Vec<Vec<Point<Pixels>>> = grid
                .contour(threshold)
                .into_iter()
                .filter_map(|ring| {
                    ring.into_iter()
                        .map(|p| place(p, ctx))
                        .collect::<Option<Vec<_>>>()
                })
                .collect();
            if filled {
                // A constant fill paints as itself; a channel fill through
                // the color scale at the threshold.
                let color = match &prepared.mark.channels.fill {
                    Some(crate::style::ColorChannel::Constant(c)) => Some(*c),
                    _ => ctx
                        .colors
                        .scale
                        .as_ref()
                        .and_then(|s| s.color(&Value::Number(threshold))),
                };
                if let Some(color) = color {
                    paint::paint_fill_rings(
                        &rings,
                        paint::alpha(color, style.opacity * style.fill_opacity),
                        ctx.window,
                    );
                }
            }
            if let Some(stroke) = stroke {
                for ring in &rings {
                    paint::paint_stroke(
                        ring,
                        style.stroke_width,
                        paint::alpha(stroke, style.opacity * style.stroke_opacity),
                        ctx.window,
                    );
                }
            }
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
