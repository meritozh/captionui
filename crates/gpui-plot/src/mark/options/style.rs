//! How a mark draws: opacities, stroke, curve, offsets, markers, clipping and
//! the arrow, vector, regression, Bollinger, hexgrid and waffle options.

use crate::mark::{BlendMode, Curve, MarkOptions, Marker, Sweep, VectorAnchor};

/// How a mark draws: opacities, stroke, curve, offsets, markers, clipping
/// and the arrow, vector, regression, Bollinger, hexgrid and waffle options.
pub trait StyleOptions: MarkOptions {
    /// Set the opacity of the mark's fills, Plot's `fillOpacity`.
    fn fill_opacity(mut self, opacity: f32) -> Self {
        self.mark_mut().style.fill_opacity = opacity;
        self
    }

    /// Set the opacity of the mark's strokes, Plot's `strokeOpacity`.
    fn stroke_opacity(mut self, opacity: f32) -> Self {
        self.mark_mut().style.stroke_opacity = opacity;
        self
    }

    /// Set the stroke width.
    fn stroke_width(mut self, width: f32) -> Self {
        self.mark_mut().style.stroke_width = width;
        self
    }

    /// Set how the mark connects its points.
    fn curve(mut self, curve: impl Into<Curve>) -> Self {
        let curve = curve.into();
        self.mark_mut().style.curve = curve;
        self
    }

    /// Round a bar's or rect's corners.
    fn corner_radius(mut self, radius: f32) -> Self {
        self.mark_mut().style.corner_radius = radius;
        self
    }

    /// Offset a text label horizontally, Plot's `dx`.
    fn dx(mut self, dx: f32) -> Self {
        self.mark_mut().style.dx = dx;
        self
    }

    /// Offset a text label vertically, Plot's `dy`.
    fn dy(mut self, dy: f32) -> Self {
        self.mark_mut().style.dy = dy;
        self
    }

    /// Plot's `strokeDasharray`.
    fn dash(mut self, dash: impl IntoIterator<Item = f32>) -> Self {
        self.mark_mut().style.dash = dash.into_iter().collect();
        self
    }

    /// Plot's `marker`: the same marker at every vertex.
    fn marker(mut self, marker: &str) -> Self {
        self.mark_mut().style.markers = [Marker::parse(marker); 3];
        self
    }

    /// Plot's `markerStart`.
    fn marker_start(mut self, marker: &str) -> Self {
        self.mark_mut().style.markers[0] = Marker::parse(marker);
        self
    }

    /// Plot's `markerMid`.
    fn marker_mid(mut self, marker: &str) -> Self {
        self.mark_mut().style.markers[1] = Marker::parse(marker);
        self
    }

    /// Plot's `markerEnd`.
    fn marker_end(mut self, marker: &str) -> Self {
        self.mark_mut().style.markers[2] = Marker::parse(marker);
        self
    }

    /// A difference mark's `positiveFill`.
    fn positive_fill(mut self, color: &str) -> Self {
        self.mark_mut().style.positive_fill = crate::css::parse(color);
        self
    }

    /// A difference mark's `negativeFill`.
    fn negative_fill(mut self, color: &str) -> Self {
        self.mark_mut().style.negative_fill = crate::css::parse(color);
        self
    }

    /// Plot's area `line: true`: also stroke the area's top line, and fade
    /// the fill to 0.3 unless a fill opacity is set.
    fn area_line(mut self, line: bool) -> Self {
        let mark = self.mark_mut();
        mark.style.area_line = line;
        if line && mark.style.fill_opacity == 1. {
            mark.style.fill_opacity = 0.3;
        }
        self
    }

    /// Clip the mark to the plot area, Plot's `clip: true`.
    fn clip(mut self, clip: bool) -> Self {
        self.mark_mut().style.clip = clip;
        self
    }

    /// Clip the mark to the projected sphere, Plot's `clip: "sphere"`.
    fn clip_sphere(mut self) -> Self {
        self.mark_mut().style.clip_sphere = true;
        self
    }

    /// Shrink a rect or bar by `pixels` on every side, Plot's `inset`. A
    /// binned rect defaults to half a pixel along its binned axes, so
    /// adjacent bins read apart; `inset(0.)` closes the gap.
    fn inset(mut self, pixels: f32) -> Self {
        self.mark_mut().style.inset = Some(pixels);
        self
    }

    /// Plot's `mixBlendMode`: how a rect or bar's fill blends with what it
    /// overlaps, `"multiply"` or `"screen"`.
    fn mix_blend_mode(mut self, mode: BlendMode) -> Self {
        self.mark_mut().style.mix_blend_mode = Some(mode);
        self
    }

    /// Curve an arrow by `bend` degrees, Plot's `bend`.
    fn bend(mut self, degrees: f32) -> Self {
        self.mark_mut().style.bend = degrees;
        self
    }

    /// Turn an arrow's bend by the order of its ends, Plot's `sweep`.
    fn sweep(mut self, sweep: Sweep) -> Self {
        self.mark_mut().style.sweep = sweep;
        self
    }

    /// Set the angle between an arrow's wings, Plot's `headAngle`.
    fn head_angle(mut self, degrees: f32) -> Self {
        self.mark_mut().style.head_angle = degrees;
        self
    }

    /// Set the length of an arrow's wings, Plot's `headLength`.
    fn head_length(mut self, pixels: f32) -> Self {
        self.mark_mut().style.head_length = pixels;
        self
    }

    /// Set how a vector is anchored at its datum, Plot's `anchor`.
    fn anchor(mut self, anchor: VectorAnchor) -> Self {
        self.mark_mut().style.anchor = anchor;
        self
    }

    /// Set a regression's confidence level; zero draws the line alone.
    fn ci(mut self, level: f32) -> Self {
        self.mark_mut().style.ci = level;
        self
    }

    /// Set a Bollinger band's window and multiplier.
    fn bollinger(mut self, window: usize, k: f32) -> Self {
        let mark = self.mark_mut();
        mark.style.window = window;
        mark.style.k = k;
        self
    }

    /// Set a hexgrid's hexagon width, Plot's `binWidth`.
    fn bin_width(mut self, pixels: f32) -> Self {
        self.mark_mut().style.bin_width = pixels;
        self
    }

    /// Set the value one waffle cell stands for, Plot's `unit`.
    fn unit(mut self, unit: f32) -> Self {
        self.mark_mut().style.unit = unit;
        self
    }

    /// Set the gap between a waffle's cells, Plot's `gap`.
    fn gap(mut self, pixels: f32) -> Self {
        self.mark_mut().style.gap = pixels;
        self
    }

    /// Set how many cells a waffle's row holds, Plot's `multiple`.
    fn multiple(mut self, cells: usize) -> Self {
        self.mark_mut().style.multiple = Some(cells.max(1));
        self
    }
}

impl<T: MarkOptions> StyleOptions for T {}
