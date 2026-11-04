//! The plot's frame: the space its marks draw in.
//!
//! Plot calls this the *plot* inside its *figure*: margins hold the axes, titles
//! and legends, and the plot's own bounds hold the marks. A [`Frame`] resolves
//! the plot's bounds from the figure's size and the margins it needs, then
//! builds the scales and axis ticks the marks read.

use gpui::{
    App, Bounds, FontWeight, Hsla, Pixels, Point, Size, TextAlign, Window, font, point, px,
};
use gpui_kit::base::plot::label::TEXT_SIZE;
use gpui_kit::base::plot::{AxisLabelSide, AxisText, PlotAxis};

use crate::facet::{FACET_LABEL_GAP, Grid, band};
use crate::paint::{self, Label};
use crate::scale::{ContinuousScale, PositionScale, Transform};
use crate::theme::PlotTheme;
use crate::ticks::{format_linear_tick, format_time_tick};
use crate::value::PlotKind;

/// The space reserved around the plot for its frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Where a positional channel's scale sits and how it is labelled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScaleKind {
    /// A continuous scale; Plot's `linear`, `log`, `pow`, `sqrt`, `symlog` and
    /// `time`.
    #[default]
    Continuous,
    /// A band scale, Plot's `band`.
    Band,
    /// A point scale, Plot's `point`.
    Point,
}

impl ScaleKind {
    /// The scale kind Plot's `x`/`y` option names imply for a channel kind.
    pub fn for_kind(kind: PlotKind) -> Self {
        if kind.is_quantitative() {
            ScaleKind::Continuous
        } else {
            ScaleKind::Point
        }
    }
}

/// How the frame resolves its scales.
#[derive(Clone, Debug)]
pub struct FrameOptions {
    /// The scale kind for the x channel; `None` infers it from the data.
    pub x_kind: Option<ScaleKind>,
    /// The scale kind for the y channel; `None` infers it from the data.
    pub y_kind: Option<ScaleKind>,
    /// The transform for a continuous x scale.
    pub x_transform: Transform,
    /// The transform for a continuous y scale.
    pub y_transform: Transform,
    /// The exponent of a `pow` x scale.
    pub x_exponent: f64,
    /// The exponent of a `pow` y scale.
    pub y_exponent: f64,
    /// A pinned x domain, overriding the data's extent.
    pub x_domain: Option<[f64; 2]>,
    /// A pinned y domain, overriding the data's extent.
    pub y_domain: Option<[f64; 2]>,
    /// Whether the x domain includes zero when the data does not reach it,
    /// Plot's default for quantitative channels.
    pub x_zero: bool,
    /// Whether the y domain includes zero, as Plot's bar charts do.
    pub y_zero: bool,
    /// Whether the x domain is widened to round values, Plot's `nice`.
    pub x_nice: bool,
    /// Whether the y domain is widened to round values.
    pub y_nice: bool,
    /// Whether x positions outside the domain clamp to the frame's edge,
    /// Plot's `clamp`.
    pub x_clamp: bool,
    /// Whether y positions outside the domain clamp to the frame's edge.
    pub y_clamp: bool,
    /// A pinned x range in pixels, replacing the plot's own extent, Plot's
    /// `range`.
    pub x_range: Option<[f32; 2]>,
    /// A pinned y range in pixels.
    pub y_range: Option<[f32; 2]>,
    /// Whether to reverse the x range, Plot's `reverse`.
    pub x_reverse: bool,
    /// Whether to reverse the y range.
    pub y_reverse: bool,
    /// The inset between a band scale's range and the plot's edge, Plot's
    /// `inset`.
    pub inset: f32,
    /// The inner padding of a band scale, Plot's `padding`.
    pub padding: f32,
    /// The outer padding of a band scale.
    pub padding_outer: f32,
    /// How many ticks the x axis asks for.
    pub x_tick_count: usize,
    /// How many ticks the y axis asks for.
    pub y_tick_count: usize,
    /// The space the frame reserves around the whole grid for its title,
    /// legend and facet labels.
    pub insets: Insets,
    /// The space one facet reserves inside its own band for its axes and
    /// labels. A faceted figure is a grid of small charts, so every facet
    /// repeats the axis gutters rather than sharing one at the figure's edge.
    pub facet_insets: Insets,
    /// The size of one facet's marks area, when the figure is faceted, Plot's
    /// `innerDimensions`. The x and y scales are laid out in this size rather
    /// than across the whole figure, because every facet shares them.
    pub facet_size: Option<[f32; 2]>,
}

