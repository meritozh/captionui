//! The plot's one color authority.
//!
//! Every place that needs a mark's color — the painters, the legend and the
//! tooltip — reads it from [`PlotColors`], built once per frame from every
//! facet's marks. Deriving the color separately in each place let the legend
//! swatch and the tooltip dot drift from the color a mark actually paints.
//!
//! The color scale follows Plot's: it spans every value the marks' color
//! channels read (`fill: "sex"`, `stroke: "Anomaly"`), its type is inferred
//! from those values — quantitative data a continuous ramp (Plot's default
//! `turbo`), anything else an ordinal palette (`observable10`) — and the
//! plot's `color: {…}` options override any of it. A mark without a color
//! channel paints in `currentColor`, however it groups its rows by `z`.

use gpui::{Hsla, SharedString};

use crate::options::{ScaleOptions, ScaleType};
use crate::render::{PreparedMark, PreparedRow};
use crate::scale::{ColorRange, ColorScale, Transform};
use crate::style::ColorChannel;
use crate::table::Table;
use crate::theme::PlotTheme;
use crate::value::{PlotKind, Value};

/// The plot-wide color scales and the theme's defaults.
#[derive(Clone)]
pub struct PlotColors {
    /// Plot's color scale, over every value a color channel reads.
    pub scale: Option<ColorScale>,
    /// The ordinal half of [`Self::scale`], for the swatch legend: empty when
    /// the scale is continuous.
    pub ordinal: ColorScale,
    /// The continuous scale [`crate::Plot::color_by`] sets, over the field it
    /// names, with a domain spanning every facet's rows.
    pub continuous: Option<(SharedString, ColorScale)>,
    mark_color: Hsla,
}

/// The values a color channel reads across every facet's marks.
fn color_values(facets: &[Vec<PreparedMark>]) -> Vec<Value> {
    let mut values = Vec::new();
    for mark in facets.iter().flatten() {
        // A density's "density" color reads Plot's density units; zero
        // anchors the scale as Plot's does.
        let density_field = |c: &Option<crate::style::ColorChannel>| matches!(c, Some(crate::style::ColorChannel::Field(f)) if f.eq_ignore_ascii_case("density"));
        if mark.mark.kind == crate::mark::MarkKind::Density
            && (density_field(&mark.mark.channels.fill)
                || density_field(&mark.mark.channels.stroke))
        {
            values.push(Value::Number(0.));
            if let Some(max) = mark.density_max {
                values.push(Value::Number(max));
            }
            continue;
        }
        // A raster's or contour's grid values feed the color scale.
        if let Some(grid) = &mark.raster {
            values.extend(grid.values.iter().filter(|v| !v.is_null()).cloned());
            continue;
        }
        if matches!(
            mark.mark.kind,
            crate::mark::MarkKind::Raster | crate::mark::MarkKind::Contour
        ) && let Some(f) = mark.mark.raster.as_ref().and_then(|r| r.function.clone())
            && let Some([x1, y1, x2, y2]) = crate::render::raster_extent(&mark.mark)
        {
            // A sampled function: its range over a coarse grid.
            for j in 0..32 {
                for i in 0..32 {
                    let x = x1 + (x2 - x1) * (i as f64 + 0.5) / 32.;
                    let y = y1 + (y2 - y1) * (j as f64 + 0.5) / 32.;
                    values.push(f(x, y));
                }
            }
            continue;
        }
        let channels = &mark.mark.channels;
        for channel in [&channels.fill, &channels.stroke].into_iter().flatten() {
            values.extend(
                mark.rows
                    .iter()
                    .filter_map(|row| channel.scaled_value(&row.row))
                    .filter(|value| {
                        value
                            .as_text()
                            .is_none_or(|t| crate::css::parse(t).is_none())
                    }),
            );
        }
    }
    values.retain(|value| !value.is_null());
    values
}

