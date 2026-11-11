//! The primitive marks: dot, line, area, bar, rect, cell, rule, text, tick,
//! vector and link.
//!
//! Each mark projects its rows through the frame's scales and paints. Shapes
//! are clipped to the plot, so a value outside the domain stops at the frame's
//! edge the way Plot's marks do.

use gpui::{App, ContentMask, Hsla, Pixels, Point, Window, point, px};

use crate::frame::Frame;
use crate::mark::{CrossAxis, MarkKind};
use crate::paint;
use crate::render::{PreparedMark, PreparedRow};
use crate::theme::PlotTheme;
use crate::value::Value;

mod area;
mod arrow;
mod bar;
mod curve;
mod difference;
mod dot;
mod geo;
mod hexgrid;
mod image;
mod line;
mod link;
mod marker;
mod raster;
mod rect;
mod rule;
mod text;
mod tick;
mod tip;
mod tree;
mod vector;
mod waffle;

pub use curve::curve_path;
pub use dot::paint_symbol;
pub use marker::paint_markers;
pub use raster::draw_contour_grid;

use self::image::draw_image;
use area::{draw_areas_x, draw_areas_y};
use arrow::draw_arrows;
use bar::{draw_bars_x, draw_bars_y};
use difference::draw_differences;
use dot::draw_dots;
use geo::draw_geo;
use hexgrid::draw_hexgrid;
use line::draw_lines;
use link::draw_links;
use raster::draw_raster;
use rect::{draw_cells, draw_rects};
use rule::{draw_rules_x, draw_rules_y};
use text::draw_texts;
use tick::{draw_ticks_x, draw_ticks_y};
use tip::draw_tips;
use tree::draw_tree;
use vector::{draw_spikes, draw_vectors};
use waffle::draw_waffle_bars;

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