impl Default for FrameOptions {
    fn default() -> Self {
        Self {
            x_kind: None,
            y_kind: None,
            x_transform: Transform::Linear,
            y_transform: Transform::Linear,
            x_exponent: 1.,
            y_exponent: 1.,
            x_domain: None,
            y_domain: None,
            x_zero: false,
            y_zero: false,
            x_nice: false,
            y_nice: false,
            x_clamp: false,
            y_clamp: false,
            x_range: None,
            y_range: None,
            x_reverse: false,
            y_reverse: false,
            inset: 0.,
            // Plot's band scale pads 0.1 inside and outside, so bars have a
            // gap between them and a margin at the plot's edges.
            padding: 0.1,
            padding_outer: 0.1,
            x_tick_count: 6,
            y_tick_count: 5,
            insets: Insets::default(),
            facet_insets: Insets::default(),
            facet_size: None,
        }
    }
}

/// One axis: its scale and its ticks.
#[derive(Clone, Debug)]
pub struct Axis {
    pub scale: PositionScale,
    pub kind: PlotKind,
    /// The tick positions along the axis, in plot-relative pixels.
    pub ticks: Vec<AxisTick>,
    /// Whether the channel is a date, which picks a time format.
    pub temporal: bool,
    /// The number of categories for a discrete axis, used for label spacing.
    pub categories: usize,
}

/// A labelled tick on an axis.
#[derive(Clone, Debug)]
pub struct AxisTick {
    /// The value the tick marks.
    pub value: crate::value::Value,
    /// Its position along the axis, in plot-relative pixels.
    pub position: f32,
    /// Its label.
    pub label: gpui::SharedString,
}

/// The resolved geometry and scales of one plot.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The figure's bounds.
    pub bounds: Bounds<Pixels>,
    /// The plot's bounds inside the figure: the area the facets' bands fill.
    pub plot: Bounds<Pixels>,
    /// The space the figure reserves around `plot` for its title, legend and
    /// facet labels.
    pub insets: Insets,
    /// The space each facet reserves inside its band for its own axes.
    pub facet_insets: Insets,
    /// The size the x and y scales are laid out in: the plot's size, or one
    /// facet's when the figure is faceted.
    pub layout: [f32; 2],
    pub x: Axis,
    pub y: Axis,
}

impl Frame {
    /// Resolve a frame from the figure's bounds, the data's extents, and the
    /// options.
    ///
    /// `x_extent` and `y_extent` are the domains the data reaches; a categorical
    /// channel passes its distinct values instead.
    pub fn resolve(
        bounds: Bounds<Pixels>,
        options: &FrameOptions,
        x_values: &[crate::value::Value],
        y_values: &[crate::value::Value],
    ) -> Self {
        let insets = options.insets;
        let plot_width = (bounds.size.width.as_f32() - insets.left - insets.right).max(1.);
        let plot_height = (bounds.size.height.as_f32() - insets.top - insets.bottom).max(1.);
        let plot = Bounds {
            origin: bounds.origin + Point::new(px(insets.left), px(insets.top)),
            size: Size::new(px(plot_width), px(plot_height)),
        };

        // A faceted figure lays its scales out in one facet, which every facet
        // shares; the frame's own bounds still hold the whole grid.
        let [facet_width, facet_height] = options.facet_size.unwrap_or([plot_width, plot_height]);

        let x = resolve_axis(
            x_values,
            &AxisOptions {
                kind: options.x_kind,
                transform: options.x_transform,
                exponent: options.x_exponent,
                pinned: options.x_domain,
                zero: options.x_zero,
                nice: options.x_nice,
                clamp: options.x_clamp,
                pinned_range: options.x_range,
                reverse: options.x_reverse,
                inset: options.inset,
                padding: options.padding,
                padding_outer: options.padding_outer,
                tick_count: options.x_tick_count,
                range: [0., facet_width],
                axis: AxisSide::X,
            },
        );
        let y = resolve_axis(
            y_values,
            &AxisOptions {
                kind: options.y_kind,
                transform: options.y_transform,
                exponent: options.y_exponent,
                pinned: options.y_domain,
                zero: options.y_zero,
                nice: options.y_nice,
                clamp: options.y_clamp,
                pinned_range: options.y_range,
                reverse: options.y_reverse,
                inset: options.inset,
                padding: options.padding,
                padding_outer: options.padding_outer,
                tick_count: options.y_tick_count,
                range: [facet_height, 0.],
                axis: AxisSide::Y,
            },
        );

        Self {
            bounds,
            plot,
            insets,
            facet_insets: options.facet_insets,
            layout: [facet_width, facet_height],
            x,
            y,
        }
    }

