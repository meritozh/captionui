//! The legend: its entries (swatches, symbols, quantities and ramps),
//! the one layout the insets and the painter share, and the painters.

use gpui::{
    App, Bounds, FontWeight, Hsla, Pixels, Point, SharedString, TextAlign, Window, point, px,
};

use crate::mark::MarkKind;
use crate::paint::{self, Label};
use crate::plot::{LegendPosition, Plot};
use crate::render::{self, PreparedMark};
use crate::style::NumberChannel;
use crate::theme::PlotTheme;
use crate::value::Value;

/// The height a legend row takes in the frame's margin.
pub(super) const LEGEND_BAND: f32 = 20.;
/// The narrowest width a side legend reserves; a longer entry widens it.
const LEGEND_SIDE: f32 = 72.;
/// The gap between a legend's swatch and its label.
const LEGEND_SWATCH_GAP: f32 = 14.;
/// The gap between one legend entry's label and the next entry's swatch.
const LEGEND_ENTRY_GAP: f32 = 14.;
/// The margin between the figure's edge and a legend's first swatch.
const LEGEND_MARGIN: f32 = 8.;
/// Plot's ramp legend: 240 pixels wide, 50 tall.
const RAMP_WIDTH: f32 = 240.;
const RAMP_HEIGHT: f32 = 50.;

impl Plot {
    /// The entries of the legends the scales ask for: a swatch per value of an
    /// ordinal color scale (a symbol per value of a symbol scale, colored when
    /// the two share their domain), or a ramp for a continuous one.
    fn scale_legend_entries(
        &self,
        colors: &render::color::PlotColors,
        facets: &[Vec<PreparedMark>],
    ) -> Vec<LegendEntry> {
        let mut entries = Vec::new();
        let symbol_values: Vec<Value> = {
            let mut values: Vec<Value> = Vec::new();
            for mark in facets.iter().flatten() {
                if let Some(channel) = &mark.mark.channels.symbol
                    && render::constant_symbol(channel).is_none()
                {
                    for row in &mark.rows {
                        let value = channel.read(&row.row);
                        if !value.is_null() && !values.iter().any(|v| v.key() == value.key()) {
                            values.push(value);
                        }
                    }
                }
            }
            values.sort();
            values
        };
        if self.scales.symbol.legend == Some(true) && !symbol_values.is_empty() {
            let filled = facets.iter().flatten().any(|mark| {
                mark.mark.channels.symbol.is_some() && mark.mark.channels.fill.is_some()
            });
            let range = if filled {
                crate::style::SYMBOLS_FILL
            } else {
                crate::style::SYMBOLS_STROKE
            };
            for (i, value) in symbol_values.iter().enumerate() {
                let color = colors
                    .scale
                    .as_ref()
                    .filter(|scale| scale.is_ordinal())
                    .and_then(|scale| scale.color(value))
                    .unwrap_or(colors.default_fill_color());
                entries.push(LegendEntry {
                    label: value.label(),
                    swatch: LegendSwatch::Symbol(range[i % range.len()], color, filled),
                    shape: LegendShape::Circle,
                });
            }
            return entries;
        }
        if self.scales.color.legend == Some(true)
            && let Some(scale) = &colors.scale
        {
            if scale.is_ordinal() && scale.thresholds().is_none() {
                for key in scale.domain() {
                    let value = key.to_value();
                    let color = scale.color(&value).unwrap_or_default();
                    let label = match &self.scales.color.tick_format {
                        Some(format) => format.format(&value).into(),
                        // Plot's legend stringifies a null category.
                        None if value.is_null() => "null".into(),
                        None => value.label(),
                    };
                    entries.push(LegendEntry {
                        label,
                        swatch: LegendSwatch::Color(color),
                        shape: LegendShape::Square,
                    });
                }
            } else {
                entries.push(LegendEntry {
                    label: self
                        .scales
                        .color
                        .label
                        .clone()
                        .flatten()
                        .or_else(|| self.color_field_label(facets))
                        .unwrap_or_default(),
                    swatch: LegendSwatch::Ramp,
                    shape: LegendShape::Square,
                });
            }
        }
        entries
    }

