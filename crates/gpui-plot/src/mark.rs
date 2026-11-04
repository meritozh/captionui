//! Mark specifications: Plot's mark types and their channels.
//!
//! A mark is a description of how to draw the data — `dot`, `line`, `barY`,
//! `cell`, … — plus the channels it reads. Channels are resolved against the
//! data at render time, so a mark value is cheap to build and holds no
//! window state.

use gpui::{Hsla, SharedString};

use crate::style::{ColorChannel, NumberChannel, Symbol};
use crate::table::{Channel, IntoChannel};
use crate::transform::{
    Anchor, Dodge, GroupReducer, Reducer, Selector, StackOffset, Thresholds, WindowReducer,
};

/// The mark types this library draws, matching Plot's mark set.
///
/// # Adding a mark kind
///
/// A new kind is classified exactly once, in [`MarkKind::category`], and
/// drawn by one arm in its renderer family. That is the whole contract:
///
/// 1. add the variant here,
/// 2. give it a name in [`MarkKind::name`],
/// 3. classify it in [`MarkKind::category`] (the compiler makes this
///    exhaustive, so a forgotten kind is a compile error, not silence),
/// 4. pick a renderer by its [`MarkFamily`] in [`MarkCategory`], and draw it
///    with one arm in `render/basic.rs`, `render/radial.rs` or
///    `render/delaunay.rs`.
///
/// Every predicate (`is_bivariate`, `is_x_only`, …), the band-and-bin
/// decisions and the paint-family dispatch derive from the one [`category`]
/// table, so nothing else needs to change for a new kind.
///
/// [`category`]: MarkKind::category
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    /// A dot per datum, Plot's `dot`.
    Dot,
    /// A dot at each x with no y, Plot's `dotX`: a one-dimensional
    /// distribution, laid along the frame's middle.
    DotX,
    /// A dot at each y with no x, Plot's `dotY`.
    DotY,
    /// A line through the data, grouped by `z`, Plot's `line`.
    Line,
    /// A line at each x with no y, Plot's `lineX`.
    LineX,
    /// A line at each y with no x, Plot's `lineY`.
    LineY,
    /// A filled area under a line, Plot's `areaY`.
    AreaY,
    /// A filled area left of a line, Plot's `areaX`.
    AreaX,
    /// A vertical bar per datum, Plot's `barY`.
    BarY,
    /// A horizontal bar per datum, Plot's `barX`.
    BarX,
    /// A rectangle spanning two x values and two y values, Plot's `rect`.
    Rect,
    /// A rectangle over one x with no y, Plot's `rectX`: a horizontal
    /// histogram bar grown from zero.
    RectX,
    /// A rectangle over one y with no x, Plot's `rectY`.
    RectY,
    /// A rectangle filling a band cell, Plot's `cell`.
    Cell,
    /// A vertical line at each x, Plot's `ruleX`.
    RuleX,
    /// A horizontal line at each y, Plot's `ruleY`.
    RuleY,
    /// A text label per datum, Plot's `text`.
    Text,
    /// A short vertical tick per datum, Plot's `tickX`.
    TickX,
    /// A short horizontal tick per datum, Plot's `tickY`.
    TickY,
    /// An arrow from one point to another, Plot's `vector`.
    Vector,
    /// A box-and-whisker summary along x, Plot's `boxX`.
    BoxX,
    /// A box-and-whisker summary along y, Plot's `boxY`.
    BoxY,
    /// A kernel-density violin along x, Plot's `violinX`.
    ViolinX,
    /// A kernel-density violin along y, Plot's `violinY`.
    ViolinY,
    /// Hexagonal binning of two quantitative channels, Plot's `hexbin`.
    Hexbin,
    /// A kernel density estimate along x, Plot's `densityX`.
    DensityX,
    /// A kernel density estimate along y, Plot's `densityY`.
    DensityY,
    /// Contour lines over a raster, Plot's `contour`.
    Contour,
    /// A pie chart of one quantitative channel, Plot's `pie`.
    Pie,
    /// An arc chart, Plot's `arc`.
    Arc,
    /// A unit-square waffle chart, Plot's `waffle`.
    Waffle,
    /// A curved link between two points, Plot's `link`.
    Link,
    /// A pinned annotation at a point, Plot's `tip`: the hover tooltip drawn
    /// permanently at one datum.
    Tip,
    /// An x axis drawn as a mark, Plot's `axisX`, for a plot that wants its
    /// axes inside the frame or in a color of its own.
    AxisX,
    /// A y axis drawn as a mark, Plot's `axisY`.
    AxisY,
    /// Grid lines across x drawn as a mark, Plot's `gridX`.
    GridX,
    /// Grid lines across y drawn as a mark, Plot's `gridY`.
    GridY,
    /// The frame's outline drawn as a mark, Plot's `frame`.
    Frame,
    /// A text label at each x with no y, Plot's `textX`.
    TextX,
    /// A text label at each y with no x, Plot's `textY`.
    TextY,
    /// A cell over one x with no y, Plot's `cellX`.
    CellX,
    /// A cell over one y with no x, Plot's `cellY`.
    CellY,
    /// A vector at each x, Plot's `vectorX`.
    VectorX,
    /// A vector at each y, Plot's `vectorY`.
    VectorY,
    /// A triangular marker at a datum, Plot's `spike`.
    Spike,
    /// A curved arrow between two points, Plot's `arrow`.
    Arrow,
    /// A waffle bar along x, Plot's `waffleX`.
    WaffleX,
    /// A waffle bar along y, Plot's `waffleY`.
    WaffleY,
    /// A static hexagonal grid, Plot's `hexgrid`.
    Hexgrid,
    /// A grid of pixels colored from the data, Plot's `raster` and `image`.
    Raster,
    /// Two areas clipped to the sign of their difference, Plot's
    /// `differenceX`.
    DifferenceX,
    /// The y-axis form of `differenceX`, Plot's `differenceY`.
    DifferenceY,
    /// A least-squares fit of x on y, Plot's `linearRegressionX`.
    LinearRegressionX,
    /// A least-squares fit of y on x, Plot's `linearRegressionY`.
    LinearRegressionY,
    /// A moving mean with deviation bands along x, Plot's `bollingerX`.
    BollingerX,
    /// The y-axis form of `bollingerX`, Plot's `bollingerY`.
    BollingerY,
    /// A Delaunay triangulation drawn as links, Plot's `delaunayLink`.
    DelaunayLink,
    /// A Delaunay triangulation drawn as one mesh, Plot's `delaunayMesh`.
    DelaunayMesh,
    /// The convex hull of the data, Plot's `hull`.
    Hull,
    /// A Voronoi cell per datum, Plot's `voronoi`.
    Voronoi,
    /// The Voronoi mesh of the data, Plot's `voronoiMesh`.
    VoronoiMesh,
    /// A tree diagram of a slash-separated hierarchy, Plot's `tree`.
    Tree,
    /// A raster image painted at a position, Plot's `image`.
    Image,
}

/// Which renderer family paints a mark.
///
/// The primitive marks draw one shape per row; the radial and statistical
/// marks summarize or arrange their rows first; the Delaunay marks build a
/// lattice over the projected points. The family picks the [`render`] module
/// that draws the mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkFamily {
    /// The primitive row-shape marks: dot, line, area, bar, rect, cell, rule,
    /// text, tick, vector, spike, arrow, link, the waffle bars, hexgrid,
    /// raster and the regression marks.
    Basic,
    /// The radial and statistical marks: pie, arc, waffle, box, violin,
    /// density, hexbin and contour.
    Radial,
    /// The geometric marks over a projected point set: the Delaunay family.
    Delaunay,
}

/// The cross-cutting shape of a mark kind: every fact that is true of all of a
/// kind's marks, decided in one place.
///
/// A [`MarkKind`] sets this table once in [`MarkKind::category`], and every
/// predicate and dispatch that reads a kind's shape derives from it. Historically
/// these were six hand-maintained `matches!` whitelists across the crate, each
/// silently returning `false` for a new kind until remembered — a mark could
/// be misclassified without an error. Here a new kind is classified exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkCategory {
    /// Which renderer family paints the mark.
    pub family: MarkFamily,
    /// Whether the mark is static plot furniture — an axis, a grid or the
    /// frame — which reads no channels and resolves no rows.
    pub furniture: bool,
    /// Whether the mark reads two positional channels (`x` and `y`).
    pub bivariate: bool,
    /// Whether the mark grows from a baseline or angle on its own axis: a
    /// bar, a pie, an arc.
    pub length: bool,
    /// The axis a one-dimensional mark reads, or `None` if it reads both.
    pub single_axis: Option<Axis>,
    /// How a one-dimensional mark sits across the axis it does not read: in
    /// the frame's middle, or spread by the datum's index.
    pub cross: CrossAxis,
    /// Whether the mark fills the width of a band when the x scale is discrete.
    pub band_x: bool,
    /// Whether the mark fills the height of a band when the y scale is discrete.
    pub band_y: bool,
    /// Whether a binned mark counts along x (a vertical bin) rather than y.
    /// `None` for a mark with no fixed bin axis: the mark bins along whatever
    /// channel it names, the way Plot's `bin` infers it from the mark.
    pub bins_on_x: Option<bool>,
    /// Whether a stacked mark's categories run along y — a horizontal mark.
    pub groups_on_y: bool,
}

