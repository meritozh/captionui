//! A mark's visual defaults, `MarkStyle`, and the enums it holds: `Curve`,
//! `BlendMode`, `Sweep`, `Marker` and the text, line, frame and vector anchors.

use gpui::{Hsla, SharedString};

use crate::style::Symbol;

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
    /// Which way an arrow bends, Plot's `sweep`.
    pub sweep: Sweep,
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
    /// How many cells a waffle's row (or column) holds, Plot's `multiple`;
    /// `None` fits as many as keep the cells roughly square.
    pub multiple: Option<usize>,
    /// Whether the mark clips to the plot area, Plot's `clip`.
    pub clip: bool,
    /// Whether the mark clips to the projected sphere, Plot's
    /// `clip: "sphere"`.
    pub clip_sphere: bool,
    /// Plot's `markerStart`, `markerMid` and `markerEnd`.
    pub markers: [Option<Marker>; 3],
    /// A difference mark's `positiveFill` and `negativeFill`.
    pub positive_fill: Option<Hsla>,
    pub negative_fill: Option<Hsla>,
    /// Plot's `textStroke`: a halo behind a text mark's glyphs.
    pub text_stroke: Option<Hsla>,
    /// Plot's `textStrokeWidth`.
    pub text_stroke_width: f32,
    /// Plot's `textAnchor`.
    pub text_anchor: TextAnchor,
    /// Whether `textAnchor` was given, so a `frameAnchor` doesn't override it.
    pub text_anchor_set: bool,
    /// Whether `lineAnchor` was given.
    pub line_anchor_set: bool,
    /// Plot's area `line: true`: stroke the area's top edge.
    pub area_line: bool,
    /// Plot's `lineAnchor`.
    pub line_anchor: LineAnchor,
    /// Plot's `frameAnchor`.
    pub frame_anchor: Option<FrameAnchor>,
    /// Plot's `fontSize`, in pixels.
    pub font_size: Option<f32>,
    /// Plot's `fontWeight`.
    pub font_weight: Option<u16>,
    /// Plot's `fontStyle: "italic"`.
    pub italic: bool,
    /// Plot's `lineWidth`: wrap text at this many ems.
    pub line_width: Option<f32>,
    /// A tip's `anchor`: the side of its point the box sits on; `None`
    /// picks the side that fits, preferring the bottom.
    pub tip_anchor: Option<FrameAnchor>,
    /// A tip's `textPadding`, in pixels (8 by default).
    pub text_padding: Option<f32>,
    /// A tip's `pointerSize`, in pixels (12 by default).
    pub pointer_size: Option<f32>,
    /// An image's `width` and `height`, in pixels (16 by default, each
    /// defaulting to the other).
    pub image_size: [Option<f32>; 2],
    /// An image's `preserveAspectRatio`, SVG's `xMidYMid meet` by default.
    pub preserve_aspect_ratio: Option<SharedString>,
    /// Plot's `strokeDasharray`, in pixels.
    pub dash: Vec<f32>,
    /// Plot's `rotate` for text, in degrees.
    pub text_rotate: f32,
    /// How far a rect is shrunk on each side, Plot's `inset`; `None` takes
    /// the mark's default (half a pixel along a binned axis, else zero).
    pub inset: Option<f32>,
    /// Plot's `mixBlendMode`, for the marks that honour it (rects and bars).
    pub mix_blend_mode: Option<BlendMode>,
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
            radius: 3.,
            symbol: Symbol::Circle,
            curve: Curve::Linear,
            fill_opacity: 1.,
            stroke_opacity: 1.,
            opacity: 1.,
            corner_radius: 0.,
            dx: 0.,
            dy: 0.,
            bend: 0.,
            sweep: Sweep::Positive,
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
            multiple: None,
            inset: None,
            mix_blend_mode: None,
            clip: false,
            clip_sphere: false,
            markers: [None; 3],
            positive_fill: None,
            negative_fill: None,
            text_stroke: None,
            text_stroke_width: 3.,
            text_anchor: TextAnchor::Middle,
            text_anchor_set: false,
            line_anchor_set: false,
            area_line: false,
            line_anchor: LineAnchor::Middle,
            frame_anchor: None,
            font_size: None,
            font_weight: None,
            italic: false,
            line_width: None,
            tip_anchor: None,
            text_padding: None,
            pointer_size: None,
            image_size: [None; 2],
            preserve_aspect_ratio: None,
            dash: Vec::new(),
            text_rotate: 0.,
        }
    }
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

