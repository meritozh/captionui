//! A Plot-style visualization library for GPUI.
//!
//! This crate follows Observable Plot's model: data is a collection of records,
//! marks read *channels* from each record, scales turn channel values into
//! positions and colors, and transforms reshape the data between the two. A
//! chart is a [`Plot`] value built from marks, and a [`Plot`] renders as a GPUI
//! element.
//!
//! ```
//! use gpui_plot::{Plot, Table, Value};
//!
//! let data = vec![
//!     Table::empty().with("x", Value::text("a")).with("y", Value::Number(4.)),
//!     Table::empty().with("x", Value::text("b")).with("y", Value::Number(7.)),
//! ];
//!
//! let plot = Plot::new(data)
//!     .dot(|mark| mark.x("x").y("y"));
//! ```
//!
//! The main pieces:
//!
//! - [`Table`] and [`Dataset`] hold the data.
//! - [`Value`] is the scalar a channel reads.
//! - [`Mark`] covers Plot's mark types: dot, line, area, bar, rect, cell, rule,
//!   text, tick, vector, box, violin, hexbin, density, contour, pie, arc,
//!   waffle and link.
//! - [`transform`] holds Plot's transforms: group, bin, stack, window, sort,
//!   filter, normalize.
//! - [`scale`] holds the scales and color ranges.
//! - [`Plot`] assembles them and renders as an element.

// GPUI Kit re-exports GPUI at its root; the alias lets this crate and the
// `IntoPlot` derive spell GPUI paths the way the rest of the ecosystem does.
extern crate gpui_kit as gpui;

pub mod auto;
pub mod css;
pub mod delaunay;
pub mod density;
pub mod facet;
pub mod format;
pub mod frame;
pub mod geo;
pub mod hierarchy;
pub mod io;
pub mod mark;
pub mod marks;
pub mod options;
pub mod paint;
pub mod plot;
pub mod projection;
pub mod raster;
pub mod render;
pub mod scale;
pub mod schemes;
pub mod style;
pub mod table;
pub mod theme;
pub mod ticks;
pub mod transform;
pub mod tree;
pub mod value;

pub use mark::{
    Axis, BlendMode, GroupConfig, GroupOutput, Interval, MapConfig, MapReducer, MapTarget,
    SelectConfig, ShiftConfig, Sweep,
};
pub use scale::{
    BandScale, ColorRange, ColorScale, ContinuousScale, OrdinalScale, PointScale, PositionScale,
    Transform,
};
pub use style::{ColorChannel, NumberChannel, Symbol};
pub use table::{Channel, Dataset, IntoChannel, Table};
pub use transform::{
    Anchor, Bin, Dodge, DodgeAnchor, GroupReducer, Mapper, Reducer, Select, Selector, StackOffset,
    StackedPoint, StackedSeries, Thresholds, WindowReducer,
};
pub use value::{PlotKind, Value};

pub use frame::{FrameOptions, ScaleKind};
pub use options::{
    AxisAnchor, LabelAnchor, ScaleOptions, ScaleTransform, ScaleType, TickFormat, Ticks,
};
pub use plot::{Interaction, LegendPosition, Plot, PlotTitle};
pub use projection::{Equirectangular, Mercator, Projection, centroid, geojson_rings};

/// Build a [`gpui::RenderImage`] from RGBA pixels, for an `image` mark.
///
/// `rgba` is a `(width, height, bytes)` tri-plet of tightly-packed RGBA bytes
/// (`height` rows of `width` pixels). The image mark paints it at its position;
/// this is how a caller supplies pixels without GPUI's async image cache.
pub fn image_from_rgba(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> std::sync::Arc<gpui::RenderImage> {
    let buffer = image::RgbaImage::from_raw(width, height, rgba)
        .expect("RGBA bytes must match width * height * 4");
    let frame = image::Frame::new(buffer);
    let render = gpui::RenderImage::new(smallvec::smallvec![frame]);
    std::sync::Arc::new(render)
}
pub use geo::{GeoProjection, Geometry, Topology};
pub use hierarchy::{TreeLayout, TreeSort, TreeSpec};
pub use render::stack::{NormalizeBasis, Offset, StackAnchor, StackOrder};