impl Default for MarkCategory {
    fn default() -> Self {
        Self {
            family: MarkFamily::Basic,
            furniture: false,
            bivariate: true,
            length: false,
            single_axis: None,
            cross: CrossAxis::Index,
            band_x: false,
            band_y: false,
            bins_on_x: None,
            groups_on_y: false,
        }
    }
}

impl MarkKind {
    /// The name Plot uses for this mark.
    pub fn name(self) -> &'static str {
        match self {
            MarkKind::Dot => "dot",
            MarkKind::DotX => "dotX",
            MarkKind::DotY => "dotY",
            MarkKind::Line => "line",
            MarkKind::LineX => "lineX",
            MarkKind::LineY => "lineY",
            MarkKind::AreaY => "areaY",
            MarkKind::AreaX => "areaX",
            MarkKind::BarY => "barY",
            MarkKind::BarX => "barX",
            MarkKind::Rect => "rect",
            MarkKind::RectX => "rectX",
            MarkKind::RectY => "rectY",
            MarkKind::Cell => "cell",
            MarkKind::RuleX => "ruleX",
            MarkKind::RuleY => "ruleY",
            MarkKind::Text => "text",
            MarkKind::TickX => "tickX",
            MarkKind::TickY => "tickY",
            MarkKind::Vector => "vector",
            MarkKind::BoxX => "boxX",
            MarkKind::BoxY => "boxY",
            MarkKind::ViolinX => "violinX",
            MarkKind::ViolinY => "violinY",
            MarkKind::Hexbin => "hexbin",
            MarkKind::DensityX => "densityX",
            MarkKind::DensityY => "densityY",
            MarkKind::Contour => "contour",
            MarkKind::Pie => "pie",
            MarkKind::Arc => "arc",
            MarkKind::Waffle => "waffle",
            MarkKind::Link => "link",
            MarkKind::Tip => "tip",
            MarkKind::AxisX => "axisX",
            MarkKind::AxisY => "axisY",
            MarkKind::GridX => "gridX",
            MarkKind::GridY => "gridY",
            MarkKind::Frame => "frame",
            MarkKind::TextX => "textX",
            MarkKind::TextY => "textY",
            MarkKind::CellX => "cellX",
            MarkKind::CellY => "cellY",
            MarkKind::VectorX => "vectorX",
            MarkKind::VectorY => "vectorY",
            MarkKind::Spike => "spike",
            MarkKind::Arrow => "arrow",
            MarkKind::WaffleX => "waffleX",
            MarkKind::WaffleY => "waffleY",
            MarkKind::Hexgrid => "hexgrid",
            MarkKind::Raster => "raster",
            MarkKind::DifferenceX => "differenceX",
            MarkKind::DifferenceY => "differenceY",
            MarkKind::LinearRegressionX => "linearRegressionX",
            MarkKind::LinearRegressionY => "linearRegressionY",
            MarkKind::BollingerX => "bollingerX",
            MarkKind::BollingerY => "bollingerY",
            MarkKind::DelaunayLink => "delaunayLink",
            MarkKind::DelaunayMesh => "delaunayMesh",
            MarkKind::Hull => "hull",
            MarkKind::Voronoi => "voronoi",
            MarkKind::VoronoiMesh => "voronoiMesh",
            MarkKind::Tree => "tree",
            MarkKind::Image => "image",
        }
    }

    /// The cross-cutting shape of this mark.
    ///
    /// This one table is the single authority for every fact about a kind's
    /// shape; the predicates below and the renderer's dispatch read from it
    /// rather than re-declaring `matches!` whitelists. A new kind is
    /// classified exactly once, here, instead of in half a dozen hand-maintained
    /// lists that each silently default to `false` for a kind they forget.
    pub fn category(self) -> MarkCategory {
        use MarkFamily::*;
        use MarkKind::*;

        // The defaults: a primitive mark reading both axes and drawing one
        // shape per row.
        let basic = MarkCategory::default();
        let radial = MarkCategory {
            family: Radial,
            ..Default::default()
        };
        let delaunay = MarkCategory {
            family: Delaunay,
            ..Default::default()
        };
        // Furniture draws no data, so it reads no channels and holds no rows.
        let furniture = MarkCategory {
            furniture: true,
            bivariate: false,
            ..Default::default()
        };

        // One-dimensional marks place themselves across the axis they do not
        // read. A one-dimensional *dot* or *line* still resolves both channels
        // (its `y`/`x` is the index or the frame's middle), so it stays
        // bivariate; a one-dimensional label, cell or vector is not.
        let axis = |single_axis, cross| MarkCategory {
            single_axis,
            cross,
            // DotX/DotY/LineX/LineY resolve a cross channel of their own, so
            // they stay bivariate; the callers below for labels and cells set
            // it false explicitly.
            ..Default::default()
        };
        let univariate = |single_axis, cross| MarkCategory {
            single_axis,
            cross,
            bivariate: false,
            ..Default::default()
        };
        let anchor = CrossAxis::Anchor;
        let index = CrossAxis::Index;

        match self {
            // Primitive row-shape marks: one shape per datum.
            Dot => basic,
            DotX => axis(Some(Axis::X), anchor),
            DotY => axis(Some(Axis::Y), anchor),
            Line => basic,
            LineX => axis(Some(Axis::X), index),
            LineY => axis(Some(Axis::Y), index),
            // A horizontal mark's categories run along y — the axis it stacks
            // on — so its groups sit on y and its values run along x.
            AreaX => MarkCategory {
                groups_on_y: true,
                ..basic
            },
            AreaY => basic,
            BarY => MarkCategory {
                length: true,
                band_x: true,
                bins_on_x: Some(true),
                ..basic
            },
            // A horizontal bar is a length mark along x and takes a y band.
            BarX => MarkCategory {
                length: true,
                band_y: true,
                groups_on_y: true,
                bins_on_x: Some(false),
                ..basic
            },
            // `rectX` and the horizontal waffle bar take a y band but are not
            // stacked by the mark stack transform, which recognizes only the
            // bar and area families.
            RectX => MarkCategory {
                single_axis: Some(Axis::X),
                band_y: true,
                bins_on_x: Some(false),
                ..basic
            },
            Rect => basic,
            // A one-axis rect grows from the baseline like a bar; `rectX` bins
            // along y (counts on x are a horizontal histogram) and `rectY` the
            // reverse.
            RectY => MarkCategory {
                single_axis: Some(Axis::Y),
                band_x: true,
                bins_on_x: Some(true),
                ..basic
            },
            Cell => MarkCategory {
                band_x: true,
                band_y: true,
                ..basic
            },
            RuleX | RuleY => basic,
            Text => basic,
            // A tick's band sits on the axis it lays against: a `tickX` draws
            // its ticks apart along y, a `tickY` along x.
            TickX => MarkCategory {
                band_y: true,
                ..basic
            },
            TickY => MarkCategory {
                band_x: true,
                ..basic
            },
            // A vector reads an (x, y) point; a spike or arrow reads two ends
            // of its own, so it resolves its own geometry.
            Vector => basic,
            Spike | Arrow => MarkCategory {
                bivariate: false,
                ..basic
            },
            // Radial and statistical marks. A pie, arc or waffle reads a length
            // (an angle or a count), not a second position channel, so it is
            // not bivariate.
            Pie | Arc | Waffle => MarkCategory {
                family: Radial,
                length: true,
                bivariate: false,
                ..Default::default()
            },
            BoxX | ViolinX | DensityX => MarkCategory {
                family: Radial,
                band_y: true,
                ..basic
            },
            BoxY | ViolinY | DensityY => MarkCategory {
                family: Radial,
                band_x: true,
                ..basic
            },
            Hexbin | Contour => radial,
            Link => basic,
            Tip => MarkCategory {
                bivariate: false,
                ..basic
            },
            AxisX | AxisY | GridX | GridY | Frame => furniture,
            // One-dimensional labels, cells and vectors read one channel only,
            // so they are not bivariate and sit in the frame's middle (a label
            // or vector) or spread by index (a cell).
            TextX => univariate(Some(Axis::X), anchor),
            TextY => univariate(Some(Axis::Y), anchor),
            CellX => univariate(Some(Axis::X), index),
            CellY => univariate(Some(Axis::Y), index),
            VectorX => univariate(Some(Axis::X), anchor),
            VectorY => univariate(Some(Axis::Y), anchor),
            // The oblique waffle bars draw through the basic renderer and take
            // a band, but are not a length mark in the `is_length_mark` sense.
            WaffleX => MarkCategory {
                band_y: true,
                bivariate: false,
                ..basic
            },
            WaffleY => MarkCategory {
                band_x: true,
                bivariate: false,
                ..basic
            },
            // The grid-like and fitted marks draw through the basic renderer
            // but summarize or arrange their rows rather than drawing one
            // shape per datum.
            Hexgrid | Raster | DifferenceX | DifferenceY | LinearRegressionX
            | LinearRegressionY | BollingerX | BollingerY => MarkCategory {
                bivariate: false,
                ..basic
            },
            // The Delaunay marks triangulate the projected points.
            DelaunayLink | DelaunayMesh | Hull | Voronoi | VoronoiMesh => delaunay,
            // The tree reads no positional channels: it lays its own hierarchy
            // out from a `path` field, so it is not bivariate and draws through
            // the basic renderer.
            Tree => MarkCategory {
                bivariate: false,
                ..Default::default()
            },
            // An image is positioned by its own width and height, not by two
            // positional channels, so it is not bivariate.
            Image => MarkCategory {
                bivariate: false,
                ..Default::default()
            },
        }
    }

    /// Whether this mark draws a part of the plot's furniture — an axis, a
    /// grid or the frame — rather than data, so it reads no channels.
    pub fn is_furniture(self) -> bool {
        self.category().furniture
    }

    /// Whether this mark reads two positional channels (`x` and `y`) rather
    /// than one axis plus a length.
    pub fn is_bivariate(self) -> bool {
        self.category().bivariate
    }

    /// Whether this mark reads a length channel on its own axis: a bar grows
    /// from a baseline, a pie takes an angle.
    pub fn is_length_mark(self) -> bool {
        self.category().length
    }

    /// Whether this mark lays its data out along the x axis only, with no y
    /// channel of its own: Plot's `dotX`, `lineX`, `rectX`.
    pub fn is_x_only(self) -> bool {
        self.category().single_axis == Some(Axis::X)
    }

    /// Whether this mark lays its data out along the y axis only.
    pub fn is_y_only(self) -> bool {
        self.category().single_axis == Some(Axis::Y)
    }

    /// How a mark with one position channel places itself across the axis it
    /// does not read.
    pub fn cross_axis(self) -> CrossAxis {
        self.category().cross
    }
}

