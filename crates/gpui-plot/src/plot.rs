//! The plot itself: marks, frame, axes, grid, legend, title and the
//! interactive layer.
//!
//! A [`Plot`] is a value built from data and marks. It renders through GPUI
//! Kit's [`PlotElement`], which gives it an element id, appear motion, hover
//! tracking and a tooltip overlay slot. The plot decides what to paint in each
//! of those phases.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, FontWeight, Hsla, IntoElement, ParentElement, Pixels,
    Point, SharedString, TextAlign, Window, point, px,
};
use gpui_kit::base::plot::{Plot as PlotTrait, PlotAppear, PlotElement, PlotHover, TooltipState};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::plot::tooltip::{CrossLine, Dot, Tooltip};

use crate::facet::Grid;
use crate::frame::{Frame, FrameOptions, Insets, ScaleKind};
use crate::mark::{Mark, MarkKind};
use crate::paint::{self, Label};
use crate::render::{self, PreparedMark, basic::DrawContext};
use crate::scale::{ColorRange, Transform};
use crate::style::NumberChannel;
use crate::table::{Dataset, Table};
use crate::theme::PlotTheme;
use crate::value::Value;

/// The height a legend row takes in the frame's margin.
const LEGEND_BAND: f32 = 20.;
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
/// The gap between a y-axis label's right edge and the axis line, matching
/// GPUI Base's `TEXT_GAP` for the axis labels it draws.
const AXIS_LABEL_GAP: f32 = 4.;

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
}

