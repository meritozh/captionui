//! Scales: Plot's continuous, band, point and ordinal scales, plus its color
//! ranges.
//!
//! A scale turns a [`Value`] into a pixel position, a band's extent, or a
//! color. Plot picks a scale from the channel's inferred [`PlotKind`](crate::PlotKind)
//! and its `Scale` option; here the caller names the transform and the library
//! supplies the ticks.

use std::rc::Rc;

use gpui::{Hsla, SharedString};
use gpui_kit::base::plot::scale::{ScaleLinear, ScaleOrdinal};

use crate::value::{Value, ValueKey};

/// A continuous transform, Plot's `scale.type`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transform {
    /// Plot's default for numbers: a linear map, with a zero baseline when the
    /// data calls for one.
    #[default]
    Linear,
    /// A logarithmic map, Plot's `log`. The domain must be positive.
    Log,
    /// An exponent map, Plot's `pow`; `exponent` is the power.
    Pow,
    /// A square-root map, Plot's `sqrt`; `pow` with an exponent of ½.
    Sqrt,
    /// A symmetric-log map, Plot's `symlog`; linear near zero and
    /// logarithmic beyond `constant`, so it can show zero and negatives.
    Symlog,
}

impl Transform {
    /// The name Plot uses for this transform in `scale.type`.
    pub fn name(self) -> &'static str {
        match self {
            Transform::Linear => "linear",
            Transform::Log => "log",
            Transform::Pow => "pow",
            Transform::Sqrt => "sqrt",
            Transform::Symlog => "symlog",
        }
    }

    /// Apply the transform to a domain value.
    fn forward(self, value: f64, exponent: f64, constant: f64) -> Option<f64> {
        match self {
            Transform::Linear => Some(value),
            Transform::Log => (value > 0.).then(|| value.ln()),
            // Pow and sqrt keep the sign, as d3's do, so a domain that
            // reaches below zero still scales.
            Transform::Pow => {
                let v = value.signum() * value.abs().powf(exponent);
                v.is_finite().then_some(v)
            }
            Transform::Sqrt => Some(value.signum() * value.abs().sqrt()),
            Transform::Symlog => {
                let sign = value.signum();
                Some(sign * (value.abs() / constant).ln_1p())
            }
        }
    }

    /// Undo [`Self::forward`]: the domain value at a transformed number.
    fn inverse(self, value: f64, exponent: f64, constant: f64) -> f64 {
        match self {
            Transform::Linear => value,
            Transform::Log => value.exp(),
            Transform::Pow => value.signum() * value.abs().powf(1. / exponent),
            Transform::Sqrt => value.signum() * value * value,
            Transform::Symlog => value.signum() * ((value.abs()).exp_m1() * constant),
        }
    }
}

/// A continuous scale from a domain extent to a range in pixels.
///
/// This is the scale behind `x` and `y` for quantitative and temporal
/// channels: Plot's `linear`, `log`, `pow`, `sqrt`, `symlog` and `time` scales.
/// A time scale is the linear scale over the millisecond domain of [`Value::Date`]s,
/// with date ticks and date labels.
#[derive(Clone, Debug)]
pub struct ContinuousScale {
    transform: Transform,
    domain: [f64; 2],
    range: [f32; 2],
    exponent: f64,
    constant: f64,
    clamp: bool,
}

impl ContinuousScale {
    /// A scale from `domain` to `range`, transformed by `transform`.
    ///
    /// The domain is normalised so `[0]` is its lower end, so a reversed range
    /// puts the domain's lower end at the range's higher end, as Plot's
    /// `y` axis does.
    pub fn new(transform: Transform, domain: [f64; 2], range: [f32; 2]) -> Self {
        let (lo, hi) = if domain[0] <= domain[1] {
            (domain[0], domain[1])
        } else {
            (domain[1], domain[0])
        };
        Self {
            transform,
            domain: [lo, hi],
            range,
            exponent: 1.,
            constant: 1.,
            clamp: false,
        }
    }

    /// Set the exponent of a [`Transform::Pow`] scale.
    ///
    /// An exponent of zero (every value maps to one point) or a non-finite
    /// one has no inverse, so it is ignored and the scale keeps its previous
    /// exponent.
    pub fn with_exponent(mut self, exponent: f64) -> Self {
        if exponent.is_finite() && exponent != 0. {
            self.exponent = exponent;
        }
        self
    }

