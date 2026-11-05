//! Plot's scale options: the `x: {…}`, `y: {…}`, `color: {…}`, `r: {…}` …
//! objects of `Plot.plot`.
//!
//! One [`ScaleOptions`] describes one scale; [`crate::Plot::x`],
//! [`crate::Plot::color`] and their siblings take a closure that fills it in,
//! so `x: {type: "log", label: "Volume", grid: true}` reads
//! `.x(|x| x.r#type(ScaleType::Log).label("Volume").grid(true))`.

use std::rc::Rc;

use gpui::{Hsla, SharedString};

use crate::value::Value;

/// Plot's scale `type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleType {
    Linear,
    Log,
    Pow,
    Sqrt,
    Symlog,
    /// Plot's `utc` and `time`: a linear scale over dates.
    Utc,
    Band,
    Point,
    /// An ordinal color (or symbol) scale over ordered values.
    Ordinal,
    /// An ordinal color scale with a categorical palette.
    Categorical,
    /// A linear color scale through a diverging scheme, centered on its pivot.
    Diverging,
    /// A color scale cut into `n` bins at thresholds, Plot's `threshold`.
    Threshold,
    /// A color scale cut into `n` equal steps, Plot's `quantize`.
    Quantize,
    /// A color scale cut into quantiles, Plot's `quantile`.
    Quantile,
    /// The channel's values are already visual values, Plot's `identity`.
    Identity,
}

impl ScaleType {
    /// Plot's name for the type.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "linear" => Self::Linear,
            "log" => Self::Log,
            "pow" => Self::Pow,
            "sqrt" => Self::Sqrt,
            "symlog" => Self::Symlog,
            "utc" | "time" => Self::Utc,
            "band" => Self::Band,
            "point" => Self::Point,
            "ordinal" => Self::Ordinal,
            "categorical" => Self::Categorical,
            "diverging" => Self::Diverging,
            "threshold" => Self::Threshold,
            "quantize" => Self::Quantize,
            "quantile" => Self::Quantile,
            "identity" => Self::Identity,
            _ => return None,
        })
    }

    /// Whether the type is discrete along a position axis.
    pub fn is_discrete(self) -> bool {
        matches!(
            self,
            Self::Band | Self::Point | Self::Ordinal | Self::Categorical
        )
    }
}

/// Where an axis sits, Plot's `axis` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisAnchor {
    Top,
    Bottom,
    Left,
    Right,
    /// Both sides, Plot's `"both"`.
    Both,
}

/// Where an axis label sits along its axis, Plot's `labelAnchor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelAnchor {
    Start,
    Center,
    End,
}

/// How an axis labels its ticks, Plot's `tickFormat`.
#[derive(Clone)]
pub enum TickFormat {
    /// A d3-format specifier for numbers or a d3-time-format one for dates.
    Spec(SharedString),
    /// A function of the tick's value.
    Fn(Rc<dyn Fn(&Value) -> String>),
}

impl std::fmt::Debug for TickFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TickFormat::Spec(spec) => f.debug_tuple("Spec").field(spec).finish(),
            TickFormat::Fn(_) => f.write_str("Fn(..)"),
        }
    }
}

impl TickFormat {
    /// Format one tick's value.
    pub fn format(&self, value: &Value) -> String {
        match self {
            TickFormat::Fn(f) => f(value),
            TickFormat::Spec(spec) => match value {
                Value::Date(date) => crate::format::format_time(spec, *date),
                Value::Number(n) => crate::format::format_number(spec, *n),
                other => other.label().to_string(),
            },
        }
    }
}

impl From<&str> for TickFormat {
    fn from(spec: &str) -> Self {
        TickFormat::Spec(spec.to_string().into())
    }
}

impl<F: Fn(&Value) -> String + 'static> From<F> for TickFormat {
    fn from(f: F) -> Self {
        TickFormat::Fn(Rc::new(f))
    }
}

/// Which ticks an axis draws, Plot's `ticks`.
#[derive(Clone, Debug)]
pub enum Ticks {
    /// About this many, at nice values.
    Count(usize),
    /// Exactly these.
    Values(Vec<Value>),
    /// At every multiple of this step (a number, or for dates an interval
    /// such as Plot's `"month"`).
    Interval(crate::mark::Interval),
}

