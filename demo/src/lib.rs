//! Observable Plot's documentation examples, ported: the datasets, and one
//! chart per example organized as the docs are. Shared by the demo window,
//! the snapshot renderer and the interaction tests.

pub mod data;
pub mod examples;

/// What every example file uses.
pub mod prelude {
    pub use crate::data;
    pub use crate::examples::{Example, example};
    pub use chrono::Datelike;
    pub use gpui_plot::ColorChannel;
    pub use gpui_plot::format::{format_number, format_time};
    pub use gpui_plot::mark::*;
    pub use gpui_plot::marks::*;
    pub use gpui_plot::{
        AxisAnchor, Channel, Dataset, GroupReducer, LabelAnchor, LegendPosition, NormalizeBasis,
        Offset, Plot, Reducer, ScaleType, Selector, StackOffset, StackOrder, Table, Thresholds,
        Value,
    };
}
