//! Data transforms: Plot's `Plot.transform` functions.
//!
//! Plot applies transforms between the data and the marks — binning for
//! histograms and hexbins, stacking for areas and bars, grouping for
//! aggregation, windowing for moving averages, and normalization. Each
//! transform here reads a [`Dataset`](crate::table::Dataset) and returns a new one, so they compose
//! the way Plot's do.

mod bin;
mod dodge;
mod group;
mod map;
mod normalize;
mod reduce;
mod select;
mod shift;
mod sort;
mod stack;
mod window;

pub use bin::{Bin, BinEdges, Thresholds, bin};
pub use dodge::{Dodge, DodgeAnchor, dodge, dodge_positions};
pub use group::group;
pub use map::{Mapper, map_field, map_values};
pub use normalize::normalize;
pub(crate) use reduce::OrderStatistics;
pub use reduce::{GroupReducer, Reducer};
pub use select::{Select, Selector, select};
pub use shift::shift;
pub use sort::{filter, reverse, shuffle, sort_by};
pub use stack::{StackOffset, StackedPoint, StackedSeries, stack_long, stack_wide};
pub use window::{Anchor, WindowReducer, window, window_map, window_values};

#[cfg(test)]
mod tests;