/// One scale's options. Every field is optional: unset means Plot's default.
#[derive(Clone, Debug, Default)]
pub struct ScaleOptions {
    pub r#type: Option<ScaleType>,
    pub domain: Option<Vec<Value>>,
    /// A numeric range: pixels for a position, a radius, an opacity …
    pub range: Option<Vec<f64>>,
    /// A color range, for a color scale.
    pub colors: Option<Vec<Hsla>>,
    pub scheme: Option<SharedString>,
    /// The axis label; `Some(None)` is Plot's `label: null`.
    pub label: Option<Option<SharedString>>,
    pub label_anchor: Option<LabelAnchor>,
    pub label_offset: Option<f32>,
    pub label_arrow: Option<bool>,
    pub grid: Option<bool>,
    /// Where the axis sits; `Some(None)` is Plot's `axis: null`.
    pub axis: Option<Option<AxisAnchor>>,
    pub ticks: Option<Ticks>,
    pub tick_format: Option<TickFormat>,
    pub tick_size: Option<f32>,
    pub tick_padding: Option<f32>,
    pub tick_rotate: Option<f32>,
    pub line: Option<bool>,
    pub nice: Option<bool>,
    pub zero: Option<bool>,
    pub clamp: Option<bool>,
    pub reverse: Option<bool>,
    pub percent: Option<bool>,
    pub inset: Option<f32>,
    pub inset_start: Option<f32>,
    pub inset_end: Option<f32>,
    pub padding: Option<f32>,
    pub padding_inner: Option<f32>,
    pub padding_outer: Option<f32>,
    pub align: Option<f32>,
    pub round: Option<bool>,
    pub exponent: Option<f64>,
    pub base: Option<f64>,
    pub constant: Option<f64>,
    pub pivot: Option<f64>,
    /// The number of steps of a quantize or quantile color scale, Plot's `n`.
    pub n: Option<usize>,
    pub legend: Option<bool>,
    /// Plot's `unknown`: the color for a value outside the domain.
    pub unknown: Option<Hsla>,
    /// Plot's scale `interval`: an ordinal scale over every step between the
    /// data's extremes, so a missing year still takes its band.
    pub interval: Option<crate::mark::Interval>,
}