    /// Set the constant of a [`Transform::Symlog`] scale: the width of the
    /// linear region around zero. Plot's default is 1.
    pub fn with_constant(mut self, constant: f64) -> Self {
        self.constant = constant.max(f64::MIN_POSITIVE);
        self
    }

    /// Clamp positions outside the domain to the range's ends, as Plot's
    /// `clamp` option does.
    pub fn clamped(mut self, clamp: bool) -> Self {
        self.clamp = clamp;
        self
    }

    pub fn transform(&self) -> Transform {
        self.transform
    }

    /// The domain extent.
    pub fn domain(&self) -> [f64; 2] {
        self.domain
    }

    /// The range extent in pixels.
    pub fn range(&self) -> [f32; 2] {
        self.range
    }

    /// Whether positions are clamped to the range.
    pub fn is_clamped(&self) -> bool {
        self.clamp
    }

    /// The position of `value` in the range.
    pub fn position(&self, value: f64) -> Option<f32> {
        let (a, b) = (
            self.transform
                .forward(self.domain[0], self.exponent, self.constant)?,
            self.transform
                .forward(self.domain[1], self.exponent, self.constant)?,
        );
        let v = self
            .transform
            .forward(value, self.exponent, self.constant)?;
        if a == b {
            return Some((self.range[0] + self.range[1]) / 2.);
        }
        let ratio = (v - a) / (b - a);
        let ratio = if self.clamp {
            ratio.clamp(0., 1.)
        } else {
            ratio
        };
        Some(self.range[0] + (ratio as f32) * (self.range[1] - self.range[0]))
    }

    /// The position of a [`Value`], or `None` when it does not scale (a
    /// categorical value, a null, or a log scale's non-positive value).
    pub fn position_of(&self, value: &Value) -> Option<f32> {
        self.position(value.as_number()?)
    }

    /// The domain value at a pixel position: the inverse of [`Self::position`].
    pub fn invert(&self, position: f32) -> f64 {
        let (a, b) = match (
            self.transform
                .forward(self.domain[0], self.exponent, self.constant),
            self.transform
                .forward(self.domain[1], self.exponent, self.constant),
        ) {
            (Some(a), Some(b)) => (a, b),
            _ => return self.domain[0],
        };
        if self.range[0] == self.range[1] {
            return self.domain[0];
        }
        let ratio = ((position - self.range[0]) / (self.range[1] - self.range[0])) as f64;
        // A clamped scale inverts to its domain too, mirroring `position`.
        let ratio = if self.clamp {
            ratio.clamp(0., 1.)
        } else {
            ratio
        };
        self.transform
            .inverse(a + ratio * (b - a), self.exponent, self.constant)
    }

    /// The evenly spaced values of the domain that Plot's axis would label.
    ///
    /// A time scale returns millisecond values at calendar boundaries; the
    /// others return [`Self::ticks`].
    pub fn time_ticks(&self, count: usize) -> Vec<f64> {
        crate::ticks::time_ticks(self.domain, count)
    }

    /// Plot's default ticks: about `count` round values within the domain.
    pub fn ticks(&self, count: usize) -> Vec<f64> {
        crate::ticks::linear_ticks(self.domain, count, self.transform)
    }

    /// Format a domain value as an axis label for this scale.
    pub fn format_tick(&self, value: f64, time: bool) -> SharedString {
        if time {
            crate::ticks::format_time_tick(value, self.domain)
        } else {
            SharedString::from(crate::value::format_number(value))
        }
    }

    /// The scale's domain widened to round values, Plot's `nice`.
    ///
    /// A domain of `0.3..97` becomes `0..100`, so its axis labels sit on round
    /// numbers and the marks stop short of the frame's edge by the amount the
    /// rounding added. The transform decides what counts as round: a log scale
    /// rounds to powers of ten, the others to a nice step.
    pub fn nice(&self, count: usize) -> Self {
        let [lo, hi] = self.domain;
        let (lo, hi) = match self.transform {
            Transform::Log => {
                let lo = if lo > 0. { lo.log10().floor() } else { lo };
                let hi = if hi > 0. { hi.log10().ceil() } else { hi };
                (10f64.powf(lo), 10f64.powf(hi))
            }
            _ => {
                let step = crate::ticks::nice_step((hi - lo) / count.max(1) as f64);
                if step <= 0. || !step.is_finite() {
                    (lo, hi)
                } else {
                    ((lo / step).floor() * step, (hi / step).ceil() * step)
                }
            }
        };
        Self {
            domain: [lo, hi],
            ..self.clone()
        }
    }

