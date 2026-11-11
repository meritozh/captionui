//! Text and tip layout: format, anchors, font, line width, rotation and halo.

use gpui::Hsla;

use crate::mark::{FrameAnchor, LineAnchor, Mark, TextAnchor};

impl Mark {
    /// Plot's text `format`: a d3-format or d3-time-format specifier, or a
    /// function, for the label's value.
    pub fn format(mut self, format: impl Into<crate::options::TickFormat>) -> Self {
        self.text_format = Some(format.into());
        self
    }

    /// Plot's `textAnchor`.
    pub fn text_anchor(mut self, anchor: TextAnchor) -> Self {
        self.style.text_anchor = anchor;
        self.style.text_anchor_set = true;
        self
    }

    /// Plot's `lineAnchor`.
    pub fn line_anchor(mut self, anchor: LineAnchor) -> Self {
        self.style.line_anchor = anchor;
        self.style.line_anchor_set = true;
        self
    }

    /// Plot's `frameAnchor`.
    pub fn frame_anchor(mut self, anchor: FrameAnchor) -> Self {
        self.style.frame_anchor = Some(anchor);
        self
    }

    /// Plot's `fontSize`.
    pub fn font_size(mut self, size: f32) -> Self {
        self.style.font_size = Some(size);
        self
    }

    /// Plot's `fontWeight`.
    pub fn font_weight(mut self, weight: u16) -> Self {
        self.style.font_weight = Some(weight);
        self
    }

    /// Plot's `fontStyle: "italic"`.
    pub fn italic(mut self) -> Self {
        self.style.italic = true;
        self
    }

    /// Plot's `lineWidth`, in ems.
    pub fn line_width(mut self, ems: f32) -> Self {
        self.style.line_width = Some(ems);
        self
    }

    /// A tip's `anchor`: which side of its point the box sits on.
    pub fn tip_anchor(mut self, anchor: FrameAnchor) -> Self {
        self.style.tip_anchor = Some(anchor);
        self
    }

    /// A tip's `textPadding`, in pixels.
    pub fn text_padding(mut self, padding: f32) -> Self {
        self.style.text_padding = Some(padding);
        self
    }

    /// A tip's `pointerSize`, in pixels.
    pub fn pointer_size(mut self, size: f32) -> Self {
        self.style.pointer_size = Some(size);
        self
    }

    /// Plot's `rotate` for a text label, in degrees.
    pub fn text_rotate(mut self, degrees: f32) -> Self {
        self.style.text_rotate = degrees;
        self
    }

    /// Plot's `textStroke`: a halo of `color` behind the glyphs.
    pub fn text_stroke(mut self, color: &str) -> Self {
        self.style.text_stroke = crate::css::parse(color).or(Some(Hsla::white()));
        self
    }

    /// Plot's `textStrokeWidth`.
    pub fn text_stroke_width(mut self, width: f32) -> Self {
        self.style.text_stroke_width = width;
        self
    }
}