/// Where a one-dimensional mark puts itself on the axis it does not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossAxis {
    /// The frame's middle, Plot's `dotX`.
    Anchor,
    /// The datum's index, Plot's `lineX` and `rectX`.
    Index,
}

/// How a vector sits against its datum, Plot's `anchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VectorAnchor {
    /// The vector starts at the datum.
    Start,
    /// The vector is centred on the datum. (Default.)
    #[default]
    Middle,
    /// The vector ends at the datum.
    End,
}

/// How a mark's data is stacked, Plot's `stack` option.
#[derive(Clone, Debug, PartialEq)]
pub struct StackConfig {
    /// Which offset the stack uses.
    pub offset: StackOffset,
    /// The field naming the series when the data is long, Plot's `z`.
    pub series_field: Option<SharedString>,
    /// The value fields to stack when the data is wide, one per series: Plot's
    /// `Plot.stackY({y1: "a", y2: "b"})` names its series as channels.
    pub fields: Vec<SharedString>,
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            offset: StackOffset::Zero,
            series_field: None,
            fields: Vec::new(),
        }
    }
}

/// How a mark's data is binned, Plot's `bin` option.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BinConfig {
    pub thresholds: Thresholds,
}

impl Default for BinConfig {
    fn default() -> Self {
        Self {
            thresholds: Thresholds::Auto,
        }
    }
}

/// How a mark groups its rows, Plot's `groupX`, `groupY` and `groupZ`.
///
/// A group collapses the rows that share a category into one row per
/// category, each output channel reduced over the rows that formed it. Plot
/// splits the work between the axis the group reads — `groupX` groups by x,
/// `groupY` by y, `groupZ` by `z` alone — and the other channels it carries
/// along.
#[derive(Clone, Debug)]
pub struct GroupConfig {
    /// The axis the groups read, or `None` for `groupZ`, which groups by the
    /// `z` channel alone.
    pub axis: Option<Axis>,
    /// The outputs: which channel each group's reduction lands in, and how.
    pub outputs: Vec<GroupOutput>,
}

/// One grouped output channel: a field or channel name and its reducer.
///
/// Plot names outputs by channel — `{y: "count", fill: "mean"}` — and the
/// reducer reads that channel's own input unless it names one. `"count"`
/// takes no input at all, and a reducer that needs a value falls back to the
/// channel it writes, so `{y: "mean"}` averages the y values.
#[derive(Clone, Debug)]
pub struct GroupOutput {
    /// The channel the reduced value lands in, Plot's output name: `"x"`,
    /// `"y"`, `"x1"`, `"y2"`, `"fill"`, or a field name.
    pub name: SharedString,
    /// How the group is reduced.
    pub reducer: GroupReducer,
    /// The field the reducer reads, when it differs from the output's own
    /// channel; Plot's `{y: {value: "v", reduce: "mean"}}`.
    pub input: Option<SharedString>,
}

impl GroupOutput {
    /// An output writing `name`, reduced with `reducer`, reading that channel.
    pub fn new(name: impl Into<SharedString>, reducer: GroupReducer) -> Self {
        Self {
            name: name.into(),
            reducer,
            input: None,
        }
    }

    /// Read a field other than the output's own channel.
    pub fn input(mut self, field: impl Into<SharedString>) -> Self {
        self.input = Some(field.into());
        self
    }
}

/// How a mark's data is ordered, Plot's `sort` option.
#[derive(Clone, Debug, Default)]
pub struct SortConfig {
    /// The field to sort by; `None` sorts by the mark's value.
    pub field: Option<SharedString>,
    /// Whether to sort descending.
    pub descending: bool,
    /// Plot's `reverse`, applied after sorting.
    pub reverse: bool,
}

/// The channels a mark reads, Plot's mark options.
#[derive(Clone, Default)]
pub struct Channels {
    /// The x position.
    pub x: Option<Channel>,
    /// The y position.
    pub y: Option<Channel>,
    /// The x interval's start, Plot's `x1`.
    pub x1: Option<Channel>,
    /// The x interval's end, Plot's `x2`.
    pub x2: Option<Channel>,
    /// The y interval's start, Plot's `y1`.
    pub y1: Option<Channel>,
    /// The y interval's end, Plot's `y2`.
    pub y2: Option<Channel>,
    /// The series channel, Plot's `z`.
    pub z: Option<Channel>,
    /// The fill color.
    pub fill: Option<ColorChannel>,
    /// The stroke color.
    pub stroke: Option<ColorChannel>,
    /// The radius of a dot.
    pub r: Option<NumberChannel>,
    /// A node's slash-separated path in a hierarchy, for the `tree` mark.
    pub path: Option<Channel>,
    /// The text of a label, Plot's `text`.
    pub text: Option<Channel>,
    /// The symbol of a dot, Plot's `symbol`.
    pub symbol: Option<Channel>,
    /// The opacity, Plot's `opacity`.
    pub opacity: Option<NumberChannel>,
    /// A datum's tooltip title.
    pub title: Option<Channel>,
    /// A datum's tooltip body.
    pub href: Option<Channel>,
}

impl std::fmt::Debug for Channels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channels")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish_non_exhaustive()
    }
}

impl Channels {
    /// The x channel a mark positions with: `x`, or `x1` when only an interval
    /// is given.
    pub fn x_channel(&self) -> Option<&Channel> {
        self.x.as_ref().or(self.x1.as_ref()).or(self.x2.as_ref())
    }

    /// The y channel a mark positions with: `y`, or `y1`/`y2`.
    pub fn y_channel(&self) -> Option<&Channel> {
        self.y.as_ref().or(self.y1.as_ref()).or(self.y2.as_ref())
    }
}

/// The visual defaults a mark falls back on when a channel is unset.
#[derive(Clone, Debug)]
pub struct MarkStyle {
    /// The fill used when the mark has no `fill` channel.
    pub fill: Hsla,
    /// The stroke used when the mark has no `stroke` channel.
    pub stroke: Hsla,
    /// The stroke width of a line, area edge or outline.
    pub stroke_width: f32,
    /// The radius of a dot, in pixels.
    pub radius: f32,
    /// The symbol of a dot.
    pub symbol: Symbol,
    /// How a line or area connects its points.
    pub curve: Curve,
    /// The opacity of the mark's fills, Plot's `fillOpacity`.
    pub fill_opacity: f32,
    /// The opacity of the mark's strokes, Plot's `strokeOpacity`.
    pub stroke_opacity: f32,
    /// The opacity of the mark as a whole, Plot's `opacity`.
    pub opacity: f32,
    /// The corner radius of a bar or rect.
    pub corner_radius: f32,
    /// The horizontal offset of a text label, Plot's `dx`, in pixels.
    pub dx: f32,
    /// The vertical offset of a text label, Plot's `dy`, in pixels.
    pub dy: f32,
    /// The bend of an arrow, in degrees, Plot's `bend`.
    pub bend: f32,
    /// The angle between an arrow's wings, in degrees, Plot's `headAngle`.
    pub head_angle: f32,
    /// The length of an arrow's wings, in pixels, Plot's `headLength`.
    pub head_length: f32,
    /// The length of a spike or vector, in pixels, Plot's `length`.
    pub length: f32,
    /// A vector's rotation, Plot's `rotate`, in degrees clockwise.
    pub rotate: f32,
    /// How a vector sits against its datum, Plot's `anchor`.
    pub anchor: VectorAnchor,
    /// The confidence level of a regression's band, Plot's `ci`; zero
    /// suppresses the band.
    pub ci: f32,
    /// The window a Bollinger band averages over, Plot's `n`.
    pub window: usize,
    /// The band's multiplier, Plot's `k`.
    pub k: f32,
    /// The width of a `hexgrid`'s hexagons, Plot's `binWidth`, in pixels.
    pub bin_width: f32,
    /// The value a `waffle` cell stands for, Plot's `unit`.
    pub unit: f32,
    /// The gap between a waffle's cells, Plot's `gap`, in pixels.
    pub gap: f32,
}