/// Build Plot's color scale from the channels' values and the options.
pub fn color_scale(
    theme: &PlotTheme,
    values: &[Value],
    options: &ScaleOptions,
) -> Option<ColorScale> {
    let explicit: Option<Vec<Value>> = options.domain.clone();
    let sample = explicit.as_ref().map_or(values, |domain| domain.as_slice());
    if sample.is_empty() && options.r#type.is_none() {
        return None;
    }
    let quantitative = sample
        .iter()
        .all(|value| matches!(value.kind(), PlotKind::Number | PlotKind::Date));
    let scheme_diverging = options
        .scheme
        .as_deref()
        .is_some_and(crate::css::is_diverging);
    let kind = options.r#type.unwrap_or(if !quantitative {
        ScaleType::Categorical
    } else if scheme_diverging || options.pivot.is_some() {
        ScaleType::Diverging
    } else {
        ScaleType::Linear
    });
    let unknown = options.unknown;

    match kind {
        ScaleType::Ordinal | ScaleType::Categorical | ScaleType::Band | ScaleType::Point => {
            let domain: Vec<_> = match &explicit {
                Some(domain) => domain.iter().map(Value::key).collect(),
                None => {
                    let mut keys = crate::scale::dedup(values.iter().map(Value::key));
                    keys.sort();
                    keys
                }
            };
            let k = domain.len().max(1);
            let colors = match (&options.colors, &options.scheme) {
                (Some(colors), _) => colors.clone(),
                (None, Some(scheme)) => {
                    crate::css::discrete(scheme, k).unwrap_or_else(|| theme.palette.clone())
                }
                // Ordered data (numbers forced ordinal) takes Plot's turbo,
                // categories its categorical palette.
                (None, None) if kind == ScaleType::Ordinal && quantitative => {
                    crate::css::discrete("turbo", k).unwrap_or_else(|| theme.palette.clone())
                }
                (None, None) => theme.palette.clone(),
            };
            let colors = if options.reverse == Some(true) {
                colors.into_iter().rev().collect()
            } else {
                colors
            };
            Some(ColorScale::ordinal(domain, colors).with_unknown(unknown))
        }
        ScaleType::Threshold | ScaleType::Quantize | ScaleType::Quantile => {
            let numbers: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
            let thresholds: Vec<f64> = match kind {
                // Plot's threshold scale defaults to a single threshold at 0.
                ScaleType::Threshold => explicit
                    .as_ref()
                    .map(|d| d.iter().filter_map(Value::as_number).collect())
                    .unwrap_or_else(|| vec![0.]),
                ScaleType::Quantile => {
                    let n = options.n.unwrap_or(5);
                    let mut sorted = numbers.clone();
                    sorted.sort_by(f64::total_cmp);
                    (1..n)
                        .filter_map(|i| quantile(&sorted, i as f64 / n as f64))
                        .collect()
                }
                _ => {
                    let n = options.n.unwrap_or(5);
                    let [lo, hi] = options
                        .numeric_domain()
                        .or_else(|| crate::scale::extent(values.iter().cloned()))?;
                    (1..n)
                        .map(|i| lo + (hi - lo) * i as f64 / n as f64)
                        .collect()
                }
            };
            let k = thresholds.len() + 1;
            let scheme = options.scheme.as_deref().unwrap_or("rdylbu");
            let colors = options
                .colors
                .clone()
                .or_else(|| crate::css::discrete(scheme, k))
                .unwrap_or_else(|| theme.palette.clone());
            Some(
                ColorScale::ordinal(Vec::new(), colors)
                    .with_thresholds(thresholds)
                    .with_unknown(unknown),
            )
        }
        _ => {
            let mut extent = options
                .numeric_domain()
                .or_else(|| crate::scale::extent(values.iter().cloned()))
                .unwrap_or([0., 1.]);
            if options.zero == Some(true) {
                extent = [extent[0].min(0.), extent[1].max(0.)];
            }
            let diverging = kind == ScaleType::Diverging;
            let default_scheme = if diverging { "rdbu" } else { "turbo" };
            let mut range = match &options.colors {
                Some(colors) => ColorRange::Continuous(colors.clone()),
                None => crate::css::scheme(options.scheme.as_deref().unwrap_or(default_scheme))
                    .unwrap_or_else(|| theme.sequential_range()),
            };
            // A categorical scheme over quantitative data interpolates its
            // colors, as d3's piecewise interpolator would.
            if let ColorRange::Ordinal(colors) = range {
                range = ColorRange::Continuous(colors);
            }
            if options.reverse == Some(true)
                && let ColorRange::Continuous(stops) = &mut range
            {
                stops.reverse();
            }
            let transform = match kind {
                ScaleType::Log => Transform::Log,
                ScaleType::Sqrt => Transform::Sqrt,
                ScaleType::Symlog => Transform::Symlog,
                _ => Transform::Linear,
            };
            let mut scale = ColorScale::continuous(extent, range)
                .with_transform(transform)
                .with_unknown(unknown);
            if diverging {
                // Plot's diverging scales are symmetric about the pivot: the
                // shorter side stretches so equal distances get equal color.
                let pivot = options.pivot.unwrap_or(0.);
                let [lo, hi] = [extent[0].min(pivot), extent[1].max(pivot)];
                let delta = (pivot - lo).max(hi - pivot);
                scale =
                    ColorScale::continuous([pivot - delta, pivot + delta], scale.range().clone())
                        .with_transform(transform)
                        .with_unknown(unknown)
                        .with_pivot(pivot);
            }
            Some(scale)
        }
    }
}

/// The `p`-quantile of sorted numbers, d3's R-7 method.
fn quantile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let h = (sorted.len() - 1) as f64 * p;
    let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * (h - h.floor()))
}

