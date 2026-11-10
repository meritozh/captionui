//! The primitive marks: dot, line, area, bar, rect, cell, rule, text, tick,
//! vector and link.
//!
//! Each mark projects its rows through the frame's scales and paints. Shapes
//! are clipped to the plot, so a value outside the domain stops at the frame's
//! edge the way Plot's marks do.

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
    // Plot's `clip: "sphere"`: cut the mark's shapes to the projected globe.
    // A raster masks its own image to it.
    if prepared.mark.style.clip_sphere
        && prepared.mark.kind != MarkKind::Raster
        && let Some(sphere) = ctx.frame.projection.as_ref().and_then(|p| p.sphere())
    {
        let ring = sphere
            .iter()
            .map(|&[x, y]| ctx.absolute(point(px(x as f32), px(y as f32))))
            .collect();
        return paint::with_clip_polygon(Some(ring), || draw_kind(prepared, ctx));
    }
    draw_kind(prepared, ctx)
}

fn draw_kind(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
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
    // With a `mixBlendMode`, the pieces painted so far with the opaque color
    // each shows, so an overlap paints its blend.
    let mut blended = BlendedPieces::new(prepared.mark.style.mix_blend_mode, ctx.theme.background);
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // Under a projection, Plot projects the corners (x1, y1) and
            // (x2, y2) as points, and the rect spans them.
            let projected = ctx.frame.projection.as_ref().map(|projection| {
                let (x_start, x_end) = row.x_interval()?;
                let (y_start, y_end) = row.y_interval()?;
                let a = projection.project(x_start.as_number()?, y_start.as_number()?)?;
                let b = projection.project(x_end.as_number()?, y_end.as_number()?)?;
                Some([a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32])
            });
            // Plot's rect spans the frame along an axis it has no channel
            // on (`rect(…, binX({fill: "count"}, {x}))` fills the height).
            let x_span = match row.x_interval() {
                _ if projected.is_some() => match projected.flatten() {
                    Some([x0, _, x1, _]) => (Some(x0), Some(x1)),
                    None => continue,
                },
                Some((start, end)) => (
                    ctx.frame.x.scale.position(&start),
                    ctx.frame.x.scale.position(&end),
                ),
                None if row.x1.is_none() && row.x2.is_none() => {
                    (Some(0.), Some(ctx.frame.plot.size.width.as_f32()))
                }
                None => continue,
            };
            let y_span = match row.y_interval() {
                _ if projected.is_some() => match projected.flatten() {
                    Some([_, y0, _, y1]) => (Some(y0), Some(y1)),
                    None => continue,
                },
                Some((start, end)) => (
                    ctx.frame.y.scale.position(&start),
                    ctx.frame.y.scale.position(&end),
                ),
                None if row.y1.is_none() && row.y2.is_none() => {
                    (Some(0.), Some(ctx.frame.plot.size.height.as_f32()))
                }
                None => continue,
            };
            let ((Some(x0), Some(x1)), (Some(y0), Some(y1))) = (x_span, y_span) else {
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
                let color = paint::alpha(colors.fill(index), ctx.row_fill_opacity(prepared, row));
                if !blended.paint(rect, color, ctx.window) {
                    paint::paint_rect(rect, prepared.mark.style.corner_radius, color, ctx.window);
                }
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

/// Rect fills under a CSS `mix-blend-mode`, which GPUI cannot composite: the
/// pieces painted so far, each with the opaque color it shows over the
/// background. A new rect paints its blend over the background, then over
/// every piece it overlaps, in painting order so the topmost piece wins, as
/// the browser composites the mark's group.
struct BlendedPieces {
    mode: Option<crate::mark::BlendMode>,
    background: Hsla,
    pieces: Vec<(Bounds<Pixels>, Hsla)>,
}

impl BlendedPieces {
    fn new(mode: Option<crate::mark::BlendMode>, background: Hsla) -> Self {
        Self {
            mode,
            background: paint::alpha(background, 1.),
            pieces: Vec::new(),
        }
    }

    /// `color` blended onto the opaque `backdrop`, opaque.
    fn blend(mode: crate::mark::BlendMode, backdrop: Hsla, color: Hsla) -> Hsla {
        let (b, s) = (backdrop.to_rgb(), color.to_rgb());
        let a = s.a;
        let mix = |b: f32, s: f32| (1. - a) * b + a * mode.channel(b, s);
        Hsla::from(gpui::Rgba {
            r: mix(b.r, s.r),
            g: mix(b.g, s.g),
            b: mix(b.b, s.b),
            a: 1.,
        })
    }

    /// Paint `rect` blended, returning whether the mark blends at all.
    fn paint(&mut self, rect: Bounds<Pixels>, color: Hsla, window: &mut Window) -> bool {
        let Some(mode) = self.mode else {
            return false;
        };
        let base = Self::blend(mode, self.background, color);
        paint::paint_rect(rect, 0., base, window);
        let mut added = vec![(rect, base)];
        for (piece, shown) in &self.pieces {
            if !piece.intersects(&rect) {
                continue;
            }
            let overlap = piece.intersect(&rect);
            if overlap.size.width <= px(0.) || overlap.size.height <= px(0.) {
                continue;
            }
            let over = Self::blend(mode, *shown, color);
            paint::paint_rect(overlap, 0., over, window);
            added.push((overlap, over));
        }
        self.pieces.extend(added);
        true
    }
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

/// Plot's `image` reads an `src` per row; here the mark carries one
/// [`gpui::RenderImage`] shared across its rows.
///
/// Plot's `image`: each row's image centred on its position in a box
/// `width` × `height` pixels (16 by default, each defaulting to the other),
/// or a circle of radius `r`, fit to the box by `preserveAspectRatio`
/// (`xMidYMid meet` by default; `slice` covers the box and crops).
fn draw_image(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let Some(image) = prepared.mark.image.clone() else {
        return;
    };
    if prepared.rows.is_empty() {
        return;
    }
    let style = &prepared.mark.style;
    let natural = image.size(0);
    let (natural_w, natural_h) = (natural.width.0 as f32, natural.height.0 as f32);
    if natural_w <= 0. || natural_h <= 0. {
        return;
    }
    let aspect = style.preserve_aspect_ratio.as_deref().unwrap_or("xMidYMid meet");
    let mut words = aspect.split_whitespace();
    let align = words.next().unwrap_or("xMidYMid");
    let slice = words.next() == Some("slice");
    // SVG's alignment: where the image sits along each axis of the box.
    let along = |axis: &str| {
        if align.contains(&format!("{axis}Min")) {
            0.
        } else if align.contains(&format!("{axis}Max")) {
            1.
        } else {
            0.5
        }
    };
    let (ax, ay) = (along("x"), along("y"));
    let none = align == "none";
    ctx.clipped(|ctx| {
        for row in prepared.rows.iter() {
            let Some(centre) = position(prepared, ctx, row) else {
                continue;
            };
            let r = prepared
                .mark
                .channels
                .r
                .as_ref()
                .map(|channel| channel.read(&row.row));
            let (w, h) = match (r, style.image_size) {
                (Some(r), _) => (r * 2., r * 2.),
                (None, [Some(w), Some(h)]) => (w, h),
                (None, [Some(s), None] | [None, Some(s)]) => (s, s),
                (None, [None, None]) => (16., 16.),
            };
            if !(w > 0. && h > 0.) {
                continue;
            }
            let at = ctx.absolute(centre);
            let origin = point(
                at.x + px(style.dx - w / 2.),
                at.y + px(style.dy - h / 2.),
            );
            let bounds = Bounds::new(origin, gpui::size(px(w), px(h)));
            let image_bounds = if none {
                bounds
            } else {
                let scale = if slice {
                    (w / natural_w).max(h / natural_h)
                } else {
                    (w / natural_w).min(h / natural_h)
                };
                let (iw, ih) = (natural_w * scale, natural_h * scale);
                Bounds::new(
                    point(
                        origin.x + px((w - iw) * ax),
                        origin.y + px((h - ih) * ay),
                    ),
                    gpui::size(px(iw), px(ih)),
                )
            };
            // Plot's `r` clips the image to a circle.
            let corners = match r {
                Some(r) => gpui::Corners::all(px(r)),
                None => Default::default(),
            };
            // `paint_image` returns a Result; a failure paints nothing.
            let _ = ctx
                .window
                .paint_image(bounds, image_bounds, corners, image.clone(), 0, false);
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
    // Under Plot's `projection`, x and y are longitude and latitude; a mark
    // with neither (a facet axis's text) sits at its frame anchor instead.
    let channels = &prepared.mark.channels;
    if let Some(projection) = &frame.projection
        && (channels.x.is_some() || channels.y.is_some())
    {
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
    // A row whose x or y channel reads no value (null or NaN) is not drawn,
    // Plot's filter of undefined channel values; only a mark without the
    // channel sits at the frame anchor.
    let channels = &prepared.mark.channels;
    if (channels.x.is_some() && !only_y && row.x_value().is_null())
        || (channels.y.is_some() && !only_x && row.y_value().is_null())
    {
        return None;
    }
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
    // Only looked up, never iterated: `order` decides the groups' order.
    let mut groups: rustc_hash::FxHashMap<crate::value::ValueKey, Vec<(usize, &PreparedRow)>> =
        Default::default();
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
    domain::<true>(prepared).collect()
}

/// The extent a mark contributes to the y domain.
pub fn y_values(prepared: &PreparedMark) -> Vec<Value> {
    domain::<false>(prepared).collect()
}

/// [`y_values`], lazily, for a reader that may stop early.
pub fn y_domain(prepared: &PreparedMark) -> impl Iterator<Item = Value> + '_ {
    domain::<false>(prepared)
}

/// The values a mark contributes to the x (`X`) or y domain: a raster's
/// extent, else each row's interval (or its value) and then the values of
/// the rows a filter dropped.
fn domain<const X: bool>(prepared: &PreparedMark) -> impl Iterator<Item = Value> + '_ {
    let raster = crate::render::raster_extent(&prepared.mark)
        .filter(|_| matches!(prepared.mark.kind, MarkKind::Raster | MarkKind::Contour));
    let extent =
        raster.map(|[x1, y1, x2, y2]| if X { [x1, x2] } else { [y1, y2] }.map(Value::Number));
    // A raster's extent stands for its rows.
    let (rows, filtered): (&[PreparedRow], &[Value]) = match raster {
        Some(_) => (&[], &[]),
        None => (
            &prepared.rows,
            &prepared.filtered_domain[if X { 0 } else { 1 }],
        ),
    };
    let rows = rows
        .iter()
        .flat_map(|row| {
            // A row's extent is its interval when it has one: a stacked bar's
            // value has been rewritten into the interval, and the datum's own
            // channel still holds the pre-stack number.
            let (start, end) = if X {
                (&row.x1, &row.x2)
            } else {
                (&row.y1, &row.y2)
            };
            if start.is_some() || end.is_some() {
                return [start.clone(), end.clone()];
            }
            [
                Some(if X { row.x_value() } else { row.y_value() }.clone()),
                None,
            ]
        })
        .flatten()
        .filter(|value| !value.is_null());
    extent
        .into_iter()
        .flatten()
        .chain(rows)
        .chain(filtered.iter().cloned())
}

/// Plot's `tip`: a box around the row's text with a pointer at the datum,
/// on the side `anchor` names or else the side that fits.
///
/// As in Plot, the box fills with the background and strokes in the
/// foreground (a `fill` or `stroke` overrides either), the text reads in the
/// foreground at `lineHeight` 1, wraps at `lineWidth` (20 ems), and sits
/// `textPadding` (8px) inside the box, whose pointer is `pointerSize` (12px).
/// Without a `title` (or text) the tip lists the row's channels as bold
/// name, value pairs.
fn draw_tips(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    use crate::mark::FrameAnchor as A;
    let style = &prepared.mark.style;
    let font = gpui::font(ctx.theme.font_family.clone());
    let size = style.font_size.unwrap_or(ctx.theme.label_size.as_f32());
    let r = style.text_padding.unwrap_or(8.);
    let m = style.pointer_size.unwrap_or(12.);
    let line_width = style.line_width.unwrap_or(20.);
    let ink = ctx.theme.mark_color;
    let defaults = crate::mark::MarkStyle::default();
    let (figure_width, figure_height) = (
        ctx.frame.bounds.size.width.as_f32(),
        ctx.frame.bounds.size.height.as_f32(),
    );

    // Tips are not clipped to the plot: a tip on the topmost point sits above
    // it, and Plot draws the whole label.
    for row in prepared.rows.iter() {
        let Some(placed) = position(prepared, ctx, row) else {
            continue;
        };
        // Each line is an optional bold name and its value.
        let mut lines: Vec<(Option<String>, String)> = Vec::new();
        let title = prepared
            .mark
            .channels
            .text
            .as_ref()
            .or(prepared.mark.channels.title.as_ref());
        if let Some(channel) = title {
            let text = channel.read(&row.row);
            if text.is_null() {
                continue;
            }
            for line in text.label().split('\n') {
                for wrapped in wrap(line, line_width, size, ctx.window, &font) {
                    lines.push((None, wrapped));
                }
            }
        } else {
            let channels = &prepared.mark.channels;
            for (name, channel, value) in [
                ("x", &channels.x, row.x_value()),
                ("y", &channels.y, row.y_value()),
            ] {
                if channel.is_none() || value.is_null() {
                    continue;
                }
                let label = match channel {
                    Some(crate::table::Channel::Field(field)) => field.to_string(),
                    _ => name.to_string(),
                };
                lines.push((Some(label), format!(" {}", value.label())));
            }
        }
        if lines.iter().all(|(l, v)| l.is_none() && v.is_empty()) {
            continue;
        }
        let shaped: Vec<(Option<Label>, Label)> = lines
            .into_iter()
            .map(|(label, value)| {
                let label = label.map(|label| {
                    Label::new(
                        label.into(),
                        px(size),
                        ink,
                        gpui::FontWeight::BOLD,
                        font.clone(),
                        ctx.window,
                    )
                });
                let value = Label::new(
                    value.into(),
                    px(size),
                    ink,
                    gpui::FontWeight::NORMAL,
                    font.clone(),
                    ctx.window,
                );
                (label, value)
            })
            .collect();
        let n = shaped.len() as f32;
        // The text's bounding box, rounded for crisp edges as Plot does: the
        // lines sit one em apart, and the last carries its descent.
        let w = shaped
            .iter()
            .map(|(label, value)| {
                label.as_ref().map_or(0., |l| l.width.as_f32()) + value.width.as_f32()
            })
            .fold(0., f32::max)
            .round();
        let h = (size * (n - 1. + 1.15)).round();

        let at = ctx.absolute(point(
            px(placed.x.as_f32().round() + style.dx),
            px(placed.y.as_f32().round() + style.dy),
        ));
        let anchor = style.tip_anchor.unwrap_or_else(|| {
            let x = at.x.as_f32() - ctx.frame.bounds.origin.x.as_f32();
            let y = at.y.as_f32() - ctx.frame.bounds.origin.y.as_f32();
            let fit_left = x + w + m + r * 2. < figure_width;
            let fit_right = x - w - m - r * 2. > 0.;
            let fit_top = y + h + m + r * 2. < figure_height;
            let fit_bottom = y - h - m - r * 2. > 0.;
            match (fit_left && fit_right, fit_top && fit_bottom) {
                (true, true) => A::Bottom,
                (true, false) if fit_bottom => A::Bottom,
                (true, false) => A::Top,
                (false, true) if fit_left => A::Left,
                (false, true) => A::Right,
                _ if (fit_left || fit_right) && (fit_top || fit_bottom) => {
                    match (fit_bottom, fit_left) {
                        (true, true) => A::BottomLeft,
                        (true, false) => A::BottomRight,
                        (false, true) => A::TopLeft,
                        (false, false) => A::TopRight,
                    }
                }
                _ => A::Bottom,
            }
        });

        // Plot's `getPath`, and its `getTextTranslate` for the text block.
        let (bw, bh, m2) = (w + r * 2., h + r * 2., m / 2.);
        let outline: Vec<(f32, f32)> = match anchor {
            A::Middle => vec![
                (-bw / 2., -bh / 2.),
                (bw / 2., -bh / 2.),
                (bw / 2., bh / 2.),
                (-bw / 2., bh / 2.),
            ],
            A::TopLeft => vec![(0., 0.), (m2, m2), (bw, m2), (bw, m2 + bh), (0., m2 + bh)],
            A::Top => vec![
                (0., 0.),
                (m2, m2),
                (bw / 2., m2),
                (bw / 2., m2 + bh),
                (-bw / 2., m2 + bh),
                (-bw / 2., m2),
                (-m2, m2),
            ],
            A::TopRight => vec![(0., 0.), (-m2, m2), (-bw, m2), (-bw, m2 + bh), (0., m2 + bh)],
            A::Right => vec![
                (0., 0.),
                (-m2, -m2),
                (-m2, -bh / 2.),
                (-m2 - bw, -bh / 2.),
                (-m2 - bw, bh / 2.),
                (-m2, bh / 2.),
                (-m2, m2),
            ],
            A::BottomLeft => vec![(0., 0.), (m2, -m2), (bw, -m2), (bw, -m2 - bh), (0., -m2 - bh)],
            A::Bottom => vec![
                (0., 0.),
                (m2, -m2),
                (bw / 2., -m2),
                (bw / 2., -m2 - bh),
                (-bw / 2., -m2 - bh),
                (-bw / 2., -m2),
                (-m2, -m2),
            ],
            A::BottomRight => vec![
                (0., 0.),
                (-m2, -m2),
                (-bw, -m2),
                (-bw, -m2 - bh),
                (0., -m2 - bh),
            ],
            A::Left => vec![
                (0., 0.),
                (m2, -m2),
                (m2, -bh / 2.),
                (m2 + bw, -bh / 2.),
                (m2 + bw, bh / 2.),
                (m2, bh / 2.),
                (m2, m2),
            ],
        };
        let (tx, ty) = match anchor {
            A::Middle => (-w / 2., h / 2.),
            A::TopLeft => (r, m2 + r),
            A::Top => (-w / 2., m2 + r),
            A::TopRight => (-w - r, m2 + r),
            A::Right => (-m2 - w - r, h / 2.),
            A::BottomLeft => (r, -m2 - r),
            A::Bottom => (-w / 2., -m2 - r),
            A::BottomRight => (-w - r, -m2 - r),
            A::Left => (r + m2, h / 2.),
        };
        // Plot's `getLineOffset`: a top anchor hangs the first baseline 0.94em
        // below the text's origin; every other anchor (its regex test is
        // always truthy) rests the last baseline 0.29em above it.
        let first_baseline = match anchor {
            A::Top | A::TopLeft | A::TopRight => 0.94 * size,
            _ => (-0.29 - n + 1.) * size,
        };

        let to_point = |(x, y): (f32, f32)| point(at.x + px(x), at.y + px(y));
        let polygon: Vec<Point<Pixels>> = outline.iter().copied().map(to_point).collect();
        let fill = match &prepared.mark.channels.fill {
            Some(channel) => ctx.colors.channel(channel, &row.row),
            None if style.fill != defaults.fill => style.fill,
            None => ctx.theme.background,
        };
        let stroke = match &prepared.mark.channels.stroke {
            Some(channel) => ctx.colors.channel(channel, &row.row),
            None if style.stroke != defaults.stroke => style.stroke,
            None => ink,
        };
        // Plot's `drop-shadow(0 3px 4px rgba(0,0,0,0.2))`, as stacked offsets
        // that fade with distance.
        for offset in 1..=4 {
            let shadow: Vec<Point<Pixels>> = polygon
                .iter()
                .map(|p| point(p.x, p.y + px(offset as f32)))
                .collect();
            paint::paint_fill(&shadow, paint::alpha(Hsla::black(), 0.05), ctx.window);
        }
        paint::paint_fill(&polygon, fill, ctx.window);
        let mut closed = polygon.clone();
        closed.push(polygon[0]);
        paint::paint_stroke(&closed, style.stroke_width.max(1.), stroke, ctx.window);

        for (i, (label, value)) in shaped.iter().enumerate() {
            let baseline = ty + first_baseline + i as f32 * size;
            // A label's top sits about 0.85em above its baseline.
            let top = at.y + px(baseline - 0.85 * size);
            let mut left = at.x + px(tx);
            if let Some(label) = label {
                label.paint_at(point(left, top), TextAlign::Left, None, ctx.window, ctx.cx);
                left += label.width;
            }
            value.paint_at(point(left, top), TextAlign::Left, None, ctx.window, ctx.cx);
        }
    }
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

    // Each series is two curves, each placed by its own channels: the first
    // at (x1, y1) and the second at (x2, y2), so a `shiftX` compares a series
    // with itself moved along x.
    let curves: Vec<[Vec<(f32, f32)>; 2]> = series_groups(prepared)
        .into_iter()
        .map(|group| {
            [true, false].map(|first| {
                group
                    .iter()
                    .filter_map(|(_, row)| {
                        Some((
                            position_along_of(ctx, row, along_x, first)?,
                            position_across(ctx, row, along_x, first)?,
                        ))
                    })
                    .collect()
            })
        })
        .collect();

    let origin = ctx.frame.plot.origin;
    // Plot clips the difference's line to the frame (`clip: true`).
    let clip = ctx.clip;
    ctx.clip = true;
    ctx.clipped(|ctx| {
        let at = |along: f32, across: f32| {
            origin
                + if along_x {
                    point(px(along), px(across))
                } else {
                    point(px(across), px(along))
                }
        };
        for [first, second] in &curves {
            // Plot clips each curve's area by the other's, so the fills cover
            // only where both curves run, split where they cross.
            for (along, a, b, positive_run) in difference_spans(first, second, along_x) {
                let color = if positive_run { positive } else { negative };
                let mut polygon: Vec<Point<Pixels>> =
                    along.iter().zip(&a).map(|(t, v)| at(*t, *v)).collect();
                polygon.extend(along.iter().zip(&b).rev().map(|(t, v)| at(*t, *v)));
                paint::paint_fill(&polygon, color, ctx.window);
            }
            // The second series' line runs over the areas, as one path so it
            // has no breaks at the sign changes.
            let line_path: Vec<Point<Pixels>> = second.iter().map(|(t, v)| at(*t, *v)).collect();
            paint::paint_stroke(&line_path, width, line, ctx.window);
        }
    });
    ctx.clip = clip;
}

/// The runs of one sign between two piecewise-linear curves, each a list of
/// `(along, across)` pixels: for each run, its along positions, the first and
/// second curves' across positions there, and whether the second is the
/// larger value. Both curves are sampled at every vertex of either within
/// the span they share, and a crossing splits a run.
#[allow(clippy::type_complexity)]
fn difference_spans(
    first: &[(f32, f32)],
    second: &[(f32, f32)],
    along_x: bool,
) -> Vec<(Vec<f32>, Vec<f32>, Vec<f32>, bool)> {
    let sorted = |curve: &[(f32, f32)]| {
        let mut curve = curve.to_vec();
        curve.sort_by(|a, b| a.0.total_cmp(&b.0));
        curve
    };
    let (first, second) = (sorted(first), sorted(second));
    let (Some(f0), Some(s0), Some(f1), Some(s1)) =
        (first.first(), second.first(), first.last(), second.last())
    else {
        return Vec::new();
    };
    let (lo, hi) = (f0.0.max(s0.0), f1.0.min(s1.0));
    if hi <= lo {
        return Vec::new();
    }
    let sample = |curve: &[(f32, f32)], t: f32| -> f32 {
        let i = curve.partition_point(|p| p.0 < t);
        if i == 0 {
            return curve[0].1;
        }
        if i >= curve.len() {
            return curve[curve.len() - 1].1;
        }
        let (a, b) = (curve[i - 1], curve[i]);
        if b.0 == a.0 {
            b.1
        } else {
            a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0)
        }
    };
    let mut ts: Vec<f32> = first
        .iter()
        .chain(&second)
        .map(|p| p.0)
        .filter(|t| *t >= lo && *t <= hi)
        .chain([lo, hi])
        .collect();
    ts.sort_by(f32::total_cmp);
    ts.dedup();
    let sign = |a: f32, b: f32| difference_is_positive(along_x, a, b);
    type Run = (Vec<f32>, Vec<f32>, Vec<f32>, bool);
    let mut runs: Vec<Run> = Vec::new();
    let push = |t: f32, a: f32, b: f32, positive: bool, runs: &mut Vec<Run>| {
        match runs.last_mut() {
            Some((along, va, vb, p)) if *p == positive => {
                along.push(t);
                va.push(a);
                vb.push(b);
            }
            _ => runs.push((vec![t], vec![a], vec![b], positive)),
        }
    };
    let mut previous: Option<(f32, f32, f32)> = None;
    for &t in &ts {
        let (a, b) = (sample(&first, t), sample(&second, t));
        if let Some((pt, pa, pb)) = previous {
            let (d0, d1) = (pb - pa, b - a);
            if d0 * d1 < 0. {
                // The curves cross: both runs meet at the crossing.
                let u = d0 / (d0 - d1);
                let (ct, cv) = (pt + (t - pt) * u, pa + (a - pa) * u);
                let before = sign(pa, pb);
                push(ct, cv, cv, before, &mut runs);
                push(ct, cv, cv, !before, &mut runs);
            }
        }
        // A tie takes the sign of the run it continues.
        let positive = if a == b {
            runs.last().is_some_and(|r| r.3)
        } else {
            sign(a, b)
        };
        push(t, a, b, positive, &mut runs);
        previous = Some((t, a, b));
    }
    runs.retain(|run| run.0.len() >= 2);
    runs
}

/// A row's first or second position along the axis a difference runs along:
/// its `x1` or `x2` (for a `differenceY`), else its `x`.
fn position_along_of(
    ctx: &DrawContext<'_>,
    row: &PreparedRow,
    along_x: bool,
    first: bool,
) -> Option<f32> {
    let (one, two, value, scale) = if along_x {
        (&row.x1, &row.x2, row.x_value(), &ctx.frame.x.scale)
    } else {
        (&row.y1, &row.y2, row.y_value(), &ctx.frame.y.scale)
    };
    let chosen = if first { one } else { two };
    scale.position(chosen.as_ref().unwrap_or(value))
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
/// Plot's geometry: one unit is `unit` times the value scale's pixels per
/// value; a row (or column) holds `multiple` cells — by default as many as
/// keep the cells about square — and the cells, `multiple` units tall, are
/// numbered from the scale's zero, so the interval `[y1, y2]` covers cells
/// `y1 / unit` to `y2 / unit` of that grid, wrapping row by row. A
/// fractional end cuts its cell short. A waffle with no cross channel spans
/// the frame.
fn draw_waffle_bars(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    let style = &prepared.mark.style;
    let along_x = prepared.mark.kind == MarkKind::WaffleX;
    let unit = style.unit.max(f32::MIN_POSITIVE) as f64;
    let gap = style.gap;
    let (value_scale, cross_scale) = if along_x {
        (&ctx.frame.x.scale, &ctx.frame.y.scale)
    } else {
        (&ctx.frame.y.scale, &ctx.frame.x.scale)
    };
    let (Some(zero), Some(one)) = (
        value_scale.position(&Value::Number(0.)),
        value_scale.position(&Value::Number(unit)),
    ) else {
        return;
    };
    // The length of a unit in pixels, and which way the values grow.
    let scale = (one - zero).abs();
    let sign = if one >= zero { 1. } else { -1. };
    if !scale.is_finite() || scale <= 0. {
        return;
    }
    let cross_channel = if along_x {
        prepared.mark.channels.y.is_some()
    } else {
        prepared.mark.channels.x.is_some()
    };
    let frame_cross = if along_x {
        ctx.frame.plot.size.height.as_f32()
    } else {
        ctx.frame.plot.size.width.as_f32()
    };
    let bar = if !cross_channel {
        frame_cross
    } else if along_x {
        bar_width_x(ctx.frame, prepared)
    } else {
        bar_width_y(ctx.frame, prepared)
    };
    let multiple = style
        .multiple
        .unwrap_or_else(|| ((bar / scale).sqrt().floor() as usize).max(1));
    let m = multiple as f32;
    // The outer size of a cell, gap included: across, and along the value.
    let cx = (bar / m).min(scale * m);
    let cy = scale * m;
    let tx = (bar - m * cx) / 2.;

    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            let cross_start = if !cross_channel {
                0.
            } else {
                let value = if along_x {
                    row.y_value()
                } else {
                    row.x_value()
                };
                let Some(centre) = cross_scale.position(value) else {
                    continue;
                };
                centre - bar / 2.
            };
            let interval = if along_x {
                row.x_interval()
            } else {
                row.y_interval()
            };
            let Some((start, end)) = interval else {
                continue;
            };
            let (Some(v1), Some(v2)) = (start.as_number(), end.as_number()) else {
                continue;
            };
            let (i1, i2) = {
                let (a, b) = (v1 / unit, v2 / unit);
                if a <= b { (a, b) } else { (b, a) }
            };
            if !(i1.is_finite() && i2.is_finite()) || i2 <= i1 {
                continue;
            }
            // Plot sets a constant `fillOpacity` on the waffle's group, which
            // both the pattern's cell (inheriting it) and the path the
            // pattern fills apply: the opacity counts twice.
            let color = paint::alpha(
                colors.fill(index),
                ctx.row_fill_opacity(prepared, row) * style.fill_opacity,
            );
            let radius = style.corner_radius;
            let mut k = i1.floor() as i64;
            while (k as f64) < i2 {
                let (lo, hi) = (
                    (i1 - k as f64).max(0.) as f32,
                    (i2 - k as f64).min(1.) as f32,
                );
                let column = k.rem_euclid(multiple as i64) as f32;
                let line = k.div_euclid(multiple as i64) as f32;
                k += 1;
                // The cell's pattern rect, inset by half the gap, and the
                // part of it the interval covers, in value-axis pixels from
                // zero.
                let (c0, c1) = (line * cy + gap / 2., line * cy + cy - gap / 2.);
                let (v0, v1) = (c0.max(line * cy + lo * cy), c1.min(line * cy + hi * cy));
                if v1 <= v0 {
                    continue;
                }
                let across0 = cross_start + tx + column * cx + gap / 2.;
                let across1 = across0 + cx - gap;
                if across1 <= across0 {
                    continue;
                }
                let (a, b) = (zero + sign * v0, zero + sign * v1);
                let (p0, p1) = if along_x {
                    (point(px(a), px(across0)), point(px(b), px(across1)))
                } else {
                    (point(px(across0), px(a)), point(px(across1), px(b)))
                };
                let rect = paint::rect_between(ctx.absolute(p0), ctx.absolute(p1));
                // SVG clamps a corner radius to half the rect (`rx: "100%"`
                // draws a circle).
                let r = radius
                    .min(rect.size.width.as_f32() / 2.)
                    .min(rect.size.height.as_f32() / 2.);
                paint::paint_rect(rect, r, color, ctx.window);
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
    let _ = row;
    let cell_color = |value: &Value, ctx: &DrawContext<'_>| -> Option<Hsla> {
        if value.is_null() {
            return None;
        }
        value
            .as_text()
            .and_then(|t| crate::css::parse(t))
            .or_else(|| ctx.colors.scale_color(value))
    };
    // Unprojected, the grid is an axis-aligned image, as Plot draws it: one
    // pixel per cell, built once and stretched over the grid's extent.
    if ctx.frame.projection.is_none() {
        let (Some(a), Some(b)) = (place(grid.x1, grid.y1, ctx), place(grid.x2, grid.y2, ctx))
        else {
            return;
        };
        // Grid row 0 is the low end of y, drawn at the bottom unless reversed.
        let (flip_x, flip_y) = (a.x > b.x, a.y > b.y);
        let bounds = paint::rect_between(ctx.absolute(a), ctx.absolute(b));
        // Upscaled to device pixels, nearest-neighbour, so GPUI's linear
        // sampler keeps each cell crisp, as the quads it replaces were.
        let scale = ctx.window.scale_factor();
        let device = (
            (bounds.size.width.as_f32() * scale).round().clamp(1., 4096.) as usize,
            (bounds.size.height.as_f32() * scale).round().clamp(1., 4096.) as usize,
        );
        let image = raster_image(&grid, opacity, flip_x, flip_y, device, |v| cell_color(v, ctx));
        ctx.clipped(|ctx| {
            let _ = ctx
                .window
                .paint_image(bounds, bounds, Default::default(), image, 0, false);
        });
        return;
    }
    // Projected, Plot projects each sample to the frame and fills a grid
    // laid over the whole frame from them (by the mark's `interpolate`,
    // placing each sample in its pixel by default), drawn as one image;
    // `clip: "sphere"` masks it to the projected globe.
    let Some(projection) = ctx.frame.projection.as_ref() else {
        return;
    };
    let (pw, ph) = (
        ctx.frame.plot.size.width.as_f32() as f64,
        ctx.frame.plot.size.height.as_f32() as f64,
    );
    let spec = prepared.mark.raster.clone().unwrap_or_default();
    let size = spec
        .size
        .unwrap_or(((pw.round() as usize).max(1), (ph.round() as usize).max(1)));
    let how = spec.interpolate.unwrap_or(crate::raster::Interpolate::None);
    let sphere = if prepared.mark.style.clip_sphere {
        projection.sphere()
    } else {
        None
    };
    let scale = ctx.window.scale_factor();
    let device = (
        (pw * scale as f64).round().clamp(1., 4096.) as usize,
        (ph * scale as f64).round().clamp(1., 4096.) as usize,
    );
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (std::rc::Rc::as_ptr(&grid) as usize, size, how as u8, device).hash(&mut hasher);
        (opacity.to_bits(), sphere.is_some()).hash(&mut hasher);
        // The projection's fit, by where it puts a few points.
        for (x, y) in [(0., 0.), (90., 45.), (-120., -30.)] {
            projection
                .project(x, y)
                .map(|p| p.map(f64::to_bits))
                .hash(&mut hasher);
        }
        let n = grid.values.len();
        for k in (0..n).step_by((n / 31).max(1)) {
            cell_color(&grid.values[k], ctx)
                .map(|c| [c.h, c.s, c.l, c.a].map(f32::to_bits))
                .hash(&mut hasher);
        }
        hasher.finish()
    };
    // Each entry holds its grid, so the grid's address in the key can't be
    // reused by another grid while the entry lives.
    type Entry = (
        u64,
        std::rc::Rc<crate::raster::RasterGrid>,
        std::sync::Arc<gpui::RenderImage>,
    );
    thread_local! {
        static PROJECTED: std::cell::RefCell<Vec<Entry>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    let cached = PROJECTED.with_borrow(|images| {
        images
            .iter()
            .find(|(k, g, _)| *k == key && std::rc::Rc::ptr_eq(g, &grid))
            .map(|(_, _, i)| i.clone())
    });
    let image = match cached {
        Some(image) => image,
        None => {
            let pixels = projected_raster(&grid, projection, size, [pw, ph], how);
            let (dw, dh) = device;
            let mut rgba = vec![0u8; dw * dh * 4];
            // Several device pixels fall in one resampled cell: each cell's
            // bytes are worked out once per row of cells, then copied.
            let mut row_sy = usize::MAX;
            let mut row_bytes: Vec<Option<Option<[u8; 4]>>> = vec![None; size.0];
            for y in 0..dh {
                let py_ = (y as f64 + 0.5) / scale as f64;
                // The sphere's spans on this row, even-odd.
                let spans = sphere.as_ref().map(|ring| {
                    let mut xs: Vec<f64> = (0..ring.len())
                        .filter_map(|k| {
                            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                            ((a[1] <= py_) != (b[1] <= py_))
                                .then(|| a[0] + (py_ - a[1]) / (b[1] - a[1]) * (b[0] - a[0]))
                        })
                        .collect();
                    xs.sort_by(f64::total_cmp);
                    xs
                });
                let sy = ((py_ / ph * size.1 as f64) as usize).min(size.1 - 1);
                if sy != row_sy {
                    row_sy = sy;
                    row_bytes.fill(None);
                }
                for x in 0..dw {
                    let px_ = (x as f64 + 0.5) / scale as f64;
                    if let Some(xs) = &spans
                        && xs.iter().filter(|&&e| e <= px_).count() % 2 == 0
                    {
                        continue;
                    }
                    let sx = ((px_ / pw * size.0 as f64) as usize).min(size.0 - 1);
                    let bytes = *row_bytes[sx].get_or_insert_with(|| {
                        let c =
                            gpui::Rgba::from(cell_color(&pixels.values[sy * size.0 + sx], ctx)?);
                        // GPUI's images are BGRA.
                        Some([
                            (c.b * 255.).round() as u8,
                            (c.g * 255.).round() as u8,
                            (c.r * 255.).round() as u8,
                            ((c.a * opacity).clamp(0., 1.) * 255.).round() as u8,
                        ])
                    });
                    let Some(bytes) = bytes else {
                        continue;
                    };
                    let k = (y * dw + x) * 4;
                    rgba[k..k + 4].copy_from_slice(&bytes);
                }
            }
            let image = crate::image_from_rgba(dw as u32, dh as u32, rgba);
            PROJECTED.with_borrow_mut(|images| {
                images.insert(0, (key, grid.clone(), image.clone()));
                images.truncate(8);
            });
            image
        }
    };
    let bounds = ctx.frame.plot;
    ctx.clipped(|ctx| {
        let _ = ctx
            .window
            .paint_image(bounds, bounds, Default::default(), image, 0, false);
    });
}

/// A raster's grid resampled into frame pixels through a projection, Plot's
/// way: each cell's centre projected onto a `size` grid over the frame,
/// then interpolated there. Plot triangulates the projected samples for
/// `barycentric`; a dense grid's samples are a regular lattice, so its own
/// cells, split in two, stand in for that triangulation.
fn projected_raster(
    grid: &crate::raster::RasterGrid,
    projection: &crate::geo::Fitted,
    size: (usize, usize),
    [pw, ph]: [f64; 2],
    how: crate::raster::Interpolate,
) -> crate::raster::RasterGrid {
    let (w, h) = size;
    let at = |i: usize, j: usize| -> Option<([f64; 2], f64)> {
        let value = grid.values[j * grid.width + i].as_number()?;
        if !value.is_finite() {
            return None;
        }
        let (x, y) = grid.center(i, j);
        let [px_, py_] = projection.project(x, y)?;
        Some(([px_ / pw * w as f64, py_ / ph * h as f64], value))
    };
    if how != crate::raster::Interpolate::Barycentric {
        let samples: Vec<(f64, f64, Value)> = (0..grid.height)
            .flat_map(|j| (0..grid.width).map(move |i| (i, j)))
            .filter_map(|(i, j)| {
                let value = &grid.values[j * grid.width + i];
                if value.is_null() || value.as_number().is_some_and(f64::is_nan) {
                    return None;
                }
                let (x, y) = grid.center(i, j);
                let [px_, py_] = projection.project(x, y)?;
                Some((px_, py_, value.clone()))
            })
            .collect();
        return crate::raster::RasterGrid::from_samples(&samples, w, h, [0., 0., pw, ph], how);
    }
    let points: Vec<Option<([f64; 2], f64)>> = (0..grid.height)
        .flat_map(|j| (0..grid.width).map(move |i| (i, j)))
        .map(|(i, j)| at(i, j))
        .collect();
    let mut values = vec![f64::NAN; w * h];
    let mut fill = |[a, b, c]: [([f64; 2], f64); 3]| {
        let det = (b.0[1] - c.0[1]) * (a.0[0] - c.0[0]) + (c.0[0] - b.0[0]) * (a.0[1] - c.0[1]);
        if det.abs() < 1e-12 {
            return;
        }
        let xs = [a.0[0], b.0[0], c.0[0]];
        let ys = [a.0[1], b.0[1], c.0[1]];
        let (x0, x1) = (xs.iter().copied().fold(f64::MAX, f64::min), xs.iter().copied().fold(f64::MIN, f64::max));
        let (y0, y1) = (ys.iter().copied().fold(f64::MAX, f64::min), ys.iter().copied().fold(f64::MIN, f64::max));
        // A cell torn across the antimeridian spans the frame: not a
        // triangle of the projected lattice.
        if x1 - x0 > w as f64 / 4. || y1 - y0 > h as f64 / 4. {
            return;
        }
        let (i0, i1) = ((x0 - 0.5).ceil().max(0.) as usize, (x1 - 0.5).floor().min(w as f64 - 1.));
        let (j0, j1) = ((y0 - 0.5).ceil().max(0.) as usize, (y1 - 0.5).floor().min(h as f64 - 1.));
        if i1 < 0. || j1 < 0. {
            return;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let (px_, py_) = (i as f64 + 0.5, j as f64 + 0.5);
                let l1 = ((b.0[1] - c.0[1]) * (px_ - c.0[0]) + (c.0[0] - b.0[0]) * (py_ - c.0[1])) / det;
                let l2 = ((c.0[1] - a.0[1]) * (px_ - c.0[0]) + (a.0[0] - c.0[0]) * (py_ - c.0[1])) / det;
                let l3 = 1. - l1 - l2;
                if l1 >= -1e-9 && l2 >= -1e-9 && l3 >= -1e-9 {
                    values[j * w + i] = l1 * a.1 + l2 * b.1 + l3 * c.1;
                }
            }
        }
    };
    for j in 0..grid.height.saturating_sub(1) {
        for i in 0..grid.width.saturating_sub(1) {
            let k = j * grid.width + i;
            let (p00, p10, p01, p11) = (points[k], points[k + 1], points[k + grid.width], points[k + grid.width + 1]);
            if let (Some(a), Some(b), Some(c)) = (p00, p10, p11) {
                fill([a, b, c]);
            }
            if let (Some(a), Some(b), Some(c)) = (p00, p11, p01) {
                fill([a, b, c]);
            }
        }
    }
    let mut out = crate::raster::RasterGrid::from_values(
        values.iter().map(|&v| if v.is_nan() { Value::Null } else { Value::number(v) }).collect(),
        w,
        h,
        [0., 0., pw, ph],
    );
    // Plot extrapolates beyond the samples' hull from its edge; the nearest
    // sample stands in for that.
    let samples: Vec<(f64, f64, Value)> = points
        .iter()
        .flatten()
        .map(|(p, v)| (p[0] / w as f64 * pw, p[1] / h as f64 * ph, Value::number(*v)))
        .collect();
    let nearest = crate::raster::RasterGrid::from_samples(
        &samples,
        w,
        h,
        [0., 0., pw, ph],
        crate::raster::Interpolate::Nearest,
    );
    for (k, v) in out.values.iter_mut().enumerate() {
        if v.is_null() {
            *v = nearest.values[k].clone();
        }
    }
    out
}

/// A raster grid as an RGBA image, one pixel per cell (top row first), kept
/// across repaints for the same grid, opacity and colors.
fn raster_image(
    grid: &std::rc::Rc<crate::raster::RasterGrid>,
    opacity: f32,
    flip_x: bool,
    flip_y: bool,
    device: (usize, usize),
    color: impl Fn(&Value) -> Option<Hsla>,
) -> std::sync::Arc<gpui::RenderImage> {
    let (w, h) = (grid.width, grid.height);
    // The grid is itself remembered across repaints, so its address names
    // it; a few cells' colors stand in for the color scale.
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (std::rc::Rc::as_ptr(grid) as usize, opacity.to_bits(), flip_x, flip_y, device)
            .hash(&mut hasher);
        let n = grid.values.len();
        for k in (0..n).step_by((n / 31).max(1)) {
            color(&grid.values[k])
                .map(|c| [c.h, c.s, c.l, c.a].map(f32::to_bits))
                .hash(&mut hasher);
        }
        hasher.finish()
    };
    type Images = Vec<(
        u64,
        std::rc::Rc<crate::raster::RasterGrid>,
        std::sync::Arc<gpui::RenderImage>,
    )>;
    thread_local! {
        static IMAGES: std::cell::RefCell<Images> = const { std::cell::RefCell::new(Vec::new()) };
    }
    if let Some(image) = IMAGES
        .with_borrow(|images| images.iter().find(|(k, _, _)| *k == key).map(|(_, _, i)| i.clone()))
    {
        return image;
    }
    let mut rgba = vec![0u8; w * h * 4];
    for j in 0..h {
        for i in 0..w {
            let Some(c) = color(&grid.values[j * w + i]) else {
                continue;
            };
            let c = gpui::Rgba::from(c);
            let (pi, pj) = (if flip_x { w - 1 - i } else { i }, if flip_y { h - 1 - j } else { j });
            let k = (pj * w + pi) * 4;
            // GPUI's images are BGRA, premultiplied by the sprite shader.
            let a = (c.a * opacity).clamp(0., 1.);
            rgba[k] = (c.b * 255.).round() as u8;
            rgba[k + 1] = (c.g * 255.).round() as u8;
            rgba[k + 2] = (c.r * 255.).round() as u8;
            rgba[k + 3] = (a * 255.).round() as u8;
        }
    }
    // Nearest-neighbour upscale, at least one device pixel per cell.
    let (dw, dh) = (device.0.max(w), device.1.max(h));
    let mut scaled = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let sy = y * h / dh;
        for x in 0..dw {
            let sx = x * w / dw;
            let (from, to) = ((sy * w + sx) * 4, (y * dw + x) * 4);
            scaled[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    let image = crate::image_from_rgba(dw as u32, dh as u32, scaled);
    IMAGES.with_borrow_mut(|images| {
        images.insert(0, (key, grid.clone(), image.clone()));
        images.truncate(8);
    });
    image
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
    // A grid's rings at a threshold are the same on every repaint: the last
    // few grids' rings are kept, each entry holding its grid so the address
    // it is found by can't be reused while it lives.
    type Rings = std::rc::Rc<Vec<Vec<[f64; 2]>>>;
    type Entry = (std::rc::Rc<crate::raster::RasterGrid>, u64, Rings);
    thread_local! {
        static CONTOURS: std::cell::RefCell<Vec<Entry>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    let contour = |threshold: f64| -> Rings {
        let bits = threshold.to_bits();
        if let Some(rings) = CONTOURS.with_borrow(|kept| {
            kept.iter()
                .find(|(g, t, _)| *t == bits && std::rc::Rc::ptr_eq(g, &grid))
                .map(|(_, _, rings)| rings.clone())
        }) {
            return rings;
        }
        let rings = std::rc::Rc::new(grid.contour(threshold));
        CONTOURS.with_borrow_mut(|kept| {
            kept.insert(0, (grid.clone(), bits, rings.clone()));
            kept.truncate(64);
        });
        rings
    };
    ctx.clipped(|ctx| {
        for threshold in thresholds {
            let rings: Vec<Vec<Point<Pixels>>> = contour(threshold)
                .iter()
                .filter_map(|ring| {
                    ring.iter()
                        .map(|p| place(*p, ctx))
                        .collect::<Option<Vec<_>>>()
                })
                .collect();
            if filled {
                // A constant fill paints as itself; a channel fill through
                // the color scale at the threshold.
                let color = match &prepared.mark.channels.fill {
                    Some(crate::style::ColorChannel::Constant(c)) => Some(*c),
                    _ => ctx.colors.scale_color(&Value::Number(threshold)),
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