    /// The scale with its range reversed, Plot's `reverse`.
    pub fn reversed(&self) -> Self {
        Self {
            range: [self.range[1], self.range[0]],
            ..self.clone()
        }
    }
}

/// A band scale: Plot's `band` scale, where each distinct category owns a slice
/// of the range.
///
/// Marks drawn against a band scale read the band's start and width; a bar
/// centered on its category spans [`Self::center`] minus half its width.
#[derive(Clone)]
pub struct BandScale {
    domain: Vec<ValueKey>,
    range: [f32; 2],
    padding_inner: f32,
    padding_outer: f32,
    align: f32,
    max_band_width: Option<f32>,
}

impl std::fmt::Debug for BandScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BandScale")
            .field("domain", &self.domain)
            .field("range", &self.range)
            .finish()
    }
}

/// d3's band layout: where the first band starts, the step between bands,
/// and a band's width, for `n` bands across `range`.
fn band_layout(n: usize, range: [f32; 2], inner: f32, outer: f32, align: f32) -> (f32, f32, f32) {
    let reverse = range[1] < range[0];
    let (lo, hi) = if reverse {
        (range[1], range[0])
    } else {
        (range[0], range[1])
    };
    let n = n as f32;
    let step = (hi - lo) / (n - inner + outer * 2.).max(1.);
    let start = lo + (hi - lo - step * (n - inner)) * align;
    let width = step * (1. - inner);
    (start, step, width)
}

impl BandScale {
    /// Lay the distinct values of `domain` out across `range`.
    pub fn new(domain: impl IntoIterator<Item = ValueKey>, range: [f32; 2]) -> Self {
        Self {
            domain: dedup(domain),
            range,
            padding_inner: 0.,
            padding_outer: 0.,
            align: 0.5,
            max_band_width: None,
        }
    }

    /// Set the inner padding between bands, as a fraction of a step.
    pub fn padding_inner(mut self, padding: f32) -> Self {
        self.padding_inner = padding.clamp(0., 1.);
        self
    }

    /// Set the outer padding at the ends of the range, as a fraction of a step.
    pub fn padding_outer(mut self, padding: f32) -> Self {
        self.padding_outer = padding.max(0.);
        self
    }

    /// Where the bands sit in the range's slack, `0` at its start to `1` at
    /// its end, Plot's `align`.
    pub fn align(mut self, align: f32) -> Self {
        self.align = align.clamp(0., 1.);
        self
    }

    /// Cap the width of a band, as Plot's `maxBandwidth` does.
    pub fn max_band_width(mut self, width: f32) -> Self {
        self.max_band_width = Some(width);
        self
    }

    fn layout(&self) -> (f32, f32, f32) {
        band_layout(
            self.domain.len(),
            self.range,
            self.padding_inner,
            self.padding_outer,
            self.align,
        )
    }

    /// The band's left edge (or top edge for a vertical band scale).
    pub fn start(&self, key: &ValueKey) -> Option<f32> {
        let index = self.domain.iter().position(|k| k == key)?;
        let (start, step, natural) = self.layout();
        let index = if self.range[1] < self.range[0] {
            self.domain.len() - 1 - index
        } else {
            index
        };
        // A band capped by `max_band_width` is narrower than its slot, so it
        // is centred there, as Plot centres it.
        let offset = ((natural - self.band_width()) / 2.).max(0.);
        Some(start + step * index as f32 + offset)
    }

    /// The band's center.
    pub fn center(&self, key: &ValueKey) -> Option<f32> {
        self.start(key).map(|start| start + self.band_width() / 2.)
    }

    /// The width of one band.
    pub fn band_width(&self) -> f32 {
        let width = self.layout().2;
        self.max_band_width.map_or(width, |cap| width.min(cap))
    }

    /// The distance between two adjacent bands' starts.
    pub fn step(&self) -> f32 {
        self.layout().1
    }

    pub fn domain(&self) -> &[ValueKey] {
        &self.domain
    }

