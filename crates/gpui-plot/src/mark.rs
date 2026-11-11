//! Mark specifications: Plot's mark types and their channels.
//!
//! A mark is a description of how to draw the data — `dot`, `line`, `barY`,
//! `cell`, … — plus the channels it reads. Channels are resolved against the
//! data at render time, so a mark value is cheap to build and holds no
//! window state.

mod builder;
mod channels;
mod composite;
mod config;
mod interval;
mod kind;
mod options;
mod style;

use gpui::SharedString;

use crate::style::NumberChannel;
use crate::transform::{Anchor, Dodge, Mapper, Reducer, WindowReducer};

pub use builder::*;
pub use channels::Channels;
pub use composite::{expand_box, expand_tree, whisker_high, whisker_low};
pub(crate) use composite::{whisker_high_with, whisker_low_with};
pub use config::{
    Axis, BinConfig, DensitySpec, DomainSort, GroupConfig, GroupOutput, MapConfig, MapReducer,
    MapTarget, RowFilter, SelectConfig, ShiftConfig, SortConfig, StackConfig,
};
pub use interval::Interval;
pub use kind::{CrossAxis, MarkCategory, MarkFamily, MarkKind};
pub use style::{
    BlendMode, Curve, FrameAnchor, LineAnchor, MarkStyle, Marker, Sweep, TextAnchor, VectorAnchor,
};

/// A mark: its kind, channels and style.
///
/// The typed builders ([`DotMark`], [`LineMark`], …) wrap this value and add
/// the options that make sense for their kind; every builder reads and writes
/// the shared channels through [`Mark`].
#[derive(Clone, Debug)]
pub struct Mark {
    pub kind: MarkKind,
    /// The mark's own data, Plot's first argument to a mark: `Plot.dot(data,
    /// …)`. `None` reads the plot's data.
    pub data: Option<crate::table::Dataset>,
    /// Whether `z` was implied by a color field rather than given: Plot drops
    /// it when the field turns out quantitative.
    pub implied_z: bool,
    /// Plot's `hexbin` transform: its outputs and its `binWidth`, in pixels.
    pub hexbin: Option<(Vec<GroupOutput>, f32)>,
    /// Plot's `treeNode` (`false`) or `treeLink` (`true`) transform.
    pub tree: Option<(crate::hierarchy::TreeSpec, bool)>,
    /// A tree transform's `treeFilter`: `node:internal` or `node:external`.
    pub tree_filter: Option<SharedString>,
    /// A density mark's options: `bandwidth`, `thresholds` and `weight`.
    pub density: Option<DensitySpec>,
    /// A raster or contour mark's grid options.
    pub raster: Option<crate::raster::RasterSpec>,
    /// Keep only each `z` group's outliers on this axis (beyond 1.5 IQR), the
    /// dots of Plot's box mark.
    pub outliers: Option<Axis>,
    /// Plot's `filter` option: only the records it accepts are drawn.
    pub filter: Option<RowFilter>,
    /// The mark's horizontal facet channel, Plot's mark-level `fx` option.
    pub fx: Option<crate::table::Channel>,
    /// The mark's vertical facet channel, Plot's mark-level `fy` option.
    pub fy: Option<crate::table::Channel>,
    pub channels: Channels,
    pub style: MarkStyle,
    /// How the mark stacks its series.
    pub stack: Option<StackConfig>,
    /// Plot's `stackY`/`stackX` transforms, when the mark asks for one; the
    /// bar, area and rect marks stack implicitly otherwise.
    pub stack_spec: Option<crate::render::stack::StackSpec>,
    /// Plot's `normalizeX`/`normalizeY`: the axis and the basis.
    pub normalize_by: Option<(Axis, crate::render::stack::NormalizeBasis)>,
    /// An axis, grid or frame mark's options, Plot's `axisX({ticks, anchor,
    /// tickFormat, label, …})`; an axis mark's `x` or `y` channel (a
    /// constant) places it, Plot's `axisY({x: 0})`.
    pub axis: crate::options::ScaleOptions,
    /// How a text mark formats a non-text value, Plot's text `format`.
    pub text_format: Option<crate::options::TickFormat>,
    /// Plot's mark `sort` option for scale domains: `sort: {y: "x1"}` orders
    /// the y domain by each value's x1.
    pub domain_sort: Vec<DomainSort>,
    /// How the mark bins its data.
    pub bin: Option<BinConfig>,
    /// How the mark orders its data.
    pub sort: Option<SortConfig>,
    /// A reducer applied over the mark's rows, Plot's `reduce`.
    pub reduce: Option<Reducer>,
    /// How the mark groups its rows before drawing, Plot's `groupX`/`groupY`.
    pub group: Option<GroupConfig>,
    /// A window transform applied to the mark's value, Plot's `window`.
    pub window: Option<(usize, WindowReducer, Anchor)>,
    /// Plot's window `strict`: a window short of `k` values, or holding a
    /// missing one, gives no value.
    pub window_strict: bool,
    /// A numeric transform of the mark's value, Plot's `normalize`.
    pub normalize: bool,
    /// How the mark dodges its points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
    pub dodge: Option<Dodge>,
    /// A mark-level title, Plot's `title`.
    pub title: Option<SharedString>,
    /// Keep one row per group before drawing, Plot's `select`.
    pub select: Option<SelectConfig>,
    /// Write a computed value into a new field, Plot's `mapX`/`mapY`.
    pub map: Option<MapConfig>,
    /// The channels Plot's `map` transform rewrites, in order: `mapX`,
    /// `mapY` or `map({channel: mapper})`. Each entry reads the channels
    /// the entries before it wrote, as nested Plot transforms do.
    pub maps: Vec<(MapTarget, Mapper)>,
    /// Move a field by an offset, Plot's `shiftX`/`shiftY`.
    pub shift: Option<ShiftConfig>,
    /// Draw the rows in a shuffled order, Plot's `shuffle`.
    pub shuffle: Option<u64>,
    /// A vector's or spike's length per datum, Plot's `length` channel.
    pub length: Option<NumberChannel>,
    /// A vector's rotation per datum, Plot's `rotate` channel, in degrees.
    pub rotate: Option<NumberChannel>,
    /// How a date or number is bucketed into an interval, Plot's `interval`.
    pub interval: Option<Interval>,
    /// The image an `image` mark paints, shared across its rows.
    pub image: Option<std::sync::Arc<gpui::RenderImage>>,
    /// The projection a `geo` mark maps lon/lat through, Plot's `geo`.
    pub projection: Option<crate::projection::Projection>,
    /// A GeoJSON-like collection of rings a `geo` mark fills — each ring a
    /// closed loop of `(lon, lat)` degree points, Plot's way of drawing shapes.
    pub geo_geometry: Option<Vec<Vec<(f64, f64)>>>,
    /// Plot's `axisFx` (`X`) or `axisFy` (`Y`): a text mark that labels each
    /// facet with its value, which the plot fills in from the facet domain.
    pub facet_axis: Option<Axis>,
}

