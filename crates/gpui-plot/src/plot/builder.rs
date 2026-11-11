//! The plot's constructors and its figure-level options: margins,
//! size, title, legend, axes, domains, insets, interaction and facets,
//! with the accessors that read them back.

use std::rc::Rc;

use gpui::{AnyElement, App, ElementId, SharedString, Window};

use crate::frame::{FrameOptions, ScaleKind};
use crate::mark::Mark;
use crate::plot::{Interaction, LegendPosition, Plot, PlotTitle, Scales};
use crate::scale::{ColorRange, Transform};
use crate::table::{Dataset, Table};

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
            width: None,
            projection: None,
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
            primed: Default::default(),
        }
    }

    /// An empty plot whose marks bring their own data, Plot's
    /// `Plot.plot({marks: […]})`.
    #[track_caller]
    #[allow(clippy::self_named_constructors)]
    pub fn plot() -> Self {
        Self::new(std::iter::empty())
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

    /// Plot's `projection`: a named projection (`"albers-usa"`,
    /// `"equal-earth"` …) or a configured [`crate::geo::GeoProjection`].
    pub fn projection(mut self, projection: impl Into<crate::geo::GeoProjection>) -> Self {
        self.projection = Some(projection.into());
        self
    }

    /// Plot's `width`.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The width the plot asked for with [`Plot::width`], if any.
    pub fn requested_width(&self) -> Option<f32> {
        self.width
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

    /// A copy of the plot's options and marks, without its caches.
    pub(super) fn clone_options(&self) -> Plot {
        Plot {
            data: self.data.clone(),
            marks: self.marks.clone(),
            frame_options: self.frame_options.clone(),
            title: self.title.clone(),
            legend: self.legend,
            legend_title: self.legend_title.clone(),
            grid: self.grid,
            x_axis: self.x_axis,
            y_axis: self.y_axis,
            scales: self.scales.clone(),
            margins: self.margins,
            no_axes: self.no_axes,
            no_labels: self.no_labels,
            width: self.width,
            projection: self.projection.clone(),
            aspect_ratio: self.aspect_ratio,
            caption: self.caption.clone(),
            x_label: self.x_label.clone(),
            y_label: self.y_label.clone(),
            interaction: self.interaction.clone(),
            id: self.id.clone(),
            height: self.height,
            tooltip_formatter: self.tooltip_formatter.clone(),
            color_range: self.color_range.clone(),
            color_field: self.color_field.clone(),
            fx: self.fx.clone(),
            fy: self.fy.clone(),
            facet_cache: None,
            facets_cache: None,
            colors_cache: None,
            hover: None,
            hover_progress: 0.,
            frame_cache: None,
            appear_progress: 1.,
            primed: Default::default(),
        }
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

    /// Plot's `Plot.auto(data, options)` with its full options: per-dimension
    /// reducers and zero, `size`, and `mark`.
    pub fn auto_with(
        data: impl IntoIterator<Item = Table>,
        options: crate::auto::AutoOptions,
    ) -> Self {
        let rows: Vec<Table> = data.into_iter().collect();
        crate::auto::build_with(&rows, &options)
    }

    /// A barcode: one quantitative field laid as a strip of dots along x,
    /// nothing binned or reduced — Plot's `auto` with a `reduce: null` y.
    pub fn barcode(data: impl IntoIterator<Item = Table>, field: &str) -> Self {
        let rows: Vec<Table> = data.into_iter().collect();
        Self::new(rows).dot_x(|mark| mark.x(field))
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

    /// Plot's `insetLeft`: the x scale's inset on the left, replacing `inset`.
    pub fn inset_left(mut self, inset: f32) -> Self {
        self.scales.x.inset_start = Some(inset);
        self
    }

    /// Plot's `insetRight`.
    pub fn inset_right(mut self, inset: f32) -> Self {
        self.scales.x.inset_end = Some(inset);
        self
    }

    /// Plot's `insetBottom`.
    pub fn inset_bottom(mut self, inset: f32) -> Self {
        self.scales.y.inset_start = Some(inset);
        self
    }

    /// Plot's `insetTop`.
    pub fn inset_top(mut self, inset: f32) -> Self {
        self.scales.y.inset_end = Some(inset);
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

    /// Plot's `label: null`: no axis labels at all.
    pub fn no_labels(mut self) -> Self {
        self.no_labels = true;
        self
    }
}
