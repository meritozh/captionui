//! The plot's frame: the space its marks draw in.
//!
//! Plot calls this the *plot* inside its *figure*: margins hold the axes, titles
//! and legends, and the plot's own bounds hold the marks. A [`Frame`] resolves
//! the plot's bounds from the figure's size and the margins it needs, then
//! builds the scales and axis ticks the marks read.

use gpui::{
    App, Bounds, FontWeight, Hsla, Pixels, Point, Size, TextAlign, Window, font, point, px,
};

use crate::facet::{FACET_LABEL_GAP, Grid, band};
use crate::paint::{self, Label};
use crate::scale::{ContinuousScale, PositionScale, Transform};
use crate::theme::PlotTheme;
use crate::ticks::format_time_tick;
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
    /// The x scale's options, Plot's `x: {…}`, read where [`FrameOptions`]'
    /// own fields do not cover them (an ordinal domain, ticks, formats).
    pub x_options: crate::options::ScaleOptions,
    /// The y scale's options, Plot's `y: {…}`.
    pub y_options: crate::options::ScaleOptions,
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
            // Zero asks for Plot's default: one tick per 80 pixels across,
            // one per 35 down.
            x_tick_count: 0,
            y_tick_count: 0,
            x_options: Default::default(),
            y_options: Default::default(),
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
    /// Plot's `projection`, fit to one facet's marks area: when set, marks
    /// place `x`/`y` as longitude/latitude through it, and `geo` marks draw
    /// their geometry with it.
    pub projection: Option<crate::geo::Fitted>,
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
                scale: &options.x_options,
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
                scale: &options.y_options,
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
            projection: None,
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
struct AxisOptions<'a> {
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
    /// Plot's scale options, for what the fields above do not carry.
    scale: &'a crate::options::ScaleOptions,
}

/// Which axis a resolved scale belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AxisSide {
    X,
    Y,
}