/// How a line or area connects its points, Plot's `curve`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    /// A smooth curve through every point, Plot's `natural`.
    #[default]
    Natural,
    /// Straight segments, Plot's `linear`.
    Linear,
    /// Straight segments that close back to the first point, Plot's
    /// `linear-closed`.
    LinearClosed,
    /// A step that holds each value until the next point, Plot's `step`.
    Step,
    /// A step centered on the point, Plot's `step-after`/`step-before` pair.
    StepAfter,
    /// A step that rises before the point.
    StepBefore,
    /// A cubic basis spline that smooths away the data's corners, Plot's
    /// `basis`.
    Basis,
    /// An open cubic basis spline.
    BasisOpen,
    /// A closed cubic basis spline.
    BasisClosed,
    /// A cardinal spline with a tension, Plot's `cardinal`.
    Cardinal,
    /// An open cardinal spline.
    CardinalOpen,
    /// A closed cardinal spline.
    CardinalClosed,
    /// A Catmull-Rom spline, Plot's `catmull-rom`.
    CatmullRom,
    /// An open Catmull-Rom spline.
    CatmullRomOpen,
    /// A closed Catmull-Rom spline.
    CatmullRomClosed,
    /// A cubic spline that preserves the data's monotonicity in x, Plot's
    /// `monotone-x`.
    MonotoneX,
    /// A cubic spline that preserves the data's monotonicity in y.
    MonotoneY,
    /// A Bézier curve whose tangents are horizontal, Plot's `bump-x`.
    BumpX,
    /// A Bézier curve whose tangents are vertical.
    BumpY,
    /// A straightened cubic basis spline, Plot's `bundle`.
    Bundle,
}

impl Curve {
    /// Whether the curve closes its path back to the first point.
    pub fn is_closed(self) -> bool {
        matches!(
            self,
            Curve::LinearClosed
                | Curve::BasisClosed
                | Curve::CardinalClosed
                | Curve::CatmullRomClosed
        )
    }

    /// Whether the curve is a spline that starts and ends on its end points.
    pub fn is_open(self) -> bool {
        matches!(
            self,
            Curve::BasisOpen | Curve::CardinalOpen | Curve::CatmullRomOpen
        )
    }
}

impl Default for MarkStyle {
    fn default() -> Self {
        Self {
            fill: Hsla {
                h: 0.55,
                s: 0.7,
                l: 0.55,
                a: 1.,
            },
            stroke: Hsla {
                h: 0.55,
                s: 0.7,
                l: 0.45,
                a: 1.,
            },
            stroke_width: 1.5,
            radius: 3.5,
            symbol: Symbol::Circle,
            curve: Curve::Natural,
            fill_opacity: 1.,
            stroke_opacity: 1.,
            opacity: 1.,
            corner_radius: 0.,
            dx: 0.,
            dy: 0.,
            bend: 0.,
            head_angle: 60.,
            head_length: 8.,
            length: 12.,
            rotate: 0.,
            anchor: VectorAnchor::Middle,
            ci: 0.95,
            window: 20,
            k: 2.,
            bin_width: 20.,
            unit: 1.,
            gap: 1.,
        }
    }
}

/// A mark: its kind, channels and style.
///
/// The typed builders ([`DotMark`], [`LineMark`], …) wrap this value and add
/// the options that make sense for their kind; every builder reads and writes
/// the shared channels through [`Mark`].
#[derive(Clone, Debug)]
pub struct Mark {
    pub kind: MarkKind,
    pub channels: Channels,
    pub style: MarkStyle,
    /// How the mark stacks its series.
    pub stack: Option<StackConfig>,
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
}

/// A time or numeric bucket, Plot's `interval` option: turn a singular value
/// into an extent `[floor, floor + step)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interval {
    /// Whole years.
    Year,
    /// Whole calendar months.
    Month,
    /// Whole days.
    Day,
    /// A fixed numeric step.
    Step(f64),
}

impl Interval {
    /// The extent this interval spans for a value, `(start, end)`, in the
    /// value's own units (dates as UTC instants, numbers as numbers).
    pub fn extent(self, value: &crate::value::Value) -> Option<(f64, f64)> {
        let instant = value.as_number()?;
        use chrono::{Datelike, Duration, TimeZone, Utc};
        let (start, end) = match self {
            Interval::Day => {
                let date = Utc.timestamp_millis_opt(instant as i64).single()?;
                let start = date
                    .date_naive()
                    .and_hms_opt(0, 0, 0)?
                    .and_utc()
                    .timestamp_millis();
                let end = start + Duration::days(1).num_milliseconds();
                (start as f64, end as f64)
            }
            Interval::Month => {
                let date = Utc.timestamp_millis_opt(instant as i64).single()?;
                // Reconstruct the first of the month directly from its
                // year and month, so a date like Jan 31 doesn't rely on
                // clamping `with_month`/`with_day` (which returns None when
                // the day doesn't fit the new month).
                let naive = date.date_naive();
                let start = Utc
                    .with_ymd_and_hms(naive.year(), naive.month(), 1, 0, 0, 0)
                    .single()?
                    .timestamp_millis();
                let (end_year, end_month) = if naive.month() == 12 {
                    (naive.year() + 1, 1)
                } else {
                    (naive.year(), naive.month() + 1)
                };
                let end = Utc
                    .with_ymd_and_hms(end_year, end_month, 1, 0, 0, 0)
                    .single()?
                    .timestamp_millis();
                (start as f64, end as f64)
            }
            Interval::Year => {
                let date = Utc.timestamp_millis_opt(instant as i64).single()?;
                let naive = date.date_naive();
                let start = Utc
                    .with_ymd_and_hms(naive.year(), 1, 1, 0, 0, 0)
                    .single()?
                    .timestamp_millis();
                let end = Utc
                    .with_ymd_and_hms(naive.year() + 1, 1, 1, 0, 0, 0)
                    .single()?
                    .timestamp_millis();
                (start as f64, end as f64)
            }
            Interval::Step(width) if width > 0. => {
                let start = (instant / width).floor() * width;
                (start, start + width)
            }
            Interval::Step(_) => return None,
        };
        Some((start, end))
    }
}

/// A [`Mark::select`] configuration, Plot's `select`.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectConfig {
    /// Which row of each group the mark keeps.
    pub selector: Selector,
    /// The field that forms the groups, Plot's `z`.
    pub group: Option<SharedString>,
    /// The axis the selector reads, for `selectMinX`/`selectMaxX`: `None`
    /// picks the first or last row rather than an extreme.
    pub axis: Option<Axis>,
}

/// Which axis a select's extreme reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// The x channel.
    X,
    /// The y channel.
    Y,
}

/// A [`Mark::map`] configuration, Plot's `mapX`/`mapY`.
#[derive(Clone, Debug, PartialEq)]
pub struct MapConfig {
    /// The channel the mapper reads: `"x"`, `"y"` or a field name.
    pub input: SharedString,
    /// The field the mapped value is written to.
    pub output: SharedString,
    /// How the series is folded: the running total, the running extreme, the
    /// running mean or the datum's rank.
    pub reducer: MapReducer,
    /// The field that restarts the fold, Plot's `z`.
    pub group: Option<SharedString>,
}

/// How [`MapConfig`] folds a series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapReducer {
    /// The running total.
    Sum,
    /// The smallest value so far.
    Min,
    /// The largest value so far.
    Max,
    /// The mean of every value so far.
    Mean,
    /// The datum's position in the series, from zero.
    Rank,
}