    /// The plot-relative point for a pair of channel values, or `None` when
    /// either does not scale.
    pub fn point(&self, x: &crate::value::Value, y: &crate::value::Value) -> Option<Point<Pixels>> {
        Some(Point::new(
            px(self.x.scale.position(x)?),
            px(self.y.scale.position(y)?),
        ))
    }

    /// The plot-relative point, moved into the figure's coordinates.
    pub fn absolute(&self, point: Point<Pixels>) -> Point<Pixels> {
        point + self.plot.origin - self.bounds.origin
    }

    /// A copy of this frame for one facet of a faceted figure.
    ///
    /// The scales are already laid out in one facet's size (see
    /// [`FrameOptions::facet_size`]), which every facet shares, so only the
    /// plot moves: it sits at the facet's marks area within its band, while
    /// the frame's bounds still hold the whole figure. The ticks come along
    /// unchanged, because they are the same values at the same offsets within
    /// each facet.
    pub fn facet(&self, plot: Bounds<Pixels>) -> Self {
        Self {
            plot,
            ..self.clone()
        }
    }
}

/// How one axis resolves its scale, the per-axis half of [`FrameOptions`].
struct AxisOptions {
    kind: Option<ScaleKind>,
    transform: Transform,
    exponent: f64,
    pinned: Option<[f64; 2]>,
    zero: bool,
    nice: bool,
    clamp: bool,
    pinned_range: Option<[f32; 2]>,
    reverse: bool,
    inset: f32,
    padding: f32,
    padding_outer: f32,
    tick_count: usize,
    range: [f32; 2],
    /// Which axis this is, for tick placement.
    axis: AxisSide,
}

/// Which axis a resolved scale belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AxisSide {
    X,
    Y,
}

