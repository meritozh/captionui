//! The plot's theme: the colors, fonts and spacing it draws its frame, axes,
//! grid and legends with.
//!
//! Plot has no theme of its own beyond CSS variables; a GPUI application has
//! GPUI Kit's [`Theme`], so [`PlotTheme`] projects the semantic tokens a plot
//! needs and keeps the plotting code independent of the component library.

use gpui::{App, Hsla, Pixels, SharedString, px};
use gpui_kit::component::{ActiveTheme, Theme};

use crate::scale::ColorRange;

/// The tokens a plot draws with.
#[derive(Clone, Debug)]
pub struct PlotTheme {
    /// The plot's background.
    pub background: Hsla,
    /// Axis lines and grid lines.
    pub grid: Hsla,
    /// The axis line itself, where it is drawn.
    pub axis: Hsla,
    /// Tick and axis labels.
    pub label: Hsla,
    /// The title's text.
    pub title: Hsla,
    /// The tooltip's background.
    pub tooltip_background: Hsla,
    /// The tooltip's border.
    pub tooltip_border: Hsla,
    /// The tooltip's title text.
    pub tooltip_foreground: Hsla,
    /// The tooltip's secondary text.
    pub tooltip_muted: Hsla,
    /// The categorical palette a series is colored from.
    ///
    /// Plot's default is Observable10. GPUI Kit's `chart.1..5` are five shades
    /// of one hue, which is a sequential ramp rather than a categorical
    /// palette, so a theme whose chart colors share a hue keeps them in
    /// [`sequential`](Self::sequential) and this falls back to Observable10.
    pub palette: Vec<Hsla>,
    /// The sequential ramp a quantitative color channel is interpolated
    /// through, from low to high: Plot's `color.range` for continuous data.
    pub sequential: Vec<Hsla>,
    /// The color a mark draws in when it names no color of its own, Plot's
    /// `currentColor` default.
    pub mark_color: Hsla,
    /// The grid line dash, in pixels; empty for a solid line.
    pub grid_dash: Vec<Pixels>,
    /// The font the labels use.
    pub font_family: SharedString,
    /// The axis and tick label size.
    pub label_size: Pixels,
    /// The title's size.
    pub title_size: Pixels,
    /// The stroke width of an axis or grid line.
    pub line_width: Pixels,
}

/// Whether the theme's chart colors work as a categorical palette.
///
/// GPUI Kit's stock themes set `chart.1..5` to five shades of one blue
/// (`#93c5fd` to `#1e40af`). That is a sequential ramp: two series painted
/// with `chart.1` and `chart.2` share a hue and differ only in lightness, so a
/// categorical chart drawn with them cannot be read. A palette whose hues all
/// sit within a narrow band is treated as a ramp and replaced with
/// Observable10; a palette that spans the hue circle is kept as it is.
fn categorical_chart_colors(colors: &[Hsla]) -> bool {
    let hues: Vec<f32> = colors
        .iter()
        .filter(|color| color.s > 0.05)
        .map(|color| color.h)
        .collect();
    if hues.len() < 2 {
        return false;
    }
    let (min, max) = hues.iter().fold((1.0_f32, 0.0_f32), |(min, max), hue| {
        (min.min(*hue), max.max(*hue))
    });
    // Hues wrap at 1; the shortest arc across the circle is the spread.
    let spread = (max - min).min(1. - (max - min));
    spread >= 0.08
}

impl PlotTheme {
    /// Project the active GPUI Kit theme onto a plot theme.
    pub fn from_theme(theme: &Theme) -> Self {
        let colors = &theme.colors;
        let chart_colors = vec![
            colors.chart_1,
            colors.chart_2,
            colors.chart_3,
            colors.chart_4,
            colors.chart_5,
        ];
        Self {
            background: colors.background,
            // Plot draws its grid in `currentColor` at a tenth of its opacity,
            // and its axes and labels in `currentColor` itself.
            grid: Hsla {
                a: colors.foreground.a * 0.1,
                ..colors.foreground
            },
            axis: colors.foreground,
            label: colors.foreground,
            title: colors.foreground,
            tooltip_background: colors.popover,
            tooltip_border: colors.border,
            tooltip_foreground: colors.popover_foreground,
            tooltip_muted: colors.muted_foreground,
            // A theme whose chart colors span the hue circle is using them as
            // categories; the stock one-blue ramp is not, so it is replaced.
            // Plot's categorical scheme is Observable10 whatever the page's
            // theme, so a chart reads the same everywhere.
            palette: {
                let _ = categorical_chart_colors(&chart_colors);
                ColorRange::observable10_colors()
            },
            sequential: chart_colors,
            mark_color: colors.foreground,
            grid_dash: Vec::new(),
            font_family: theme.font_family.clone(),
            // Plot's default font is 10px.
            label_size: px(10.),
            title_size: px(13.),
            line_width: px(1.),
        }
    }

    /// The active theme's plot tokens.
    pub fn global(cx: &App) -> Self {
        Self::from_theme(cx.theme())
    }

    /// The palette color at `index`, cycling.
    pub fn palette_color(&self, index: usize) -> Hsla {
        if self.palette.is_empty() {
            return Hsla::default();
        }
        self.palette[index % self.palette.len()]
    }

    /// The ramp a continuous color channel interpolates through.
    ///
    /// The theme's sequential colors when it has at least two; a light-to-dark
    /// version of its first palette color otherwise.
    pub fn sequential_range(&self) -> crate::scale::ColorRange {
        if self.sequential.len() >= 2 {
            return crate::scale::ColorRange::Continuous(self.sequential.clone());
        }
        let base = self.palette_color(0);
        crate::scale::ColorRange::Continuous(vec![
            Hsla { l: 0.92, ..base },
            base,
            Hsla { l: 0.35, ..base },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_cycles() {
        let theme = PlotTheme {
            palette: vec![Hsla::default(), Hsla::default()],
            ..PlotTheme::from_theme(&Theme::default())
        };
        assert_eq!(theme.palette_color(0), theme.palette_color(2));
        assert_eq!(theme.palette_color(3), theme.palette[1]);
    }
}
