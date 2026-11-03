//! The plot's frame: the space its marks draw in.
//!
//! Plot calls this the *plot* inside its *figure*: margins hold the axes, titles
//! and legends, and the plot's own bounds hold the marks. A [`Frame`] resolves
//! the plot's bounds from the figure's size and the margins it needs, then
//! builds the scales and axis ticks the marks read.

use gpui::{Bounds, Pixels, Point, Size, px};

use crate::scale::{ContinuousScale, PositionScale, Transform};
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
    /// The space the frame reserves for its axes and labels.
    pub insets: Insets,
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
    /// The plot's bounds inside the figure.
    pub plot: Bounds<Pixels>,
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
                range: [0., plot_width],
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
                range: [plot_height, 0.],
                axis: AxisSide::Y,
            },
        );

        Self { bounds, plot, x, y }
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
}