    pub fn range(&self) -> [f32; 2] {
        self.range
    }
}

/// A point scale: Plot's `point` scale, where each category owns a single
/// position with a fixed step: d3's band scale with no width.
#[derive(Clone)]
pub struct PointScale {
    domain: Vec<ValueKey>,
    range: [f32; 2],
    padding: f32,
    align: f32,
}

impl std::fmt::Debug for PointScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointScale")
            .field("domain", &self.domain)
            .field("range", &self.range)
            .field("padding", &self.padding)
            .finish()
    }
}

impl PointScale {
    pub fn new(domain: impl IntoIterator<Item = ValueKey>, range: [f32; 2]) -> Self {
        Self {
            domain: dedup(domain),
            range,
            padding: 0.5,
            align: 0.5,
        }
    }

    /// The padding at the range's ends, in steps (Plot's default is `0.5`),
    /// so the first and last points sit inside the frame.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.);
        self
    }

    /// Where the points sit in the range's slack, Plot's `align`.
    pub fn align(mut self, align: f32) -> Self {
        self.align = align.clamp(0., 1.);
        self
    }

    /// The position of a category.
    pub fn position(&self, key: &ValueKey) -> Option<f32> {
        let index = self.domain.iter().position(|k| k == key)?;
        let (start, step, _) =
            band_layout(self.domain.len(), self.range, 1., self.padding, self.align);
        let index = if self.range[1] < self.range[0] {
            self.domain.len() - 1 - index
        } else {
            index
        };
        Some(start + step * index as f32)
    }

    /// The distance between two adjacent categories' positions.
    pub fn step(&self) -> f32 {
        band_layout(self.domain.len(), self.range, 1., self.padding, self.align).1
    }

    pub fn domain(&self) -> &[ValueKey] {
        &self.domain
    }

    pub fn range(&self) -> [f32; 2] {
        self.range
    }
}

/// An ordinal scale: Plot's `ordinal` scale, which maps categories onto a
/// range of values (most often colors) in domain order, cycling the range.
#[derive(Clone)]
pub struct OrdinalScale<R> {
    inner: ScaleOrdinal<ValueKey, R>,
    domain: Vec<ValueKey>,
}

impl<R> std::fmt::Debug for OrdinalScale<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrdinalScale")
            .field("domain", &self.domain)
            .finish()
    }
}

impl<R: Clone> OrdinalScale<R> {
    pub fn new(domain: impl IntoIterator<Item = ValueKey>, range: Vec<R>) -> Self {
        let domain: Vec<ValueKey> = dedup(domain);
        let inner = ScaleOrdinal::new(domain.clone(), range);
        Self { inner, domain }
    }

    /// The range value for a category.
    pub fn get(&self, key: &ValueKey) -> Option<R> {
        self.inner.map(key)
    }

    pub fn domain(&self) -> &[ValueKey] {
        &self.domain
    }
}

/// The scale behind one positional channel.
#[derive(Clone, Debug)]
pub enum PositionScale {
    Continuous(ContinuousScale),
    Band(BandScale),
    Point(PointScale),
}

impl PositionScale {
    /// The position of a value, as a point scale or a continuous scale would
    /// place it.
    ///
    /// A band scale places a category at its band's center, which is what a
    /// dot, a rule or a line reads; bars read [`BandScale::start`] instead.
    pub fn position(&self, value: &Value) -> Option<f32> {
        match self {
            PositionScale::Continuous(scale) => scale.position_of(value),
            PositionScale::Band(scale) => scale.center(&value.key()),
            PositionScale::Point(scale) => scale.position(&value.key()),
        }
    }

    /// The band width when this scale is a band scale; `0` otherwise.
    pub fn band_width(&self) -> f32 {
        match self {
            PositionScale::Band(scale) => scale.band_width(),
            _ => 0.,
        }
    }

    /// The distance between adjacent categories, for bar width defaults.
    pub fn step(&self) -> Option<f32> {
        match self {
            PositionScale::Band(scale) => Some(scale.step()),
            PositionScale::Point(scale) => Some(scale.step()),
            PositionScale::Continuous(_) => None,
        }
    }

    /// The scale's kind, for axis and mark defaults.
    pub fn is_discrete(&self) -> bool {
        !matches!(self, PositionScale::Continuous(_))
    }
}