    /// The field a color channel reads, for a ramp legend's label.
    fn color_field_label(&self, facets: &[Vec<PreparedMark>]) -> Option<SharedString> {
        facets.iter().flatten().find_map(|mark| {
            let c = &mark.mark.channels;
            c.fill
                .as_ref()
                .and_then(|f| f.field())
                .or_else(|| c.stroke.as_ref().and_then(|f| f.field()))
                .cloned()
        })
    }

    /// Where the legend goes: where [`Plot::legend`] put it, else above the
    /// plot when a scale asks for one, Plot's `color: {legend: true}`.
    pub(super) fn legend_position(&self) -> LegendPosition {
        if self.legend != LegendPosition::None {
            return self.legend;
        }
        let wants = |options: &crate::options::ScaleOptions| options.legend == Some(true);
        if wants(&self.scales.color)
            || wants(&self.scales.symbol)
            || wants(&self.scales.r)
            || wants(&self.scales.opacity)
        {
            LegendPosition::Top
        } else {
            LegendPosition::None
        }
    }

    /// Where each legend entry sits, relative to the legend's own top-left,
    /// and the height the legend takes.
    ///
    /// This is the one layout both the frame's insets and the painter read,
    /// so the band the frame reserves is exactly the band the legend fills.
    /// A top or bottom legend wraps its entries at `width`, the figure's
    /// width; a side legend stacks them in a column.
    pub(super) fn legend_layout(
        &self,
        theme: &PlotTheme,
        window: &mut Window,
        entries: &[LegendEntry],
        width: f32,
    ) -> (Vec<(f32, f32)>, f32) {
        let vertical = matches!(
            self.legend_position(),
            LegendPosition::Left | LegendPosition::Right
        );
        let available = (width - LEGEND_MARGIN * 2.).max(1.);
        let mut positions = Vec::with_capacity(entries.len());
        let (mut x, mut y) = (0.0_f32, 0.0_f32);
        if self.legend_title.is_some() {
            y += LEGEND_BAND;
        }
        for (index, entry) in entries.iter().enumerate() {
            // A ramp is Plot's 240 × 50 continuous legend, on its own row.
            if entry.swatch == LegendSwatch::Ramp {
                if x > 0. {
                    y += LEGEND_BAND;
                    x = 0.;
                }
                positions.push((0., y));
                y += RAMP_HEIGHT - LEGEND_BAND;
                continue;
            }
            if vertical {
                positions.push((0., y));
                y += LEGEND_BAND;
                continue;
            }
            // A size or opacity legend is a separate list from the color one,
            // so it starts on a fresh row rather than running on from it.
            let starts_quantities =
                index > 0 && entry.swatch.is_quantity() && !entries[index - 1].swatch.is_quantity();
            let label = paint::measure(&entry.label, theme.label_size, window)
                .width
                .as_f32();
            // A row that would run past the figure wraps to the next one.
            if starts_quantities || (x > 0. && x + LEGEND_SWATCH_GAP + label > available) {
                x = 0.;
                y += LEGEND_BAND;
            }
            positions.push((x, y));
            x += LEGEND_SWATCH_GAP + label + LEGEND_ENTRY_GAP;
        }
        let height = if vertical { y } else { y + LEGEND_BAND };
        (positions, height)
    }

    /// The width a side legend needs: its widest label plus its swatch.
    ///
    /// A legend stacked vertically on the left or right of the plot is as wide
    /// as its longest entry; reserving a fixed width would clip the labels.
    pub(super) fn legend_width(
        &self,
        theme: &PlotTheme,
        window: &mut Window,
        entries: &[LegendEntry],
    ) -> f32 {
        let widest = entries
            .iter()
            .map(|entry| {
                paint::measure(&entry.label, theme.label_size, window)
                    .width
                    .as_f32()
            })
            .fold(0.0_f32, f32::max);
        // A heading sits flush left above the swatches, so it may be the
        // widest line.
        let heading = self.legend_title.as_ref().map_or(0., |title| {
            paint::measure(title, theme.label_size, window)
                .width
                .as_f32()
                - LEGEND_SWATCH_GAP
        });
        (LEGEND_SWATCH_GAP + widest.max(heading) + LEGEND_MARGIN * 2.).max(LEGEND_SIDE)
    }