/// A CSS `mix-blend-mode`, Plot's `mixBlendMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// The product of the colors, `multiply`: overlaps darken.
    Multiply,
    /// The inverted product of the inverses, `screen`: overlaps lighten.
    Screen,
}

impl BlendMode {
    /// The CSS names `"multiply"` and `"screen"`.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "multiply" => Some(Self::Multiply),
            "screen" => Some(Self::Screen),
            _ => None,
        }
    }

    /// One channel of a source `s` blended onto a backdrop `b`, both in 0–1.
    pub fn channel(self, b: f32, s: f32) -> f32 {
        match self {
            Self::Multiply => b * s,
            Self::Screen => b + s - b * s,
        }
    }
}

/// How a line or area connects its points, Plot's `curve`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    /// A smooth curve through every point, Plot's `natural`.
    Natural,
    /// Straight segments, Plot's `linear` and its default.
    #[default]
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
    /// Plot's curve names: `"linear"`, `"step"`, `"monotone-x"`, `"basis"` …
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "linear" | "auto" => Curve::Linear,
            "linear-closed" => Curve::LinearClosed,
            "natural" => Curve::Natural,
            "step" => Curve::Step,
            "step-after" => Curve::StepAfter,
            "step-before" => Curve::StepBefore,
            "basis" => Curve::Basis,
            "basis-open" => Curve::BasisOpen,
            "basis-closed" => Curve::BasisClosed,
            "cardinal" => Curve::Cardinal,
            "cardinal-open" => Curve::CardinalOpen,
            "cardinal-closed" => Curve::CardinalClosed,
            "catmull-rom" => Curve::CatmullRom,
            "catmull-rom-open" => Curve::CatmullRomOpen,
            "catmull-rom-closed" => Curve::CatmullRomClosed,
            "monotone-x" => Curve::MonotoneX,
            "monotone-y" => Curve::MonotoneY,
            "bump-x" => Curve::BumpX,
            "bump-y" => Curve::BumpY,
            "bundle" => Curve::Bundle,
            _ => return None,
        })
    }
}

impl From<&str> for Curve {
    fn from(name: &str) -> Self {
        Curve::parse(name).unwrap_or(Curve::Linear)
    }
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

/// Which way an arrow's bend turns, Plot's `sweep`: always one way, or by
/// the order of its ends along x or y (in pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sweep {
    /// Plot's default `1`: the bend as given.
    #[default]
    Positive,
    /// `-1`: the bend reversed.
    Negative,
    /// `"+x"`: the bend's sign is `ascending(x1, x2)`.
    PlusX,
    /// `"-x"`: `descending(x1, x2)`.
    MinusX,
    /// `"+y"`: `ascending(y1, y2)`.
    PlusY,
    /// `"-y"`: `descending(y1, y2)`.
    MinusY,
}

impl Sweep {
    /// The sign the bend takes for an arrow between two pixel points.
    pub fn sign(self, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        let ascending = |a: f32, b: f32| -> f32 {
            if a < b {
                -1.
            } else if a > b {
                1.
            } else {
                0.
            }
        };
        match self {
            Sweep::Positive => 1.,
            Sweep::Negative => -1.,
            Sweep::PlusX => ascending(x1, x2),
            Sweep::MinusX => -ascending(x1, x2),
            Sweep::PlusY => ascending(y1, y2),
            Sweep::MinusY => -ascending(y1, y2),
        }
    }
}

/// A marker drawn at a line's vertices, Plot's `marker` options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Arrow,
    ArrowReverse,
    Dot,
    CircleFill,
    CircleStroke,
    Tick,
    TickX,
    TickY,
}

impl Marker {
    /// Plot's marker names; `"none"` is no marker.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "arrow" => Marker::Arrow,
            "arrow-reverse" => Marker::ArrowReverse,
            "dot" => Marker::Dot,
            "circle" | "circle-fill" | "true" => Marker::CircleFill,
            "circle-stroke" => Marker::CircleStroke,
            "tick" => Marker::Tick,
            "tick-x" => Marker::TickX,
            "tick-y" => Marker::TickY,
            _ => return None,
        })
    }
}

/// How a text label sits against its point, Plot's `textAnchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAnchor {
    Start,
    #[default]
    Middle,
    End,
}

/// How a text label's lines sit against its point, Plot's `lineAnchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineAnchor {
    Top,
    #[default]
    Middle,
    Bottom,
}

/// Where a mark without a position sits in the frame, Plot's `frameAnchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameAnchor {
    #[default]
    Middle,
    Top,
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