/// A color range: Plot's `color.range` option.
#[derive(Clone, Debug)]
pub enum ColorRange {
    /// A categorical palette, cycled over the domain.
    Ordinal(Vec<Hsla>),
    /// A continuous ramp interpolated between its stops, as Plot's
    /// `interpolateRgb`/`interpolateHcl` ranges are.
    Continuous(Vec<Hsla>),
    /// A diverging ramp interpolated from low, through `mid` at the domain's
    /// midpoint, to high.
    Diverging(Hsla, Hsla, Hsla),
}

impl ColorRange {
    /// Plot's default categorical palette, `Observable10`, as sRGB colors.
    pub fn observable10() -> Self {
        ColorRange::Ordinal(
            [
                0x4269d0u32,
                0xefb118,
                0xff725c,
                0x6cc5b0,
                0x3ca951,
                0xff8ab7,
                0xa463f2,
                0x97bbf5,
                0x9c6b4e,
                0x9498a0,
            ]
            .into_iter()
            .map(hsla_from_hex)
            .collect(),
        )
    }

    /// Observable10 as a bare color list, for a theme's categorical palette.
    pub fn observable10_colors() -> Vec<Hsla> {
        match Self::observable10() {
            ColorRange::Ordinal(colors) => colors,
            _ => unreachable!(),
        }
    }

    /// A diverging blue-to-red ramp around a neutral midpoint, Plot's
    /// `RdBu`-style default for diverging data.
    pub fn blue_red() -> Self {
        ColorRange::Diverging(
            hsla_from_hex(0x2166ac),
            hsla_from_hex(0xf7f7f7),
            hsla_from_hex(0xb2182b),
        )
    }
}

/// A scale behind a color channel: ordinal over categories, or continuous over
/// a numeric domain.
#[derive(Clone, Debug)]
pub struct ColorScale {
    domain: Vec<ValueKey>,
    extent: [f64; 2],
    range: ColorRange,
    /// A diverging scale's pivot: the extent's low end maps to 0, the pivot
    /// to ½ and the high end to 1, Plot's `diverging` type.
    pivot: Option<f64>,
    /// The transform a continuous scale applies before interpolating, for
    /// Plot's `log`, `sqrt`, `pow` and `symlog` color types.
    transform: Transform,
    /// The thresholds of a `threshold`, `quantize` or `quantile` scale:
    /// values below the first take the first color, and so on.
    thresholds: Option<Vec<f64>>,
    /// Plot's `unknown`: the color of a value the scale cannot place.
    unknown: Option<Hsla>,
    /// Whether the scale's domain is dates.
    temporal: bool,
}

impl ColorScale {
    /// An ordinal color scale: distinct categories take the range's colors in
    /// domain order, cycling.
    pub fn ordinal(domain: impl IntoIterator<Item = ValueKey>, range: Vec<Hsla>) -> Self {
        let domain: Vec<ValueKey> = dedup(domain);
        Self {
            domain,
            extent: [0., 1.],
            range: ColorRange::Ordinal(range),
            pivot: None,
            transform: Transform::Linear,
            thresholds: None,
            unknown: None,
            temporal: false,
        }
    }

    /// A continuous color scale over `extent`.
    pub fn continuous(extent: [f64; 2], range: ColorRange) -> Self {
        Self {
            domain: Vec::new(),
            extent,
            range,
            pivot: None,
            transform: Transform::Linear,
            thresholds: None,
            unknown: None,
            temporal: false,
        }
    }

    /// Mark the scale's domain as dates, for its legend.
    pub fn with_temporal(mut self, temporal: bool) -> Self {
        self.temporal = temporal;
        self
    }

    /// Whether the scale's domain is dates.
    pub fn is_temporal(&self) -> bool {
        self.temporal
    }

    /// Center the scale on `pivot`, Plot's diverging scales.
    pub fn with_pivot(mut self, pivot: f64) -> Self {
        self.pivot = Some(pivot);
        self
    }

    /// Interpolate after a transform, Plot's `log` or `sqrt` color types.
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Cut the scale at `thresholds` into `thresholds.len() + 1` colors.
    pub fn with_thresholds(mut self, thresholds: Vec<f64>) -> Self {
        self.thresholds = Some(thresholds);
        self
    }

