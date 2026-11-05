//! The plot's one color authority.
//!
//! Every place that needs a mark's color — the painters, the legend and the
//! tooltip — reads it from [`PlotColors`], built once per frame from every
//! facet's marks. Deriving the color separately in each place let the legend
//! swatch and the tooltip dot drift from the color a mark actually paints.

use gpui::{Hsla, SharedString};

use crate::render::{PreparedMark, PreparedRow};
use crate::scale::{ColorRange, ColorScale};
use crate::theme::PlotTheme;

/// The plot-wide color scales and the theme's defaults.
#[derive(Clone)]
pub struct PlotColors {
    /// The ordinal scale over every `z` series, one for the whole plot so a
    /// series keeps its color across marks and facets.
    pub ordinal: ColorScale,
    /// The continuous scale [`crate::Plot::color`] sets, over the field it
    /// names, with a domain spanning every facet's rows.
    pub continuous: Option<(SharedString, ColorScale)>,
    mark_color: Hsla,
    palette: Vec<Hsla>,
}

impl PlotColors {
    /// Build the scales from the resolved marks.
    pub fn new(
        theme: &PlotTheme,
        facets: &[Vec<PreparedMark>],
        color: Option<(&SharedString, &ColorRange)>,
    ) -> Self {
        let ordinal = ColorScale::ordinal(
            crate::scale::dedup(
                facets
                    .iter()
                    .flatten()
                    .filter(|mark| mark.mark.channels.fill.is_none())
                    .flat_map(|mark| mark.rows.iter())
                    .filter_map(|row| row.z.as_ref().map(|z| z.key())),
            ),
            theme.palette.clone(),
        );
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
            ordinal,
            continuous,
            mark_color: theme.mark_color,
            palette: theme.palette.clone(),
        }
    }

    fn palette_color(&self, index: usize) -> Hsla {
        if self.palette.is_empty() {
            return self.mark_color;
        }
        self.palette[index % self.palette.len()]
    }

    /// The fill a mark paints when no channel or scale colors a row.
    ///
    /// A mark that draws series takes a palette color; a plain mark uses the
    /// theme's one mark color, the way Plot paints every uncolored mark in
    /// `currentColor`, so a line and a dot over the same data match.
    pub fn default_fill(&self, prepared: &PreparedMark, mark_index: usize) -> Hsla {
        if prepared.mark.channels.fill.is_some() {
            return prepared.mark.style.fill;
        }
        if has_series(prepared) {
            self.palette_color(mark_index)
        } else {
            self.mark_color
        }
    }

    /// The stroke a mark paints by default, matching its fill.
    pub fn default_stroke(&self, prepared: &PreparedMark, mark_index: usize) -> Hsla {
        if prepared.mark.channels.stroke.is_some() {
            return prepared.mark.style.stroke;
        }
        if has_series(prepared) {
            self.palette_color(mark_index)
        } else {
            self.mark_color
        }
    }

    /// The color a scale gives this row's fill, when one does: the mark's
    /// fill channel, then the plot's continuous color, then its `z` series.
    pub fn scaled_fill(&self, prepared: &PreparedMark, row: &PreparedRow) -> Option<Hsla> {
        if let Some(channel) = &prepared.mark.channels.fill {
            return Some(channel.read(&row.row));
        }
        if let Some((field, scale)) = &self.continuous
            && let Some(color) = row.row.get(field).and_then(|value| scale.color(value))
        {
            return Some(color);
        }
        row.z.as_ref().and_then(|z| self.ordinal.color(z))
    }

    /// The fill one row paints in.
    pub fn row_fill(&self, prepared: &PreparedMark, mark_index: usize, row: &PreparedRow) -> Hsla {
        self.scaled_fill(prepared, row)
            .unwrap_or_else(|| self.default_fill(prepared, mark_index))
    }

    /// The color of one row's series, for a mark that paints per group rather
    /// than per row: a line's stroke follows its stroke channel, else its `z`
    /// value through the shared ordinal scale.
    pub fn series_color(
        &self,
        prepared: &PreparedMark,
        mark_index: usize,
        row: &PreparedRow,
    ) -> Hsla {
        if let Some(channel) = &prepared.mark.channels.stroke {
            return channel.read(&row.row);
        }
        row.z
            .as_ref()
            .and_then(|z| self.ordinal.color(z))
            .unwrap_or_else(|| self.default_stroke(prepared, mark_index))
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