fn resolve_axis(values: &[crate::value::Value], options: &AxisOptions) -> Axis {
    let AxisOptions {
        kind,
        transform,
        exponent,
        pinned,
        zero,
        nice,
        clamp,
        pinned_range,
        reverse,
        inset,
        padding,
        padding_outer,
        tick_count,
        range,
        axis,
    } = *options;
    let is_x = axis == AxisSide::X;
    let inferred = values
        .iter()
        .find(|v| !v.is_null())
        .map(|v| v.kind())
        .unwrap_or(PlotKind::Null);
    let kind = kind.unwrap_or_else(|| ScaleKind::for_kind(inferred));
    let temporal = inferred == PlotKind::Date;
    let range = pinned_range.unwrap_or(range);
    let range = [range[0] + inset, range[1] - inset];
    let range = if reverse { [range[1], range[0]] } else { range };

    match kind {
        ScaleKind::Continuous => {
            let domain = pinned
                .or_else(|| crate::scale::extent(values.iter().cloned()))
                .unwrap_or([0., 1.]);
            let mut domain = domain;
            if zero {
                domain[0] = domain[0].min(0.);
                domain[1] = domain[1].max(0.);
            }
            let mut scale = ContinuousScale::new(transform, domain, range).with_exponent(exponent);
            if nice {
                scale = scale.nice(tick_count);
            }
            if clamp {
                scale = scale.clamped(true);
            }
            let domain = scale.domain();
            let ticks = if temporal {
                let millis = scale.time_ticks(tick_count);
                millis
                    .into_iter()
                    .filter_map(|millis| {
                        scale.position(millis).map(|position| AxisTick {
                            value: crate::value::Value::date_from_millis(millis as i64),
                            position,
                            label: format_time_tick(millis, domain),
                        })
                    })
                    .collect()
            } else {
                let step = tick_step(domain, tick_count);
                scale
                    .ticks(tick_count)
                    .into_iter()
                    .filter_map(|tick| {
                        scale.position(tick).map(|position| AxisTick {
                            value: crate::value::Value::Number(tick),
                            position,
                            label: format_linear_tick(tick, step),
                        })
                    })
                    .collect()
            };
            Axis {
                scale: PositionScale::Continuous(scale),
                kind: inferred,
                ticks,
                temporal,
                categories: 0,
            }
        }
        ScaleKind::Band => {
            let domain: Vec<_> = crate::scale::dedup(values.iter().map(|v| v.key()));
            let categories = domain.len();
            let scale = crate::scale::BandScale::new(domain.clone(), range)
                .padding_inner(padding)
                .padding_outer(padding_outer);
            let ticks = band_ticks(&domain, &scale, values, is_x, tick_count);
            Axis {
                scale: PositionScale::Band(scale),
                kind: inferred,
                ticks,
                temporal: false,
                categories,
            }
        }
        ScaleKind::Point => {
            let domain: Vec<_> = crate::scale::dedup(values.iter().map(|v| v.key()));
            let categories = domain.len();
            let scale = crate::scale::PointScale::new(domain.clone(), range);
            let ticks = point_ticks(&domain, &scale, values, is_x, tick_count);
            Axis {
                scale: PositionScale::Point(scale),
                kind: inferred,
                ticks,
                temporal: false,
                categories,
            }
        }
    }
}

/// The step the linear ticks will use, for label precision.
fn tick_step(domain: [f64; 2], count: usize) -> f64 {
    let ticks = crate::ticks::linear_ticks(domain, count, Transform::Linear);
    match ticks.windows(2).next() {
        Some(pair) => (pair[1] - pair[0]).abs(),
        None => 1.,
    }
}

fn band_ticks(
    domain: &[crate::value::ValueKey],
    scale: &crate::scale::BandScale,
    values: &[crate::value::Value],
    is_x: bool,
    tick_count: usize,
) -> Vec<AxisTick> {
    // A discrete axis shows one label per category: a category's label is its
    // only identity, so dropping some would leave bars or boxes unlabelled.
    // The axis only strides them when there are so many that the labels would
    // collide, which is what `tick_count` bounds.
    let _ = tick_count;
    let stride = 1;
    let _ = is_x;
    domain
        .iter()
        .enumerate()
        .filter(|(index, _)| index % stride == 0)
        .filter_map(|(_, key)| {
            let value = key.to_value();
            let center = scale.center(key)?;
            let label = category_label(&value, values);
            Some(AxisTick {
                value,
                position: center,
                label,
            })
        })
        .collect()
}

fn point_ticks(
    domain: &[crate::value::ValueKey],
    scale: &crate::scale::PointScale,
    values: &[crate::value::Value],
    _is_x: bool,
    _tick_count: usize,
) -> Vec<AxisTick> {
    // A point scale labels every category: the labels are the categories.
    let stride = 1;
    domain
        .iter()
        .enumerate()
        .filter(|(index, _)| index % stride == 0)
        .filter_map(|(_, key)| {
            let value = key.to_value();
            let position = scale.position(key)?;
            let label = category_label(&value, values);
            Some(AxisTick {
                value,
                position,
                label,
            })
        })
        .collect()
}

/// A category's axis label: its own text, or its label.
fn category_label(
    value: &crate::value::Value,
    _values: &[crate::value::Value],
) -> gpui::SharedString {
    value.label()
}