/// A [`Mark::shift`] configuration, Plot's `shiftX`/`shiftY`.
#[derive(Clone, Debug, PartialEq)]
pub struct ShiftConfig {
    /// The channel the shift reads: `"x"`, `"y"` or a field name.
    pub input: SharedString,
    /// The field the shifted value is written to.
    pub output: SharedString,
    /// How far to move it: a number of units, or days for a date.
    pub offset: f64,
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
        if kind == MarkKind::Spike {
            style.stroke_width = 1.;
            style.fill_opacity = 0.3;
            style.anchor = VectorAnchor::Start;
        }
        // The meshes are a faint underlay: Plot strokes them at 0.2 opacity so
        // the data drawn over them stays the figure's subject.
        if matches!(kind, MarkKind::DelaunayMesh | MarkKind::VoronoiMesh) {
            style.stroke_opacity = 0.2;
        }
        Self {
            kind,
            channels: Channels::default(),
            style,
            stack: None,
            bin: None,
            sort: None,
            reduce: None,
            group: None,
            window: None,
            normalize: false,
            dodge: None,
            title: None,
            select: None,
            map: None,
            shift: None,
            shuffle: None,
            length: None,
            rotate: None,
            interval: None,
            image: None,
        }
    }

    /// Set the x channel.
    pub fn x(mut self, channel: impl IntoChannel) -> Self {
        self.channels.x = Some(channel.into_channel());
        self
    }

    /// Set the y channel.
    pub fn y(mut self, channel: impl IntoChannel) -> Self {
        self.channels.y = Some(channel.into_channel());
        self
    }

    /// Set the x interval's start.
    pub fn x1(mut self, channel: impl IntoChannel) -> Self {
        self.channels.x1 = Some(channel.into_channel());
        self
    }

    /// Set the x interval's end.
    pub fn x2(mut self, channel: impl IntoChannel) -> Self {
        self.channels.x2 = Some(channel.into_channel());
        self
    }

    /// Set the y interval's start.
    pub fn y1(mut self, channel: impl IntoChannel) -> Self {
        self.channels.y1 = Some(channel.into_channel());
        self
    }

    /// Set the y interval's end.
    pub fn y2(mut self, channel: impl IntoChannel) -> Self {
        self.channels.y2 = Some(channel.into_channel());
        self
    }

    /// Set the series channel.
    pub fn z(mut self, channel: impl IntoChannel) -> Self {
        self.channels.z = Some(channel.into_channel());
        self
    }

    /// Set the fill.
    pub fn fill(mut self, color: impl Into<ColorChannel>) -> Self {
        self.channels.fill = Some(color.into());
        self
    }

    /// Set the stroke.
    pub fn stroke(mut self, color: impl Into<ColorChannel>) -> Self {
        self.channels.stroke = Some(color.into());
        self
    }

    /// Set the radius of a dot, in pixels.
    pub fn r(mut self, radius: impl Into<NumberChannel>) -> Self {
        self.channels.r = Some(radius.into());
        self
    }

    /// Set a node's hierarchy path, for the `tree` mark.
    pub fn path(mut self, channel: impl IntoChannel) -> Self {
        self.channels.path = Some(channel.into_channel());
        self
    }

    /// Set the text of a label.
    pub fn text(mut self, channel: impl IntoChannel) -> Self {
        self.channels.text = Some(channel.into_channel());
        self
    }

    /// Set the symbol of a dot.
    pub fn symbol(mut self, channel: impl IntoChannel) -> Self {
        self.channels.symbol = Some(channel.into_channel());
        self
    }

    /// Set the opacity.
    pub fn opacity(mut self, opacity: impl Into<NumberChannel>) -> Self {
        self.channels.opacity = Some(opacity.into());
        self
    }

    /// Set the opacity of the mark's fills, Plot's `fillOpacity`.
    pub fn fill_opacity(mut self, opacity: f32) -> Self {
        self.style.fill_opacity = opacity;
        self
    }

    /// Set the opacity of the mark's strokes, Plot's `strokeOpacity`.
    pub fn stroke_opacity(mut self, opacity: f32) -> Self {
        self.style.stroke_opacity = opacity;
        self
    }

    /// Set a datum's tooltip title.
    pub fn tip_title(mut self, channel: impl IntoChannel) -> Self {
        self.channels.title = Some(channel.into_channel());
        self
    }

    /// Set the mark's own title, shown in its tooltip and legend.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the stroke width.
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.style.stroke_width = width;
        self
    }

    /// Set how the mark connects its points.
    pub fn curve(mut self, curve: Curve) -> Self {
        self.style.curve = curve;
        self
    }

    /// Round a bar's or rect's corners.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.style.corner_radius = radius;
        self
    }

    /// Offset a text label horizontally, Plot's `dx`.
    pub fn dx(mut self, dx: f32) -> Self {
        self.style.dx = dx;
        self
    }

    /// Offset a text label vertically, Plot's `dy`.
    pub fn dy(mut self, dy: f32) -> Self {
        self.style.dy = dy;
        self
    }

    /// Stack the mark's series.
    pub fn stack(mut self, offset: StackOffset) -> Self {
        let existing = self.stack.unwrap_or_default();
        self.stack = Some(StackConfig { offset, ..existing });
        self
    }

    /// Name the field that separates the mark's series when stacking long
    /// data, Plot's `z`.
    pub fn stack_series(mut self, field: impl Into<SharedString>) -> Self {
        let existing = self.stack.unwrap_or_default();
        self.stack = Some(StackConfig {
            series_field: Some(field.into()),
            ..existing
        });
        self
    }

    /// Name the value fields to stack when the data is wide, one per series:
    /// Plot's `Plot.stackY({y1: "a", y2: "b"})`.
    pub fn stack_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SharedString>,
    {
        let existing = self.stack.unwrap_or_default();
        self.stack = Some(StackConfig {
            fields: fields.into_iter().map(Into::into).collect(),
            ..existing
        });
        self
    }

    /// Bin the mark's data.
    pub fn bin(mut self, thresholds: Thresholds) -> Self {
        self.bin = Some(BinConfig { thresholds });
        self
    }

    /// Bucket a date or number into an interval, Plot's `interval`.
    ///
    /// A mark with a singular `x` (or `y`) value and an interval replaces it
    /// with an x interval spanning its bucket: a `Day` on a date turns each
    /// datum into `[midnight, next midnight)`, so a `rectY` draws one bar per
    /// day. A `Month`/`Year` bucket the calendar; a `Step(n)` buckets numbers
    /// by a fixed width.
    pub fn interval(mut self, interval: Interval) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Set the image an `image` mark paints, shared across its rows.
    pub fn image(mut self, image: std::sync::Arc<gpui::RenderImage>) -> Self {
        self.image = Some(image);
        self
    }

    /// Sort the mark's rows.
    pub fn sort(mut self, field: impl Into<SharedString>, descending: bool) -> Self {
        self.sort = Some(SortConfig {
            field: Some(field.into()),
            descending,
            reverse: false,
        });
        self
    }

    /// Reverse the mark's row order, Plot's `reverse`.
    pub fn reverse(mut self) -> Self {
        let mut config = self.sort.unwrap_or_default();
        config.reverse = !config.reverse;
        self.sort = Some(config);
        self
    }

    /// Reduce the mark's rows with a reducer, Plot's `reduce`.
    pub fn reduce(mut self, reducer: Reducer) -> Self {
        self.reduce = Some(reducer);
        self
    }

    /// Group the mark's rows by x and reduce them, Plot's `groupX`.
    ///
    /// Plot's default output is a count, which is what turns a scatter's rows
    /// into a histogram: every x value becomes one row whose y is how many
    /// readings shared it.
    pub fn group_x(mut self) -> Self {
        self.group = Some(GroupConfig {
            axis: Some(Axis::X),
            outputs: vec![GroupOutput::new("y", GroupReducer::Count)],
        });
        self
    }

    /// Group the mark's rows by y and reduce them, Plot's `groupY`.
    pub fn group_y(mut self) -> Self {
        self.group = Some(GroupConfig {
            axis: Some(Axis::Y),
            outputs: vec![GroupOutput::new("x", GroupReducer::Count)],
        });
        self
    }

    /// Group the mark's rows by its `z` channel and reduce them, Plot's
    /// `groupZ`. Without a `z` the whole dataset is one group.
    pub fn group_z(mut self) -> Self {
        self.group = Some(GroupConfig {
            axis: None,
            outputs: vec![GroupOutput::new("fill", GroupReducer::Count)],
        });
        self
    }

    /// Reduce a named channel over the mark's groups, Plot's
    /// `groupX({y: "mean"}, …)`. The reducer reads the channel it writes
    /// unless [`GroupOutput::input`] names another field.
    pub fn group_output(mut self, name: impl Into<SharedString>, reducer: GroupReducer) -> Self {
        let output = GroupOutput::new(name, reducer);
        match &mut self.group {
            Some(config) => config.outputs.push(output),
            None => {
                self.group = Some(GroupConfig {
                    axis: Some(Axis::X),
                    outputs: vec![output],
                });
            }
        }
        self
    }

    /// Apply a window transform to the mark's value, Plot's `window`.
    pub fn window(mut self, k: usize, reducer: WindowReducer, anchor: Anchor) -> Self {
        self.window = Some((k, reducer, anchor));
        self
    }

    /// Normalize the mark's value across the data, Plot's `normalize`.
    pub fn normalize(mut self) -> Self {
        self.normalize = true;
        self
    }

    /// Pack the mark's points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
    pub fn dodge(mut self, dodge: Dodge) -> Self {
        self.dodge = Some(dodge);
        self
    }

    /// Keep only the first, last or extreme row of each group, Plot's
    /// `select`. A mark that draws one labelled point per series uses it.
    pub fn select(mut self, selector: Selector, group: Option<impl Into<SharedString>>) -> Self {
        let axis = match &selector {
            Selector::Min(field) | Selector::Max(field) if &**field == "x" => Some(Axis::X),
            Selector::Min(field) | Selector::Max(field) if &**field == "y" => Some(Axis::Y),
            _ => None,
        };
        self.select = Some(SelectConfig {
            selector,
            group: group.map(Into::into),
            axis,
        });
        self
    }

    /// Fold the mark's value into a new field, Plot's `mapX`/`mapY`.
    pub fn map(
        mut self,
        input: impl Into<SharedString>,
        output: impl Into<SharedString>,
        reducer: MapReducer,
    ) -> Self {
        self.map = Some(MapConfig {
            input: input.into(),
            output: output.into(),
            reducer,
            group: None,
        });
        self
    }

    /// Restart the mark's fold at each series, Plot's `mapX` with a `z`.
    pub fn map_group(mut self, group: impl Into<SharedString>) -> Self {
        if let Some(map) = &mut self.map {
            map.group = Some(group.into());
        }
        self
    }

    /// Move the mark's value by an offset, Plot's `shiftX`/`shiftY`.
    pub fn shift(
        mut self,
        input: impl Into<SharedString>,
        output: impl Into<SharedString>,
        offset: f64,
    ) -> Self {
        self.shift = Some(ShiftConfig {
            input: input.into(),
            output: output.into(),
            offset,
        });
        self
    }

    /// Draw the rows in a shuffled order, Plot's `shuffle`.
    pub fn shuffle(mut self, seed: u64) -> Self {
        self.shuffle = Some(seed);
        self
    }

    /// Curve an arrow by `bend` degrees, Plot's `bend`.
    pub fn bend(mut self, degrees: f32) -> Self {
        self.style.bend = degrees;
        self
    }

    /// Set the angle between an arrow's wings, Plot's `headAngle`.
    pub fn head_angle(mut self, degrees: f32) -> Self {
        self.style.head_angle = degrees;
        self
    }

    /// Set the length of an arrow's wings, Plot's `headLength`.
    pub fn head_length(mut self, pixels: f32) -> Self {
        self.style.head_length = pixels;
        self
    }

    /// Set a spike's or vector's length, Plot's `length`.
    pub fn length(mut self, pixels: impl Into<NumberChannel>) -> Self {
        self.length = Some(pixels.into());
        self
    }

    /// Set a vector's rotation, Plot's `rotate`, in degrees clockwise.
    pub fn rotate(mut self, degrees: impl Into<NumberChannel>) -> Self {
        self.rotate = Some(degrees.into());
        self
    }

    /// Set how a vector is anchored at its datum, Plot's `anchor`.
    pub fn anchor(mut self, anchor: VectorAnchor) -> Self {
        self.style.anchor = anchor;
        self
    }

    /// Set a regression's confidence level; zero draws the line alone.
    pub fn ci(mut self, level: f32) -> Self {
        self.style.ci = level;
        self
    }

    /// Set a Bollinger band's window and multiplier.
    pub fn bollinger(mut self, window: usize, k: f32) -> Self {
        self.style.window = window;
        self.style.k = k;
        self
    }

    /// Set a hexgrid's hexagon width, Plot's `binWidth`.
    pub fn bin_width(mut self, pixels: f32) -> Self {
        self.style.bin_width = pixels;
        self
    }

    /// Set the value one waffle cell stands for, Plot's `unit`.
    pub fn unit(mut self, unit: f32) -> Self {
        self.style.unit = unit;
        self
    }

    /// Set the gap between a waffle's cells, Plot's `gap`.
    pub fn gap(mut self, pixels: f32) -> Self {
        self.style.gap = pixels;
        self
    }
}