    /// Where the top legend's row starts, below the title the frame reserves.
    fn legend_top(&self, theme: &PlotTheme) -> f32 {
        let mut y = LEGEND_MARGIN;
        if self.title.text.is_some() {
            y += theme.title_size.as_f32() + 6.;
            if self.title.subtitle.is_some() {
                y += theme.label_size.as_f32() + 8.;
            }
        }
        y
    }

    /// The legend's entries: one per series or one per categorical value.
    ///
    /// Every facet's marks are read, and an entry appears once however many
    /// facets draw it: a faceted chart's legend describes the series, not each
    /// small multiple.
    pub(super) fn legend_entries(
        &self,
        colors: &render::color::PlotColors,
        facets: &[Vec<PreparedMark>],
    ) -> Vec<LegendEntry> {
        // A legend a scale asks for (`color: {legend: true}`) lists that
        // scale's domain, as Plot's do.
        if self.legend == LegendPosition::None {
            return self.scale_legend_entries(colors, facets);
        }
        let mut entries: Vec<LegendEntry> = Vec::new();
        // Every facet holds the same marks in the same order, so a mark's
        // place in its facet's list is its place in the plot's.
        for (index, mark) in facets.iter().flatten().enumerate() {
            let index = index % self.marks.len().max(1);
            let mut series: Vec<(SharedString, Hsla)> = Vec::new();
            for row in mark.rows.iter() {
                if let Some(z) = &row.z {
                    // The swatch is the color the mark draws this series in.
                    let color = colors.identity(mark, index, row);
                    if !series.iter().any(|(name, _)| *name == z.label()) {
                        series.push((z.label(), color));
                    }
                }
            }
            // The legend reads in the color scale's (sorted) domain order, as
            // Plot's does, not in the order the rows happen to come.
            let domain = colors.ordinal.domain();
            let rank = |label: &SharedString| {
                domain
                    .iter()
                    .position(|key| key.to_value().label() == *label)
                    .unwrap_or(usize::MAX)
            };
            series.sort_by_key(|(label, _)| rank(label));
            let shape = legend_shape(mark.mark.kind);
            // One legend entry per series, however many marks draw it: a line
            // with a labelled dot at its peak is one series, not three.
            if !series.is_empty() {
                for (label, color) in series {
                    if entries.iter().any(|entry| entry.label == label) {
                        continue;
                    }
                    entries.push(LegendEntry {
                        label,
                        swatch: LegendSwatch::Color(color),
                        shape,
                    });
                }
            } else if let Some(title) = &mark.mark.title {
                // A titled mark's swatch is the color it draws in: its first
                // row's, or its default when it has no rows.
                let color = match mark.rows.first() {
                    Some(row) => colors.identity(mark, index, row),
                    None if mark.mark.channels.stroke.is_some() => {
                        colors.default_stroke(mark, index)
                    }
                    None => colors.default_fill(mark, index),
                };
                if !entries.iter().any(|entry| entry.label == *title) {
                    entries.push(LegendEntry {
                        label: title.clone(),
                        swatch: LegendSwatch::Color(color),
                        shape,
                    });
                }
            }
            // A `r` or `opacity` accessor is a third dimension the color
            // legend cannot show, so it gets a legend of its own: the smallest
            // and largest value, and the middle one when the range is wide.
            if let Some(channel) = &mark.mark.channels.r {
                let values = numbers(mark, channel);
                for entry in
                    quantity_entries("r", &values, |value| LegendSwatch::Size(value.max(1.)))
                {
                    if !entries.iter().any(|existing| existing.label == entry.label) {
                        entries.push(entry);
                    }
                }
            }
            if let Some(channel) = &mark.mark.channels.opacity {
                let values = numbers(mark, channel);
                for entry in quantity_entries("opacity", &values, |value| {
                    LegendSwatch::Opacity(value.clamp(0., 1.))
                }) {
                    if !entries.iter().any(|existing| existing.label == entry.label) {
                        entries.push(entry);
                    }
                }
            }
        }
        entries
    }

