//! Text and tip layout: format, anchors, font, line width, rotation and halo.

use gpui::Hsla;

use crate::mark::{FrameAnchor, LineAnchor, MarkOptions, TextAnchor};

/// Text and tip layout: format, anchors, font, line width, rotation and halo.
pub trait TextOptions: MarkOptions {
    /// Plot's text `format`: a d3-format or d3-time-format specifier, or a
    /// function, for the label's value.
    fn format(mut self, format: impl Into<crate::options::TickFormat>) -> Self {
        self.mark_mut().text_format = Some(format.into());
        self
    }

    /// Plot's `textAnchor`.
    fn text_anchor(mut self, anchor: TextAnchor) -> Self {
        let mark = self.mark_mut();
        mark.style.text_anchor = anchor;
        mark.style.text_anchor_set = true;
        self
    }

    /// Plot's `lineAnchor`.
    fn line_anchor(mut self, anchor: LineAnchor) -> Self {
        let mark = self.mark_mut();
        mark.style.line_anchor = anchor;
        mark.style.line_anchor_set = true;
        self
    }

    /// Plot's `frameAnchor`.
    fn frame_anchor(mut self, anchor: FrameAnchor) -> Self {
        self.mark_mut().style.frame_anchor = Some(anchor);
        self
    }

    /// Plot's `fontSize`.
    fn font_size(mut self, size: f32) -> Self {
        self.mark_mut().style.font_size = Some(size);
        self
    }

    /// Plot's `fontWeight`.
    fn font_weight(mut self, weight: u16) -> Self {
        self.mark_mut().style.font_weight = Some(weight);
        self
    }

    /// Plot's `fontStyle: "italic"`.
    fn italic(mut self) -> Self {
        self.mark_mut().style.italic = true;
        self
    }

    /// Plot's `lineWidth`, in ems.
    fn line_width(mut self, ems: f32) -> Self {
        self.mark_mut().style.line_width = Some(ems);
        self
    }

    /// A tip's `anchor`: which side of its point the box sits on.
    fn tip_anchor(mut self, anchor: FrameAnchor) -> Self {
        self.mark_mut().style.tip_anchor = Some(anchor);
        self
    }

    /// A tip's `textPadding`, in pixels.
    fn text_padding(mut self, padding: f32) -> Self {
        self.mark_mut().style.text_padding = Some(padding);
        self
    }

    /// A tip's `pointerSize`, in pixels.
    fn pointer_size(mut self, size: f32) -> Self {
        self.mark_mut().style.pointer_size = Some(size);
        self
    }

    /// Plot's `rotate` for a text label, in degrees.
    fn text_rotate(mut self, degrees: f32) -> Self {
        self.mark_mut().style.text_rotate = degrees;
        self
    }

    /// Plot's `textStroke`: a halo of `color` behind the glyphs.
    fn text_stroke(mut self, color: &str) -> Self {
        self.mark_mut().style.text_stroke = crate::css::parse(color).or(Some(Hsla::white()));
        self
    }

    /// Plot's `textStrokeWidth`.
    fn text_stroke_width(mut self, width: f32) -> Self {
        self.mark_mut().style.text_stroke_width = width;
        self
    }
}

impl<T: MarkOptions> TextOptions for T {}