/// Every scale's options, Plot's `x`, `y`, `fx`, `fy`, `color`, `r`,
/// `opacity`, `symbol` and `length` objects.
#[derive(Clone, Debug, Default)]
pub struct Scales {
    pub x: crate::options::ScaleOptions,
    pub y: crate::options::ScaleOptions,
    pub fx: crate::options::ScaleOptions,
    pub fy: crate::options::ScaleOptions,
    pub color: crate::options::ScaleOptions,
    pub r: crate::options::ScaleOptions,
    pub opacity: crate::options::ScaleOptions,
    pub symbol: crate::options::ScaleOptions,
    pub length: crate::options::ScaleOptions,
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
            // Plot draws no grid unless asked.
            grid: false,
            x_axis: true,
            y_axis: true,
            scales: Scales::default(),
            margins: [None; 4],
            no_axes: false,
            no_labels: false,
            aspect_ratio: None,
            caption: None,
            x_label: None,
            y_label: None,
            interaction: Interaction::default(),
            id: ElementId::CodeLocation(*std::panic::Location::caller()),
            height: None,
            tooltip_formatter: None,
            color_range: None,
            color_field: None,
            fx: None,
            fy: None,
            facet_cache: None,
            facets_cache: None,
            colors_cache: None,
            hover: None,
            hover_progress: 0.,
            frame_cache: None,
            appear_progress: 1.,
        }
    }

    /// An empty plot whose marks bring their own data, Plot's
    /// `Plot.plot({marks: […]})`.
    #[track_caller]
    #[allow(clippy::self_named_constructors)]
    pub fn plot() -> Self {
        Self::new(std::iter::empty())
    }

    /// Plot's `x: {…}` scale options.
    pub fn x(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.x = f(std::mem::take(&mut self.scales.x));
        self
    }

    /// Plot's `y: {…}` scale options.
    pub fn y(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.y = f(std::mem::take(&mut self.scales.y));
        self
    }

    /// Plot's `fx: {…}` scale options.
    pub fn fx_scale(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.fx = f(std::mem::take(&mut self.scales.fx));
        self
    }

    /// Plot's `fy: {…}` scale options.
    pub fn fy_scale(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.fy = f(std::mem::take(&mut self.scales.fy));
        self
    }

    /// Plot's `color: {…}` scale options.
    pub fn color(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.color = f(std::mem::take(&mut self.scales.color));
        self
    }

    /// Plot's `r: {…}` scale options.
    pub fn r(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.r = f(std::mem::take(&mut self.scales.r));
        self
    }

    /// Plot's `opacity: {…}` scale options.
    pub fn opacity(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.opacity = f(std::mem::take(&mut self.scales.opacity));
        self
    }

    /// Plot's `symbol: {…}` scale options.
    pub fn symbol(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.symbol = f(std::mem::take(&mut self.scales.symbol));
        self
    }

    /// Plot's `length: {…}` scale options.
    pub fn length(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.length = f(std::mem::take(&mut self.scales.length));
        self
    }

    /// The scale options set so far.
    pub fn scales(&self) -> &Scales {
        &self.scales
    }

    /// Plot's `margin`: every side.
    pub fn margin(mut self, margin: f32) -> Self {
        self.margins = [Some(margin); 4];
        self
    }

    /// Plot's `marginTop`.
    pub fn margin_top(mut self, margin: f32) -> Self {
        self.margins[0] = Some(margin);
        self
    }

    /// Plot's `marginRight`.
    pub fn margin_right(mut self, margin: f32) -> Self {
        self.margins[1] = Some(margin);
        self
    }

    /// Plot's `marginBottom`.
    pub fn margin_bottom(mut self, margin: f32) -> Self {
        self.margins[2] = Some(margin);
        self
    }

    /// Plot's `marginLeft`.
    pub fn margin_left(mut self, margin: f32) -> Self {
        self.margins[3] = Some(margin);
        self
    }

    /// Plot's `axis: null`: no axes at all.
    pub fn no_axes(mut self) -> Self {
        self.no_axes = true;
        self
    }

    /// Plot's `aspectRatio`.
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }

    /// Plot's `caption`, below the plot.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// The height the figure takes at `width`: the chart's height (its
    /// [`Plot::height`], or Plot's `autoHeight` — 400-odd pixels for a
    /// continuous y, twenty per value for an ordinal one, sixty for a plot
    /// with no y at all, or what `aspectRatio` asks for) plus the title,
    /// legend and caption above and below it.
    pub fn natural_height(&self, width: f32) -> f32 {
        let grid = self.facet_grid();
        let (facets, _) = render::prepare_facets(
            &self.data,
            &self.marks,
            &grid,
            self.fx.as_deref(),
            self.fy.as_deref(),
        );
        let [mt, mr, mb, ml] = self.plot_margins(&grid);
        let y_values = self.y_domain_values(&facets);
        let x_values = self.x_domain_values(&facets);
        let nfy = grid.fy.as_ref().map_or(1, |d| d.len().max(1)) as f32;
        let nfx = grid.fx.as_ref().map_or(1, |d| d.len().max(1)) as f32;
        let ordinal = |values: &[Value], options: &crate::options::ScaleOptions| {
            options.r#type.is_some_and(|t| t.is_discrete())
                || (options.r#type.is_none()
                    && values
                        .iter()
                        .find(|v| !v.is_null())
                        .is_some_and(|v| !v.kind().is_quantitative()))
        };
        // Plot's autoHeight counts a y scale whether or not its axis draws.
        let has_y = !y_values.is_empty() || grid.fy.is_some();
        let chart = match (self.height, self.aspect_ratio) {
            (Some(height), _) => height,
            (None, Some(ratio)) => {
                let length = |values: &[Value], options: &crate::options::ScaleOptions| -> f32 {
                    if ordinal(values, options) {
                        let n = crate::scale::dedup(values.iter().map(Value::key))
                            .len()
                            .max(1);
                        return n as f32;
                    }
                    let [lo, hi] = options
                        .numeric_domain()
                        .or_else(|| crate::scale::extent(values.iter().cloned()))
                        .unwrap_or([0., 1.]);
                    let t = |v: f64| match options.r#type {
                        Some(crate::options::ScaleType::Log) => v.max(f64::MIN_POSITIVE).log10(),
                        Some(crate::options::ScaleType::Sqrt) => v.sqrt(),
                        _ => v,
                    };
                    (t(hi) - t(lo)).abs().max(f64::MIN_POSITIVE) as f32
                };
                let ratio =
                    length(&y_values, &self.scales.y) / (length(&x_values, &self.scales.x) * ratio);
                let w = (width - ml - mr) / (1.1 * nfx - 0.1).max(1.);
                ratio * w * (1.1 * nfy - 0.1).max(1.) + mt + mb
            }
            (None, None) => {
                let ny = if !has_y {
                    1.
                } else if ordinal(&y_values, &self.scales.y) {
                    crate::scale::dedup(y_values.iter().map(Value::key))
                        .len()
                        .max(1) as f32
                } else {
                    (17. / nfy).max(7.)
                };
                let base = if has_y {
                    (ny * nfy).clamp(1., 60.) * 20.
                } else {
                    0.
                };
                base + if grid.fx.is_some() { 30. } else { 0. } + 60.
            }
        };
        // The figure's extras: the title, legend and caption bands.
        let mut extras = 0.;
        if self.title.text.is_some() {
            extras += 13. + 6.;
            if self.title.subtitle.is_some() {
                extras += 14.;
            }
        }
        if self.legend_position() != LegendPosition::None {
            extras += if self.color_is_continuous(&facets) {
                50.
            } else {
                LEGEND_BAND + 13.
            };
        }
        if self.caption.is_some() {
            extras += 18.;
        }
        chart + extras
    }

    /// Whether the plot's color scale is a continuous ramp.
    fn color_is_continuous(&self, facets: &[Vec<PreparedMark>]) -> bool {
        let theme = PlotTheme {
            palette: ColorRange::observable10_colors(),
            ..PlotTheme::from_theme(&gpui_kit::component::Theme::default())
        };
        self.plot_colors(&theme, facets)
            .scale
            .is_some_and(|scale| !scale.is_ordinal())
    }

    /// The height the plot asked for with [`Plot::height`], if any.
    pub fn requested_height(&self) -> Option<f32> {
        self.height
    }

    /// A plot over an existing [`Dataset`].
    pub fn dataset(dataset: Dataset) -> Self {
        let mut plot = Self::new(std::iter::empty());
        plot.data = dataset;
        plot
    }

    /// A plot whose mark is chosen from the data's shape, Plot's `auto`.
    ///
    /// `x`/`y` name the fields to plot, and `color` optionally colors the mark.
    /// Two quantitative fields scatter (or line when x is monotonic), one
    /// quantitative field histograms, and an ordinal field bars against a
    /// count.
    pub fn auto(
        data: impl IntoIterator<Item = Table>,
        x: Option<&str>,
        y: Option<&str>,
        color: Option<&str>,
    ) -> Self {
        let rows: Vec<Table> = data.into_iter().collect();
        crate::auto::build(&rows, x, y, color)
    }

    /// A barcode: one quantitative field laid as a strip of dots along x,
    /// nothing binned or reduced — Plot's `auto` with a `reduce: null` y.
    pub fn barcode(data: impl IntoIterator<Item = Table>, field: &str) -> Self {
        let rows: Vec<Table> = data.into_iter().collect();
        Self::new(rows).dot_x(|mark| mark.x(field))
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

    /// Add a `textX` mark, Plot's `textX`: A text label at each x with no y.
    pub fn text_x(
        mut self,
        configure: impl FnOnce(crate::mark::TextXMark) -> crate::mark::TextXMark,
    ) -> Self {
        let builder = configure(crate::mark::TextXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `textY` mark, Plot's `textY`: A text label at each y with no x.
    pub fn text_y(
        mut self,
        configure: impl FnOnce(crate::mark::TextYMark) -> crate::mark::TextYMark,
    ) -> Self {
        let builder = configure(crate::mark::TextYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `cellX` mark, Plot's `cellX`: A cell over one x with no y.
    pub fn cell_x(
        mut self,
        configure: impl FnOnce(crate::mark::CellXMark) -> crate::mark::CellXMark,
    ) -> Self {
        let builder = configure(crate::mark::CellXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `cellY` mark, Plot's `cellY`: A cell over one y with no x.
    pub fn cell_y(
        mut self,
        configure: impl FnOnce(crate::mark::CellYMark) -> crate::mark::CellYMark,
    ) -> Self {
        let builder = configure(crate::mark::CellYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `vectorX` mark, Plot's `vectorX`: A vector at each x.
    pub fn vector_x(
        mut self,
        configure: impl FnOnce(crate::mark::VectorXMark) -> crate::mark::VectorXMark,
    ) -> Self {
        let builder = configure(crate::mark::VectorXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `vectorY` mark, Plot's `vectorY`: A vector at each y.
    pub fn vector_y(
        mut self,
        configure: impl FnOnce(crate::mark::VectorYMark) -> crate::mark::VectorYMark,
    ) -> Self {
        let builder = configure(crate::mark::VectorYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `spike` mark, Plot's `spike`: A triangular marker at each datum.
    pub fn spike(
        mut self,
        configure: impl FnOnce(crate::mark::SpikeMark) -> crate::mark::SpikeMark,
    ) -> Self {
        let builder = configure(crate::mark::SpikeMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `arrow` mark, Plot's `arrow`: A curved arrow between two points.
    pub fn arrow(
        mut self,
        configure: impl FnOnce(crate::mark::ArrowMark) -> crate::mark::ArrowMark,
    ) -> Self {
        let builder = configure(crate::mark::ArrowMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `waffleX` mark, Plot's `waffleX`: A waffle bar along x.
    pub fn waffle_x(
        mut self,
        configure: impl FnOnce(crate::mark::WaffleXMark) -> crate::mark::WaffleXMark,
    ) -> Self {
        let builder = configure(crate::mark::WaffleXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `waffleY` mark, Plot's `waffleY`: A waffle bar along y.
    pub fn waffle_y(
        mut self,
        configure: impl FnOnce(crate::mark::WaffleYMark) -> crate::mark::WaffleYMark,
    ) -> Self {
        let builder = configure(crate::mark::WaffleYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `hexgrid` mark, Plot's `hexgrid`: A static hexagonal grid.
    pub fn hexgrid(
        mut self,
        configure: impl FnOnce(crate::mark::HexgridMark) -> crate::mark::HexgridMark,
    ) -> Self {
        let builder = configure(crate::mark::HexgridMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `raster` mark, Plot's `raster`: A pixel grid colored from the data.
    pub fn raster(
        mut self,
        configure: impl FnOnce(crate::mark::RasterMark) -> crate::mark::RasterMark,
    ) -> Self {
        let builder = configure(crate::mark::RasterMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `differenceX` mark, Plot's `differenceX`: A difference chart along x.
    pub fn difference_x(
        mut self,
        configure: impl FnOnce(crate::mark::DifferenceXMark) -> crate::mark::DifferenceXMark,
    ) -> Self {
        let builder = configure(crate::mark::DifferenceXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `differenceY` mark, Plot's `differenceY`: A difference chart along y.
    pub fn difference_y(
        mut self,
        configure: impl FnOnce(crate::mark::DifferenceYMark) -> crate::mark::DifferenceYMark,
    ) -> Self {
        let builder = configure(crate::mark::DifferenceYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `linearRegressionX` mark, Plot's `linearRegressionX`: A least-squares fit of x on y.
    pub fn linear_regression_x(
        mut self,
        configure: impl FnOnce(crate::mark::LinearRegressionXMark) -> crate::mark::LinearRegressionXMark,
    ) -> Self {
        let builder = configure(crate::mark::LinearRegressionXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `linearRegressionY` mark, Plot's `linearRegressionY`: A least-squares fit of y on x.
    pub fn linear_regression_y(
        mut self,
        configure: impl FnOnce(crate::mark::LinearRegressionYMark) -> crate::mark::LinearRegressionYMark,
    ) -> Self {
        let builder = configure(crate::mark::LinearRegressionYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `bollingerX` mark, Plot's `bollingerX`: A moving mean with deviation bands along x.
    pub fn bollinger_x(
        mut self,
        configure: impl FnOnce(crate::mark::BollingerXMark) -> crate::mark::BollingerXMark,
    ) -> Self {
        let builder = configure(crate::mark::BollingerXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `bollingerY` mark, Plot's `bollingerY`: A moving mean with deviation bands along y.
    pub fn bollinger_y(
        mut self,
        configure: impl FnOnce(crate::mark::BollingerYMark) -> crate::mark::BollingerYMark,
    ) -> Self {
        let builder = configure(crate::mark::BollingerYMark::new());
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

    /// Add a density mark, Plot's `density`: contours of the two-dimensional
    /// kernel density of x and y, one set per `z` series (or per `stroke`
    /// field), over thresholds shared by every series.
    pub fn density(
        mut self,
        configure: impl FnOnce(crate::mark::DensityMark) -> crate::mark::DensityMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DensityMark::new()).into());
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

    /// Add a tree diagram of a slash-separated hierarchy, Plot's `tree`.
    pub fn tree(
        mut self,
        configure: impl FnOnce(crate::mark::TreeMark) -> crate::mark::TreeMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TreeMark::new()).into());
        self
    }

    /// Add a raster image painted at a position, Plot's `image`.
    pub fn image(
        mut self,
        configure: impl FnOnce(crate::mark::ImageMark) -> crate::mark::ImageMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ImageMark::new()).into());
        self
    }

    /// Add a geographic mark, Plot's `geo`: lon/lat projected equirectangularly.
    pub fn geo(
        mut self,
        configure: impl FnOnce(crate::mark::GeoMark) -> crate::mark::GeoMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::GeoMark::new()).into());
        self
    }

    /// Add a Delaunay triangulation drawn as links, Plot's `delaunayLink`.
    pub fn delaunay_link(
        mut self,
        configure: impl FnOnce(crate::mark::DelaunayLinkMark) -> crate::mark::DelaunayLinkMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DelaunayLinkMark::new()).into());
        self
    }

    /// Add a Delaunay triangulation drawn as one mesh, Plot's `delaunayMesh`.
    pub fn delaunay_mesh(
        mut self,
        configure: impl FnOnce(crate::mark::DelaunayMeshMark) -> crate::mark::DelaunayMeshMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DelaunayMeshMark::new()).into());
        self
    }

    /// Add the convex hull of the data, Plot's `hull`.
    pub fn hull(
        mut self,
        configure: impl FnOnce(crate::mark::HullMark) -> crate::mark::HullMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::HullMark::new()).into());
        self
    }

    /// Add a Voronoi cell per datum, Plot's `voronoi`.
    pub fn voronoi(
        mut self,
        configure: impl FnOnce(crate::mark::VoronoiMark) -> crate::mark::VoronoiMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::VoronoiMark::new()).into());
        self
    }

    /// Add the Voronoi mesh of the data, Plot's `voronoiMesh`.
    pub fn voronoi_mesh(
        mut self,
        configure: impl FnOnce(crate::mark::VoronoiMeshMark) -> crate::mark::VoronoiMeshMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::VoronoiMeshMark::new()).into());
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
    /// Set the x axis label, Plot's `x: {label}`. By default the label is
    /// the marks' x field, with an arrow pointing the way values grow; an
    /// empty label hides it, Plot's `label: null`.
    pub fn x_label(mut self, label: impl Into<SharedString>) -> Self {
        self.x_label = Some(label.into());
        self
    }

    /// Set the y axis label, Plot's `y: {label}`.
    pub fn y_label(mut self, label: impl Into<SharedString>) -> Self {
        self.y_label = Some(label.into());
        self
    }

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

    /// Configure the whole interactive layer at once — whether the pointer is
    /// tracked, and whether the crosshair and the hovered-datum highlight
    /// draw — Plot's `pointer` options.
    pub fn interaction(mut self, interaction: Interaction) -> Self {
        self.interaction = interaction;
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

    /// Color the marks by a quantitative field through `range`.
    pub fn color_by(mut self, field: impl Into<SharedString>, range: ColorRange) -> Self {
        self.color_field = Some(field.into());
        self.color_range = Some(range);
        self
    }

    /// Split the figure into one column per value of `field`, Plot's `fx`.
    ///
    /// Every facet draws the rows whose `field` matches it, and the x and y
    /// scales stay shared across the figure, so the small multiples compare
    /// directly. A facet axis labels the columns above the plot.
    pub fn fx(mut self, field: impl Into<SharedString>) -> Self {
        self.fx = Some(field.into());
        self
    }

    /// Split the figure into one row per value of `field`, Plot's `fy`.
    ///
    /// A facet axis labels the rows to the right of the plot, and the two
    /// facet options together make a grid: `fx` varies across a row, `fy`
    /// down the figure.
    pub fn fy(mut self, field: impl Into<SharedString>) -> Self {
        self.fy = Some(field.into());
        self
    }

    /// The field that splits the figure into columns, if any.
    pub fn facet_x(&self) -> Option<&SharedString> {
        self.fx.as_ref()
    }

    /// The field that splits the figure into rows, if any.
    pub fn facet_y(&self) -> Option<&SharedString> {
        self.fy.as_ref()
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

    /// The facet grid the plot's `fx`/`fy` fields resolve.
    ///
    /// The domains come from the data's values, so a facet exists for every
    /// value a mark could draw there — the same values Plot's facet band
    /// scales read.
    fn facet_grid(&self) -> Grid {
        // The domains are every facet value a mark could draw: the plot's
        // `fx`/`fy` over its data, and each mark's own facet channels over
        // the mark's data.
        let mut fx: Vec<Value> = Vec::new();
        let mut fy: Vec<Value> = Vec::new();
        if let Some(field) = &self.fx {
            fx.extend(self.data.values(field));
        }
        if let Some(field) = &self.fy {
            fy.extend(self.data.values(field));
        }
        for mark in &self.marks {
            let data = mark.data.as_ref().unwrap_or(&self.data);
            for (channel, values) in [(&mark.fx, &mut fx), (&mark.fy, &mut fy)] {
                if let Some(channel) = channel {
                    values.extend(data.rows().iter().map(|row| channel.read(row)));
                }
            }
        }
        fx.retain(|value| !value.is_null());
        fy.retain(|value| !value.is_null());
        Grid::from_values(fx, fy)
    }

    /// The resolved layout: the insets the frame reserves.
    ///
    /// Plot's margins follow the axes and legend it draws, so the plot shrinks
    /// to fit them. A faceted figure repeats its axis gutters inside every
    /// facet, so the axis insets are returned separately from the figure's
    /// own margins for the title, legend and facet labels.
    fn insets(
        &self,
        theme: &PlotTheme,
        window: &mut Window,
        facets: &[Vec<PreparedMark>],
        grid: &Grid,
        width: f32,
    ) -> (Insets, Insets) {
        let margins = self.plot_margins(grid);
        let _ = AXIS_LABEL_GAP;
        let mut insets = Insets {
            top: margins[0],
            right: margins[1],
            bottom: margins[2],
            left: margins[3],
        };
        // Plot draws a facet's axes at the grid's edges, in the margins, so a
        // facet's own band holds only its marks.
        let facet_insets = Insets::default();
        if let Some(caption) = &self.caption {
            let _ = caption;
            insets.bottom += theme.label_size.as_f32() + 8.;
        }

        if self.title.text.is_some() {
            insets.top += theme.title_size.as_f32() + 6.;
            if self.title.subtitle.is_some() {
                insets.top += theme.label_size.as_f32() + 4.;
            }
        }

        let colors = self.plot_colors(theme, facets);
        let legend_entries = self.legend_entries(&colors, facets);
        if self.legend_position() != LegendPosition::None && !legend_entries.is_empty() {
            // The band is the height the legend's own layout takes at this
            // width: a heading, wrapped rows and quantity rows all count.
            let (_, band) = self.legend_layout(theme, window, &legend_entries, width);
            match self.legend_position() {
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

        (insets, facet_insets)
    }

    /// Plot's margins, `[top, right, bottom, left]`: the space the axes ask
    /// for around the plot, or the explicit `margin*` options.
    fn plot_margins(&self, grid: &Grid) -> [f32; 4] {
        // Plot's margins come from its axes: a bottom x axis wants 30 pixels
        // below and 20 at either side; a left y axis 40 to the left and 20
        // above and below. A top x axis or a right y axis move theirs, and the
        // facet axes (fx on top, fy on the right) add theirs. An explicit
        // `margin*` option wins.
        let (x_axis, y_axis) = (self.x_axis_anchor(), self.y_axis_anchor());
        let mut margins = [0.0_f32; 4];
        let mut want = |side: usize, value: f32| margins[side] = margins[side].max(value);
        use crate::options::AxisAnchor;
        if let Some(anchor) = x_axis {
            want(1, 20.);
            want(3, 20.);
            match anchor {
                AxisAnchor::Top => want(0, 30.),
                AxisAnchor::Both => {
                    want(0, 30.);
                    want(2, 30.);
                }
                _ => want(2, 30.),
            }
        }
        if let Some(anchor) = y_axis {
            want(0, 20.);
            want(2, 20.);
            match anchor {
                AxisAnchor::Right => want(1, 40.),
                AxisAnchor::Both => {
                    want(1, 40.);
                    want(3, 40.);
                }
                _ => want(3, 40.),
            }
        }
        if grid.fx.is_some() && self.scales.fx.axis != Some(None) {
            want(0, 30.);
        }
        if grid.fy.is_some() && self.scales.fy.axis != Some(None) {
            want(1, 40.);
        }
        for (side, explicit) in self.margins.iter().enumerate() {
            if let Some(margin) = explicit {
                margins[side] = *margin;
            }
        }
        margins
    }

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
    fn legend_position(&self) -> LegendPosition {
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

    /// Where the x axis sits, or `None` when the plot draws none: Plot's
    /// `x.axis`, the plot's `axis: null`, and whether any mark reads x.
    pub(crate) fn x_axis_anchor(&self) -> Option<crate::options::AxisAnchor> {
        if let Some(mark) = self.axis_mark(true) {
            return Some(
                mark.axis
                    .axis
                    .flatten()
                    .unwrap_or(crate::options::AxisAnchor::Bottom),
            );
        }
        if !self.x_axis {
            return None;
        }
        match self.scales.x.axis {
            Some(anchor) => anchor,
            None if self.no_axes => None,
            None => self
                .reads_axis(true)
                .then_some(crate::options::AxisAnchor::Bottom),
        }
    }

    /// Where the y axis sits, or `None` when the plot draws none.
    pub(crate) fn y_axis_anchor(&self) -> Option<crate::options::AxisAnchor> {
        if let Some(mark) = self.axis_mark(false) {
            return Some(
                mark.axis
                    .axis
                    .flatten()
                    .unwrap_or(crate::options::AxisAnchor::Left),
            );
        }
        if !self.y_axis {
            return None;
        }
        match self.scales.y.axis {
            Some(anchor) => anchor,
            None if self.no_axes => None,
            None => self
                .reads_axis(false)
                .then_some(crate::options::AxisAnchor::Left),
        }
    }

    /// The first axis mark for x (or y), Plot's `axisX`: when a plot has one,
    /// it draws no implicit axis of its own for that scale.
    fn axis_mark(&self, x: bool) -> Option<&Mark> {
        let kind = if x { MarkKind::AxisX } else { MarkKind::AxisY };
        self.marks.iter().find(|mark| mark.kind == kind)
    }

    /// Plot's `label: null`: no axis labels at all.
    pub fn no_labels(mut self) -> Self {
        self.no_labels = true;
        self
    }

    /// Whether any data mark reads a position on the axis, which is when Plot
    /// draws it: a plot of `dotX` has no y axis. A radial mark lays its rows
    /// out on its own and reads neither.
    fn reads_axis(&self, x: bool) -> bool {
        // A scale given a type or domain exists, and Plot draws its axis.
        let options = if x { &self.scales.x } else { &self.scales.y };
        if options.r#type.is_some() || options.domain.is_some() {
            return true;
        }
        let grid_kind = if x { MarkKind::GridX } else { MarkKind::GridY };
        if self.marks.iter().any(|mark| mark.kind == grid_kind) {
            return true;
        }
        self.marks.iter().any(|mark| {
            if mark.kind.is_furniture()
                || matches!(mark.kind, MarkKind::Pie | MarkKind::Arc | MarkKind::Waffle)
            {
                return false;
            }
            let c = &mark.channels;
            let named = if x {
                c.x.is_some() || c.x1.is_some() || c.x2.is_some()
            } else {
                c.y.is_some() || c.y1.is_some() || c.y2.is_some()
            };
            // A mark over plain values reads its single axis by default, and a
            // one-dimensional mark spreads along the other by index.
            let single = mark.kind.category().single_axis;
            let by_value = mark
                .data
                .as_ref()
                .is_some_and(|data| data.is_values() || data.is_tuples());
            let indexed = mark.kind.cross_axis() == crate::mark::CrossAxis::Index
                && single
                    == Some(if x {
                        crate::mark::Axis::Y
                    } else {
                        crate::mark::Axis::X
                    });
            let cross_rule = matches!(
                (mark.kind, x),
                (MarkKind::RuleX | MarkKind::TickX | MarkKind::BarX, true)
                    | (MarkKind::RuleY | MarkKind::TickY | MarkKind::BarY, false)
            );
            named
                || indexed
                || (by_value
                    && (single
                        == Some(if x {
                            crate::mark::Axis::X
                        } else {
                            crate::mark::Axis::Y
                        })
                        || cross_rule
                        || mark.data.as_ref().is_some_and(Dataset::is_tuples)))
                || mark.geo_geometry.is_some()
        })
    }

    /// Where each legend entry sits, relative to the legend's own top-left,
    /// and the height the legend takes.
    ///
    /// This is the one layout both the frame's insets and the painter read,
    /// so the band the frame reserves is exactly the band the legend fills.
    /// A top or bottom legend wraps its entries at `width`, the figure's
    /// width; a side legend stacks them in a column.
    fn legend_layout(
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
    fn legend_width(&self, theme: &PlotTheme, window: &mut Window, entries: &[LegendEntry]) -> f32 {
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
                y += theme.label_size.as_f32() + 4.;
            }
        }
        y
    }

    fn x_domain_values(&self, facets: &[Vec<PreparedMark>]) -> Vec<Value> {
        facets
            .iter()
            .flatten()
            .flat_map(render::basic::x_values)
            .collect()
    }

    fn y_domain_values(&self, facets: &[Vec<PreparedMark>]) -> Vec<Value> {
        facets
            .iter()
            .flatten()
            .flat_map(render::basic::y_values)
            .collect()
    }

    /// The legend's entries: one per series or one per categorical value.
    ///
    /// Every facet's marks are read, and an entry appears once however many
    /// facets draw it: a faceted chart's legend describes the series, not each
    /// small multiple.
    fn legend_entries(
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

/// Every `(facet, mark, row)` a pointer can land on, in the order
/// `tooltip_state` searches them. Both `tooltip_state` and `tooltip` build
/// this list, so the state's index means the same thing to each.
///
/// A facet draws its marks in the order the marks were declared, and the
/// facets come in the grid's own order, which is the order `paint` draws in.
fn hover_targets(facets: &[Vec<PreparedMark>]) -> Vec<(usize, usize, usize)> {
    facets
        .iter()
        .enumerate()
        .flat_map(|(cell, prepared)| {
            prepared
                .iter()
                .enumerate()
                .flat_map(move |(mark_index, mark)| {
                    (0..mark.rows.len()).map(move |row_index| (cell, mark_index, row_index))
                })
        })
        .collect()
}

/// The pixel rectangle a row's body spans, so the pointer can hit an interval
/// mark — a bar, rect or cell — across its whole extent, not just its anchor.
///
/// Both ends of each interval are projected; a mark with no x interval uses its
/// x value, and likewise y, falling back to no rect when a channel does not
/// scale. The rectangle is in the facet frame's plot-local pixels.
fn interval_rect(
    mark: &PreparedMark,
    row: &crate::render::PreparedRow,
    frame: &Frame,
) -> Option<Bounds<Pixels>> {
    let cat = mark.mark.kind.category();
    // Only a mark that paints a body has one to hover: a row with an interval
    // (a bar's length, a rect's extent) or a band-filling mark (a cell). A dot
    // or a line vertex is hit by its point instead, so a padded rectangle
    // around it cannot steal the hover from a nearer datum.
    let has_interval = row.x1.is_some() || row.x2.is_some() || row.y1.is_some() || row.y2.is_some();
    if !(has_interval || (cat.band_x && cat.band_y)) || cat.furniture {
        return None;
    }
    let x = |v: &crate::value::Value| frame.x.scale.position(v);
    let y = |v: &crate::value::Value| frame.y.scale.position(v);
    let x0 = row.x1.as_ref().or(Some(&row.x)).and_then(x);
    let x1 = row.x2.as_ref().or(Some(&row.x)).and_then(x);
    let y0 = row.y1.as_ref().or(Some(&row.y)).and_then(y);
    let y1 = row.y2.as_ref().or(Some(&row.y)).and_then(y);
    let (Some(x0), Some(x1), Some(y0), Some(y1)) = (x0, x1, y0, y1) else {
        return None;
    };
    let (mut x0, mut x1) = (x0.min(x1), x0.max(x1));
    let (mut y0, mut y1) = (y0.min(y1), y0.max(y1));
    // A length mark spans a single value on its other axis (a vertical bar one
    // x, a horizontal bar one y). On a band axis the body is the band's width,
    // centred on that value; elsewhere it is padded a little so it is still
    // hoverable.
    let widen = |lo: &mut f32, hi: &mut f32, band: f32| {
        if *hi - *lo <= 1. {
            let half = if band > 1. { band / 2. } else { 6. };
            let centre = (*lo + *hi) / 2.;
            *lo = centre - half;
            *hi = centre + half;
        }
    };
    widen(&mut x0, &mut x1, frame.x.scale.band_width());
    widen(&mut y0, &mut y1, frame.y.scale.band_width());
    Some(Bounds::new(
        point(px(x0), px(y0)),
        gpui::size(px(x1 - x0), px(y1 - y0)),
    ))
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

impl Plot {
    /// Resolve the frame for `bounds`, including the margins the axes and
    /// title need.
    ///
    /// A faceted figure lays its x and y scales out inside one facet, so the
    /// frame is told that facet's size up front; the plot's own bounds still
    /// hold the whole grid.
    fn resolve_frame(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        facets: &[Vec<PreparedMark>],
        grid: &Grid,
    ) -> Frame {
        let mut options = self.frame_options.clone();
        options.x_options = self.scales.x.clone();
        options.y_options = self.scales.y.clone();
        // A mark's `sort` option orders an ordinal domain the scale left open.
        if options.x_options.domain.is_none() {
            options.x_options.domain = sorted_domain(facets, "x");
        }
        if options.y_options.domain.is_none() {
            options.y_options.domain = sorted_domain(facets, "y");
        }
        let (insets, facet_insets) =
            self.insets(theme, window, facets, grid, bounds.size.width.as_f32());
        options.insets = insets;
        options.facet_insets = facet_insets;
        // A mark that draws a band on an axis — a bar's width, a cell's
        // extent — needs a band scale there, not a point scale: a point
        // scale puts the first and last category on the plot's edges, and
        // their bands would hang off the plot and be clipped in half. Plot
        // infers `band` for barX/barY/cell and `point` for everything else.
        if options.x_kind.is_none() && marks_use_band(facets, true) {
            options.x_kind = Some(ScaleKind::Band);
        }
        if options.y_kind.is_none() && marks_use_band(facets, false) {
            options.y_kind = Some(ScaleKind::Band);
        }
        // A plot may ask for its own height, Plot's `height`: the height of
        // its chart, below whatever title and legend the figure adds. The
        // element fills its container, so the request shrinks the figure
        // to that rather than growing it.
        let margins = self.plot_margins(grid);
        let extras = (options.insets.top - margins[0]) + (options.insets.bottom - margins[2]);
        let bounds = match self.height {
            Some(height) if height > 0. => Bounds {
                size: gpui::size(
                    bounds.size.width,
                    px(height + extras).min(bounds.size.height.max(px(height + extras))),
                ),
                ..bounds
            },
            _ => bounds,
        };
        // The grid divides the plot the insets leave, so one facet's size
        // follows from the plot's size, the number of facets and the axes each
        // facet draws inside its own band.
        let plot_width =
            (bounds.size.width.as_f32() - options.insets.left - options.insets.right).max(1.);
        let plot_height =
            (bounds.size.height.as_f32() - options.insets.top - options.insets.bottom).max(1.);
        options.facet_size = grid.is_faceted().then(|| {
            let band_width = grid.facet_width(plot_width);
            let band_height = grid.facet_height(plot_height);
            [
                (band_width - facet_insets.left - facet_insets.right).max(1.),
                (band_height - facet_insets.top - facet_insets.bottom).max(1.),
            ]
        });
        Frame::resolve(
            bounds,
            &options,
            &self.x_domain_values(facets),
            &self.y_domain_values(facets),
        )
    }
}

/// The domain a mark's `sort: {scale: by}` option orders, Plot's
/// `sort` channel option: each of the scale's values is ranked by the `by`
/// channel's values on its rows, reduced (by `max` unless said otherwise).
pub(crate) fn sorted_domain(facets: &[Vec<PreparedMark>], scale: &str) -> Option<Vec<Value>> {
    let mut ranked: Vec<(Value, Vec<Value>)> = Vec::new();
    let mut sort: Option<crate::mark::DomainSort> = None;
    for prepared in facets.iter().flatten() {
        let Some(spec) = prepared.mark.domain_sort.iter().find(|s| s.scale == scale) else {
            continue;
        };
        sort.get_or_insert_with(|| spec.clone());
        for (index, row) in prepared.rows.iter().enumerate() {
            let key = match scale {
                "x" => row.x_value().clone(),
                "y" => row.y_value().clone(),
                "color" => match prepared.mark.channels.fill.as_ref().or(prepared
                    .mark
                    .channels
                    .stroke
                    .as_ref())
                {
                    Some(channel) => channel
                        .field()
                        .map(|f| row.row.value(f))
                        .unwrap_or(Value::Null),
                    None => Value::Null,
                },
                _ => Value::Null,
            };
            if key.is_null() {
                continue;
            }
            let by = match spec.by.as_ref() {
                "x" => row.x_value().clone(),
                "x1" => row.x1.clone().unwrap_or_else(|| row.x_value().clone()),
                "x2" => row.x2.clone().unwrap_or_else(|| row.x_value().clone()),
                "y" => row.y_value().clone(),
                "y1" => row.y1.clone().unwrap_or_else(|| row.y_value().clone()),
                "y2" => row.y2.clone().unwrap_or_else(|| row.y_value().clone()),
                "r" => row.r_value.map(Value::Number).unwrap_or(Value::Null),
                // A bar's extent along its value axis, Plot's `width` and
                // `height` sort channels.
                "width" | "height" => {
                    let (a, b) = if spec.by.as_ref() == "width" {
                        (&row.x1, &row.x2)
                    } else {
                        (&row.y1, &row.y2)
                    };
                    match (
                        a.as_ref().and_then(Value::as_number),
                        b.as_ref().and_then(Value::as_number),
                    ) {
                        (Some(a), Some(b)) => Value::Number((b - a).abs()),
                        _ => Value::Null,
                    }
                }
                "data" => Value::Number(index as f64),
                // Plot sorts `x` by `-y` as `{x: "y", reverse: true}`; any
                // other name reads the record's field.
                field => row.row.value(field),
            };
            match ranked.iter_mut().find(|(k, _)| k.key() == key.key()) {
                Some((_, values)) => values.push(by),
                None => ranked.push((key, vec![by])),
            }
        }
    }
    let sort = sort?;
    let mut scored: Vec<(Value, Value)> = ranked
        .into_iter()
        .map(|(key, values)| {
            let records: Vec<&Table> = Vec::new();
            let reduced = sort.reduce.reduce(&values, &records);
            (key, reduced)
        })
        .collect();
    // Plot's `ascendingDefined`: missing values last, whichever the order.
    scored.sort_by(|(_, a), (_, b)| match (a.is_null(), b.is_null()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => {
            let ordering = a.cmp(b);
            if sort.reverse {
                ordering.reverse()
            } else {
                ordering
            }
        }
    });
    let mut domain: Vec<Value> = scored.into_iter().map(|(key, _)| key).collect();
    if let Some(limit) = sort.limit {
        domain.truncate(limit);
    }
    Some(domain)
}

/// Whether any mark draws a band along the x axis (`along_x`) or the y axis.
///
/// Only matters for a discrete axis: a continuous scale has no bands.
fn marks_use_band(facets: &[Vec<PreparedMark>], along_x: bool) -> bool {
    facets.iter().flatten().any(|prepared| {
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
        // Plot's bar and cell marks read their band axis as ordinal whatever
        // its values, so a bar per year takes a band per year.
        let forced = match prepared.mark.kind {
            MarkKind::BarY | MarkKind::Cell | MarkKind::CellX if along_x => {
                prepared.mark.channels.x.is_some() && prepared.mark.interval.is_none()
            }
            MarkKind::BarX | MarkKind::Cell | MarkKind::CellY if !along_x => {
                prepared.mark.channels.y.is_some() && prepared.mark.interval.is_none()
            }
            _ => false,
        };
        if forced {
            return true;
        }
        if !discrete {
            return false;
        }
        // A mark's band direction is part of its classification: a bar's
        // width, a cell's extent, a tick's ordinal secondary axis and a
        // box/violin/density's category axis all fill a band.
        let cat = prepared.mark.kind.category();
        if along_x { cat.band_x } else { cat.band_y }
    })
}

impl PlotTrait for Plot {
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        // Resolve everything the later phases read here, once per frame: the
        // marks (the transform pipeline), the frame (the layout), the dodge
        // (which needs the frame's pixels) and the color scales. `paint`,
        // `tooltip_state` and `tooltip` then share one consistent view.
        let theme = PlotTheme::from_theme(cx.theme());
        let mut grid = self.facet_grid();
        let (mut facets, populated) = render::prepare_facets(
            &self.data,
            &self.marks,
            &grid,
            self.fx.as_deref(),
            self.fy.as_deref(),
        );
        grid.set_empty(&populated);
        render::apply_quantity_scales(&mut facets, &self.scales.r, &self.scales.opacity);
        render::apply_symbol_scale(&mut facets, &self.scales.symbol);
        let frame = self.resolve_frame(bounds, &theme, window, &facets, &grid);
        // A dodge works in pixels, so it needs the frame resolved first. Every
        // facet is the same size, so one facet's frame packs the same way.
        let dodge_frame = cell_frame(&frame, &grid, 0);
        for marks in facets.iter_mut() {
            render::apply_dodge(marks, &dodge_frame);
        }
        let colors = self.plot_colors(&theme, &facets);
        self.facets_cache = Some(Rc::new(facets));
        self.colors_cache = Some(Rc::new(colors));
        self.frame_cache = Some(frame);
        self.facet_cache = Some(grid);
        vec![]
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let theme = PlotTheme::from_theme(cx.theme());
        let (Some(facets), Some(frame), Some(grid), Some(colors)) = (
            self.facets_cache.clone(),
            self.frame_cache.clone(),
            self.facet_cache.clone(),
            self.colors_cache.clone(),
        ) else {
            return;
        };

        let appear = self.appear_progress;

        // The title sits above the plot, inside the figure's top margin.
        self.paint_title(bounds, &theme, window, cx);
        // The legend sits in the margin its position reserves, inside the
        // figure's bounds (already shrunk to the plot's `height`).
        self.paint_legend(frame.bounds, &theme, window, cx, &facets, &colors);
        // The grid goes under the marks.
        // Plot's grid belongs to its axis: an axis drawn with `grid` (its own
        // or the plot's) draws its lines; `axis: null` takes both away.
        let (x_grid, y_grid) = (
            self.scales.x.grid.unwrap_or(self.grid) && self.x_axis_anchor().is_some(),
            self.scales.y.grid.unwrap_or(self.grid) && self.y_axis_anchor().is_some(),
        );
        if x_grid || y_grid {
            crate::frame::paint_grid(&frame, &theme, window, x_grid, y_grid, &grid);
        }
        // The marks. Every mark shares one set of color scales, so a series
        // keeps its color across marks, and across facets.
        let mut row_offset = 0;
        for (cell, marks) in facets.iter().enumerate() {
            let facet_frame = cell_frame(&frame, &grid, cell);
            // Plot skips a facet that holds no data: it keeps the slot, and
            // the facet axis, but draws no marks inside it.
            let skip = grid.is_faceted() && grid.is_empty(cell);
            for (index, mark) in marks.iter().enumerate() {
                let offset = row_offset;
                // The row offset counts every facet's rows, drawn or not, so it
                // matches the flattened list the hover indexes.
                row_offset += mark.rows.len();
                if skip {
                    continue;
                }
                let mut ctx = DrawContext {
                    frame: &facet_frame,
                    theme: &theme,
                    mark_index: index,
                    appear,
                    hover_progress: self.hover_progress,
                    hovered: self.hover,
                    row_offset: offset,
                    colors: &colors,
                    clip: mark.mark.style.clip,
                    window,
                    cx,
                };
                // Which renderer family paints the mark, read from its single
                // classification table rather than a hand-maintained list here.
                match mark.mark.kind.category().family {
                    crate::mark::MarkFamily::Radial => render::radial::draw(mark, &mut ctx),
                    // The Delaunay marks build their geometry from the
                    // projected points, so they draw through the frame's
                    // pixel space rather than the row shapes.
                    crate::mark::MarkFamily::Delaunay => render::delaunay::draw(mark, &mut ctx),
                    crate::mark::MarkFamily::Basic => render::basic::draw(mark, &mut ctx),
                }
            }
        }
        // The axes sit on top of the marks. A faceted figure draws each
        // facet's axes at the grid's outer edges, Plot's facet anchors.
        let x_style = self
            .x_axis_anchor()
            .filter(|_| self.axis_mark(true).is_none())
            .map(|anchor| crate::frame::AxisStyle::new(anchor, &self.scales.x));
        let y_style = self
            .y_axis_anchor()
            .filter(|_| self.axis_mark(false).is_none())
            .map(|anchor| crate::frame::AxisStyle::new(anchor, &self.scales.y));
        if grid.is_faceted() {
            for cell in 0..grid.cells.len() {
                if grid.is_empty(cell) {
                    continue;
                }
                crate::frame::paint_axes(
                    &cell_frame(&frame, &grid, cell),
                    &theme,
                    window,
                    cx,
                    x_style,
                    y_style,
                    &crate::frame::AxisEdges::from_cell(cell, &grid),
                );
            }
            crate::frame::paint_facet_labels(&frame, &theme, window, cx, &grid);
        } else {
            crate::frame::paint_axes(
                &frame,
                &theme,
                window,
                cx,
                x_style,
                y_style,
                &crate::frame::AxisEdges::default(),
            );
        }
        self.paint_axis_labels(&frame, &theme, window, cx);
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
        let frame = self.frame_cache.as_ref()?;
        let grid = self.facet_cache.as_ref()?;
        let facets = self.facets_cache.as_ref()?;

        // Ignore the axis gutters: hovering a label shows nothing.
        let absolute = position + bounds.origin;
        if !frame.plot.contains(&absolute) {
            return None;
        }
        let local = absolute - frame.plot.origin;
        let cell_frames: Vec<Frame> = (0..facets.len())
            .map(|cell| cell_frame(frame, grid, cell))
            .collect();

        // The datum under the pointer. A mark that draws an interval — a bar,
        // a rect, a cell — is hoverable across its whole body, so the pointer
        // selects it when it lies inside that rectangle (Plot's `nearest`)
        // rather than only near its anchor point; among overlapping bodies the
        // last-painted, topmost one wins. A mark without an interval (a dot, a
        // line vertex) is picked by distance to the point it was painted at.
        let targets = hover_targets(facets);
        let mut contained: Option<(usize, Point<Pixels>)> = None;
        let mut best: Option<(usize, Point<Pixels>, f32)> = None;
        for (flat, &(cell, mark_index, row_index)) in targets.iter().enumerate() {
            if grid.is_faceted() && grid.is_empty(cell) {
                continue;
            }
            let mark = &facets[cell][mark_index];
            if !hoverable(mark) {
                continue;
            }
            let Some(row) = mark.rows.get(row_index) else {
                continue;
            };
            let facet_frame = &cell_frames[cell];
            // Everything is lifted from the facet's plot-local pixels into the
            // whole figure's plot coordinates, where `local` lives.
            let lift = facet_frame.plot.origin - frame.plot.origin;
            let Some(point) = render::basic::position_in(mark, facet_frame, row) else {
                continue;
            };
            let point = point + lift;
            if interval_rect(mark, row, facet_frame).is_some_and(|r| (r + lift).contains(&local)) {
                contained = Some((flat, point));
            }
            let dx = point.x.as_f32() - local.x.as_f32();
            let dy = point.y.as_f32() - local.y.as_f32();
            let distance = dx * dx + dy * dy;
            if best.as_ref().is_none_or(|(_, _, best)| distance < *best) {
                best = Some((flat, point, distance));
            }
        }
        let (flat, hit) = contained.or_else(|| best.map(|(flat, point, _)| (flat, point)))?;
        let (cell, mark_index, row_index) = targets[flat];
        let facet_frame = &cell_frames[cell];
        let lift = facet_frame.plot.origin - frame.plot.origin;
        let mark = &facets[cell][mark_index];
        let row = mark.rows.get(row_index)?;

        // One dot per series at the hovered datum: the rows of the same mark
        // that share its position on the independent axis. A bar's body is
        // already highlighted, so an interval mark marks only its own datum.
        let dots: Vec<Point<Pixels>> = if interval_rect(mark, row, facet_frame).is_some() {
            vec![hit]
        } else {
            let along_y = mark.mark.kind.is_y_only();
            let key = |other: &render::PreparedRow| {
                if along_y {
                    other.y_value().key()
                } else {
                    other.x_value().key()
                }
            };
            let hovered = key(row);
            let dots: Vec<Point<Pixels>> = mark
                .rows
                .iter()
                .filter(|other| key(other) == hovered)
                .filter_map(|other| render::basic::position_in(mark, facet_frame, other))
                .map(|point| point + lift)
                .collect();
            if dots.is_empty() { vec![hit] } else { dots }
        };

        // The tooltip overlay is placed at the element's own origin (its
        // top-left, including the title and legend margins), so the crosshair
        // and hovered dots must be in the element's coordinates, not the
        // plot's. `frame.absolute` lifts a plot-local point up by the plot's
        // offset within the figure.
        let hit = frame.absolute(hit);
        let dots = dots.into_iter().map(|p| frame.absolute(p)).collect();

        Some(TooltipState::new(flat, hit, dots))
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
        let facets = self.facets_cache.as_ref()?;
        let colors = self.colors_cache.as_ref()?;
        let frame = self.frame_cache.as_ref()?;
        let grid = self.facet_cache.as_ref()?;

        // The state carries the index the pointer resolved to, in the same
        // flattened row list `tooltip_state` searched.
        let (cell, mark_index, row_index) = *hover_targets(facets).get(state.index)?;
        let mark = facets.get(cell)?.get(mark_index)?;
        let row = mark.rows.get(row_index)?;

        // The swatch and dots take the color the mark painted this row in.
        let color = colors.identity(mark, mark_index, row);

        // The box hugs the cursor and carries the crosshair and hovered dots;
        // the body is either a custom formatter's freeform element or the
        // default title-and-rows.
        let mut tooltip = Tooltip::new(cursor, bounds.size).gap(px(8.));
        if let Some(formatter) = &self.tooltip_formatter {
            // A custom formatter replaces the default body, but the crosshair
            // and dots still render and the box still follows the cursor: the
            // returned element becomes the box's freeform base.
            let element = formatter(&row.row, window, cx);
            tooltip.extend([element]);
        } else {
            let title = mark
                .mark
                .channels
                .title
                .as_ref()
                .map(|channel| channel.read(&row.row).label())
                .unwrap_or_else(|| row.x_value().label());

            tooltip = tooltip.title(title).row(
                color,
                mark.mark
                    .title
                    .clone()
                    .unwrap_or_else(|| mark.mark.kind.name().into()),
                row.y_value().label(),
            );

            // Add the channels a tooltip conventionally reads: the series and
            // any other fields the mark named.
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
        }

        if self.interaction.crosshair {
            // The crosshair spans the hovered facet's plot area only, not the
            // title, legend or axis gutters around it.
            let facet_frame = cell_frame(frame, grid, cell);
            let top = facet_frame.plot.origin.y - bounds.origin.y;
            tooltip = tooltip.cross_line(
                CrossLine::new(state.cross_line)
                    .span(top.as_f32(), facet_frame.plot.size.height.as_f32())
                    .band(px(6.)),
            );
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

/// The frame one facet cell draws in: the figure's frame moved to the cell's
/// marks area. An unfaceted figure has one cell, the whole plot.
fn cell_frame(frame: &Frame, grid: &Grid, cell: usize) -> Frame {
    if grid.is_faceted() {
        frame.facet(crate::frame::facet_marks(
            grid.facet(frame.plot, cell),
            frame,
        ))
    } else {
        frame.clone()
    }
}

/// Whether the pointer can hover a mark's rows.
///
/// A pie, arc or waffle lays its rows out in polar or grid space around the
/// plot's center, not at their x/y positions, so a Cartesian hit-test would
/// land on meaningless spots; they are not hoverable rather than wrong.
fn hoverable(mark: &PreparedMark) -> bool {
    !matches!(
        mark.mark.kind,
        MarkKind::Pie | MarkKind::Arc | MarkKind::Waffle
    )
}

impl Plot {
    /// The plot's color scales over the resolved marks.
    fn plot_colors(
        &self,
        theme: &PlotTheme,
        facets: &[Vec<PreparedMark>],
    ) -> render::color::PlotColors {
        let color = self.color_field.as_ref().zip(self.color_range.as_ref());
        render::color::PlotColors::new(theme, facets, color, &self.scales.color)
    }

    /// The label an axis shows, Plot's default scale label: the first data
    /// mark's field on that axis, or `Frequency` for the count a bin grows
    /// along it. An explicit label overrides it, and an empty one hides it.
    fn axis_label(&self, x: bool) -> Option<SharedString> {
        let scale = if x { &self.scales.x } else { &self.scales.y };
        if let Some(label) = &scale.label {
            return label.clone();
        }
        if let Some(mark) = self.axis_mark(x)
            && let Some(label) = &mark.axis.label
        {
            return label.clone();
        }
        if self.no_labels {
            return None;
        }
        let label = self.derived_axis_label(x)?;
        // Plot's `percent` marks a derived label as a percentage.
        if scale.percent == Some(true) {
            return Some(format!("{label} (%)").into());
        }
        Some(label)
    }

    /// The label an axis derives from its channels, before scale options.
    fn derived_axis_label(&self, x: bool) -> Option<SharedString> {
        let explicit = if x { &self.x_label } else { &self.y_label };
        if let Some(label) = explicit {
            return (!label.is_empty()).then(|| label.clone());
        }
        // Plot's `inferScaleLabel`: every channel bound to the scale offers a
        // label (its field's name; an accessor or constant offers none), and
        // the scale takes it only when all agree. A bin's count is
        // "Frequency".
        let field = |channel: Option<&crate::table::Channel>| -> Option<SharedString> {
            match channel {
                Some(crate::table::Channel::Field(name)) if !name.starts_with('$') => {
                    Some(name.clone())
                }
                _ => None,
            }
        };
        let mut labels: Vec<Option<SharedString>> = Vec::new();
        for mark in &self.marks {
            if mark.kind.is_furniture() {
                continue;
            }
            let channels = &mark.channels;
            if let Some(bin) = &mark.bin {
                let (bin_x, bin_y) = render::bin_axes(mark);
                let length = if x { bin_y && !bin_x } else { bin_x && !bin_y };
                if length {
                    let name = if x { "x" } else { "y" };
                    labels.push(
                        match bin.outputs.iter().find(|output| output.name == name) {
                            Some(output)
                                if !matches!(
                                    output.reducer,
                                    crate::transform::GroupReducer::Count
                                ) =>
                            {
                                output.input.clone()
                            }
                            _ => Some("Frequency".into()),
                        },
                    );
                    continue;
                }
                if (x && bin_x && !bin_y) || (!x && bin_y && !bin_x) {
                    labels.push(field(channels.x.as_ref()).or_else(|| field(channels.y.as_ref())));
                    continue;
                }
            }
            // A group's count output is "Frequency" too.
            if let Some(group) = &mark.group {
                let names: &[&str] = if x {
                    &["x", "x1", "x2"]
                } else {
                    &["y", "y1", "y2"]
                };
                if let Some(output) = group
                    .outputs
                    .iter()
                    .find(|o| names.contains(&o.name.as_ref()))
                {
                    labels.push(match output.reducer {
                        crate::transform::GroupReducer::Count => Some("Frequency".into()),
                        _ => field(if x {
                            channels.x.as_ref()
                        } else {
                            channels.y.as_ref()
                        }),
                    });
                    continue;
                }
            }
            if !x
                && mark
                    .stack
                    .as_ref()
                    .is_some_and(|stack| !stack.fields.is_empty())
            {
                return None;
            }
            let bound: [Option<&crate::table::Channel>; 3] = if x {
                [
                    channels.x.as_ref(),
                    channels.x1.as_ref(),
                    channels.x2.as_ref(),
                ]
            } else {
                [
                    channels.y.as_ref(),
                    channels.y1.as_ref(),
                    channels.y2.as_ref(),
                ]
            };
            for channel in bound.into_iter().flatten() {
                labels.push(field(Some(channel)));
            }
        }
        let mut named = labels.iter().flatten();
        let first = named.next()?.clone();
        if named.any(|label| *label != first) {
            return None;
        }
        Some(first)
    }

    /// Paint the axis labels where Plot puts them: the y label in the top
    /// margin at the figure's left as `↑ label`, the x label in the bottom
    /// margin at the plot's right as `label →`. A categorical axis has no
    /// direction, so no arrow.
    fn paint_axis_labels(
        &self,
        frame: &Frame,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::options::AxisAnchor;
        let font = gpui::font(theme.font_family.clone());
        let continuous = |scale: &crate::scale::PositionScale| {
            matches!(scale, crate::scale::PositionScale::Continuous(_))
        };
        let grid = self
            .facet_cache
            .clone()
            .unwrap_or_else(|| self.facet_grid());
        let [top, right, bottom, left] = self.plot_margins(&grid);
        let insets = Insets {
            top,
            right,
            bottom,
            left,
        };
        let plot = frame.plot;
        let make = |text: String, window: &mut Window| {
            Label::new(
                text.into(),
                theme.label_size,
                theme.label,
                FontWeight::NORMAL,
                font.clone(),
                window,
            )
        };
        if let Some(anchor) = self.y_axis_anchor()
            && let Some(label) = self.axis_label(false)
        {
            let arrow = self.scales.y.label_arrow.unwrap_or(true) && continuous(&frame.y.scale);
            let up = !self
                .scales
                .y
                .reverse
                .unwrap_or(self.frame_options.y_reverse);
            let text = if arrow {
                format!("{} {label}", if up { "↑" } else { "↓" })
            } else {
                label.to_string()
            };
            let text = make(text, window);
            let center = self.scales.y.label_anchor == Some(crate::options::LabelAnchor::Center);
            let right = anchor == AxisAnchor::Right;
            let offset = self
                .scales
                .y
                .label_offset
                .unwrap_or(if right { insets.right } else { insets.left } - 3.);
            let x = if right {
                plot.origin.x + plot.size.width + px(offset)
            } else {
                plot.origin.x - px(offset)
            };
            let y = if center {
                plot.origin.y + plot.size.height / 2. - text.height / 2.
            } else {
                plot.origin.y + px(3. - insets.top)
            };
            let align = if right {
                TextAlign::Right
            } else {
                TextAlign::Left
            };
            text.paint_at(point(x, y), align, None, window, cx);
        }
        if let Some(anchor) = self.x_axis_anchor()
            && let Some(label) = self.axis_label(true)
        {
            let arrow = self.scales.x.label_arrow.unwrap_or(true) && continuous(&frame.x.scale);
            let forward = !self
                .scales
                .x
                .reverse
                .unwrap_or(self.frame_options.x_reverse);
            let text = if arrow {
                format!("{label} {}", if forward { "→" } else { "←" })
            } else {
                label.to_string()
            };
            let text = make(text, window);
            let top = anchor == AxisAnchor::Top;
            // A band scale centres its label, as Plot does; a continuous one
            // puts it at the axis's end.
            let center = self.scales.x.label_anchor == Some(crate::options::LabelAnchor::Center)
                || (self.scales.x.label_anchor.is_none() && !continuous(&frame.x.scale));
            let offset = self
                .scales
                .x
                .label_offset
                .unwrap_or(if top { insets.top } else { insets.bottom } - 3.);
            let y = if top {
                plot.origin.y - px(offset)
            } else {
                plot.origin.y + plot.size.height + px(offset) - text.height
            };
            let (x, align) = if center {
                (plot.origin.x + plot.size.width / 2., TextAlign::Center)
            } else {
                (
                    plot.origin.x + plot.size.width + px(insets.right - 3.),
                    TextAlign::Right,
                )
            };
            text.paint_at(point(x, y), align, None, window, cx);
        }
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
}

impl Plot {
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
        // Plot's ramp ticks: about one per 64 pixels, labelled below the bar.
        let count = (RAMP_WIDTH / 64.).round() as usize;
        let ticks = crate::ticks::linear_ticks([lo, hi], count, crate::scale::Transform::Linear);
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
        // The tip reads the data; a furniture mark reads one placeholder
        // record, which its constant channels read.
        assert_eq!(prepared[0].rows.len(), 2);
        assert_eq!(prepared[1].rows.len(), 1);
        assert_eq!(prepared[2].rows.len(), 1);
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

    /// A frame laid out over a plot's own marks, without a window.
    fn frame_for(plot: &Plot, facets: &[Vec<PreparedMark>]) -> Frame {
        let mut options = plot.frame_options.clone();
        if marks_use_band(facets, true) {
            options.x_kind = Some(ScaleKind::Band);
        }
        Frame::resolve(
            Bounds::new(point(px(0.), px(0.)), gpui::size(px(400.), px(200.))),
            &options,
            &plot.x_domain_values(facets),
            &plot.y_domain_values(facets),
        )
    }

    #[test]
    fn one_dimensional_marks_have_a_hover_point() {
        // A barcode's rows have no y; the hit-test must place them where the
        // painter does (mid-height) instead of dropping them.
        let data = [1., 2., 3.].map(|v| Table::empty().with("v", Value::Number(v)));
        let plot = Plot::barcode(data, "v");
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let frame = frame_for(&plot, &facets);
        let mark = &facets[0][0];
        for row in &mark.rows {
            assert!(render::basic::position_in(mark, &frame, row).is_some());
        }
    }

    #[test]
    fn a_band_bar_is_hoverable_across_its_width() {
        let data = ["a", "b", "c"].map(|x| {
            Table::empty()
                .with("x", Value::text(x))
                .with("y", Value::Number(4.))
        });
        let plot = Plot::new(data).bar_y(|mark| mark.x("x").y("y"));
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let frame = frame_for(&plot, &facets);
        let mark = &facets[0][0];
        let rect = interval_rect(mark, &mark.rows[0], &frame).expect("a bar has a body");
        let band = frame.x.scale.band_width();
        assert!(band > 12.);
        assert!((rect.size.width.as_f32() - band).abs() < 0.5);
    }

    #[test]
    fn a_dot_has_no_hover_body() {
        let data = [Table::empty()
            .with("x", Value::Number(1.))
            .with("y", Value::Number(1.))];
        let plot = Plot::new(data).dot(|mark| mark.x("x").y("y"));
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let frame = frame_for(&plot, &facets);
        let mark = &facets[0][0];
        assert!(interval_rect(mark, &mark.rows[0], &frame).is_none());
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

    #[test]
    fn the_color_option_colors_the_marks() {
        let data = [1., 5.].map(|v| {
            Table::empty()
                .with("x", Value::Number(v))
                .with("y", Value::Number(v))
                .with("v", Value::Number(v))
        });
        let ramp = ColorRange::Continuous(vec![gpui::black(), gpui::white()]);
        let plot = Plot::new(data)
            .dot(|mark| mark.x("x").y("y"))
            .color_by("v", ramp);
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let theme = PlotTheme::from_theme(&gpui_kit::component::Theme::default());
        let colors = plot.plot_colors(&theme, &facets);
        let mark = &facets[0][0];
        let (low, high) = (
            colors.row_fill(mark, 0, &mark.rows[0]),
            colors.row_fill(mark, 0, &mark.rows[1]),
        );
        assert_ne!(low, high, "the two ends of the domain differ in color");
        assert_ne!(low, theme.mark_color);
    }

    #[test]
    fn facet_grid_domains_come_from_the_data() {
        let data = [
            Table::empty()
                .with("r", Value::text("a"))
                .with("c", Value::text("x")),
            Table::empty()
                .with("r", Value::text("b"))
                .with("c", Value::text("x")),
            Table::empty()
                .with("r", Value::text("a"))
                .with("c", Value::text("y")),
        ];
        let plot = Plot::new(data).dot(|mark| mark.x("c")).fx("r").fy("c");
        let grid = plot.facet_grid();
        // Both domains are distinct values in the order they appear, so the
        // grid is the cross product: two columns by two rows.
        assert_eq!(grid.fx_labels(), vec!["a", "b"]);
        assert_eq!(grid.fy_labels(), vec!["x", "y"]);
        assert_eq!(grid.columns(), 2);
        assert_eq!(grid.rows(), 2);
        assert_eq!(grid.cells.len(), 4);
    }
}
