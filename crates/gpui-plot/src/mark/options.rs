//! Plot's mark options as builder traits, one file per concern. Each trait's
//! methods are written once, as default methods over [`MarkOptions`], and
//! every builder — [`Mark`] and the typed builders — gets them all.

mod axis;
mod channels;
mod geo;
mod image;
mod raster;
mod stack;
mod style;
mod text;
mod transform;

pub use axis::AxisOptions;
pub use channels::ChannelOptions;
pub use geo::GeoOptions;
pub use image::ImageOptions;
pub use raster::RasterOptions;
pub use stack::StackOptions;
pub use style::StyleOptions;
pub use text::TextOptions;
pub use transform::TransformOptions;

use crate::mark::Mark;

/// A mark builder: [`Mark`] itself or a typed builder such as
/// [`DotMark`](crate::mark::DotMark).
///
/// Implementing the one accessor gives a type every mark option: the
/// per-concern traits ([`ChannelOptions`], [`StyleOptions`], [`TextOptions`],
/// [`AxisOptions`], [`StackOptions`], [`TransformOptions`], [`RasterOptions`],
/// [`GeoOptions`] and [`ImageOptions`]) are implemented for every
/// `MarkOptions` type, the way GPUI's `Styled` builds on `style()`. Bring them
/// into scope with `use gpui_plot::prelude::*`.
pub trait MarkOptions: Sized {
    /// The mark this builder configures.
    fn mark_mut(&mut self) -> &mut Mark;

    /// Draw this mark alone, Plot's `mark.plot()`.
    fn plot(self) -> crate::plot::Plot
    where
        Self: Into<Mark>,
    {
        crate::plot::Plot::plot().mark(self)
    }
}

impl MarkOptions for Mark {
    fn mark_mut(&mut self) -> &mut Mark {
        self
    }
}