/// Generates a typed mark builder over [`Mark`].
macro_rules! mark_builder {
    ($(#[$meta:meta])* $name:ident, $kind:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug)]
        pub struct $name(pub Mark);

        impl $name {
            /// A mark of this kind with default channels and style.
            pub fn new() -> Self {
                Self(Mark::new($kind))
            }

            /// Wrap an already-configured [`Mark`].
            pub fn from_mark(mark: Mark) -> Self {
                Self(mark)
            }

            /// The mark this builder configures.
            pub fn into_mark(self) -> Mark {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        // Reads go through `Deref` so a builder's channels and style can be
        // inspected; the builder methods are inherent and consuming, so a
        // chain returns the same builder type.
        impl std::ops::Deref for $name {
            type Target = Mark;

            fn deref(&self) -> &Mark {
                &self.0
            }
        }

        impl From<$name> for Mark {
            fn from(builder: $name) -> Mark {
                builder.0
            }
        }

        impl From<Mark> for $name {
            fn from(mark: Mark) -> Self {
                Self(mark)
            }
        }

        impl $name {
            /// Set the x channel.
            pub fn x(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.x(channel);
                self
            }

            /// Set the y channel.
            pub fn y(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.y(channel);
                self
            }

            /// Set the x interval's start.
            pub fn x1(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.x1(channel);
                self
            }

            /// Set the x interval's end.
            pub fn x2(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.x2(channel);
                self
            }

            /// Set the y interval's start.
            pub fn y1(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.y1(channel);
                self
            }

            /// Set the y interval's end.
            pub fn y2(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.y2(channel);
                self
            }

            /// Set the series channel, Plot's `z`.
            pub fn z(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.z(channel);
                self
            }

            /// Set the fill.
            pub fn fill(mut self, color: impl Into<ColorChannel>) -> Self {
                self.0 = self.0.fill(color);
                self
            }

            /// Set the stroke.
            pub fn stroke(mut self, color: impl Into<ColorChannel>) -> Self {
                self.0 = self.0.stroke(color);
                self
            }

            /// Set the radius of a dot, in pixels.
            pub fn r(mut self, radius: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.r(radius);
                self
            }

            /// Set a node's hierarchy path, for the `tree` mark.
            pub fn path(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.path(channel);
                self
            }

            /// Set the text of a label.
            pub fn text(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.text(channel);
                self
            }

            /// Set the symbol of a dot.
            pub fn symbol(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.symbol(channel);
                self
            }

            /// Set the opacity.
            pub fn opacity(mut self, opacity: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.opacity(opacity);
                self
            }

            /// Set the opacity of the mark's fills, Plot's `fillOpacity`.
            pub fn fill_opacity(mut self, opacity: f32) -> Self {
                self.0 = self.0.fill_opacity(opacity);
                self
            }

            /// Set the opacity of the mark's strokes, Plot's `strokeOpacity`.
            pub fn stroke_opacity(mut self, opacity: f32) -> Self {
                self.0 = self.0.stroke_opacity(opacity);
                self
            }

            /// Set a datum's tooltip title.
            pub fn tip_title(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.tip_title(channel);
                self
            }

            /// Set the mark's own title.
            pub fn title(mut self, title: impl Into<SharedString>) -> Self {
                self.0 = self.0.title(title);
                self
            }

            /// Set the stroke width.
            pub fn stroke_width(mut self, width: f32) -> Self {
                self.0 = self.0.stroke_width(width);
                self
            }

            /// Set how the mark connects its points.
            pub fn curve(mut self, curve: Curve) -> Self {
                self.0 = self.0.curve(curve);
                self
            }

            /// Round a bar's or rect's corners.
            pub fn corner_radius(mut self, radius: f32) -> Self {
                self.0 = self.0.corner_radius(radius);
                self
            }

            /// Offset a text label horizontally, Plot's `dx`.
            pub fn dx(mut self, dx: f32) -> Self {
                self.0 = self.0.dx(dx);
                self
            }

            /// Offset a text label vertically, Plot's `dy`.
            pub fn dy(mut self, dy: f32) -> Self {
                self.0 = self.0.dy(dy);
                self
            }

            /// Stack the mark's series.
            pub fn stack(mut self, offset: StackOffset) -> Self {
                self.0 = self.0.stack(offset);
                self
            }

            /// Name the field that separates the mark's series.
            pub fn stack_series(mut self, field: impl Into<SharedString>) -> Self {
                self.0 = self.0.stack_series(field);
                self
            }

            /// Name the value fields to stack, one per series.
            pub fn stack_fields<I, S>(mut self, fields: I) -> Self
            where
                I: IntoIterator<Item = S>,
                S: Into<SharedString>,
            {
                self.0 = self.0.stack_fields(fields);
                self
            }

            /// Bin the mark's data.
            pub fn bin(mut self, thresholds: Thresholds) -> Self {
                self.0 = self.0.bin(thresholds);
                self
            }

            /// Bucket a date or number into an interval, Plot's `interval`.
            pub fn interval(mut self, interval: Interval) -> Self {
                self.0 = self.0.interval(interval);
                self
            }

            /// Set the image an `image` mark paints, shared across its rows.
            pub fn image(mut self, image: std::sync::Arc<gpui::RenderImage>) -> Self {
                self.0 = self.0.image(image);
                self
            }

            /// Sort the mark's rows by a field.
            pub fn sort(mut self, field: impl Into<SharedString>, descending: bool) -> Self {
                self.0 = self.0.sort(field, descending);
                self
            }

            /// Reverse the mark's row order.
            pub fn reverse(mut self) -> Self {
                self.0 = self.0.reverse();
                self
            }

            /// Reduce the mark's rows.
            pub fn reduce(mut self, reducer: Reducer) -> Self {
                self.0 = self.0.reduce(reducer);
                self
            }

            /// Group the mark's rows by x, Plot's `groupX`.
            pub fn group_x(mut self) -> Self {
                self.0 = self.0.group_x();
                self
            }

            /// Group the mark's rows by y, Plot's `groupY`.
            pub fn group_y(mut self) -> Self {
                self.0 = self.0.group_y();
                self
            }

            /// Group the mark's rows by its `z` channel, Plot's `groupZ`.
            pub fn group_z(mut self) -> Self {
                self.0 = self.0.group_z();
                self
            }

            /// Reduce a named channel over the mark's groups.
            pub fn group_output(
                mut self,
                name: impl Into<SharedString>,
                reducer: GroupReducer,
            ) -> Self {
                self.0 = self.0.group_output(name, reducer);
                self
            }

            /// Apply a window transform to the mark's value.
            pub fn window(mut self, k: usize, reducer: WindowReducer, anchor: Anchor) -> Self {
                self.0 = self.0.window(k, reducer, anchor);
                self
            }

            /// Normalize the mark's value.
            pub fn normalize(mut self) -> Self {
                self.0 = self.0.normalize();
                self
            }

            /// Pack the mark's points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
            pub fn dodge(mut self, dodge: Dodge) -> Self {
                self.0 = self.0.dodge(dodge);
                self
            }

            /// Keep one row per group, Plot's `select`.
            pub fn select(
                mut self,
                selector: Selector,
                group: Option<impl Into<SharedString>>,
            ) -> Self {
                self.0 = self.0.select(selector, group);
                self
            }

            /// Fold the mark's value into a new field, Plot's `mapX`/`mapY`.
            pub fn map(
                mut self,
                input: impl Into<SharedString>,
                output: impl Into<SharedString>,
                reducer: MapReducer,
            ) -> Self {
                self.0 = self.0.map(input, output, reducer);
                self
            }

            /// Restart the mark's fold at each series.
            pub fn map_group(mut self, group: impl Into<SharedString>) -> Self {
                self.0 = self.0.map_group(group);
                self
            }

            /// Move the mark's value by an offset, Plot's `shiftX`/`shiftY`.
            pub fn shift(
                mut self,
                input: impl Into<SharedString>,
                output: impl Into<SharedString>,
                offset: f64,
            ) -> Self {
                self.0 = self.0.shift(input, output, offset);
                self
            }

            /// Draw the rows in a shuffled order, Plot's `shuffle`.
            pub fn shuffle(mut self, seed: u64) -> Self {
                self.0 = self.0.shuffle(seed);
                self
            }

            /// Curve an arrow by `bend` degrees, Plot's `bend`.
            pub fn bend(mut self, degrees: f32) -> Self {
                self.0 = self.0.bend(degrees);
                self
            }

            /// Set the angle between an arrow's wings, Plot's `headAngle`.
            pub fn head_angle(mut self, degrees: f32) -> Self {
                self.0 = self.0.head_angle(degrees);
                self
            }

            /// Set the length of an arrow's wings, Plot's `headLength`.
            pub fn head_length(mut self, pixels: f32) -> Self {
                self.0 = self.0.head_length(pixels);
                self
            }

            /// Set a spike's or vector's length, Plot's `length`.
            pub fn length(mut self, pixels: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.length(pixels);
                self
            }

            /// Set a vector's rotation, Plot's `rotate`, in degrees clockwise.
            pub fn rotate(mut self, degrees: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.rotate(degrees);
                self
            }

            /// Set how a vector is anchored at its datum, Plot's `anchor`.
            pub fn anchor(mut self, anchor: VectorAnchor) -> Self {
                self.0 = self.0.anchor(anchor);
                self
            }

            /// Set a regression's confidence level; zero draws the line alone.
            pub fn ci(mut self, level: f32) -> Self {
                self.0 = self.0.ci(level);
                self
            }

            /// Set a Bollinger band's window and multiplier.
            pub fn bollinger(mut self, window: usize, k: f32) -> Self {
                self.0 = self.0.bollinger(window, k);
                self
            }

            /// Set a hexgrid's hexagon width, Plot's `binWidth`.
            pub fn bin_width(mut self, pixels: f32) -> Self {
                self.0 = self.0.bin_width(pixels);
                self
            }

            /// Set the value one waffle cell stands for, Plot's `unit`.
            pub fn unit(mut self, unit: f32) -> Self {
                self.0 = self.0.unit(unit);
                self
            }

            /// Set the gap between a waffle's cells, Plot's `gap`.
            pub fn gap(mut self, pixels: f32) -> Self {
                self.0 = self.0.gap(pixels);
                self
            }
        }
    };
}

mark_builder!(
    /// A dot mark, Plot's `dot`: one symbol per datum.
    ///
    /// ```
    /// # use gpui_plot::mark::DotMark;
    /// DotMark::new().x("date").y("value").r(4.);
    /// ```
    DotMark,
    MarkKind::Dot
);
mark_builder!(
    /// A dot at each x with no y, Plot's `dotX`.
    DotXMark,
    MarkKind::DotX
);
mark_builder!(
    /// A dot at each y with no x, Plot's `dotY`.
    DotYMark,
    MarkKind::DotY
);
mark_builder!(
    /// A line mark, Plot's `line`: a polyline through the data, one per series.
    LineMark,
    MarkKind::Line
);
mark_builder!(
    /// A line at each x with no y, Plot's `lineX`.
    LineXMark,
    MarkKind::LineX
);
mark_builder!(
    /// A line at each y with no x, Plot's `lineY`.
    LineYMark,
    MarkKind::LineY
);
mark_builder!(
    /// A vertical area mark, Plot's `areaY`.
    AreaYMark,
    MarkKind::AreaY
);
mark_builder!(
    /// A horizontal area mark, Plot's `areaX`.
    AreaXMark,
    MarkKind::AreaX
);
mark_builder!(
    /// A vertical bar mark, Plot's `barY`.
    BarYMark,
    MarkKind::BarY
);
mark_builder!(
    /// A horizontal bar mark, Plot's `barX`.
    BarXMark,
    MarkKind::BarX
);
mark_builder!(
    /// A rectangle mark spanning two intervals, Plot's `rect`.
    RectMark,
    MarkKind::Rect
);
mark_builder!(
    /// A rectangle over one x, Plot's `rectX`: a horizontal histogram bar.
    RectXMark,
    MarkKind::RectX
);
mark_builder!(
    /// A rectangle over one y, Plot's `rectY`.
    RectYMark,
    MarkKind::RectY
);
mark_builder!(
    /// A cell mark filling one band of each axis, Plot's `cell`.
    CellMark,
    MarkKind::Cell
);
mark_builder!(
    /// A vertical rule at each x, Plot's `ruleX`.
    RuleXMark,
    MarkKind::RuleX
);
mark_builder!(
    /// A horizontal rule at each y, Plot's `ruleY`.
    RuleYMark,
    MarkKind::RuleY
);
mark_builder!(
    /// A text label per datum, Plot's `text`.
    TextMark,
    MarkKind::Text
);
mark_builder!(
    /// A vertical tick per datum, Plot's `tickX`.
    TickXMark,
    MarkKind::TickX
);
mark_builder!(
    /// A horizontal tick per datum, Plot's `tickY`.
    TickYMark,
    MarkKind::TickY
);
mark_builder!(
    /// An arrow between two points, Plot's `vector`.
    VectorMark,
    MarkKind::Vector
);
mark_builder!(
    /// A box-and-whisker summary along x, Plot's `boxX`.
    BoxXMark,
    MarkKind::BoxX
);
mark_builder!(
    /// A box-and-whisker summary along y, Plot's `boxY`.
    BoxYMark,
    MarkKind::BoxY
);
mark_builder!(
    /// A density violin along x, Plot's `violinX`.
    ViolinXMark,
    MarkKind::ViolinX
);
mark_builder!(
    /// A density violin along y, Plot's `violinY`.
    ViolinYMark,
    MarkKind::ViolinY
);
mark_builder!(
    /// A hexagonal binning mark, Plot's `hexbin`.
    HexbinMark,
    MarkKind::Hexbin
);
mark_builder!(
    /// A density estimate along x, Plot's `densityX`.
    DensityXMark,
    MarkKind::DensityX
);
mark_builder!(
    /// A density estimate along y, Plot's `densityY`.
    DensityYMark,
    MarkKind::DensityY
);
mark_builder!(
    /// A contour mark over a binned raster, Plot's `contour`.
    ContourMark,
    MarkKind::Contour
);
mark_builder!(
    /// A pie mark, Plot's `pie`.
    PieMark,
    MarkKind::Pie
);
mark_builder!(
    /// An arc mark, Plot's `arc`.
    ArcMark,
    MarkKind::Arc
);
mark_builder!(
    /// A waffle mark, Plot's `waffle`.
    WaffleMark,
    MarkKind::Waffle
);
mark_builder!(
    /// A link mark, Plot's `link`.
    LinkMark,
    MarkKind::Link
);
mark_builder!(
    /// A Delaunay triangulation drawn as links, Plot's `delaunayLink`.
    DelaunayLinkMark,
    MarkKind::DelaunayLink
);
mark_builder!(
    /// A Delaunay triangulation drawn as one mesh, Plot's `delaunayMesh`.
    DelaunayMeshMark,
    MarkKind::DelaunayMesh
);
mark_builder!(
    /// The convex hull of the data, Plot's `hull`.
    HullMark,
    MarkKind::Hull
);
mark_builder!(
    /// A Voronoi cell per datum, Plot's `voronoi`.
    VoronoiMark,
    MarkKind::Voronoi
);
mark_builder!(
    /// The Voronoi mesh of the data, Plot's `voronoiMesh`.
    VoronoiMeshMark,
    MarkKind::VoronoiMesh
);
mark_builder!(
    /// A tree diagram of a slash-separated hierarchy, Plot's `tree`.
    TreeMark,
    MarkKind::Tree
);
mark_builder!(
    /// A raster image painted at a position, Plot's `image`.
    ImageMark,
    MarkKind::Image
);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_classifies_every_kind() {
        use MarkFamily::*;
        use MarkKind::*;
        // Every mark is classified exactly once, in `MarkKind::category`. This
        // test pins the classification so a new kind or a regression can't
        // silently reclassify a mark. We assert the *derived predicates* hold,
        // because those are the contract the renderer relies on.
        let bivariate = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_bivariate());
        let x_only = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_x_only());
        let y_only = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_y_only());
        let furniture = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_furniture());
        let length = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_length_mark());
        let family = |kinds: &[MarkKind], f| kinds.iter().all(|k| k.category().family == f);

        assert!(bivariate(&[
            Dot,
            DotX,
            DotY,
            Line,
            LineX,
            LineY,
            AreaY,
            AreaX,
            BarY,
            BarX,
            Rect,
            RectY,
            RectX,
            Cell,
            RuleX,
            RuleY,
            Text,
            TickX,
            TickY,
            Vector,
            BoxX,
            BoxY,
            ViolinX,
            ViolinY,
            Hexbin,
            DensityX,
            DensityY,
            Contour,
            Link,
            DelaunayLink,
            DelaunayMesh,
            Hull,
            Voronoi,
            VoronoiMesh
        ]));
        // A one-dimensional label, cell or vector reads one channel only; a
        // bar's baseline and the radial marks read a length, not a second
        // position.
        assert!(!bivariate(&[
            TextX,
            TextY,
            CellX,
            CellY,
            VectorX,
            VectorY,
            Tip,
            Spike,
            Arrow,
            Hexgrid,
            Raster,
            Pie,
            Arc,
            Waffle,
            WaffleX,
            WaffleY,
            AxisX,
            AxisY,
            GridX,
            GridY,
            Frame,
            DifferenceX,
            DifferenceY,
            LinearRegressionX,
            LinearRegressionY,
            BollingerX,
            BollingerY
        ]));

        assert!(x_only(&[DotX, LineX, RectX, TextX, CellX, VectorX]));
        assert!(y_only(&[DotY, LineY, RectY, TextY, CellY, VectorY]));

        assert!(furniture(&[AxisX, AxisY, GridX, GridY, Frame]));
        assert!(!furniture(&[Dot, Line, Tip]));

        assert!(length(&[BarY, BarX, Pie, Arc, Waffle]));
        assert!(!length(&[RectX, Cell, WaffleX, WaffleY, Line]));

        // The renderer family drives which module draws the mark.
        assert!(family(
            &[
                Pie, Arc, Waffle, BoxX, BoxY, ViolinX, ViolinY, DensityX, DensityY, Hexbin, Contour
            ],
            Radial
        ));
        assert!(family(
            &[DelaunayLink, DelaunayMesh, Hull, Voronoi, VoronoiMesh],
            Delaunay
        ));
        assert!(family(
            &[Dot, Line, BarY, Cell, Raster, Hexgrid, Rect, Link],
            Basic
        ));

        // Cross-axis anchoring: one-dimensional dots, labels and vectors sit
        // in the frame's middle; lines, cells and rects spread by index.
        assert_eq!(DotX.cross_axis(), CrossAxis::Anchor);
        assert_eq!(TextY.cross_axis(), CrossAxis::Anchor);
        assert_eq!(VectorX.cross_axis(), CrossAxis::Anchor);
        assert_eq!(LineX.cross_axis(), CrossAxis::Index);
        assert_eq!(CellY.cross_axis(), CrossAxis::Index);
        assert_eq!(RectX.cross_axis(), CrossAxis::Index);
    }

    #[test]
    fn builders_share_channels() {
        let dot = DotMark::new().x("a").y("b").r(5.);
        assert_eq!(dot.kind, MarkKind::Dot);
        assert!(dot.channels.x.is_some());
        assert_eq!(dot.style.radius, 3.5);
        assert!(dot.into_mark().channels.y.is_some());
    }

    #[test]
    fn channels_accept_constants_and_accessors() {
        let line = LineMark::new().x(crate::value::Value::Number(1.));
        assert!(line.channels.x.is_some());
    }

    #[test]
    fn stack_and_sort_compose() {
        let area = AreaYMark::new()
            .stack(StackOffset::Normalize)
            .stack_series("group")
            .sort("date", true);
        let stack = area.stack.clone().unwrap();
        assert_eq!(stack.offset, StackOffset::Normalize);
        assert_eq!(stack.series_field.as_deref(), Some("group"));
        assert!(area.sort.as_ref().unwrap().descending);
    }

    #[test]
    fn reverse_flips_sort() {
        let mark = BarYMark::new().reverse();
        assert!(mark.sort.as_ref().unwrap().reverse);
        let mark = mark.reverse();
        assert!(!mark.sort.as_ref().unwrap().reverse);
    }
}