/// Paint the grid lines across the plot, under the marks.
///
/// A faceted figure draws its grid inside each facet's marks area, the same
/// way its axes repeat there. Vertical lines appear only for a continuous x
/// scale: a band scale's categories are already clear from their labels.
pub(crate) fn paint_grid(
    frame: &Frame,
    theme: &PlotTheme,
    window: &mut Window,
    x_axis: bool,
    y_axis: bool,
    grid: &Grid,
) {
    let areas: Vec<Bounds<Pixels>> = if grid.is_faceted() {
        (0..grid.cells.len())
            .filter(|cell| !grid.is_empty(*cell))
            .map(|cell| facet_marks(grid.facet(frame.plot, cell), frame))
            .collect()
    } else {
        vec![frame.plot]
    };
    let width = theme.line_width;
    for plot in areas {
        if y_axis {
            for tick in frame.y.ticks.iter() {
                let y = plot.origin.y + px(tick.position);
                let start = point(plot.origin.x, y);
                let end = point(plot.origin.x + plot.size.width, y);
                if theme.grid_dash.is_empty() {
                    paint::paint_stroke(&[start, end], width.as_f32(), theme.grid, window);
                } else {
                    paint_dashed(start, end, &theme.grid_dash, width, theme.grid, window);
                }
            }
        }
        if x_axis && !frame.x.scale.is_discrete() {
            for tick in frame.x.ticks.iter() {
                let x = plot.origin.x + px(tick.position);
                let start = point(x, plot.origin.y);
                let end = point(x, plot.origin.y + plot.size.height);
                if theme.grid_dash.is_empty() {
                    paint::paint_stroke(&[start, end], width.as_f32(), theme.grid, window);
                } else {
                    paint_dashed(start, end, &theme.grid_dash, width, theme.grid, window);
                }
            }
        }
    }
}

/// Paint the axis lines and their tick labels.
pub(crate) fn paint_axes(
    frame: &Frame,
    theme: &PlotTheme,
    window: &mut Window,
    cx: &mut App,
    x_axis: bool,
    y_axis: bool,
    edges: &AxisEdges,
) {
    let plot = frame.plot;
    let mut axis = PlotAxis::new().stroke(theme.axis);

    if x_axis && edges.bottom {
        let labels: Vec<AxisText> = frame
            .x
            .ticks
            .iter()
            .map(|tick| {
                AxisText::new(tick.label.clone(), px(tick.position), theme.label)
                    .font_size(theme.label_size)
                    .align(TextAlign::Center)
            })
            .collect();
        axis = axis.x(plot.size.height).x_axis(false).x_label(labels);
    }
    if y_axis && edges.left {
        let labels: Vec<AxisText> = frame
            .y
            .ticks
            .iter()
            .map(|tick| {
                // `PlotAxis` centers a y label by subtracting `TEXT_SIZE / 2`
                // from its tick, which assumes the default 10px labels. A plot
                // with a larger font would sit its labels high, so the tick is
                // nudged by the difference.
                let shift = (theme.label_size.as_f32() - TEXT_SIZE) / 2.;
                AxisText::new(tick.label.clone(), px(tick.position - shift), theme.label)
                    .font_size(theme.label_size)
                    .align(TextAlign::Right)
            })
            .collect();
        // The labels belong in the gutter the frame reserved for them, left
        // of the line; the default `End` side would draw them inside the plot,
        // where the line cuts through them.
        axis = axis
            .y(px(0.))
            .y_axis(false)
            .y_label_side(AxisLabelSide::Start)
            .y_label(labels);
    }
    axis.paint(&plot, window, cx);
    if y_axis && edges.left {
        // `PlotAxis` draws the y line only where it was told to, and it
        // right-aligns its labels against that line, so the line is drawn
        // here rather than duplicated there.
        paint::paint_stroke(
            &[
                point(plot.origin.x, plot.origin.y),
                point(plot.origin.x, plot.origin.y + plot.size.height),
            ],
            theme.line_width.as_f32(),
            theme.axis,
            window,
        );
    }
}

