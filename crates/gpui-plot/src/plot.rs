//! The plot itself: marks, frame, axes, grid, legend, title and the
//! interactive layer.
//!
//! A [`Plot`] is a value built from data and marks. It renders through GPUI
//! Kit's [`PlotElement`], which gives it an element id, appear motion, hover
//! tracking and a tooltip overlay slot. The plot decides what to paint in each
//! of those phases.
//!
//! [`PlotElement`]: gpui_kit::base::plot::PlotElement

mod axes;
mod axis_labels;
mod builder;
mod colors;
mod domain;
mod element;
mod facets;
mod hover;
mod layout;
mod legend;
mod marks;
mod pipeline;
mod scales;
mod title;

pub use scales::Scales;

use std::rc::Rc;

use gpui::{AnyElement, App, ElementId, SharedString, Window};

use crate::facet::Grid;
use crate::frame::{Frame, FrameOptions};
use crate::mark::Mark;
use crate::render::{self, PreparedMark};
use crate::scale::ColorRange;
use crate::table::{Dataset, Table};

/// How a legend lays its entries out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LegendPosition {
    /// Above the plot, where Plot puts its legends.
    Top,
    /// Below the plot.
    Bottom,
    /// Left of the plot.
    Left,
    /// Right of the plot.
    Right,
    /// No legend, Plot's default unless a scale asks for one with
    /// `legend: true`.
    #[default]
    None,
}

/// The plot's title, Plot's `title` option.
#[derive(Clone, Debug, Default)]
pub struct PlotTitle {
    /// The title's text.
    pub text: Option<SharedString>,
    /// A subtitle below the title.
    pub subtitle: Option<SharedString>,
}

impl PlotTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: Some(text.into()),
            subtitle: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }
}

/// The interactive layer's configuration, Plot's `pointer` options.
#[derive(Clone, Debug)]
pub struct Interaction {
    /// Whether the plot tracks the pointer and shows a tooltip.
    pub enabled: bool,
    /// Whether a crosshair marks the hovered datum.
    pub crosshair: bool,
    /// Whether the hovered datum's marks are highlighted.
    pub highlight: bool,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            enabled: true,
            crosshair: true,
            highlight: true,
        }
    }
}

/// How the plot resolves a tooltip's content for a datum.
pub type TooltipFormatter = Rc<dyn Fn(&Table, &mut Window, &mut App) -> AnyElement>;

/// What the transform pipeline resolves, [`Plot::prepare_marks`]: the facet
/// grid, each facet's marks and which facets hold any faceted rows.
pub(crate) type Prepared = (Grid, Vec<Vec<PreparedMark>>, Vec<bool>);

/// A plot: data, marks and the frame around them.
///
/// ```
/// use gpui_plot::{Plot, Table, Value};
///
/// let data = vec![
///     Table::empty().with("x", Value::text("a")).with("y", Value::Number(4.)),
///     Table::empty().with("x", Value::text("b")).with("y", Value::Number(7.)),
/// ];
///
/// let plot = Plot::new(data).dot(|mark| mark.x("x").y("y"));
/// ```
pub struct Plot {
    data: Dataset,
    marks: Vec<Mark>,
    frame_options: FrameOptions,
    title: PlotTitle,
    legend: LegendPosition,
    /// A heading above the legend's entries, Plot's `legend.label`.
    legend_title: Option<SharedString>,
    grid: bool,
    x_axis: bool,
    y_axis: bool,
    /// Plot's per-scale options: `x: {…}`, `color: {…}` and so on.
    scales: Scales,
    /// Plot's `marginTop`, `marginRight`, `marginBottom` and `marginLeft`;
    /// `None` takes the default the axes imply.
    margins: [Option<f32>; 4],
    /// Plot's `axis: null`, which hides every axis.
    no_axes: bool,
    /// Plot's `label: null`, which hides every axis label.
    no_labels: bool,
    /// Plot's `width`: the figure's width, 640 by default; the element
    /// shrinks to it when its container is wider.
    width: Option<f32>,
    /// Plot's `projection`.
    projection: Option<crate::geo::GeoProjection>,
    /// Plot's `aspectRatio`: the plot's height follows from its width so a
    /// unit of x is this many units of y.
    aspect_ratio: Option<f32>,
    /// Plot's `caption`.
    caption: Option<SharedString>,
    /// The x axis label, Plot's `x.label`: `None` derives it from the marks'
    /// x field; an empty label hides it.
    x_label: Option<SharedString>,
    /// The y axis label, Plot's `y.label`.
    y_label: Option<SharedString>,
    interaction: Interaction,
    id: ElementId,
    height: Option<f32>,
    tooltip_formatter: Option<TooltipFormatter>,
    color_range: Option<ColorRange>,
    color_field: Option<SharedString>,
    /// The field that splits the figure into columns, Plot's `fx`.
    fx: Option<SharedString>,
    /// The field that splits the figure into rows, Plot's `fy`.
    fy: Option<SharedString>,
    /// The resolved facet grid, kept from the last frame so the tooltip
    /// phases place a facet the same way the marks did.
    facet_cache: Option<crate::facet::Grid>,
    /// The marks resolved against the data — and dodged against the frame —
    /// kept from `prepaint` so paint and the tooltip phases neither re-run the
    /// transform pipeline nor copy the rows.
    facets_cache: Option<Rc<Vec<Vec<render::PreparedMark>>>>,
    /// The color scales resolved in `prepaint`, the one color authority the
    /// marks, legend and tooltip all read.
    colors_cache: Option<Rc<render::color::PlotColors>>,
    /// The hovered datum's index in the flattened row list, kept between
    /// frames so the hover presentation can fade out over the last datum.
    hover: Option<usize>,
    /// How far the hover has faded in this frame, from `0` to `1`.
    hover_progress: f32,
    /// The frame resolved in `prepaint`, shared with the tooltip phases so
    /// they agree on one layout.
    frame_cache: Option<Frame>,
    /// How far the marks have drawn in this frame, from `0` to `1`.
    appear_progress: f32,
    /// The pipeline's result a retained plot's `natural_height` resolved
    /// before its first frame, which that frame's `prepaint` takes instead
    /// of running the pipeline again (see `RetainedPlot::natural_height`).
    primed: std::cell::RefCell<Option<Prepared>>,
}

/// The plot's state that the trait methods share between phases.
#[cfg(test)]
mod tests;