impl Mark {
    pub fn new(kind: MarkKind) -> Self {
        let mut style = MarkStyle::default();
        // Plot's own defaults for the marks that differ from the shared ones:
        // a hexgrid is a faint underlay, and a spike is a thin, translucent
        // triangle rising from its datum.
        if kind == MarkKind::Hexgrid {
            style.stroke_opacity = 0.1;
        }
        // Plot's Bollinger band fills at its `opacity`, 0.2 by default.
        if matches!(kind, MarkKind::BollingerX | MarkKind::BollingerY) {
            style.fill_opacity = 0.2;
        }
        if kind == MarkKind::Spike {
            style.stroke_width = 1.;
            style.fill_opacity = 0.3;
            style.anchor = VectorAnchor::Start;
        }
        // Plot leaves SVG's one-pixel stroke on its rules, ticks, links and
        // frames; only its lines, dots and the like take 1.5.
        if matches!(
            kind,
            MarkKind::RuleX
                | MarkKind::RuleY
                | MarkKind::TickX
                | MarkKind::TickY
                | MarkKind::Link
                | MarkKind::Frame
                | MarkKind::Rect
                | MarkKind::RectX
                | MarkKind::RectY
                | MarkKind::BarX
                | MarkKind::BarY
                | MarkKind::Cell
                | MarkKind::Hull
                | MarkKind::Geo
                | MarkKind::DelaunayLink
                | MarkKind::DelaunayMesh
                | MarkKind::Voronoi
                | MarkKind::VoronoiMesh
                | MarkKind::Contour
                | MarkKind::Density
        ) {
            style.stroke_width = 1.;
        }
        // Plot's grid marks draw in `currentColor` at a tenth of its opacity.
        if matches!(kind, MarkKind::GridX | MarkKind::GridY) {
            style.stroke_opacity = 0.1;
        }
        // The meshes are a faint underlay: Plot strokes them at 0.2 opacity so
        // the data drawn over them stays the figure's subject.
        if matches!(kind, MarkKind::DelaunayMesh | MarkKind::VoronoiMesh) {
            style.stroke_opacity = 0.2;
        }
        Self {
            kind,
            data: None,
            implied_z: false,
            hexbin: None,
            tree: None,
            tree_filter: None,
            density: None,
            raster: None,
            outliers: None,
            filter: None,
            fx: None,
            fy: None,
            channels: Channels::default(),
            style,
            stack: None,
            stack_spec: None,
            normalize_by: None,
            domain_sort: Vec::new(),
            text_format: None,
            axis: Default::default(),
            bin: None,
            sort: None,
            reduce: None,
            group: None,
            window: None,
            window_strict: false,
            normalize: false,
            dodge: None,
            title: None,
            select: None,
            map: None,
            maps: Vec::new(),
            shift: None,
            shuffle: None,
            length: None,
            rotate: None,
            interval: None,
            image: None,
            projection: None,
            geo_geometry: None,
            facet_axis: None,
        }
    }
}

impl Mark {
    /// A mark that draws dots.
    pub fn dot() -> DotMark {
        DotMark::new()
    }

    /// A mark that draws a line.
    pub fn line() -> LineMark {
        LineMark::new()
    }
}
