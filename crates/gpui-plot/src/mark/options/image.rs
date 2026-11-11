//! The image mark's options: the image it paints, its size and aspect ratio.

use crate::mark::Mark;

impl Mark {
    /// Set the image an `image` mark paints, shared across its rows.
    pub fn image(mut self, image: std::sync::Arc<gpui::RenderImage>) -> Self {
        self.image = Some(image);
        self
    }

    /// An image's `width`, in pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.style.image_size[0] = Some(width);
        self
    }

    /// An image's `height`, in pixels.
    pub fn height(mut self, height: f32) -> Self {
        self.style.image_size[1] = Some(height);
        self
    }

    /// An image's `preserveAspectRatio`, as SVG spells it
    /// (`"xMidYMin slice"`).
    pub fn preserve_aspect_ratio(mut self, value: &str) -> Self {
        self.style.preserve_aspect_ratio = Some(value.to_string().into());
        self
    }
}
