//! The plot itself: marks, frame, axes, grid, legend, title and the
//! interactive layer.
//!
//! A [`Plot`] is a value built from data and marks. It renders through GPUI
//! Kit's [`PlotElement`], which gives it an element id, appear motion, hover
//! tracking and a tooltip overlay slot. The plot decides what to paint in each
//! of those phases.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, FontWeight, Hsla, IntoElement, Pixels, Point, SharedString,
    TextAlign, Window, point, px,
};
use gpui_kit::base::plot::{
    Plot as PlotTrait, PlotAppear, PlotAxis, PlotElement, PlotHover, TooltipState, axis_gutter,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::plot::tooltip::{CrossLine, Dot, Tooltip};

use crate::frame::{Frame, FrameOptions, Insets, ScaleKind};
use crate::mark::{Mark, MarkKind};
use crate::paint::{self, Label};
use crate::render::{self, PreparedMark, basic::DrawContext};
use crate::scale::{ColorRange, ColorScale, Transform};
use crate::style::NumberChannel;
use crate::table::{Dataset, Table};
use crate::theme::PlotTheme;
use crate::value::{PlotKind, Value};

/// The height a legend row takes in the frame's margin.
const LEGEND_BAND: f32 = 20.;
/// The narrowest width a side legend reserves; a longer entry widens it.
const LEGEND_SIDE: f32 = 72.;
/// The gap between a legend's swatch and its label.
const LEGEND_SWATCH_GAP: f32 = 14.;
/// The margin between the figure's edge and a legend's first swatch.
const LEGEND_MARGIN: f32 = 8.;
/// The gap between a y-axis label's right edge and the axis line, matching
/// GPUI Base's `TEXT_GAP` for the axis labels it draws.
const AXIS_LABEL_GAP: f32 = 4.;

/// How a legend lays its entries out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LegendPosition {
    /// Above the plot, as Plot's default legend. (Default.)
    #[default]
    Top,
    /// Below the plot.
    Bottom,
    /// Left of the plot.
    Left,
    /// Right of the plot.
    Right,
    /// No legend.
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
    interaction: Interaction,
    id: ElementId,
    height: Option<f32>,
    tooltip_formatter: Option<TooltipFormatter>,
    color_range: Option<ColorRange>,
    color_field: Option<SharedString>,
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
}

impl Plot {
    /// A plot of `data` with no marks yet.
    #[track_caller]
    pub fn new(data: impl IntoIterator<Item = Table>) -> Self {
        Self {
            data: Dataset::new(data),
            marks: Vec::new(),
            frame_options: FrameOptions::default(),
            title: PlotTitle::default(),
            legend: LegendPosition::default(),
            legend_title: None,
            grid: true,
            x_axis: true,
            y_axis: true,
            interaction: Interaction::default(),
            id: ElementId::CodeLocation(*std::panic::Location::caller()),
            height: None,
            tooltip_formatter: None,
            color_range: None,
            color_field: None,
            hover: None,
            hover_progress: 0.,
            frame_cache: None,
            appear_progress: 1.,
        }
    }

    /// A plot over an existing [`Dataset`].
    pub fn dataset(dataset: Dataset) -> Self {
        let mut plot = Self::new(std::iter::empty());
        plot.data = dataset;
        plot
    }

    /// Add a mark from its builder.
    pub fn mark(mut self, mark: impl Into<Mark>) -> Self {
        self.marks.push(mark.into());
        self
    }

