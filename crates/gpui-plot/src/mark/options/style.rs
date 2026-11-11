//! How a mark draws: opacities, stroke, curve, offsets, markers, clipping and
//! the arrow, vector, regression, Bollinger, hexgrid and waffle options.

use crate::mark::{BlendMode, Curve, Mark, Marker, Sweep, VectorAnchor};

impl Mark {
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

    /// Set the stroke width.
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.style.stroke_width = width;
        self
    }

    /// Set how the mark connects its points.
    pub fn curve(mut self, curve: impl Into<Curve>) -> Self {
        let curve = curve.into();
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

    /// Plot's `strokeDasharray`.
    pub fn dash(mut self, dash: impl IntoIterator<Item = f32>) -> Self {
        self.style.dash = dash.into_iter().collect();
        self
    }

    /// Plot's `marker`: the same marker at every vertex.
    pub fn marker(mut self, marker: &str) -> Self {
        self.style.markers = [Marker::parse(marker); 3];
        self
    }

    /// Plot's `markerStart`.
    pub fn marker_start(mut self, marker: &str) -> Self {
        self.style.markers[0] = Marker::parse(marker);
        self
    }

    /// Plot's `markerMid`.
    pub fn marker_mid(mut self, marker: &str) -> Self {
        self.style.markers[1] = Marker::parse(marker);
        self
    }

    /// Plot's `markerEnd`.
    pub fn marker_end(mut self, marker: &str) -> Self {
        self.style.markers[2] = Marker::parse(marker);
        self
    }

    /// A difference mark's `positiveFill`.
    pub fn positive_fill(mut self, color: &str) -> Self {
        self.style.positive_fill = crate::css::parse(color);
        self
    }

    /// A difference mark's `negativeFill`.
    pub fn negative_fill(mut self, color: &str) -> Self {
        self.style.negative_fill = crate::css::parse(color);
        self
    }

    /// Plot's area `line: true`: also stroke the area's top line, and fade
    /// the fill to 0.3 unless a fill opacity is set.
    pub fn area_line(mut self, line: bool) -> Self {
        self.style.area_line = line;
        if line && self.style.fill_opacity == 1. {
            self.style.fill_opacity = 0.3;
        }
        self
    }

    /// Clip the mark to the plot area, Plot's `clip: true`.
    pub fn clip(mut self, clip: bool) -> Self {
        self.style.clip = clip;
        self
    }

    /// Clip the mark to the projected sphere, Plot's `clip: "sphere"`.
    pub fn clip_sphere(mut self) -> Self {
        self.style.clip_sphere = true;
        self
    }

    /// Shrink a rect or bar by `pixels` on every side, Plot's `inset`. A
    /// binned rect defaults to half a pixel along its binned axes, so
    /// adjacent bins read apart; `inset(0.)` closes the gap.
    pub fn inset(mut self, pixels: f32) -> Self {
        self.style.inset = Some(pixels);
        self
    }

    /// Plot's `mixBlendMode`: how a rect or bar's fill blends with what it
    /// overlaps, `"multiply"` or `"screen"`.
    pub fn mix_blend_mode(mut self, mode: BlendMode) -> Self {
        self.style.mix_blend_mode = Some(mode);
        self
    }

    /// Curve an arrow by `bend` degrees, Plot's `bend`.
    pub fn bend(mut self, degrees: f32) -> Self {
        self.style.bend = degrees;
        self
    }

    /// Turn an arrow's bend by the order of its ends, Plot's `sweep`.
    pub fn sweep(mut self, sweep: Sweep) -> Self {
        self.style.sweep = sweep;
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

    /// Set how many cells a waffle's row holds, Plot's `multiple`.
    pub fn multiple(mut self, cells: usize) -> Self {
        self.style.multiple = Some(cells.max(1));
        self
    }
}