mark_builder!(
    /// A pinned annotation, Plot's `tip`.
    TipMark,
    MarkKind::Tip
);
mark_builder!(
    /// An x axis drawn as a mark, Plot's `axisX`.
    AxisXMark,
    MarkKind::AxisX
);
mark_builder!(
    /// A y axis drawn as a mark, Plot's `axisY`.
    AxisYMark,
    MarkKind::AxisY
);
mark_builder!(
    /// Grid lines across x, Plot's `gridX`.
    GridXMark,
    MarkKind::GridX
);
mark_builder!(
    /// Grid lines across y, Plot's `gridY`.
    GridYMark,
    MarkKind::GridY
);
mark_builder!(
    /// The frame's outline, Plot's `frame`.
    FrameMark,
    MarkKind::Frame
);

mark_builder!(
    /// A text label at each x, Plot's `textX`.
    TextXMark,
    MarkKind::TextX
);
mark_builder!(
    /// A text label at each y, Plot's `textY`.
    TextYMark,
    MarkKind::TextY
);
mark_builder!(
    /// A cell over one x, Plot's `cellX`.
    CellXMark,
    MarkKind::CellX
);
mark_builder!(
    /// A cell over one y, Plot's `cellY`.
    CellYMark,
    MarkKind::CellY
);
mark_builder!(
    /// A vector at each x, Plot's `vectorX`.
    VectorXMark,
    MarkKind::VectorX
);
mark_builder!(
    /// A vector at each y, Plot's `vectorY`.
    VectorYMark,
    MarkKind::VectorY
);
mark_builder!(
    /// A triangular marker, Plot's `spike`.
    SpikeMark,
    MarkKind::Spike
);
mark_builder!(
    /// A curved arrow, Plot's `arrow`.
    ArrowMark,
    MarkKind::Arrow
);
mark_builder!(
    /// A waffle bar along x, Plot's `waffleX`.
    WaffleXMark,
    MarkKind::WaffleX
);
mark_builder!(
    /// A waffle bar along y, Plot's `waffleY`.
    WaffleYMark,
    MarkKind::WaffleY
);
mark_builder!(
    /// A static hex grid, Plot's `hexgrid`.
    HexgridMark,
    MarkKind::Hexgrid
);
mark_builder!(
    /// A pixel grid, Plot's `raster`.
    RasterMark,
    MarkKind::Raster
);
mark_builder!(
    /// A difference chart along x, Plot's `differenceX`.
    DifferenceXMark,
    MarkKind::DifferenceX
);
mark_builder!(
    /// A difference chart along y, Plot's `differenceY`.
    DifferenceYMark,
    MarkKind::DifferenceY
);
mark_builder!(
    /// A regression of x on y, Plot's `linearRegressionX`.
    LinearRegressionXMark,
    MarkKind::LinearRegressionX
);
mark_builder!(
    /// A regression of y on x, Plot's `linearRegressionY`.
    LinearRegressionYMark,
    MarkKind::LinearRegressionY
);
mark_builder!(
    /// A moving mean with deviation bands along x, Plot's `bollingerX`.
    BollingerXMark,
    MarkKind::BollingerX
);
mark_builder!(
    /// A moving mean with deviation bands along y, Plot's `bollingerY`.
    BollingerYMark,
    MarkKind::BollingerY
);