    /// Add a dot mark.
    pub fn dot(
        mut self,
        configure: impl FnOnce(crate::mark::DotMark) -> crate::mark::DotMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DotMark::new()).into());
        self
    }

    /// Add a dot mark along x only, Plot's `dotX`.
    pub fn dot_x(
        mut self,
        configure: impl FnOnce(crate::mark::DotXMark) -> crate::mark::DotXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DotXMark::new()).into());
        self
    }

    /// Add a dot mark along y only, Plot's `dotY`.
    pub fn dot_y(
        mut self,
        configure: impl FnOnce(crate::mark::DotYMark) -> crate::mark::DotYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DotYMark::new()).into());
        self
    }

    /// Add a line mark.
    pub fn line(
        mut self,
        configure: impl FnOnce(crate::mark::LineMark) -> crate::mark::LineMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LineMark::new()).into());
        self
    }

    /// Add a line mark along x only, Plot's `lineX`.
    pub fn line_x(
        mut self,
        configure: impl FnOnce(crate::mark::LineXMark) -> crate::mark::LineXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LineXMark::new()).into());
        self
    }

    /// Add a line mark along y only, Plot's `lineY`.
    pub fn line_y(
        mut self,
        configure: impl FnOnce(crate::mark::LineYMark) -> crate::mark::LineYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LineYMark::new()).into());
        self
    }

    /// Add a vertical area mark.
    pub fn area_y(
        mut self,
        configure: impl FnOnce(crate::mark::AreaYMark) -> crate::mark::AreaYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::AreaYMark::new()).into());
        self
    }

    /// Add a horizontal area mark.
    pub fn area_x(
        mut self,
        configure: impl FnOnce(crate::mark::AreaXMark) -> crate::mark::AreaXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::AreaXMark::new()).into());
        self
    }

    /// Add a vertical bar mark.
    pub fn bar_y(
        mut self,
        configure: impl FnOnce(crate::mark::BarYMark) -> crate::mark::BarYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::BarYMark::new()).into());
        self
    }

    /// Add a horizontal bar mark.
    pub fn bar_x(
        mut self,
        configure: impl FnOnce(crate::mark::BarXMark) -> crate::mark::BarXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::BarXMark::new()).into());
        self
    }

    /// Add a rectangle mark.
    pub fn rect(
        mut self,
        configure: impl FnOnce(crate::mark::RectMark) -> crate::mark::RectMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RectMark::new()).into());
        self
    }

    /// Add a rectangle over one x, Plot's `rectX`.
    pub fn rect_x(
        mut self,
        configure: impl FnOnce(crate::mark::RectXMark) -> crate::mark::RectXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RectXMark::new()).into());
        self
    }

    /// Add a rectangle over one y, Plot's `rectY`.
    pub fn rect_y(
        mut self,
        configure: impl FnOnce(crate::mark::RectYMark) -> crate::mark::RectYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RectYMark::new()).into());
        self
    }

    /// Add a cell mark.
    pub fn cell(
        mut self,
        configure: impl FnOnce(crate::mark::CellMark) -> crate::mark::CellMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::CellMark::new()).into());
        self
    }

    /// Add a vertical rule mark.
    pub fn rule_x(
        mut self,
        configure: impl FnOnce(crate::mark::RuleXMark) -> crate::mark::RuleXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RuleXMark::new()).into());
        self
    }

    /// Add a horizontal rule mark.
    /// Add a `tip` mark, Plot's `tip`: the hover tooltip, drawn permanently at
    /// the datum the mark selects.
    pub fn tip(
        mut self,
        configure: impl FnOnce(crate::mark::TipMark) -> crate::mark::TipMark,
    ) -> Self {
        let builder = configure(crate::mark::TipMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add an `axisX` mark, Plot's `axisX`: an x axis drawn as a mark rather
    /// than as a plot-level option, so a chart can put one inside the frame.
    pub fn axis_x(
        mut self,
        configure: impl FnOnce(crate::mark::AxisXMark) -> crate::mark::AxisXMark,
    ) -> Self {
        let builder = configure(crate::mark::AxisXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add an `axisY` mark, Plot's `axisY`.
    pub fn axis_y(
        mut self,
        configure: impl FnOnce(crate::mark::AxisYMark) -> crate::mark::AxisYMark,
    ) -> Self {
        let builder = configure(crate::mark::AxisYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `gridX` mark, Plot's `gridX`: grid lines at each x tick.
    pub fn grid_x(
        mut self,
        configure: impl FnOnce(crate::mark::GridXMark) -> crate::mark::GridXMark,
    ) -> Self {
        let builder = configure(crate::mark::GridXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `gridY` mark, Plot's `gridY`.
    pub fn grid_y(
        mut self,
        configure: impl FnOnce(crate::mark::GridYMark) -> crate::mark::GridYMark,
    ) -> Self {
        let builder = configure(crate::mark::GridYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `frame` mark, Plot's `frame`: the plot's outline, which Plot
    /// draws only when the plot asks for it.
    pub fn frame_mark(
        mut self,
        configure: impl FnOnce(crate::mark::FrameMark) -> crate::mark::FrameMark,
    ) -> Self {
        let builder = configure(crate::mark::FrameMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    pub fn rule_y(
        mut self,
        configure: impl FnOnce(crate::mark::RuleYMark) -> crate::mark::RuleYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RuleYMark::new()).into());
        self
    }

    /// Add a text mark.
    pub fn text(
        mut self,
        configure: impl FnOnce(crate::mark::TextMark) -> crate::mark::TextMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TextMark::new()).into());
        self
    }

    /// Add a vertical tick mark.
    pub fn tick_x(
        mut self,
        configure: impl FnOnce(crate::mark::TickXMark) -> crate::mark::TickXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TickXMark::new()).into());
        self
    }

    /// Add a horizontal tick mark.
    pub fn tick_y(
        mut self,
        configure: impl FnOnce(crate::mark::TickYMark) -> crate::mark::TickYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TickYMark::new()).into());
        self
    }

    /// Add an arrow mark.
    pub fn vector(
        mut self,
        configure: impl FnOnce(crate::mark::VectorMark) -> crate::mark::VectorMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::VectorMark::new()).into());
        self
    }

    /// Add a box plot along x.
    pub fn box_x(
        mut self,
        configure: impl FnOnce(crate::mark::BoxXMark) -> crate::mark::BoxXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::BoxXMark::new()).into());
        self
    }

    /// Add a box plot along y.
    pub fn box_y(
        mut self,
        configure: impl FnOnce(crate::mark::BoxYMark) -> crate::mark::BoxYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::BoxYMark::new()).into());
        self
    }

    /// Add a violin plot along x.
    pub fn violin_x(
        mut self,
        configure: impl FnOnce(crate::mark::ViolinXMark) -> crate::mark::ViolinXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ViolinXMark::new()).into());
        self
    }

    /// Add a violin plot along y.
    pub fn violin_y(
        mut self,
        configure: impl FnOnce(crate::mark::ViolinYMark) -> crate::mark::ViolinYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ViolinYMark::new()).into());
        self
    }

    /// Add a hexagonal binning mark.
    pub fn hexbin(
        mut self,
        configure: impl FnOnce(crate::mark::HexbinMark) -> crate::mark::HexbinMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::HexbinMark::new()).into());
        self
    }

    /// Add a density estimate along x.
    pub fn density_x(
        mut self,
        configure: impl FnOnce(crate::mark::DensityXMark) -> crate::mark::DensityXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DensityXMark::new()).into());
        self
    }

    /// Add a density estimate along y.
    pub fn density_y(
        mut self,
        configure: impl FnOnce(crate::mark::DensityYMark) -> crate::mark::DensityYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DensityYMark::new()).into());
        self
    }

    /// Add a contour mark.
    pub fn contour(
        mut self,
        configure: impl FnOnce(crate::mark::ContourMark) -> crate::mark::ContourMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ContourMark::new()).into());
        self
    }

    /// Add a pie mark.
    pub fn pie(
        mut self,
        configure: impl FnOnce(crate::mark::PieMark) -> crate::mark::PieMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::PieMark::new()).into());
        self
    }

    /// Add an arc mark.
    pub fn arc(
        mut self,
        configure: impl FnOnce(crate::mark::ArcMark) -> crate::mark::ArcMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ArcMark::new()).into());
        self
    }

    /// Add a waffle mark.
    pub fn waffle(
        mut self,
        configure: impl FnOnce(crate::mark::WaffleMark) -> crate::mark::WaffleMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::WaffleMark::new()).into());
        self
    }

    /// Add a link mark.
    pub fn link(
        mut self,
        configure: impl FnOnce(crate::mark::LinkMark) -> crate::mark::LinkMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LinkMark::new()).into());
        self
    }

    /// Name the plot's element id, replacing the default taken from the
    /// construction site. Use one where a single call site renders several
    /// plots as siblings.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Set the plot's title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title.text = Some(title.into());
        self
    }

    /// Set the plot's subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.title.subtitle = Some(subtitle.into());
        self
    }

    /// Place the legend.
    pub fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position;
        self
    }

    /// Give the legend a heading, Plot's `legend.label`.
    pub fn legend_title(mut self, title: impl Into<SharedString>) -> Self {
        self.legend_title = Some(title.into());
        self
    }

    /// Show or hide the grid.
    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }

    /// Show or hide the x axis.
    pub fn x_axis(mut self, show: bool) -> Self {
        self.x_axis = show;
        self
    }

    /// Show or hide the y axis.
    pub fn y_axis(mut self, show: bool) -> Self {
        self.y_axis = show;
        self
    }

    /// Set the plot's height in pixels, Plot's `height`.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Pin the x domain, Plot's `x.domain`.
    pub fn x_domain(mut self, min: f64, max: f64) -> Self {
        self.frame_options.x_domain = Some([min, max]);
        self
    }

    /// Pin the y domain, Plot's `y.domain`.
    pub fn y_domain(mut self, min: f64, max: f64) -> Self {
        self.frame_options.y_domain = Some([min, max]);
        self
    }

    /// Include zero in the x domain.
    pub fn x_zero(mut self, zero: bool) -> Self {
        self.frame_options.x_zero = zero;
        self
    }

    /// Include zero in the y domain, as a bar chart does.
    pub fn y_zero(mut self, zero: bool) -> Self {
        self.frame_options.y_zero = zero;
        self
    }

    /// Widen the x domain to round values, Plot's `x.nice`.
    pub fn x_nice(mut self, nice: bool) -> Self {
        self.frame_options.x_nice = nice;
        self
    }

    /// Widen the y domain to round values, Plot's `y.nice`.
    pub fn y_nice(mut self, nice: bool) -> Self {
        self.frame_options.y_nice = nice;
        self
    }

    /// Clamp x positions outside the domain to the plot's edge, Plot's
    /// `x.clamp`.
    pub fn x_clamp(mut self, clamp: bool) -> Self {
        self.frame_options.x_clamp = clamp;
        self
    }

    /// Clamp y positions outside the domain to the plot's edge.
    pub fn y_clamp(mut self, clamp: bool) -> Self {
        self.frame_options.y_clamp = clamp;
        self
    }

    /// Reverse the x axis, Plot's `x.reverse`.
    pub fn x_reverse(mut self, reverse: bool) -> Self {
        self.frame_options.x_reverse = reverse;
        self
    }

    /// Reverse the y axis, Plot's `y.reverse`.
    pub fn y_reverse(mut self, reverse: bool) -> Self {
        self.frame_options.y_reverse = reverse;
        self
    }

    /// Set the x scale's transform.
    pub fn x_transform(mut self, transform: Transform) -> Self {
        self.frame_options.x_transform = transform;
        self
    }

    /// Set the y scale's transform.
    pub fn y_transform(mut self, transform: Transform) -> Self {
        self.frame_options.y_transform = transform;
        self
    }

    /// Force the x scale kind, Plot's `x: {type: "band"}`.
    pub fn x_scale(mut self, kind: ScaleKind) -> Self {
        self.frame_options.x_kind = Some(kind);
        self
    }

    /// Force the y scale kind.
    pub fn y_scale(mut self, kind: ScaleKind) -> Self {
        self.frame_options.y_kind = Some(kind);
        self
    }

    /// Set the x axis's tick count.
    pub fn x_ticks(mut self, count: usize) -> Self {
        self.frame_options.x_tick_count = count;
        self
    }

    /// Set the y axis's tick count.
    pub fn y_ticks(mut self, count: usize) -> Self {
        self.frame_options.y_tick_count = count;
        self
    }

    /// Set the inset between a band scale and the plot's edge, Plot's `inset`.
    pub fn inset(mut self, inset: f32) -> Self {
        self.frame_options.inset = inset;
        self
    }

    /// Set the inner padding between bands, Plot's `padding`.
    pub fn padding(mut self, padding: f32) -> Self {
        self.frame_options.padding = padding;
        self
    }

    /// Turn the interactive layer on or off.
    pub fn interactive(mut self, enabled: bool) -> Self {
        self.interaction.enabled = enabled;
        self
    }

    /// Draw a custom tooltip body for a datum, replacing the default rows.
    pub fn tooltip(
        mut self,
        formatter: impl Fn(&Table, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.tooltip_formatter = Some(Rc::new(formatter));
        self
    }

    /// Color the marks by a quantitative field, Plot's `fill` with a
    /// continuous range.
    pub fn color(mut self, field: impl Into<SharedString>, range: ColorRange) -> Self {
        self.color_field = Some(field.into());
        self.color_range = Some(range);
        self
    }

    /// The marks, for a caller that wants to inspect them.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// The data.
    pub fn data(&self) -> &Dataset {
        &self.data
    }

    /// The frame options.
    pub fn frame_options(&self) -> &FrameOptions {
        &self.frame_options
    }

    /// The resolved layout: the insets the frame reserves.
    ///
    /// Plot's margins follow the axes and legend it draws, so the plot shrinks
    /// to fit them.
    fn insets(&self, theme: &PlotTheme, window: &mut Window, prepared: &[PreparedMark]) -> Insets {
        let mut insets = Insets {
            top: 8.,
            right: 12.,
            bottom: 8.,
            left: 12.,
        };

        if self.x_axis {
            insets.bottom += axis_gutter(theme.label_size);
        }
        if self.y_axis {
            // The axis line sits at the plot's left edge and its labels
            // right-align against it, one `TEXT_GAP` to its left.
            let widest = self
                .y_tick_labels(prepared)
                .into_iter()
                .map(|label| {
                    paint::measure(&label, theme.label_size, window)
                        .width
                        .as_f32()
                })
                .fold(0.0_f32, f32::max);
            insets.left += widest + AXIS_LABEL_GAP;
        }

        if self.title.text.is_some() {
            insets.top += theme.title_size.as_f32() + 6.;
            if self.title.subtitle.is_some() {
                insets.top += theme.label_size.as_f32() + 4.;
            }
        }

        let legend_entries = self.legend_entries(theme, prepared);
        if self.legend != LegendPosition::None && !legend_entries.is_empty() {
            // A heading above the entries takes one more row on a top or
            // bottom legend, and one more row of height on a side one; a
            // legend that wraps or lists several quantities takes more.
            let band = LEGEND_BAND * (1. + self.legend_rows(theme, window, &legend_entries) as f32)
                + if self.legend_title.is_some() {
                    LEGEND_BAND
                } else {
                    0.
                };
            match self.legend {
                LegendPosition::Top => insets.top += band,
                LegendPosition::Bottom => insets.bottom += band,
                LegendPosition::Left => {
                    insets.left += self.legend_width(theme, window, &legend_entries)
                }
                LegendPosition::Right => {
                    insets.right += self.legend_width(theme, window, &legend_entries)
                }
                LegendPosition::None => {}
            }
        }

        insets
    }

    /// How many rows a top or bottom legend's entries take once they wrap at
    /// the figure's width. A side legend is a column, so it needs one row.
    fn legend_rows(
        &self,
        theme: &PlotTheme,
        window: &mut Window,
        entries: &[LegendEntry],
    ) -> usize {
        if matches!(self.legend, LegendPosition::Left | LegendPosition::Right) {
            return entries.len().saturating_sub(1);
        }
        // The figure's width, which the frame's insets are about to reduce by
        // the width this legend reserves.
        let width = self.legend_band_width() - LEGEND_MARGIN * 2.;
        let mut rows: usize = 1;
        let mut x = 0.;
        let mut previous_quantity = false;
        for entry in entries {
            if entry.swatch.is_quantity() && !previous_quantity {
                rows += 1;
                x = 0.;
            }
            previous_quantity = entry.swatch.is_quantity();
            let label = paint::measure(&entry.label, theme.label_size, window)
                .width
                .as_f32();
            let advance = LEGEND_SWATCH_GAP + label + 14.;
            if x + advance > width && x > 0. {
                rows += 1;
                x = 0.;
            }
            x += advance;
        }
        rows.saturating_sub(1)
    }

    /// The width a top or bottom legend has to lay its entries out in. The
    /// plot does not know the figure's width until it is laid out, so this
    /// uses the same 640px the legend has always assumed.
    fn legend_band_width(&self) -> f32 {
        640.
    }

    /// The width a side legend needs: its widest label plus its swatch.
    ///
    /// A legend stacked vertically on the left or right of the plot is as wide
    /// as its longest entry; reserving a fixed width would clip the labels.
    fn legend_width(&self, theme: &PlotTheme, window: &mut Window, entries: &[LegendEntry]) -> f32 {
        let widest = entries
            .iter()
            .map(|entry| {
                paint::measure(&entry.label, theme.label_size, window)
                    .width
                    .as_f32()
            })
            .fold(0.0_f32, f32::max);
        (LEGEND_SWATCH_GAP + widest + LEGEND_MARGIN * 2.).max(LEGEND_SIDE)
    }

    /// Where the top legend's row starts, below the title the frame reserves.
    fn legend_top(&self, theme: &PlotTheme) -> f32 {
        let mut y = LEGEND_MARGIN;
        if self.title.text.is_some() {
            y += theme.title_size.as_f32() + 6.;
            if self.title.subtitle.is_some() {
                y += theme.label_size.as_f32() + 4.;
            }
        }
        y
    }

    /// The labels the y axis will draw, for measuring its gutter.
    fn y_tick_labels(&self, prepared: &[PreparedMark]) -> Vec<SharedString> {
        let values = self.y_domain_values(prepared);
        let kind = values
            .iter()
            .find(|value| !value.is_null())
            .map(Value::kind)
            .unwrap_or(PlotKind::Null);
        let frame = Frame::resolve(
            Bounds::new(point(px(0.), px(0.)), gpui::size(px(400.), px(200.))),
            &FrameOptions {
                insets: Insets::default(),
                ..self.frame_options.clone()
            },
            &self.x_domain_values(prepared),
            &values,
        );
        let _ = kind;
        frame
            .y
            .ticks
            .iter()
            .map(|tick| tick.label.clone())
            .collect()
    }

    fn x_domain_values(&self, prepared: &[PreparedMark]) -> Vec<Value> {
        prepared.iter().flat_map(render::basic::x_values).collect()
    }

    fn y_domain_values(&self, prepared: &[PreparedMark]) -> Vec<Value> {
        prepared.iter().flat_map(render::basic::y_values).collect()
    }

    /// The plot's ordinal color scale: the distinct `z` values across every
    /// mark, colored from the theme's palette.
    fn ordinal_scale(&self, theme: &PlotTheme, prepared: &[PreparedMark]) -> ColorScale {
        ColorScale::ordinal(
            crate::scale::dedup(
                prepared
                    .iter()
                    .filter(|mark| mark.mark.channels.fill.is_none())
                    .flat_map(|mark| mark.rows.iter())
                    .filter_map(|row| row.z.as_ref().map(|z| z.key())),
            ),
            theme.palette.clone(),
        )
    }

    /// The legend's entries: one per series or one per categorical value.
    fn legend_entries(&self, theme: &PlotTheme, prepared: &[PreparedMark]) -> Vec<LegendEntry> {
        let mut entries: Vec<LegendEntry> = Vec::new();
        // The color scale spans every mark's `z` values, the way Plot's is one
        // scale for the whole plot: a chart drawn as one line per series has a
        // `z` value in each mark, and each mark's own domain would give every
        // line the first palette color.
        let ordinal = self.ordinal_scale(theme, prepared);
        for (index, mark) in prepared.iter().enumerate() {
            let mut series: Vec<(SharedString, Hsla)> = Vec::new();
            let mark_ordinal = mark.mark.channels.fill.is_none().then(|| ordinal.clone());
            for row in mark.rows.iter() {
                if let Some(z) = &row.z {
                    let color = match (&mark.mark.channels.fill, &mark_ordinal) {
                        (Some(channel), _) => channel.read(&row.row),
                        (None, Some(scale)) => {
                            scale.color(z).unwrap_or_else(|| theme.palette_color(index))
                        }
                        (None, None) => theme.palette_color(index),
                    };
                    if !series.iter().any(|(name, _)| *name == z.label()) {
                        series.push((z.label(), color));
                    }
                }
            }
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
                // A titled mark's swatch is the color it draws in, which for a
                // line is its stroke: a mark with only a stroke channel is
                // otherwise listed in a palette color it never paints.
                let color = mark
                    .mark
                    .channels
                    .fill
                    .as_ref()
                    .or(mark.mark.channels.stroke.as_ref())
                    .and_then(|channel| mark.rows.first().map(|row| channel.read(&row.row)))
                    .unwrap_or_else(|| theme.palette_color(index));
                entries.push(LegendEntry {
                    label: title.clone(),
                    swatch: LegendSwatch::Color(color),
                    shape,
                });
            }
            // A `r` or `opacity` accessor is a third dimension the color
            // legend cannot show, so it gets a legend of its own: the smallest
            // and largest value, and the middle one when the range is wide.
            if let Some(channel) = &mark.mark.channels.r {
                let values = numbers(mark, channel);
                entries.extend(quantity_entries("r", &values, |value| {
                    LegendSwatch::Size(value.max(1.))
                }));
            }
            if let Some(channel) = &mark.mark.channels.opacity {
                let values = numbers(mark, channel);
                entries.extend(quantity_entries("opacity", &values, |value| {
                    LegendSwatch::Opacity(value.clamp(0., 1.))
                }));
            }
        }
        entries
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

/// Every `(mark, row)` a pointer can land on, in the order `tooltip_state`
/// searches them. Both `tooltip_state` and `tooltip` build this list, so the
/// state's index means the same thing to each.
fn hover_targets(prepared: &[PreparedMark]) -> Vec<(usize, usize)> {
    prepared
        .iter()
        .enumerate()
        .flat_map(|(mark_index, mark)| {
            (0..mark.rows.len()).map(move |row_index| (mark_index, row_index))
        })
        .collect()
}

/// One legend entry.
#[derive(Clone, Debug)]
struct LegendEntry {
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
}

impl LegendSwatch {
    /// The color a swatch draws, for the swatches that are colors. A size or
    /// opacity swatch draws in the theme's label color instead.
    fn color(&self) -> Option<Hsla> {
        match self {
            LegendSwatch::Color(color) => Some(*color),
            LegendSwatch::Size(_) | LegendSwatch::Opacity(_) => None,
        }
    }

    /// Whether the swatch stands for a size or an opacity rather than a
    /// color, which decides where its list of entries starts.
    fn is_quantity(&self) -> bool {
        !matches!(self, LegendSwatch::Color(_))
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

impl Plot {
    /// Resolve the frame for `bounds`, including the margins the axes and
    /// title need.
    fn resolve_frame(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        prepared: &[PreparedMark],
    ) -> Frame {
        let mut options = self.frame_options.clone();
        options.insets = self.insets(theme, window, prepared);
        // A mark that draws a band on an axis — a bar's width, a cell's
        // extent — needs a band scale there, not a point scale: a point
        // scale puts the first and last category on the plot's edges, and
        // their bands would hang off the plot and be clipped in half. Plot
        // infers `band` for barX/barY/cell and `point` for everything else.
        if options.x_kind.is_none() && marks_use_band(prepared, true) {
            options.x_kind = Some(ScaleKind::Band);
        }
        if options.y_kind.is_none() && marks_use_band(prepared, false) {
            options.y_kind = Some(ScaleKind::Band);
        }
        // A plot may ask for its own height, Plot's `height`. The element fills
        // its container, so the request shrinks the figure to that height
        // rather than growing it: a barcode or a swarm wants a short frame.
        let bounds = match self.height {
            Some(height) if height > 0. => Bounds {
                size: gpui::size(bounds.size.width, px(height)),
                ..bounds
            },
            _ => bounds,
        };
        Frame::resolve(
            bounds,
            &options,
            &self.x_domain_values(prepared),
            &self.y_domain_values(prepared),
        )
    }
}

/// Whether any mark draws a band along the x axis (`along_x`) or the y axis.
///
/// Only matters for a discrete axis: a continuous scale has no bands.
fn marks_use_band(prepared: &[PreparedMark], along_x: bool) -> bool {
    prepared.iter().any(|prepared| {
        let discrete = if along_x {
            prepared
                .rows
                .iter()
                .find(|row| !row.x_value().is_null())
                .is_some_and(|row| !row.x_value().kind().is_quantitative())
        } else {
            prepared
                .rows
                .iter()
                .find(|row| !row.y_value().is_null())
                .is_some_and(|row| !row.y_value().kind().is_quantitative())
        };
        if !discrete {
            return false;
        }
        match prepared.mark.kind {
            // A bar's width and a cell's extent fill a band.
            MarkKind::BarY | MarkKind::Cell if along_x => true,
            MarkKind::BarX | MarkKind::Cell if !along_x => true,
            // A band `y` is the ordinal secondary axis of tickX, and the
            // category axis of the box and violin marks.
            MarkKind::TickX | MarkKind::BoxX | MarkKind::ViolinX | MarkKind::DensityX
                if !along_x =>
            {
                true
            }
            MarkKind::TickY | MarkKind::BoxY | MarkKind::ViolinY | MarkKind::DensityY
                if along_x =>
            {
                true
            }
            _ => false,
        }
    })
}

impl PlotTrait for Plot {
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        // Resolve the frame here so `paint` and `tooltip_state` share one
        // layout, and measure the labels that decide the margins.
        let theme = PlotTheme::from_theme(cx.theme());
        let prepared = render::prepare(&self.data, &self.marks);
        let frame = self.resolve_frame(bounds, &theme, window, &prepared);
        self.frame_cache = Some(frame);
        vec![]
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let theme = PlotTheme::from_theme(cx.theme());

        let mut prepared = render::prepare(&self.data, &self.marks);
        let frame = self
            .frame_cache
            .clone()
            .unwrap_or_else(|| self.resolve_frame(bounds, &theme, window, &prepared));
        // A dodge works in pixels, so it needs the frame resolved first.
        render::apply_dodge(&mut prepared, &frame);

        let appear = self.appear_progress;

        // The title sits above the plot, inside the figure's top margin.
        self.paint_title(bounds, &theme, window, cx);
        // The legend sits in the margin its position reserves.
        self.paint_legend(bounds, &theme, window, cx, &prepared);
        // The grid goes under the marks.
        if self.grid {
            paint_grid(&frame, &theme, window);
        }
        // The marks. Every mark shares one ordinal color scale, so a series
        // keeps its color across marks.
        let ordinal = self.ordinal_scale(&theme, &prepared);
        let mut row_offset = 0;
        for (index, mark) in prepared.iter().enumerate() {
            let mut ctx = DrawContext {
                frame: &frame,
                theme: &theme,
                mark_index: index,
                appear,
                hover_progress: self.hover_progress,
                hovered: self.hover,
                row_offset,
                ordinal: &ordinal,
                window,
                cx,
            };
            row_offset += mark.rows.len();
            match mark.mark.kind {
                MarkKind::Pie
                | MarkKind::Arc
                | MarkKind::Waffle
                | MarkKind::BoxX
                | MarkKind::BoxY
                | MarkKind::ViolinX
                | MarkKind::ViolinY
                | MarkKind::DensityX
                | MarkKind::DensityY
                | MarkKind::Hexbin
                | MarkKind::Contour => render::radial::draw(mark, &mut ctx),
                _ => render::basic::draw(mark, &mut ctx),
            }
        }
        // The axes sit on top of the marks' clipping edge.
        paint_axes(&frame, &theme, window, cx, self.x_axis, self.y_axis);
    }

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn interactive(&self) -> bool {
        self.interaction.enabled
    }

    fn appear(&mut self, appear: PlotAppear, _window: &mut Window, _cx: &mut App) {
        self.appear_progress = appear.progress();
    }

    /// A fixed generation: a plot draws its data in once, the first time it
    /// paints, and never again.
    ///
    /// The generation is the key the appear's element state is kept under, so
    /// a constant one means the motion runs on the plot's first frame and the
    /// state remembers it as finished. A generation that changed with the data
    /// would replay the draw-in whenever the caller rebuilt the plot — on
    /// every hover, every resize — and the marks would flicker away and back.
    fn appear_generation(&self) -> Option<u64> {
        Some(0)
    }

    fn hover(&mut self, hover: Option<&PlotHover>, _window: &mut Window, _cx: &mut App) {
        // The state's index addresses the flattened row list, which both
        // `tooltip_state` and `tooltip` build the same way.
        self.hover = hover.map(|hover| hover.state().index);
        self.hover_progress = hover.map_or(0., |hover| hover.progress());
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _cx: &App,
    ) -> Option<TooltipState> {
        if !self.interaction.enabled {
            return None;
        }
        let frame = self.frame_cache.clone()?;
        let mut prepared = render::prepare(&self.data, &self.marks);
        render::apply_dodge(&mut prepared, &frame);

        // Ignore the axis gutters: hovering a label shows nothing.
        let absolute = position + bounds.origin;
        if !frame.plot.contains(&absolute) {
            return None;
        }
        let local = absolute - frame.plot.origin;

        // The nearest datum across every mark, measured by distance to its
        // projected position, which is Plot's `nearest` pointer. The state's
        // index addresses the flattened row list, so it identifies the mark
        // and row together.
        let mut best: Option<(usize, Point<Pixels>, f32)> = None;
        for (flat, (mark_index, row_index)) in hover_targets(&prepared).into_iter().enumerate() {
            let mark = &prepared[mark_index];
            let Some(row) = mark.rows.get(row_index) else {
                continue;
            };
            let Some(point) = frame.point(row.x_value(), row.y_value()) else {
                continue;
            };
            let dx = point.x.as_f32() - local.x.as_f32();
            let dy = point.y.as_f32() - local.y.as_f32();
            let distance = dx * dx + dy * dy;
            if best.as_ref().is_none_or(|(_, _, best)| distance < *best) {
                best = Some((flat, point, distance));
            }
        }
        let (flat, hit, _) = best?;
        let (mark_index, row_index) = hover_targets(&prepared)[flat];
        let mark = &prepared[mark_index];
        let row = mark.rows.get(row_index)?;

        let dots: Vec<Point<Pixels>> = mark
            .rows
            .iter()
            .filter_map(|other| {
                if other.z.as_ref().map(|z| z.key()) == row.z.as_ref().map(|z| z.key()) {
                    frame.point(other.x_value(), other.y_value())
                } else {
                    None
                }
            })
            .collect();
        let dots = if dots.is_empty() { vec![hit] } else { dots };

        Some(TooltipState::new(flat, gpui::point(hit.x, px(0.)), dots))
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let theme = PlotTheme::from_theme(cx.theme());
        let prepared = render::prepare(&self.data, &self.marks);

        // The hover carries the index the pointer resolved to, in the same
        // flattened row list `tooltip_state` searched.
        let flat = self.hover?;
        let (mark_index, row_index) = *hover_targets(&prepared).get(flat)?;
        let mark = prepared.get(mark_index)?;
        let row = mark.rows.get(row_index)?;

        // A custom formatter replaces the default rows entirely.
        if let Some(formatter) = &self.tooltip_formatter {
            let element = formatter(&row.row, window, cx);
            return Some(element);
        }

        let color = self.color_for_row(mark_index, mark, row, &theme);
        let title = self
            .marks
            .get(mark_index)
            .and_then(|mark| mark.channels.title.as_ref())
            .map(|channel| channel.read(&row.row).label())
            .unwrap_or_else(|| row.x_value().label());

        let mut tooltip = Tooltip::new(cursor, bounds.size)
            .gap(px(8.))
            .title(title)
            .row(
                color,
                mark.mark
                    .title
                    .clone()
                    .unwrap_or_else(|| mark.mark.kind.name().into()),
                row.y_value().label(),
            );

        // Add the channels a tooltip conventionally reads: the series and any
        // other fields the mark named.
        if let Some(z) = &row.z {
            tooltip = tooltip.plain_row("series", z.label());
        }
        for name in ["group", "name", "label"] {
            if let Some(value) = row.row.get(name)
                && !value.is_null()
            {
                tooltip = tooltip.plain_row(name, value.label());
            }
        }

        if self.interaction.crosshair {
            let height = bounds.size.height.as_f32();
            tooltip =
                tooltip.cross_line(CrossLine::new(state.cross_line).height(height).band(px(6.)));
        }
        if self.interaction.highlight {
            let dots: Vec<Dot> = state
                .dots
                .iter()
                .map(|dot| {
                    Dot::new(*dot)
                        .size(px(8.))
                        .halo(px(14.))
                        .fill(color)
                        .stroke(theme.background)
                })
                .collect();
            tooltip = tooltip.dots(dots);
        }

        Some(tooltip.into_any_element())
    }
}

impl Plot {
    fn color_for_row(
        &self,
        mark_index: usize,
        mark: &PreparedMark,
        row: &crate::render::PreparedRow,
        theme: &PlotTheme,
    ) -> Hsla {
        if let Some(channel) = &mark.mark.channels.fill {
            return channel.read(&row.row);
        }
        // A `color` option over a continuous field wins over the palette.
        if let (Some(field), Some(range)) = (&self.color_field, &self.color_range) {
            let values: Vec<Value> = mark.rows.iter().map(|row| row.row.value(field)).collect();
            if let Some(extent) = crate::scale::extent(values) {
                let scale = ColorScale::continuous(extent, range.clone());
                if let Some(color) = row.row.get(field).and_then(|value| scale.color(value)) {
                    return color;
                }
            }
        }
        if row.z.is_some() {
            return theme.palette_color(mark_index);
        }
        theme.palette_color(mark_index)
    }

    fn paint_title(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(title) = &self.title.text else {
            return;
        };
        let font = gpui::font(theme.font_family.clone());
        let label = Label::new(
            title.clone(),
            theme.title_size,
            theme.title,
            FontWeight::SEMIBOLD,
            font.clone(),
            window,
        );
        label.paint_at(
            bounds.origin + point(px(12.), px(6.)),
            TextAlign::Left,
            None,
            window,
            cx,
        );
        if let Some(subtitle) = &self.title.subtitle {
            let sub = Label::new(
                subtitle.clone(),
                theme.label_size,
                theme.label,
                FontWeight::NORMAL,
                font,
                window,
            );
            sub.paint_at(
                bounds.origin + point(px(12.), px(6. + theme.title_size.as_f32() + 4.)),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }

    fn paint_legend(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
        prepared: &[PreparedMark],
    ) {
        let entries = self.legend_entries(theme, prepared);
        if self.legend == LegendPosition::None || entries.is_empty() {
            return;
        }
        let font = gpui::font(theme.font_family.clone());
        let swatch = px(8.);
        // The gap between one entry's label and the next entry's swatch.
        let entry_gap = 14.;
        // A side legend is a column, so its entries stack downwards.
        let vertical = matches!(self.legend, LegendPosition::Left | LegendPosition::Right);
        let side_width = if vertical {
            self.legend_width(theme, window, &entries)
        } else {
            0.
        };

        // A top legend sits under the title, inside the band the frame
        // reserved for it; a bottom one above the axis gutter.
        let (mut x, mut y) = match self.legend {
            LegendPosition::Top => (LEGEND_MARGIN, self.legend_top(theme)),
            LegendPosition::Bottom => (
                LEGEND_MARGIN,
                bounds.size.height.as_f32() - LEGEND_BAND + LEGEND_MARGIN,
            ),
            LegendPosition::Left => (LEGEND_MARGIN, self.legend_top(theme)),
            LegendPosition::Right => (
                bounds.size.width.as_f32() - side_width + LEGEND_MARGIN,
                self.legend_top(theme),
            ),
            LegendPosition::None => return,
        };
        let origin = bounds.origin;

        // A legend heading sits on its own row above the entries, so a long
        // row of entries does not push it off the figure's edge.
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
                point(origin.x + px(x), origin.y + px(y)),
                TextAlign::Left,
                None,
                window,
                cx,
            );
            y += LEGEND_BAND;
            x = LEGEND_MARGIN;
        }

        for (index, entry) in entries.iter().enumerate() {
            // A size or opacity legend is a separate list from the color one,
            // so it starts on a fresh row rather than running on from it.
            if !vertical
                && index > 0
                && entry.swatch.is_quantity()
                && !entries[index - 1].swatch.is_quantity()
            {
                x = LEGEND_MARGIN;
                y += LEGEND_BAND;
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
            let width = label.width.as_f32();
            label.paint_at(
                point(origin.x + px(x + LEGEND_SWATCH_GAP), origin.y + px(y)),
                TextAlign::Left,
                None,
                window,
                cx,
            );
            if vertical {
                y += LEGEND_BAND;
            } else {
                x += LEGEND_SWATCH_GAP + width + entry_gap;
                // A row that would run past the figure wraps to the next one.
                if x > bounds.size.width.as_f32() - LEGEND_MARGIN - width {
                    x = LEGEND_MARGIN;
                    y += LEGEND_BAND;
                }
            }
        }
    }
}

/// Paint the grid lines behind the marks.
fn paint_grid(frame: &Frame, theme: &PlotTheme, window: &mut Window) {
    let plot = frame.plot;
    let width = theme.line_width;
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
    // Vertical grid lines only for a continuous x scale: a band scale's
    // categories are already clear from their labels.
    if !frame.x.scale.is_discrete() {
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

/// Paint the axis lines and their tick labels.
fn paint_axes(
    frame: &Frame,
    theme: &PlotTheme,
    window: &mut Window,
    cx: &mut App,
    x_axis: bool,
    y_axis: bool,
) {
    let plot = frame.plot;
    let mut axis = PlotAxis::new().stroke(theme.axis);
    let font = gpui::font(theme.font_family.clone());

    if x_axis {
        let labels: Vec<gpui_kit::base::plot::AxisText> = frame
            .x
            .ticks
            .iter()
            .map(|tick| {
                gpui_kit::base::plot::AxisText::new(
                    tick.label.clone(),
                    px(tick.position),
                    theme.label,
                )
                .font_size(theme.label_size)
                .align(TextAlign::Center)
            })
            .collect();
        axis = axis.x(plot.size.height).x_axis(false).x_label(labels);
    }
    if y_axis {
        let labels: Vec<gpui_kit::base::plot::AxisText> = frame
            .y
            .ticks
            .iter()
            .map(|tick| {
                // `PlotAxis` centers a y label by subtracting `TEXT_SIZE / 2`
                // from its tick, which assumes the default 10px labels. A plot
                // with a larger font would sit its labels high, so the tick is
                // nudged by the difference.
                let shift =
                    (theme.label_size.as_f32() - gpui_kit::base::plot::label::TEXT_SIZE) / 2.;
                gpui_kit::base::plot::AxisText::new(
                    tick.label.clone(),
                    px(tick.position - shift),
                    theme.label,
                )
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
            .y_label_side(gpui_kit::base::plot::AxisLabelSide::Start)
            .y_label(labels);
    }
    axis.paint(&plot, window, cx);
    if y_axis {
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
    let _ = font;
}

impl IntoElement for Plot {
    type Element = PlotElement<Self>;

    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}

/// The plot's state that the trait methods share between phases.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tip_and_furniture_marks_prepare() {
        let data = vec![
            Table::empty()
                .with("x", Value::Number(1.))
                .with("y", Value::Number(2.)),
            Table::empty()
                .with("x", Value::Number(2.))
                .with("y", Value::Number(5.)),
        ];
        let plot = Plot::new(data)
            .tip(|mark| mark.x("x").y("y").text("hi"))
            .grid_x(|mark| mark)
            .frame_mark(|mark| mark);
        let prepared = render::prepare(plot.data(), plot.marks());
        // The tip reads the data; the furniture marks draw no rows.
        assert_eq!(prepared[0].rows.len(), 2);
        assert!(prepared[1].rows.is_empty());
        assert!(prepared[2].rows.is_empty());
        assert!(prepared[1].mark.kind.is_furniture());
    }

    fn sample() -> Vec<Table> {
        [("a", 4.), ("b", 7.), ("c", 2.)]
            .into_iter()
            .map(|(x, y)| {
                Table::empty()
                    .with("x", Value::text(x))
                    .with("y", Value::Number(y))
            })
            .collect()
    }

    #[test]
    fn plot_builds_marks() {
        let plot = Plot::new(sample())
            .dot(|mark| mark.x("x").y("y"))
            .line(|mark| mark.x("x").y("y").stroke_width(2.));
        assert_eq!(plot.marks().len(), 2);
        assert_eq!(plot.marks()[0].kind, MarkKind::Dot);
        assert_eq!(plot.marks()[1].style.stroke_width, 2.);
    }

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
        let plot = Plot::new(data).line(|mark| mark.x("x").y("y").z("z"));
        let prepared = render::prepare(plot.data(), plot.marks());
        let theme = PlotTheme::from_theme(&gpui_kit::component::Theme::default());
        let entries = plot.legend_entries(&theme, &prepared);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "one");
    }
}