    /// The color for a value outside the scale, Plot's `unknown`.
    pub fn with_unknown(mut self, unknown: Option<Hsla>) -> Self {
        self.unknown = unknown;
        self
    }

    /// Whether the scale is ordinal: categories, not a numeric ramp.
    pub fn is_ordinal(&self) -> bool {
        matches!(self.range, ColorRange::Ordinal(_)) && self.thresholds.is_none()
    }

    /// The numeric extent a continuous scale spans.
    pub fn extent(&self) -> [f64; 2] {
        self.extent
    }

    /// The thresholds of a threshold-type scale.
    pub fn thresholds(&self) -> Option<&[f64]> {
        self.thresholds.as_deref()
    }

    pub fn domain(&self) -> &[ValueKey] {
        &self.domain
    }

    pub fn range(&self) -> &ColorRange {
        &self.range
    }

    /// The color for a value, or `None` for a null.
    pub fn color(&self, value: &Value) -> Option<Hsla> {
        // An ordinal domain may hold null, as Plot's does, sorted last.
        if value.is_null() && !self.domain.contains(&value.key()) {
            return self.unknown;
        }
        if let (Some(thresholds), ColorRange::Ordinal(colors)) = (&self.thresholds, &self.range) {
            let number = value.as_number()?;
            let index = thresholds.iter().take_while(|t| number >= **t).count();
            return colors
                .get(index.min(colors.len().saturating_sub(1)))
                .copied();
        }
        match &self.range {
            ColorRange::Ordinal(colors) => {
                if colors.is_empty() {
                    return None;
                }
                let Some(index) = self.domain.iter().position(|key| *key == value.key()) else {
                    // Plot's ordinal scale extends its domain with a new
                    // value; with an explicit `unknown` it uses that.
                    return self
                        .unknown
                        .or(Some(colors[self.domain.len() % colors.len()]));
                };
                Some(colors[index % colors.len()])
            }
            ColorRange::Continuous(colors) => {
                let number = value.as_number()?;
                Some(interpolate_stops(colors, self.ratio(number)))
            }
            ColorRange::Diverging(low, mid, high) => {
                let number = value.as_number()?;
                let ratio = self.ratio(number);
                Some(if ratio <= 0.5 {
                    lerp_hsla(*low, *mid, ratio * 2.)
                } else {
                    lerp_hsla(*mid, *high, (ratio - 0.5) * 2.)
                })
            }
        }
    }

    /// Where `number` sits along the ramp, from 0 to 1.
    pub fn ratio(&self, number: f64) -> f32 {
        let t = |v: f64| match self.transform {
            Transform::Log => v.abs().max(f64::MIN_POSITIVE).ln() * v.signum(),
            Transform::Sqrt => v.signum() * v.abs().sqrt(),
            Transform::Symlog => v.signum() * (1. + v.abs()).ln(),
            _ => v,
        };
        let [lo, hi] = [t(self.extent[0]), t(self.extent[1])];
        let x = t(number);
        if let Some(pivot) = self.pivot {
            let p = t(pivot);
            let r = if x < p {
                if p == lo {
                    0.5
                } else {
                    0.5 * (x - lo) / (p - lo)
                }
            } else if hi == p {
                0.5
            } else {
                0.5 + 0.5 * (x - p) / (hi - p)
            };
            return (r as f32).clamp(0., 1.);
        }
        if hi == lo {
            return 0.5;
        }
        (((x - lo) / (hi - lo)) as f32).clamp(0., 1.)
    }
}

/// The distinct keys of `values`, keeping first-occurrence order.
pub fn dedup(values: impl IntoIterator<Item = ValueKey>) -> Vec<ValueKey> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for value in values {
        if !seen.contains(&value) {
            seen.push(value.clone());
            out.push(value);
        }
    }
    out
}

/// A color from a 24-bit sRGB hex value.
pub fn hsla_from_hex(hex: u32) -> Hsla {
    let r = ((hex >> 16) & 0xff) as f32 / 255.;
    let g = ((hex >> 8) & 0xff) as f32 / 255.;
    let b = (hex & 0xff) as f32 / 255.;
    rgb_to_hsla(r, g, b, 1.)
}