impl ScaleOptions {
    /// Plot's `type`.
    pub fn r#type(mut self, kind: ScaleType) -> Self {
        self.r#type = Some(kind);
        self
    }

    /// Plot's `type`, by Plot's name (`"log"`, `"band"` …).
    pub fn type_name(mut self, name: &str) -> Self {
        self.r#type = ScaleType::parse(name);
        self
    }

    /// Plot's `domain`: the extent of a continuous scale (`[min, max]`), or
    /// the values of a discrete one, in order.
    pub fn domain<V: Into<Value>>(mut self, domain: impl IntoIterator<Item = V>) -> Self {
        self.domain = Some(domain.into_iter().map(Into::into).collect());
        self
    }

    /// Plot's `range` for a numeric scale.
    pub fn range(mut self, range: impl IntoIterator<Item = f64>) -> Self {
        self.range = Some(range.into_iter().collect());
        self
    }

    /// Plot's `range` for a color scale: CSS colors.
    pub fn colors<'a>(mut self, colors: impl IntoIterator<Item = &'a str>) -> Self {
        self.colors = Some(
            colors
                .into_iter()
                .map(|c| crate::css::parse(c).unwrap_or_default())
                .collect(),
        );
        self
    }

    /// Plot's `scheme`, by name.
    pub fn scheme(mut self, scheme: impl Into<SharedString>) -> Self {
        self.scheme = Some(scheme.into());
        self
    }

    /// Plot's `label`.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(Some(label.into()));
        self
    }

    /// Plot's `label: null`.
    pub fn no_label(mut self) -> Self {
        self.label = Some(None);
        self
    }

    pub fn label_anchor(mut self, anchor: LabelAnchor) -> Self {
        self.label_anchor = Some(anchor);
        self
    }

    pub fn label_offset(mut self, offset: f32) -> Self {
        self.label_offset = Some(offset);
        self
    }

    pub fn label_arrow(mut self, arrow: bool) -> Self {
        self.label_arrow = Some(arrow);
        self
    }

    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = Some(grid);
        self
    }

    /// Plot's `axis: "top"` and friends.
    pub fn axis(mut self, anchor: AxisAnchor) -> Self {
        self.axis = Some(Some(anchor));
        self
    }

    /// Plot's `axis: null`.
    pub fn no_axis(mut self) -> Self {
        self.axis = Some(None);
        self
    }

    /// Plot's `ticks: n`.
    pub fn ticks(mut self, count: usize) -> Self {
        self.ticks = Some(Ticks::Count(count));
        self
    }

    /// Plot's `ticks: [values]`.
    pub fn tick_values<V: Into<Value>>(mut self, values: impl IntoIterator<Item = V>) -> Self {
        self.ticks = Some(Ticks::Values(values.into_iter().map(Into::into).collect()));
        self
    }

    /// Plot's `ticks: "month"` or `ticks: 5` as an interval.
    pub fn tick_interval(mut self, interval: impl Into<crate::mark::Interval>) -> Self {
        self.ticks = Some(Ticks::Interval(interval.into()));
        self
    }

    /// Plot's `tickFormat`: a specifier (`"+f"`, `"%Y"`) or a function.
    pub fn tick_format(mut self, format: impl Into<TickFormat>) -> Self {
        self.tick_format = Some(format.into());
        self
    }

    pub fn tick_size(mut self, size: f32) -> Self {
        self.tick_size = Some(size);
        self
    }

    pub fn tick_padding(mut self, padding: f32) -> Self {
        self.tick_padding = Some(padding);
        self
    }

    pub fn tick_rotate(mut self, degrees: f32) -> Self {
        self.tick_rotate = Some(degrees);
        self
    }

    pub fn line(mut self, line: bool) -> Self {
        self.line = Some(line);
        self
    }

    pub fn nice(mut self, nice: bool) -> Self {
        self.nice = Some(nice);
        self
    }

    pub fn zero(mut self, zero: bool) -> Self {
        self.zero = Some(zero);
        self
    }

    pub fn clamp(mut self, clamp: bool) -> Self {
        self.clamp = Some(clamp);
        self
    }

    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = Some(reverse);
        self
    }

    /// Plot's `percent`: values are proportions shown as percentages.
    pub fn percent(mut self, percent: bool) -> Self {
        self.percent = Some(percent);
        self
    }

    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = Some(inset);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn padding_inner(mut self, padding: f32) -> Self {
        self.padding_inner = Some(padding);
        self
    }

    pub fn padding_outer(mut self, padding: f32) -> Self {
        self.padding_outer = Some(padding);
        self
    }

    pub fn align(mut self, align: f32) -> Self {
        self.align = Some(align);
        self
    }

    pub fn round(mut self, round: bool) -> Self {
        self.round = Some(round);
        self
    }

    pub fn exponent(mut self, exponent: f64) -> Self {
        self.exponent = Some(exponent);
        self
    }

    pub fn base(mut self, base: f64) -> Self {
        self.base = Some(base);
        self
    }

    pub fn constant(mut self, constant: f64) -> Self {
        self.constant = Some(constant);
        self
    }

    pub fn pivot(mut self, pivot: f64) -> Self {
        self.pivot = Some(pivot);
        self
    }

    pub fn n(mut self, n: usize) -> Self {
        self.n = Some(n);
        self
    }

    pub fn legend(mut self, legend: bool) -> Self {
        self.legend = Some(legend);
        self
    }

    /// Plot's scale `interval`.
    pub fn interval(mut self, interval: impl Into<crate::mark::Interval>) -> Self {
        self.interval = Some(interval.into());
        self
    }

    pub fn unknown(mut self, color: &str) -> Self {
        self.unknown = crate::css::parse(color);
        self
    }

    /// The numeric domain `[min, max]`, for a continuous scale.
    pub fn numeric_domain(&self) -> Option<[f64; 2]> {
        let domain = self.domain.as_ref()?;
        let numbers: Vec<f64> = domain.iter().filter_map(Value::as_number).collect();
        match numbers.as_slice() {
            [lo, .., hi] => Some([*lo, *hi]),
            _ => None,
        }
    }
}