/// Paint the facet axis: the `fx` values above each column, the `fy` values
/// right of each row.
///
/// Plot's facet axes label the grid rather than the data, so each label is
/// centred on its band and sits outside the plot.
pub(crate) fn paint_facet_labels(
    frame: &Frame,
    theme: &PlotTheme,
    window: &mut Window,
    cx: &mut App,
    grid: &Grid,
) {
    let plot = frame.plot;
    let font = font(theme.font_family.clone());
    let color = theme.label;

    if let Some(fx) = &grid.fx {
        let labels = grid.fx_labels();
        for (index, text) in labels.into_iter().enumerate() {
            let (start, width) = band(index, fx.len(), plot.size.width.as_f32());
            let label = Label::new(
                text,
                theme.label_size,
                color,
                FontWeight::NORMAL,
                font.clone(),
                window,
            );
            // Above the plot, in the gutter the insets reserved for it.
            let y = plot.origin.y - px(theme.label_size.as_f32() + FACET_LABEL_GAP);
            label.paint_at(
                point(plot.origin.x + px(start + width / 2.), y),
                TextAlign::Center,
                None,
                window,
                cx,
            );
        }
    }

    if let Some(fy) = &grid.fy {
        let labels = grid.fy_labels();
        for (index, text) in labels.into_iter().enumerate() {
            let (start, height) = band(index, fy.len(), plot.size.height.as_f32());
            let label = Label::new(
                text,
                theme.label_size,
                color,
                FontWeight::NORMAL,
                font.clone(),
                window,
            );
            // Right of the plot, centred on the facet's band.
            let x = plot.origin.x + plot.size.width + px(FACET_LABEL_GAP);
            label.paint_at(
                point(
                    x,
                    plot.origin.y + px(start + height / 2. - theme.label_size.as_f32() / 2.),
                ),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}

/// The marks area of one facet band: the band, less the axes the facet draws
/// inside it.
pub(crate) fn facet_marks(band: Bounds<Pixels>, frame: &Frame) -> Bounds<Pixels> {
    let insets = frame.facet_insets;
    Bounds::new(
        point(
            band.origin.x + px(insets.left),
            band.origin.y + px(insets.top),
        ),
        gpui::size(
            px((band.size.width.as_f32() - insets.left - insets.right).max(1.)),
            px((band.size.height.as_f32() - insets.top - insets.bottom).max(1.)),
        ),
    )
}

/// Which of a facet's edges draw an axis, Plot's `facetAnchor`.
///
/// A left y axis and a bottom x axis run to the outermost facet of each row
/// and column, so the grid reads like a table of small charts.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AxisEdges {
    pub(crate) left: bool,
    pub(crate) bottom: bool,
}

impl Default for AxisEdges {
    fn default() -> Self {
        Self {
            left: true,
            bottom: true,
        }
    }
}

impl AxisEdges {
    /// The edges of facet `cell` that draw an axis.
    ///
    /// A y axis belongs to the first column and an x axis to the last row,
    /// which is Plot's `facetAnchorLeft` and `facetAnchorBottom`. A facet
    /// further in draws the axis too when the neighbour it would share it
    /// with holds no data, which is the `-empty` variant those anchors take
    /// by default: the axis then stops where the facets do.
    pub(crate) fn from_cell(cell: usize, grid: &Grid) -> Self {
        let (column, row) = (grid.column(cell), grid.row(cell));
        let left = column == 0 || grid.is_empty(cell - 1);
        let below = cell + grid.columns();
        let bottom = row == grid.rows() - 1 || below >= grid.cells.len() || grid.is_empty(below);
        Self { left, bottom }
    }
}

/// Paint a dashed line from `start` to `end`.
fn paint_dashed(
    start: Point<Pixels>,
    end: Point<Pixels>,
    dash: &[Pixels],
    width: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    let dx = end.x.as_f32() - start.x.as_f32();
    let dy = end.y.as_f32() - start.y.as_f32();
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0. || dash.is_empty() {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let mut offset = 0.0_f32;
    let mut on = true;
    let mut index = 0;
    while offset < length {
        let segment = dash[index % dash.len()].as_f32().max(0.5);
        let end_offset = (offset + segment).min(length);
        if on {
            let from = point(
                px(start.x.as_f32() + ux * offset),
                px(start.y.as_f32() + uy * offset),
            );
            let to = point(
                px(start.x.as_f32() + ux * end_offset),
                px(start.y.as_f32() + uy * end_offset),
            );
            paint::paint_stroke(&[from, to], width.as_f32(), color, window);
        }
        offset = end_offset;
        on = !on;
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use gpui::{bounds, point, px, size};

    #[test]
    fn continuous_frame_positions_values() {
        let frame = Frame::resolve(
            bounds(point(px(0.), px(0.)), size(px(400.), px(300.))),
            &FrameOptions {
                insets: Insets {
                    top: 0.,
                    right: 0.,
                    bottom: 30.,
                    left: 40.,
                },
                ..Default::default()
            },
            &[Value::Number(0.), Value::Number(10.)],
            &[Value::Number(0.), Value::Number(100.)],
        );
        assert_eq!(frame.plot.size.width.as_f32(), 360.);
        assert_eq!(frame.plot.size.height.as_f32(), 270.);
        // The y axis is reversed: its zero is at the plot's bottom.
        let origin = frame.point(&Value::Number(0.), &Value::Number(0.)).unwrap();
        assert_eq!(origin.x.as_f32(), 0.);
        assert_eq!(origin.y.as_f32(), 270.);
    }

    #[test]
    fn categorical_frame_uses_points() {
        let frame = Frame::resolve(
            bounds(point(px(0.), px(0.)), size(px(300.), px(200.))),
            &FrameOptions::default(),
            &[Value::text("a"), Value::text("b"), Value::text("c")],
            &[Value::Number(1.), Value::Number(2.), Value::Number(3.)],
        );
        let a = frame.x.scale.position(&Value::text("a")).unwrap();
        let c = frame.x.scale.position(&Value::text("c")).unwrap();
        assert_eq!(a, 0.);
        assert_eq!(c, 300.);
    }

    #[test]
    fn pinned_domain_overrides_extent() {
        let frame = Frame::resolve(
            bounds(point(px(0.), px(0.)), size(px(100.), px(100.))),
            &FrameOptions {
                y_domain: Some([0., 10.]),
                ..Default::default()
            },
            &[Value::Number(1.)],
            &[Value::Number(2.)],
        );
        let two = frame.y.scale.position(&Value::Number(2.)).unwrap();
        assert_eq!(two, 80.);
    }

    #[test]
    fn ticks_are_labeled() {
        let frame = Frame::resolve(
            bounds(point(px(0.), px(0.)), size(px(400.), px(300.))),
            &FrameOptions::default(),
            &[Value::Number(0.), Value::Number(100.)],
            &[Value::Number(0.), Value::Number(100.)],
        );
        assert!(frame.x.ticks.len() >= 4);
        assert!(frame.x.ticks.iter().any(|tick| tick.label == "20"));
    }

    #[test]
    fn facet_axes_run_along_the_first_column_and_last_row() {
        let grid = Grid::from_values(
            vec![
                crate::value::Value::text("a"),
                crate::value::Value::text("b"),
                crate::value::Value::text("c"),
            ],
            vec![
                crate::value::Value::text("1"),
                crate::value::Value::text("2"),
            ],
        );
        let edges = |cell: usize| AxisEdges::from_cell(cell, &grid);
        // The first row: a y axis on the first facet only, an x axis nowhere
        // because another row sits below it.
        assert!(edges(0).left && !edges(0).bottom);
        assert!(!edges(1).left && !edges(1).bottom);
        // The last row draws the x axis under every facet, and the y axis
        // still only on the first column.
        assert!(edges(3).left && edges(3).bottom);
        assert!(!edges(4).left && edges(4).bottom);
    }

    #[test]
    fn an_empty_facet_hands_its_axis_to_its_neighbour() {
        let mut grid = Grid::from_values(
            vec![
                crate::value::Value::text("a"),
                crate::value::Value::text("b"),
            ],
            vec![
                crate::value::Value::text("1"),
                crate::value::Value::text("2"),
            ],
        );
        // Only the second row's first facet holds data: the row above it is
        // empty, so that facet draws the x axis too, and the one beside it
        // draws the y axis.
        grid.set_empty(&[false, false, true, false]);
        assert!(AxisEdges::from_cell(2, &grid).left);
        assert!(AxisEdges::from_cell(2, &grid).bottom);
        assert!(!AxisEdges::from_cell(3, &grid).left);
    }
}
