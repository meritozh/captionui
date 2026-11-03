//! A Plot-style visualization library for GPUI.
//!
//! This crate follows Observable Plot's model: data is a collection of records,
//! marks read *channels* from each record, scales turn channel values into
//! positions and colors, and transforms reshape the data between the two. A
//! chart is a [`Plot`](plot::Plot) value built from marks, and a
//! [`Plot`](plot::Plot) renders as a GPUI element.
//!
//! ```
//! use gpui_plot::{PlotKind, Value};
//!
//! let value = Value::text("North");
//! assert_eq!(value.kind(), PlotKind::Text);
//! assert_eq!(value.label(), "North");
//! ```
//!
//! The main pieces:
//!
//! - [`Table`](table::Table) and [`Dataset`](table::Dataset) hold the data.
//! - [`Value`] is the scalar a channel reads.
//! - [`Mark`](mark::Mark) covers Plot's mark types: dot, line, area, bar, rect,
//!   cell, rule, text, tick, vector, box, violin, hexbin, density, contour,
//!   pie, arc, waffle and link.
//! - [`transform`] holds Plot's transforms: group, bin, stack, window, sort,
//!   filter, normalize.
//! - [`scale`] holds the scales and color ranges.
//! - [`Plot`](plot::Plot) assembles them and renders as an element.

// GPUI Kit re-exports GPUI at its root; the alias lets this crate and the
// `IntoPlot` derive spell GPUI paths the way the rest of the ecosystem does.
extern crate gpui_kit as gpui;

pub mod density;
pub mod mark;
pub mod scale;
pub mod style;
pub mod table;
pub mod ticks;
pub mod transform;
pub mod value;

pub use scale::{
    BandScale, ColorRange, ColorScale, ContinuousScale, OrdinalScale, PointScale, PositionScale,
    Transform,
};
pub use style::{ColorChannel, NumberChannel, Symbol};
pub use table::{Channel, Dataset, IntoChannel, Table};
pub use transform::{
    Anchor, Bin, Reducer, StackOffset, StackedPoint, StackedSeries, Thresholds, WindowReducer,
};
pub use value::{PlotKind, Value};