    pub(super) fn paint_legend(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
        facets: &[Vec<PreparedMark>],
        colors: &render::color::PlotColors,
    ) {
        let entries = self.legend_entries(colors, facets);
        let position = self.legend_position();
        if position == LegendPosition::None || entries.is_empty() {
            return;
        }
        let font = gpui::font(theme.font_family.clone());
        let swatch = px(8.);
        let vertical = matches!(position, LegendPosition::Left | LegendPosition::Right);
        let side_width = if vertical {
            self.legend_width(theme, window, &entries)
        } else {
            0.
        };
        let (positions, height) =
            self.legend_layout(theme, window, &entries, bounds.size.width.as_f32());

        // A top legend sits under the title, inside the band the frame
        // reserved for it; a bottom one in the band above the figure's bottom
        // margin. `bounds` is the figure's, already shrunk to its `height`.
        let (left, top) = match position {
            LegendPosition::Top => (LEGEND_MARGIN, self.legend_top(theme)),
            LegendPosition::Bottom => (
                LEGEND_MARGIN,
                bounds.size.height.as_f32() - height + LEGEND_MARGIN,
            ),
            LegendPosition::Left => (LEGEND_MARGIN, self.legend_top(theme)),
            LegendPosition::Right => (
                bounds.size.width.as_f32() - side_width + LEGEND_MARGIN,
                self.legend_top(theme),
            ),
            LegendPosition::None => return,
        };
        let origin = bounds.origin;

        // A legend heading sits on its own row above the entries.
        if let Some(title) = &self.legend_title {
            let heading = Label::new(
                title.clone(),
                theme.label_size,
                theme.label,
                FontWeight::SEMIBOLD,
                font.clone(),
                window,
            );
            heading.paint_at(
                point(origin.x + px(left), origin.y + px(top)),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }

        for (entry, (dx, dy)) in entries.iter().zip(positions) {
            let (x, y) = (left + dx, top + dy);
            if entry.swatch == LegendSwatch::Ramp {
                if let Some(scale) = &colors.scale {
                    self.paint_ramp(
                        scale,
                        &entry.label,
                        point(origin.x + px(x), origin.y + px(y)),
                        theme,
                        window,
                        cx,
                    );
                }
                continue;
            }
            if let LegendSwatch::Symbol(symbol, color, filled) = entry.swatch {
                let centre = point(origin.x + px(x + 4.), origin.y + px(y + 6.));
                for (points, closed) in symbol.subpaths(centre, 4.5) {
                    if filled && closed {
                        paint::paint_fill(&points, color, window);
                    } else {
                        let mut outline = points.clone();
                        if closed && let Some(first) = points.first() {
                            outline.push(*first);
                        }
                        paint::paint_stroke(&outline, 1.5, color, window);
                    }
                }
                let label = Label::new(
                    entry.label.clone(),
                    theme.label_size,
                    theme.label,
                    FontWeight::NORMAL,
                    font.clone(),
                    window,
                );
                label.paint_at(
                    point(origin.x + px(x + LEGEND_SWATCH_GAP), origin.y + px(y)),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
                continue;
            }
            let centre = point(origin.x + px(x + 4.), origin.y + px(y + 6.));
            // A size or opacity swatch draws in the theme's label color: it
            // shows how big or how faint a mark is, not which series it is.
            let color = entry.swatch.color().unwrap_or(theme.label);
            match entry.swatch {
                LegendSwatch::Size(radius) => {
                    let radius = radius.min(6.);
                    paint::paint_fill(&paint::circle_points(centre, radius, 20), color, window)
                }
                LegendSwatch::Opacity(opacity) => paint::paint_fill(
                    &paint::circle_points(centre, 4., 20),
                    Hsla {
                        a: opacity,
                        ..color
                    },
                    window,
                ),
                _ => match entry.shape {
                    LegendShape::Square => paint::paint_rect(
                        Bounds::new(
                            point(origin.x + px(x), origin.y + px(y + 2.)),
                            gpui::size(swatch, swatch),
                        ),
                        2.,
                        color,
                        window,
                    ),
                    LegendShape::Line => paint::paint_stroke(
                        &[
                            point(px(centre.x.as_f32() - 5.), centre.y),
                            point(px(centre.x.as_f32() + 5.), centre.y),
                        ],
                        2.,
                        color,
                        window,
                    ),
                    LegendShape::Circle => {
                        paint::paint_fill(&paint::circle_points(centre, 4., 20), color, window)
                    }
                },
            }
            let label = Label::new(
                entry.label.clone(),
                theme.label_size,
                theme.label,
                FontWeight::NORMAL,
                font.clone(),
                window,
            );
            label.paint_at(
                point(origin.x + px(x + LEGEND_SWATCH_GAP), origin.y + px(y)),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }

    /// Paint Plot's ramp legend at `origin`: the label above, a 10-pixel
    /// gradient bar, and ticks with their values below.
    fn paint_ramp(
        &self,
        scale: &crate::scale::ColorScale,
        label: &SharedString,
        origin: Point<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
    ) {
        let font = gpui::font(theme.font_family.clone());
        let text = |t: SharedString, window: &mut Window| {
            Label::new(
                t,
                theme.label_size,
                theme.label,
                FontWeight::NORMAL,
                font.clone(),
                window,
            )
        };
        if !label.is_empty() {
            text(label.clone(), window).paint_at(origin, TextAlign::Left, None, window, cx);
        }
        let bar_top = origin.y + px(16.);
        let bar_height = 10.;
        let [lo, hi] = scale.extent();
        if let Some(thresholds) = scale.thresholds() {
            // A threshold scale's legend is a row of equal swatches, one per
            // bin, its thresholds labelled at their joins.
            let n = thresholds.len() + 1;
            let w = RAMP_WIDTH / n as f32;
            for i in 0..n {
                let value = if i == 0 {
                    thresholds.first().copied().unwrap_or(0.) - 1.
                } else {
                    thresholds[i - 1]
                };
                let color = scale.color(&Value::Number(value)).unwrap_or_default();
                paint::paint_rect(
                    Bounds::new(
                        point(origin.x + px(w * i as f32), bar_top),
                        gpui::size(px(w), px(bar_height)),
                    ),
                    0.,
                    color,
                    window,
                );
            }
            for (i, t) in thresholds.iter().enumerate() {
                let x = origin.x + px(w * (i + 1) as f32);
                let label = match &self.scales.color.tick_format {
                    Some(format) => format.format(&Value::Number(*t)),
                    None => crate::format::format_number("", *t),
                };
                text(label.into(), window).paint_at(
                    point(x, bar_top + px(bar_height + 4.)),
                    TextAlign::Center,
                    None,
                    window,
                    cx,
                );
            }
            return;
        }
        let slices = 120;
        for i in 0..slices {
            let t = i as f64 / (slices - 1) as f64;
            let value = lo + (hi - lo) * t;
            let color = scale.color(&Value::Number(value)).unwrap_or_default();
            let x = origin.x + px(RAMP_WIDTH * i as f32 / slices as f32);
            paint::paint_rect(
                Bounds::new(
                    point(x, bar_top),
                    gpui::size(px(RAMP_WIDTH / slices as f32 + 0.5), px(bar_height)),
                ),
                0.,
                color,
                window,
            );
        }
        // Plot's ramp ticks: about one per 64 pixels, labelled below the bar;
        // a temporal scale ticks at calendar boundaries.
        let count = (RAMP_WIDTH / 64.).round() as usize;
        let temporal = scale.is_temporal();
        let ticks = if temporal {
            crate::ticks::time_ticks([lo, hi], count)
        } else {
            crate::ticks::linear_ticks([lo, hi], count, crate::scale::Transform::Linear)
        };
        let format = crate::format::tick_format(
            lo,
            hi,
            count,
            match &self.scales.color.tick_format {
                Some(crate::options::TickFormat::Spec(spec)) => Some(spec.as_ref()),
                _ => None,
            },
        );
        for tick in ticks {
            if hi == lo {
                break;
            }
            let ratio = scale.ratio(tick);
            let x = origin.x + px(RAMP_WIDTH * ratio);
            paint::paint_stroke(
                &[point(x, bar_top), point(x, bar_top + px(bar_height + 4.))],
                1.,
                theme.label,
                window,
            );
            let label: SharedString = match &self.scales.color.tick_format {
                Some(crate::options::TickFormat::Fn(f)) => f(&Value::Number(tick)).into(),
                _ if temporal => crate::format::format_time(
                    "%Y",
                    Value::date_from_millis(tick as i64)
                        .as_date()
                        .unwrap_or_default(),
                )
                .into(),
                _ => format.format(tick).into(),
            };
            text(label, window).paint_at(
                point(x, bar_top + px(bar_height + 5.)),
                TextAlign::Center,
                None,
                window,
                cx,
            );
        }
    }
}

/// Every distinct value a numeric channel reads, in reading order.
fn numbers(mark: &PreparedMark, channel: &NumberChannel) -> Vec<f32> {
    let mut values: Vec<f32> = Vec::new();
    for row in mark.rows.iter() {
        let value = channel.read(&row.row);
        if value.is_finite() && !values.contains(&value) {
            values.push(value);
        }
    }
    values
}

/// The legend entries for a numeric channel: its ends and, when the range is
/// wide enough to matter, a middle value. A channel with a single value draws
/// no legend, the same way a constant color draws none.
fn quantity_entries(
    name: &str,
    values: &[f32],
    swatch: impl Fn(f32) -> LegendSwatch,
) -> Vec<LegendEntry> {
    let (Some(min), Some(max)) = (
        values.iter().copied().reduce(f32::min),
        values.iter().copied().reduce(f32::max),
    ) else {
        return Vec::new();
    };
    if min == max {
        return Vec::new();
    }
    let middle = (min + max) / 2.;
    let mut ticks = vec![min];
    if middle != min && middle != max {
        ticks.push(middle);
    }
    ticks.push(max);
    ticks
        .into_iter()
        .map(|value| LegendEntry {
            label: format!("{name} {}", trim_number(value)).into(),
            swatch: swatch(value),
            shape: LegendShape::Circle,
        })
        .collect()
}

/// A number without its trailing zeros, so a legend reads `r 2` not `r 2.0`.
fn trim_number(value: f32) -> String {
    if value.fract() == 0. {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// The swatch shape a mark's legend entries draw.
fn legend_shape(kind: MarkKind) -> LegendShape {
    match kind {
        MarkKind::Line | MarkKind::LineX | MarkKind::LineY => LegendShape::Line,
        MarkKind::Dot | MarkKind::DotX | MarkKind::DotY => LegendShape::Circle,
        _ => LegendShape::Square,
    }
}

/// One legend entry.
#[derive(Clone, Debug)]
pub(super) struct LegendEntry {
    label: SharedString,
    /// What the swatch shows: a color, a size or an opacity. Plot gives each
    /// varying channel its own legend, so a dot plot that encodes a value in
    /// its radius says so in the legend rather than in the caption.
    swatch: LegendSwatch,
    /// The shape of the entry's swatch, so a line's legend reads as a line and
    /// a dot's as a dot rather than every mark sharing one square.
    shape: LegendShape,
}

/// The quantity a legend swatch stands for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LegendSwatch {
    /// A color, which the swatch draws in the mark's shape.
    Color(Hsla),
    /// A radius in pixels, for a `r` channel: the swatch is a circle of it.
    Size(f32),
    /// An opacity, for an `opacity` channel: the swatch is a faded circle.
    Opacity(f32),
    /// A symbol, for a symbol legend: the symbol, its color, and whether it
    /// is filled.
    Symbol(crate::style::Symbol, Hsla, bool),
    /// A continuous color ramp, Plot's `legendRamp`.
    Ramp,
}

impl LegendSwatch {
    /// The color a swatch draws, for the swatches that are colors. A size or
    /// opacity swatch draws in the theme's label color instead.
    fn color(&self) -> Option<Hsla> {
        match self {
            LegendSwatch::Color(color) | LegendSwatch::Symbol(_, color, _) => Some(*color),
            LegendSwatch::Size(_) | LegendSwatch::Opacity(_) | LegendSwatch::Ramp => None,
        }
    }

    /// Whether the swatch stands for a size or an opacity rather than a
    /// color, which decides where its list of entries starts.
    fn is_quantity(&self) -> bool {
        !matches!(
            self,
            LegendSwatch::Color(_) | LegendSwatch::Symbol(..) | LegendSwatch::Ramp
        )
    }
}

/// The shape a legend swatch draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LegendShape {
    /// A filled square, Plot's default for a rect or a cell.
    #[default]
    Square,
    /// A horizontal line, for a line or an area's edge.
    Line,
    /// A filled circle, for a dot.
    Circle,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Table;

    #[test]
    fn legend_entries_follow_series() {
        let data = [
            Table::empty()
                .with("x", Value::text("a"))
                .with("y", Value::Number(1.))
                .with("z", Value::text("one")),
            Table::empty()
                .with("x", Value::text("b"))
                .with("y", Value::Number(2.))
                .with("z", Value::text("two")),
        ];
        let plot = Plot::new(data)
            .line(|mark| mark.x("x").y("y").stroke("z"))
            .legend(LegendPosition::Top);
        let prepared = render::prepare(plot.data(), plot.marks());
        let theme = PlotTheme::from_theme(&gpui_kit::component::Theme::default());
        let facets = [prepared];
        let colors = plot.plot_colors(&theme, &facets);
        let entries = plot.legend_entries(&colors, &facets);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "one");
    }

    #[test]
    fn tooltip_and_legend_colors_match_the_painted_series() {
        let data = ["a", "b", "c"].map(|z| {
            Table::empty()
                .with("x", Value::Number(1.))
                .with("y", Value::Number(1.))
                .with("z", Value::text(z))
        });
        // A field-named color is Plot's color channel: it colors the rows
        // through the ordinal scale, and the legend lists the same colors.
        let plot = Plot::new(data.clone())
            .dot(|mark| mark.x("x").y("y").fill("z"))
            .legend(LegendPosition::Top);
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let theme = PlotTheme::from_theme(&gpui_kit::component::Theme::default());
        let colors = plot.plot_colors(&theme, &facets);
        let mark = &facets[0][0];
        for (index, row) in mark.rows.iter().enumerate() {
            assert_eq!(colors.identity(mark, 0, row), theme.palette_color(index));
        }
        // A `z` alone groups but does not color, as in Plot.
        let grouped = Plot::new(data).dot(|mark| mark.x("x").y("y").z("z"));
        let facets_z = vec![render::prepare(grouped.data(), grouped.marks())];
        let colors_z = grouped.plot_colors(&theme, &facets_z);
        let mark_z = &facets_z[0][0];
        assert_eq!(
            colors_z.identity(mark_z, 0, &mark_z.rows[0]),
            theme.mark_color
        );
        let entries = plot.legend_entries(&colors, &facets);
        assert_eq!(
            entries[2].swatch,
            LegendSwatch::Color(theme.palette_color(2))
        );
    }

    #[test]
    fn a_titled_plain_mark_is_listed_in_its_paint_color() {
        let data = [Table::empty()
            .with("x", Value::Number(1.))
            .with("y", Value::Number(1.))];
        let plot = Plot::new(data)
            .line(|mark| mark.x("x").y("y").title("Sales"))
            .legend(LegendPosition::Top);
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let theme = PlotTheme::from_theme(&gpui_kit::component::Theme::default());
        let colors = plot.plot_colors(&theme, &facets);
        let entries = plot.legend_entries(&colors, &facets);
        assert_eq!(entries[0].swatch, LegendSwatch::Color(theme.mark_color));
    }
}
