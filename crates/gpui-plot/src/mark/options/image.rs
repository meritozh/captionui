//! The image mark's options: the image it paints, its size and aspect ratio.

use crate::mark::MarkOptions;

/// The image mark's options: the image it paints, its size and aspect
/// ratio.
pub trait ImageOptions: MarkOptions {
    /// Set the image an `image` mark paints, shared across its rows.
    fn image(mut self, image: std::sync::Arc<gpui::RenderImage>) -> Self {
        self.mark_mut().image = Some(image);
        self
    }

    /// An image's `width`, in pixels.
    fn width(mut self, width: f32) -> Self {
        self.mark_mut().style.image_size[0] = Some(width);
        self
    }

    /// An image's `height`, in pixels.
    fn height(mut self, height: f32) -> Self {
        self.mark_mut().style.image_size[1] = Some(height);
        self
    }

    /// An image's `preserveAspectRatio`, as SVG spells it
    /// (`"xMidYMin slice"`).
    fn preserve_aspect_ratio(mut self, value: &str) -> Self {
        self.mark_mut().style.preserve_aspect_ratio = Some(value.to_string().into());
        self
    }
}

impl<T: MarkOptions> ImageOptions for T {}