/// Convert sRGB components in `0..=1` to HSLA, which is what GPUI paints.
pub fn rgb_to_hsla(r: f32, g: f32, b: f32, a: f32) -> Hsla {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;
    let d = max - min;
    let s = if d == 0. {
        0.
    } else {
        d / (1. - (2. * l - 1.).abs())
    };
    let h = if d == 0. {
        0.
    } else if max == r {
        ((g - b) / d).rem_euclid(6.) / 6.
    } else if max == g {
        ((b - r) / d + 2.) / 6.
    } else {
        ((r - g) / d + 4.) / 6.
    };
    Hsla { h, s, l, a }
}

/// Interpolate a color ramp at `ratio` in `0..=1`.
/// The color `ratio` of the way along evenly spaced `colors`, interpolated in
/// RGB as d3's schemes are.
pub fn interpolate_stops(colors: &[Hsla], ratio: f32) -> Hsla {
    if colors.is_empty() {
        return Hsla::default();
    }
    if colors.len() == 1 {
        return colors[0];
    }
    let position = ratio.clamp(0., 1.) * (colors.len() - 1) as f32;
    let index = (position.floor() as usize).min(colors.len() - 2);
    let local = position - index as f32;
    lerp_rgb(colors[index], colors[index + 1], local)
}

/// Interpolate two colors in RGB, d3's `interpolateRgb`.
fn lerp_rgb(from: Hsla, to: Hsla, ratio: f32) -> Hsla {
    let a = gpui::Rgba::from(from);
    let b = gpui::Rgba::from(to);
    let mix = |x: f32, y: f32| x + (y - x) * ratio;
    rgb_to_hsla(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b), mix(a.a, b.a))
}

/// Interpolate two colors in HSLA space, taking the shorter way around the hue
/// circle.
fn lerp_hsla(from: Hsla, to: Hsla, ratio: f32) -> Hsla {
    let mut delta = to.h - from.h;
    if delta > 0.5 {
        delta -= 1.;
    } else if delta < -0.5 {
        delta += 1.;
    }
    Hsla {
        h: (from.h + delta * ratio).rem_euclid(1.),
        s: from.s + (to.s - from.s) * ratio,
        l: from.l + (to.l - from.l) * ratio,
        a: from.a + (to.a - from.a) * ratio,
    }
}

/// A [`Scale`] adapter, so a [`ContinuousScale`] can paint through GPUI Kit's
/// own shape helpers that take a `PlotValue` scale.
pub fn as_linear(scale: &ContinuousScale) -> ScaleLinear<f64> {
    ScaleLinear::new(scale.domain(), scale.range())
}

/// The extent of `values`, ignoring nulls; `None` when nothing is left.
pub fn extent(values: impl IntoIterator<Item = Value>) -> Option<[f64; 2]> {
    let mut min: Option<f64> = None;
    let mut max: Option<f64> = None;
    for value in values {
        // NaN and ±inf have no place on an axis; one of them would turn the
        // whole domain into NaN or collapse every position onto one edge.
        let Some(number) = value.as_number().filter(|number| number.is_finite()) else {
            continue;
        };
        min = Some(min.map_or(number, |min| min.min(number)));
        max = Some(max.map_or(number, |max| max.max(number)));
    }
    Some([min?, max?])
}

