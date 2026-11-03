//! Mark specifications: Plot's mark types and their channels.
//!
//! A mark is a description of how to draw the data — `dot`, `line`, `barY`,
//! `cell`, … — plus the channels it reads. Channels are resolved against the
//! data at render time, so a mark value is cheap to build and holds no
//! window state.

use gpui::{Hsla, SharedString};

use crate::style::{ColorChannel, NumberChannel, Symbol};
use crate::table::{Channel, IntoChannel};
use crate::transform::{Anchor, Dodge, Reducer, StackOffset, Thresholds, WindowReducer};

/// The mark types this library draws, matching Plot's mark set.
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
        }
    }

    /// Whether this mark reads two positional channels (`x` and `y`) rather
    /// than one axis plus a length.
    pub fn is_bivariate(self) -> bool {
        matches!(
            self,
            MarkKind::Dot
                | MarkKind::DotX
                | MarkKind::DotY
                | MarkKind::Line
                | MarkKind::LineX
                | MarkKind::LineY
                | MarkKind::AreaY
                | MarkKind::AreaX
                | MarkKind::BarY
                | MarkKind::BarX
                | MarkKind::Rect
                | MarkKind::RectX
                | MarkKind::RectY
                | MarkKind::Cell
                | MarkKind::RuleX
                | MarkKind::RuleY
                | MarkKind::Text
                | MarkKind::TickX
                | MarkKind::TickY
                | MarkKind::Vector
                | MarkKind::BoxX
                | MarkKind::BoxY
                | MarkKind::ViolinX
                | MarkKind::ViolinY
                | MarkKind::Hexbin
                | MarkKind::DensityX
                | MarkKind::DensityY
                | MarkKind::Contour
                | MarkKind::Link
        )
    }

    /// Whether this mark reads a length channel on its own axis: a bar grows
    /// from a baseline, a pie takes an angle.
    pub fn is_length_mark(self) -> bool {
        matches!(
            self,
            MarkKind::BarY | MarkKind::BarX | MarkKind::Pie | MarkKind::Arc | MarkKind::Waffle
        )
    }

    /// Whether this mark lays its data out along the x axis only, with no y
    /// channel of its own: Plot's `dotX`, `lineX`, `rectX`.
    pub fn is_x_only(self) -> bool {
        matches!(self, MarkKind::DotX | MarkKind::LineX | MarkKind::RectX)
    }

    /// Whether this mark lays its data out along the y axis only.
    pub fn is_y_only(self) -> bool {
        matches!(self, MarkKind::DotY | MarkKind::LineY | MarkKind::RectY)
    }

    /// How a mark with one position channel places itself across the axis it
    /// does not read.
    ///
    /// Plot's `dotX` puts every dot in the frame's middle, so a one-dimensional
    /// distribution reads as a row; `lineX` and `rectX` instead spread the
    /// data across that axis by its index, so a line runs across the plot and
    /// a histogram's bars each get a row.
    pub fn cross_axis(self) -> CrossAxis {
        match self {
            MarkKind::DotX | MarkKind::DotY => CrossAxis::Anchor,
            _ => CrossAxis::Index,
        }
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
    /// A window transform applied to the mark's value, Plot's `window`.
    pub window: Option<(usize, WindowReducer, Anchor)>,
    /// A numeric transform of the mark's value, Plot's `normalize`.
    pub normalize: bool,
    /// How the mark dodges its points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
    pub dodge: Option<Dodge>,
    /// A mark-level title, Plot's `title`.
    pub title: Option<SharedString>,
}

impl Mark {
    pub fn new(kind: MarkKind) -> Self {
        Self {
            kind,
            channels: Channels::default(),
            style: MarkStyle::default(),
            stack: None,
            bin: None,
            sort: None,
            reduce: None,
            window: None,
            normalize: false,
            dodge: None,
            title: None,
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