fn resolve_axis(values: &[crate::value::Value], options: &AxisOptions) -> Axis {
    use crate::options::{ScaleType, Ticks};
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
        scale: plot,
    } = *options;
    let is_x = axis == AxisSide::X;
    // Plot's scale options override the frame's fields they overlap.
    let (kind, transform, exponent) = match plot.r#type {
        Some(ScaleType::Log) => (Some(ScaleKind::Continuous), Transform::Log, exponent),
        Some(ScaleType::Sqrt) => (Some(ScaleKind::Continuous), Transform::Sqrt, 0.5),
        Some(ScaleType::Pow) => (
            Some(ScaleKind::Continuous),
            Transform::Pow,
            plot.exponent.unwrap_or(exponent),
        ),
        Some(ScaleType::Symlog) => (Some(ScaleKind::Continuous), Transform::Symlog, exponent),
        Some(ScaleType::Linear | ScaleType::Utc) => {
            (Some(ScaleKind::Continuous), Transform::Linear, exponent)
        }
        Some(ScaleType::Band) => (Some(ScaleKind::Band), transform, exponent),
        Some(ScaleType::Point | ScaleType::Ordinal) => {
            (Some(ScaleKind::Point), transform, exponent)
        }
        _ => (kind, transform, plot.exponent.unwrap_or(exponent)),
    };
    let zero = plot.zero.unwrap_or(zero);
    let nice = plot.nice.unwrap_or(nice);
    let clamp = plot.clamp.unwrap_or(clamp);
    let reverse = plot.reverse.unwrap_or(reverse);
    let inset = plot.inset.unwrap_or(inset);
    let padding_inner = plot.padding_inner.or(plot.padding).unwrap_or(padding);
    let padding_outer = plot.padding_outer.or(plot.padding).unwrap_or(padding_outer);
    let percent = plot.percent.unwrap_or(false);
    let inferred = values
        .iter()
        .chain(plot.domain.iter().flatten())
        .find(|v| !v.is_null())
        .map(|v| v.kind())
        .unwrap_or(PlotKind::Null);
    let kind = kind.unwrap_or_else(|| ScaleKind::for_kind(inferred));
    let temporal = inferred == PlotKind::Date || plot.r#type == Some(ScaleType::Utc);
    let range = pinned_range.unwrap_or(range);
    let range = [
        // Plot's `insetLeft` (…) replaces the `inset` on its side.
        range[0] + plot.inset_start.unwrap_or(inset) * range_sign(range),
        range[1] - plot.inset_end.unwrap_or(inset) * range_sign(range),
    ];
    let range = if reverse { [range[1], range[0]] } else { range };
    // Plot's default tick density: one per 80 pixels across x, one per 35
    // pixels down y.
    let tick_count = match plot.ticks {
        Some(Ticks::Count(count)) => count,
        _ if tick_count > 0 => tick_count,
        _ => {
            // Plot's `tickSpacing`: one tick per 80 pixels across x, per 35
            // down y; d3 reads the fractional count, which rounding stands in
            // for.
            let length = (range[1] - range[0]).abs();
            ((length / if is_x { 80. } else { 35. }).round() as usize).max(1)
        }
    };
    let label = |value: &crate::value::Value, default: gpui::SharedString| -> gpui::SharedString {
        match &plot.tick_format {
            Some(format) => {
                let shown = match (percent, value) {
                    (true, crate::value::Value::Number(n)) => crate::value::Value::Number(n * 100.),
                    _ => value.clone(),
                };
                format.format(&shown).into()
            }
            None => default,
        }
    };

    match kind {
        ScaleKind::Continuous => {
            let domain = plot
                .numeric_domain()
                .or(pinned)
                .or_else(|| crate::scale::extent(values.iter().cloned()))
                .unwrap_or([0., 1.]);
            let mut domain = domain;
            if zero {
                domain[0] = domain[0].min(0.);
                domain[1] = domain[1].max(0.);
            }
            let mut scale = ContinuousScale::new(transform, domain, range).with_exponent(exponent);
            if let Some(constant) = plot.constant {
                scale = scale.with_constant(constant);
            }
            if nice {
                scale = scale.nice(tick_count);
            }
            if clamp {
                scale = scale.clamped(true);
            }
            let domain = scale.domain();
            let explicit: Option<Vec<f64>> = match &plot.ticks {
                Some(Ticks::Values(values)) => {
                    Some(values.iter().filter_map(|v| v.as_number()).collect())
                }
                Some(Ticks::Interval(interval)) => {
                    Some(interval_ticks(*interval, domain, temporal))
                }
                _ => None,
            };
            let ticks = if temporal {
                let millis = explicit.unwrap_or_else(|| scale.time_ticks(tick_count));
                // Plot's multi-line time labels, `Jan\n2021`.
                let top = is_x && plot.axis == Some(Some(crate::options::AxisAnchor::Top));
                let plot_labels = crate::ticks::plot_time_labels(&millis, top);
                let _ = format_time_tick;
                millis
                    .into_iter()
                    .zip(plot_labels)
                    .filter_map(|(millis, text)| {
                        let value = crate::value::Value::date_from_millis(millis as i64);
                        scale.position(millis).map(|position| AxisTick {
                            label: label(&value, text.into()),
                            value,
                            position,
                        })
                    })
                    .collect()
            } else {
                // A percent scale labels its proportions as percentages: the
                // ticks are nice in percent, then mapped back.
                let factor = if percent { 100. } else { 1. };
                let shown = [domain[0] * factor, domain[1] * factor];
                let log = transform == Transform::Log;
                let ticks = explicit.unwrap_or_else(|| {
                    if log {
                        crate::ticks::d3_log_ticks(domain[0], domain[1], tick_count)
                    } else if percent {
                        crate::ticks::linear_ticks(shown, tick_count, Transform::Linear)
                            .into_iter()
                            .map(|t| t / factor)
                            .collect()
                    } else {
                        scale.ticks(tick_count)
                    }
                });
                let spec = match &plot.tick_format {
                    Some(crate::options::TickFormat::Spec(spec)) => Some(spec.to_string()),
                    _ => None,
                };
                let custom = matches!(plot.tick_format, Some(crate::options::TickFormat::Fn(_)));
                // d3's tick formats: the precision follows the tick step, and
                // a log scale labels only the multiples the count has room for.
                let linear_format =
                    crate::format::tick_format(shown[0], shown[1], tick_count, spec.as_deref());
                let total = ticks.len();
                ticks
                    .into_iter()
                    .filter_map(|tick| {
                        let value = crate::value::Value::Number(tick);
                        let text: gpui::SharedString = if custom {
                            label(&value, gpui::SharedString::default())
                        } else if log && spec.is_none() {
                            crate::ticks::d3_log_tick_label(tick, tick_count, total, None).into()
                        } else {
                            linear_format.format(tick * factor).into()
                        };
                        scale.position(tick).map(|position| AxisTick {
                            label: text,
                            value,
                            position,
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
        ScaleKind::Band | ScaleKind::Point => {
            // Plot lays an ordinal y scale out top-down: its first value at
            // the top, where a continuous y puts its maximum.
            let range = if is_x { range } else { [range[1], range[0]] };
            // An explicit domain fixes the categories and their order;
            // otherwise Plot sorts the distinct values ascending.
            let domain: Vec<_> = match &plot.domain {
                Some(domain) => crate::scale::dedup(domain.iter().map(|v| v.key())),
                // An interval fills the domain with every step from the
                // least value to the greatest.
                None if plot.interval.is_some() => {
                    let interval = plot.interval.expect("checked");
                    let numbers: Vec<f64> = values.iter().filter_map(|v| v.as_number()).collect();
                    let lo = numbers.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    let temporal = values
                        .iter()
                        .any(|v| matches!(v, crate::value::Value::Date(_)));
                    let make = |n: f64| {
                        if temporal {
                            crate::value::Value::date_from_millis(n as i64)
                        } else {
                            crate::value::Value::Number(n)
                        }
                    };
                    let mut keys = Vec::new();
                    if lo.is_finite() && hi.is_finite() {
                        let mut current = interval
                            .extent(&make(lo))
                            .map(|(start, _)| start)
                            .unwrap_or(lo);
                        while current <= hi + 1e-9 && keys.len() < 10_000 {
                            keys.push(make(current).key());
                            match interval.extent(&make(current)) {
                                Some((_, next)) if next > current => current = next,
                                _ => break,
                            }
                        }
                    }
                    keys
                }
                None => {
                    let mut keys = crate::scale::dedup(values.iter().map(|v| v.key()));
                    keys.sort();
                    keys
                }
            };
            let categories = domain.len();
            let explicit: Option<Vec<crate::value::ValueKey>> = match &plot.ticks {
                Some(Ticks::Values(values)) => Some(values.iter().map(|v| v.key()).collect()),
                _ => None,
            };
            let tick_keys = explicit.unwrap_or_else(|| domain.clone());
            let (scale, ticks) = if kind == ScaleKind::Band {
                let mut band = crate::scale::BandScale::new(domain.clone(), range)
                    .padding_inner(padding_inner)
                    .padding_outer(padding_outer);
                if let Some(align) = plot.align {
                    band = band.align(align);
                }
                let ticks = band_ticks(&tick_keys, &band, values, is_x, tick_count);
                (PositionScale::Band(band), ticks)
            } else {
                let mut point_scale = crate::scale::PointScale::new(domain.clone(), range);
                if let Some(padding) = plot.padding {
                    point_scale = point_scale.padding(padding);
                }
                if let Some(align) = plot.align {
                    point_scale = point_scale.align(align);
                }
                let ticks = point_ticks(&tick_keys, &point_scale, values, is_x, tick_count);
                (PositionScale::Point(point_scale), ticks)
            };
            let ticks = ticks
                .into_iter()
                .map(|tick| AxisTick {
                    label: label(&tick.value, tick.label.clone()),
                    ..tick
                })
                .collect();
            Axis {
                scale,
                kind: inferred,
                ticks,
                temporal: false,
                categories,
            }
        }
    }
}

/// `+1` for an ascending range, `-1` for a descending one, so an inset moves
/// inward either way.
fn range_sign(range: [f32; 2]) -> f32 {
    if range[1] >= range[0] { 1. } else { -1. }
}

/// The ticks at every step of `interval` across `domain`.
pub(crate) fn interval_ticks(
    interval: crate::mark::Interval,
    domain: [f64; 2],
    temporal: bool,
) -> Vec<f64> {
    let (lo, hi) = (domain[0].min(domain[1]), domain[0].max(domain[1]));
    let mut ticks = Vec::new();
    let value = |n: f64| {
        if temporal {
            crate::value::Value::date_from_millis(n as i64)
        } else {
            crate::value::Value::Number(n)
        }
    };
    let Some((mut start, _)) = interval.extent(&value(lo)) else {
        return ticks;
    };
    if start < lo {
        start = interval
            .extent(&value(lo))
            .map(|(_, end)| end)
            .unwrap_or(hi);
    }
    let mut current = start;
    while current <= hi + 1e-9 && ticks.len() < 1000 {
        ticks.push(current);
        match interval.extent(&value(current)) {
            Some((_, next)) if next > current => current = next,
            _ => break,
        }
    }
    ticks
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

/// Paint the grid lines across the plot, under the marks: Plot's `grid`,
/// one line per tick in `currentColor` at a tenth of its opacity.
///
/// A faceted figure draws its grid inside each facet's marks area, the same
/// way its axes repeat there.
pub(crate) fn paint_grid(
    frame: &Frame,
    theme: &PlotTheme,
    window: &mut Window,
    x_grid: bool,
    y_grid: bool,
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
        if y_grid {
            for tick in frame.y.ticks.iter() {
                let y = plot.origin.y + px(tick.position);
                let start = point(plot.origin.x, y);
                let end = point(plot.origin.x + plot.size.width, y);
                if theme.grid_dash.is_empty() {
                    paint::paint_stroke(&[start, end], width.as_f32(), theme.grid, window);
                } else {
                    paint_dashed_frame(start, end, &theme.grid_dash, width, theme.grid, window);
                }
            }
        }
        if x_grid {
            for tick in frame.x.ticks.iter() {
                let x = plot.origin.x + px(tick.position);
                let start = point(x, plot.origin.y);
                let end = point(x, plot.origin.y + plot.size.height);
                if theme.grid_dash.is_empty() {
                    paint::paint_stroke(&[start, end], width.as_f32(), theme.grid, window);
                } else {
                    paint_dashed_frame(start, end, &theme.grid_dash, width, theme.grid, window);
                }
            }
        }
    }
}

/// How one axis draws, Plot's axis options.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AxisStyle {
    pub(crate) anchor: crate::options::AxisAnchor,
    pub(crate) tick_size: f32,
    pub(crate) tick_padding: f32,
    pub(crate) tick_rotate: f32,
    pub(crate) line: bool,
}

impl AxisStyle {
    /// The style `options` ask for at `anchor`, with Plot's defaults: six-pixel
    /// ticks, labels three pixels past them, no domain line.
    pub(crate) fn new(
        anchor: crate::options::AxisAnchor,
        options: &crate::options::ScaleOptions,
    ) -> Self {
        let tick_size = options.tick_size.unwrap_or(6.);
        let tick_rotate = options.tick_rotate.unwrap_or(0.);
        // Plot's default padding: at least 3, more for short ticks, more
        // still for rotated labels on x.
        let rotated = if tick_rotate.abs() >= 10. {
            4. * tick_rotate.to_radians().cos()
        } else {
            0.
        };
        Self {
            anchor,
            tick_size,
            tick_padding: options
                .tick_padding
                .unwrap_or((9. - tick_size).max(3.) + rotated),
            tick_rotate,
            line: options.line.unwrap_or(false),
        }
    }
}

/// Paint the axes of one plot area: tick marks and tick labels, the way
/// Plot's `axisX` and `axisY` draw them.
pub(crate) fn paint_axes(
    frame: &Frame,
    theme: &PlotTheme,
    window: &mut Window,
    cx: &mut App,
    x: Option<AxisStyle>,
    y: Option<AxisStyle>,
    edges: &AxisEdges,
) {
    use crate::options::AxisAnchor;
    let plot = frame.plot;
    let font = font(theme.font_family.clone());
    let color = theme.label;
    let size = theme.label_size;
    let line_width = theme.line_width.as_f32();
    let text = |label: &gpui::SharedString, window: &mut Window| {
        Label::new(
            label.clone(),
            size,
            color,
            FontWeight::NORMAL,
            font.clone(),
            window,
        )
    };

    if let Some(style) = x {
        let sides: &[bool] = match style.anchor {
            AxisAnchor::Top => &[true],
            AxisAnchor::Both => &[true, false],
            _ => &[false],
        };
        for &top in sides {
            if !top && !edges.bottom || top && !edges.top {
                continue;
            }
            let edge = if top {
                plot.origin.y
            } else {
                plot.origin.y + plot.size.height
            };
            let direction = if top { -1. } else { 1. };
            if style.line {
                paint::paint_stroke(
                    &[
                        point(plot.origin.x, edge),
                        point(plot.origin.x + plot.size.width, edge),
                    ],
                    line_width,
                    color,
                    window,
                );
            }
            for tick in &frame.x.ticks {
                let tx = plot.origin.x + px(tick.position);
                if style.tick_size > 0. {
                    paint::paint_stroke(
                        &[
                            point(tx, edge),
                            point(tx, edge + px(style.tick_size * direction)),
                        ],
                        line_width,
                        color,
                        window,
                    );
                }
                if tick.label.is_empty() {
                    continue;
                }
                // Plot's `tickRotate`: the label turns about its tick, its
                // start (or end) against the axis.
                if style.tick_rotate.abs() >= 10. {
                    let offset = style.tick_size.max(0.) + style.tick_padding;
                    let ty = if top {
                        edge - px(offset)
                    } else {
                        edge + px(offset)
                    };
                    let start = (style.tick_rotate < 0.) ^ !top;
                    let align = if start {
                        TextAlign::Left
                    } else {
                        TextAlign::Right
                    };
                    paint::paint_rotated_text(
                        &tick.label,
                        size.as_f32(),
                        color,
                        &theme.font_family,
                        point(tx, ty),
                        style.tick_rotate,
                        align,
                        window,
                        cx,
                    );
                    continue;
                }
                // A label may run to several lines, Plot's `Jan\n2021`.
                let lines: Vec<&str> = tick.label.split('\n').collect();
                let line_height = size.as_f32() * 1.1;
                let offset = style.tick_size.max(0.) + style.tick_padding;
                for (i, line) in lines.iter().enumerate() {
                    let label = text(&gpui::SharedString::from(line.to_string()), window);
                    let ty = if top {
                        edge - px(offset)
                            - label.height
                            - px(line_height * (lines.len() - 1 - i) as f32)
                    } else {
                        edge + px(offset + line_height * i as f32)
                    };
                    label.paint_at(point(tx, ty), TextAlign::Center, None, window, cx);
                }
            }
        }
    }

    if let Some(style) = y {
        let sides: &[bool] = match style.anchor {
            AxisAnchor::Right => &[true],
            AxisAnchor::Both => &[false, true],
            _ => &[false],
        };
        for &right in sides {
            if !right && !edges.left || right && !edges.right {
                continue;
            }
            let edge = if right {
                plot.origin.x + plot.size.width
            } else {
                plot.origin.x
            };
            let direction = if right { 1. } else { -1. };
            if style.line {
                paint::paint_stroke(
                    &[
                        point(edge, plot.origin.y),
                        point(edge, plot.origin.y + plot.size.height),
                    ],
                    line_width,
                    color,
                    window,
                );
            }
            for tick in &frame.y.ticks {
                let ty = plot.origin.y + px(tick.position);
                if style.tick_size > 0. {
                    paint::paint_stroke(
                        &[
                            point(edge, ty),
                            point(edge + px(style.tick_size * direction), ty),
                        ],
                        line_width,
                        color,
                        window,
                    );
                }
                if tick.label.is_empty() {
                    continue;
                }
                let label = text(&tick.label, window);
                let offset = style.tick_size.max(0.) + style.tick_padding;
                let origin = point(edge + px(offset * direction), ty - label.height / 2.);
                let align = if right {
                    TextAlign::Left
                } else {
                    TextAlign::Right
                };
                label.paint_at(origin, align, None, window, cx);
            }
        }
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
    fx_options: Option<&crate::options::ScaleOptions>,
    fy_options: Option<&crate::options::ScaleOptions>,
    fx_bottom: bool,
    fy_left: bool,
) {
    let format = |options: Option<&crate::options::ScaleOptions>,
                  keys: &[crate::value::ValueKey]|
     -> Vec<gpui::SharedString> {
        keys.iter()
            .map(|key| {
                let value = key.to_value();
                match options.and_then(|o| o.tick_format.as_ref()) {
                    Some(format) => format.format(&value).into(),
                    None => match &value {
                        crate::value::Value::Null => "null".into(),
                        crate::value::Value::Date(d) => crate::format::format_time("%Y", *d).into(),
                        other => other.label(),
                    },
                }
            })
            .collect()
    };
    let plot = frame.plot;
    let font = font(theme.font_family.clone());
    let color = theme.label;

    if let (Some(fx), Some(_)) = (&grid.fx, fx_options) {
        let labels = format(fx_options, fx);
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
            // Above the plot (below it when the x axis is absent or on
            // top), in the margin reserved for it.
            let y = if fx_bottom {
                plot.origin.y + plot.size.height + px(FACET_LABEL_GAP + 3.)
            } else {
                plot.origin.y - px(theme.label_size.as_f32() + FACET_LABEL_GAP)
            };
            label.paint_at(
                point(plot.origin.x + px(start + width / 2.), y),
                TextAlign::Center,
                None,
                window,
                cx,
            );
        }
    }

    if let (Some(fy), Some(_)) = (&grid.fy, fy_options) {
        let labels = format(fy_options, fy);
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
            // Right of the plot (left when the y axis is absent or on the
            // right), centred on the facet's band.
            let (x, align) = if fy_left {
                (plot.origin.x - px(FACET_LABEL_GAP), TextAlign::Right)
            } else {
                (
                    plot.origin.x + plot.size.width + px(FACET_LABEL_GAP),
                    TextAlign::Left,
                )
            };
            label.paint_at(
                point(
                    x,
                    plot.origin.y + px(start + height / 2. - theme.label_size.as_f32() / 2.),
                ),
                align,
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
    pub(crate) top: bool,
    pub(crate) right: bool,
}

impl Default for AxisEdges {
    fn default() -> Self {
        Self {
            left: true,
            bottom: true,
            top: true,
            right: true,
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
        let top = row == 0 || grid.is_empty(cell - grid.columns());
        let right = column == grid.columns() - 1 || grid.is_empty(cell + 1);
        Self {
            left,
            bottom,
            top,
            right,
        }
    }
}

/// Paint a dashed line from `start` to `end`.
pub(crate) fn paint_dashed_frame(
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
        // Half a step of padding at each end, Plot's point scale.
        assert_eq!(a, 50.);
        assert_eq!(c, 250.);
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