/// A trait-object scale for callers that want to box a scale.
pub type DynScale = Rc<dyn Fn(&Value) -> Option<f32>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_positions_and_inverse() {
        let scale = ContinuousScale::new(Transform::Linear, [0., 10.], [0., 100.]);
        assert_eq!(scale.position(5.), Some(50.));
        assert_eq!(scale.invert(50.), 5.);

        // Reversed range: the y axis of a chart.
        let scale = ContinuousScale::new(Transform::Linear, [0., 10.], [100., 0.]);
        assert_eq!(scale.position(0.), Some(100.));
        assert_eq!(scale.position(10.), Some(0.));
    }

    #[test]
    fn log_scale_places_decades_evenly() {
        let scale = ContinuousScale::new(Transform::Log, [1., 100.], [0., 100.]);
        assert_eq!(scale.position(1.), Some(0.));
        assert_eq!(scale.position(10.), Some(50.));
        assert_eq!(scale.position(100.), Some(100.));
        assert_eq!(scale.position(0.), None);
    }

    #[test]
    fn sqrt_scale_matches_pow_half() {
        let sqrt = ContinuousScale::new(Transform::Sqrt, [0., 100.], [0., 100.]);
        let pow = ContinuousScale::new(Transform::Pow, [0., 100.], [0., 100.]).with_exponent(0.5);
        assert_eq!(sqrt.position(25.), pow.position(25.));
        assert_eq!(sqrt.position(25.), Some(50.));
    }

    #[test]
    fn symlog_handles_zero_and_negatives() {
        let scale = ContinuousScale::new(Transform::Symlog, [-100., 100.], [0., 100.]);
        assert_eq!(scale.position(0.), Some(50.));
        assert!(scale.position(-100.).unwrap() < 50.);
    }

    #[test]
    fn nice_rounds_the_domain_outward() {
        let scale = ContinuousScale::new(Transform::Linear, [0.3, 97.], [0., 100.]);
        let nice = scale.nice(5);
        assert_eq!(nice.domain(), [0., 100.]);

        // A log domain rounds to powers of ten.
        let scale = ContinuousScale::new(Transform::Log, [3., 60.], [0., 100.]);
        assert_eq!(scale.nice(5).domain(), [1., 100.]);
    }

    #[test]
    fn clamp_pins_out_of_domain_positions() {
        let scale = ContinuousScale::new(Transform::Linear, [0., 10.], [0., 100.]);
        assert_eq!(scale.position(20.), Some(200.));
        let clamped = scale.clamped(true);
        assert_eq!(clamped.position(20.), Some(100.));
        assert_eq!(clamped.position(-5.), Some(0.));
    }

    #[test]
    fn reverse_flips_the_range() {
        let scale = ContinuousScale::new(Transform::Linear, [0., 10.], [0., 100.]);
        let reversed = scale.reversed();
        assert_eq!(reversed.range(), [100., 0.]);
        assert_eq!(reversed.position(0.), Some(100.));
    }

    #[test]
    fn band_places_categories() {
        let keys: Vec<ValueKey> = ["a", "b", "c"]
            .into_iter()
            .map(|v| Value::text(v).key())
            .collect();
        let scale = BandScale::new(keys, [0., 300.]).padding_inner(0.);
        assert_eq!(scale.band_width(), 100.);
        assert_eq!(scale.center(&Value::text("b").key()), Some(150.));
        assert_eq!(scale.start(&Value::text("c").key()), Some(200.));
    }

    #[test]
    fn point_scale_pads_half_a_step() {
        let keys: Vec<ValueKey> = [1., 2., 3.]
            .into_iter()
            .map(|v| Value::Number(v).key())
            .collect();
        // Plot's point scale pads half a step at each end: three points
        // across 0..120 sit at 20, 60, 100.
        let scale = PointScale::new(keys.clone(), [0., 120.]);
        assert_eq!(scale.position(&Value::Number(1.).key()), Some(20.));
        assert_eq!(scale.position(&Value::Number(3.).key()), Some(100.));
        // With no padding they pin the ends, d3's default.
        let flush = PointScale::new(keys, [0., 100.]).padding(0.);
        assert_eq!(flush.position(&Value::Number(1.).key()), Some(0.));
        assert_eq!(flush.position(&Value::Number(3.).key()), Some(100.));
    }

    #[test]
    fn ordinal_colors_cycle() {
        let range = vec![hsla_from_hex(0xff0000), hsla_from_hex(0x00ff00)];
        let keys: Vec<ValueKey> = ["a", "b", "c"]
            .into_iter()
            .map(|v| Value::text(v).key())
            .collect();
        let scale = ColorScale::ordinal(keys, range.clone());
        assert_eq!(scale.color(&Value::text("a")), Some(range[0]));
        assert_eq!(scale.color(&Value::text("c")), Some(range[0]));
    }

    #[test]
    fn continuous_colors_interpolate() {
        let scale = ColorScale::continuous(
            [0., 100.],
            ColorRange::Continuous(vec![hsla_from_hex(0x000000), hsla_from_hex(0xffffff)]),
        );
        let mid = scale.color(&Value::Number(50.)).unwrap();
        assert!((mid.l - 0.5).abs() < 0.05);
    }

    #[test]
    fn hex_round_trip_is_close() {
        let color = hsla_from_hex(0x3366cc);
        assert!((color.h - (220. / 360.)).abs() < 0.01);
        assert!(color.a == 1.);
    }
}