impl PlotColors {
    /// Build the scales from the resolved marks and the plot's `color`
    /// options.
    pub fn new(
        theme: &PlotTheme,
        facets: &[Vec<PreparedMark>],
        color: Option<(&SharedString, &ColorRange)>,
        options: &ScaleOptions,
    ) -> Self {
        let scale = color_scale(theme, &color_values(facets), options);
        let ordinal = match &scale {
            Some(scale) if scale.is_ordinal() => scale.clone(),
            _ => ColorScale::ordinal(Vec::new(), theme.palette.clone()),
        };
        let continuous = color.and_then(|(field, range)| {
            let values = facets
                .iter()
                .flatten()
                .filter(|mark| mark.mark.channels.fill.is_none())
                .flat_map(|mark| mark.rows.iter())
                .map(|row| row.row.value(field));
            let extent = crate::scale::extent(values)?;
            Some((field.clone(), ColorScale::continuous(extent, range.clone())))
        });
        Self {
            scale,
            ordinal,
            continuous,
            mark_color: theme.mark_color,
        }
    }

    /// The color a mark's color channel gives a row: a constant, an
    /// accessor's result, `currentColor`, or a field's value through the
    /// plot's color scale.
    pub fn channel(&self, channel: &ColorChannel, row: &Table) -> Hsla {
        match channel {
            ColorChannel::Field(_) | ColorChannel::Value(_) => {
                let Some(value) = channel.scaled_value(row) else {
                    return Hsla::transparent_black();
                };
                // A value that is itself a CSS color paints as that color,
                // Plot's identity color scale.
                if let Some(color) = value.as_text().and_then(|t| crate::css::parse(t)) {
                    return color;
                }
                self.scale
                    .as_ref()
                    .and_then(|scale| scale.color(&value))
                    .unwrap_or(Hsla::transparent_black())
            }
            ColorChannel::Current => self.mark_color,
            _ => channel.read(row),
        }
    }

    /// The theme's `currentColor`.
    pub fn default_fill_color(&self) -> Hsla {
        self.mark_color
    }

    /// The fill a mark paints when no channel colors a row: its style's fill
    /// when it set one, else `currentColor`, the way Plot paints every
    /// uncolored mark.
    pub fn default_fill(&self, prepared: &PreparedMark, _mark_index: usize) -> Hsla {
        let style = prepared.mark.style.fill;
        if style != crate::mark::MarkStyle::default().fill {
            return style;
        }
        self.mark_color
    }

    /// The stroke a mark paints by default.
    pub fn default_stroke(&self, prepared: &PreparedMark, _mark_index: usize) -> Hsla {
        let style = prepared.mark.style.stroke;
        if style != crate::mark::MarkStyle::default().stroke {
            return style;
        }
        self.mark_color
    }

    /// The color a scale gives this row's fill, when one does: the mark's
    /// fill channel, then the plot's [`crate::Plot::color_by`] field.
    pub fn scaled_fill(&self, prepared: &PreparedMark, row: &PreparedRow) -> Option<Hsla> {
        if let Some(channel) = &prepared.mark.channels.fill {
            return Some(self.channel(channel, &row.row));
        }
        if let Some((field, scale)) = &self.continuous
            && let Some(color) = row.row.get(field).and_then(|value| scale.color(value))
        {
            return Some(color);
        }
        None
    }

    /// The fill one row paints in.
    pub fn row_fill(&self, prepared: &PreparedMark, mark_index: usize, row: &PreparedRow) -> Hsla {
        self.scaled_fill(prepared, row)
            .unwrap_or_else(|| self.default_fill(prepared, mark_index))
    }

    /// The color of one row's series, for a mark that paints per group rather
    /// than per row: a line's stroke channel, else its fill channel (an
    /// area's), else `currentColor`.
    pub fn series_color(
        &self,
        prepared: &PreparedMark,
        mark_index: usize,
        row: &PreparedRow,
    ) -> Hsla {
        if let Some(channel) = &prepared.mark.channels.stroke {
            return self.channel(channel, &row.row);
        }
        if let Some(channel) = &prepared.mark.channels.fill {
            return self.channel(channel, &row.row);
        }
        self.default_stroke(prepared, mark_index)
    }

    /// The color that identifies a row in the legend and the tooltip: the
    /// color its mark draws it in. A stroked mark (a line, a rule) is
    /// identified by its stroke, a filled one by its fill.
    pub fn identity(&self, prepared: &PreparedMark, mark_index: usize, row: &PreparedRow) -> Hsla {
        if prepared.mark.channels.fill.is_none() && prepared.mark.channels.stroke.is_some() {
            return self.series_color(prepared, mark_index, row);
        }
        self.row_fill(prepared, mark_index, row)
    }
}

/// Whether any row of the mark belongs to a `z` series.
pub fn has_series(prepared: &PreparedMark) -> bool {
    prepared.rows.iter().any(|row| row.z.is_some())
}
