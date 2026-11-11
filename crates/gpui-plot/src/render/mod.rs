//! Mark rendering: resolve a mark's channels against the data, then draw the
//! shapes Plot's mark types describe.
//!
//! Resolution and drawing are separate: [`prepare`] turns a [`Mark`] and a
//! [`Dataset`] into [`PreparedMark`]s, which hold plain values, and the frame
//! unions their values to build the scales. Drawing then only reads pixels.
//!
//! [`prepare`]: fn@prepare
//! [`Dataset`]: crate::table::Dataset

pub(crate) mod basic;
mod bin;
mod bind;
pub mod color;
mod defaults;
pub(crate) mod delaunay;
mod dodge;
pub(crate) mod furniture;
mod group;
mod hexbin;
mod interval;
mod map;
mod normalize;
mod prepare;
pub mod radial;
mod raster;
mod reduce;
mod resolve;
mod scales;
mod select;
mod sort;
pub mod stack;
mod stack_rows;
pub(crate) mod stat;
#[cfg(test)]
mod tests;
mod window;

pub use bin::bin_axes;
pub use defaults::with_value_defaults;
pub use dodge::apply_dodge;
pub use hexbin::apply_hexbin;
pub(crate) use prepare::prepare_facets_reading;
pub use prepare::{PreparedMark, PreparedRow, prepare, prepare_facets};
pub use raster::{raster_extent, resolve_raster};
pub use resolve::constant_symbol;
pub use scales::{apply_quantity_scales, apply_scale_transforms, apply_symbol_scale};

use gpui::{Hsla, SharedString};

use crate::mark::Mark;
use crate::table::Table;
use crate::transform::{Anchor, StackOffset, Thresholds};
use crate::value::Value;

/// A value channel as text, for a mark's label.
pub fn channel_text(channel: &Option<crate::table::Channel>, row: &Table) -> Option<SharedString> {
    channel.as_ref().map(|channel| channel.read(row).label())
}

/// The default color of a mark at `index` in the mark list.
pub fn mark_color(theme: &crate::theme::PlotTheme, index: usize) -> Hsla {
    theme.palette_color(index)
}

/// The tick count a mark wants on its value axis.
pub fn tick_count(mark: &Mark) -> usize {
    let _ = mark;
    5
}

/// A mark's rows grouped by a categorical field, keeping first-occurrence
/// order — the grouping a box, violin or density mark draws per category.
pub fn group_rows<'a>(
    rows: &'a [PreparedRow],
    field: &dyn Fn(&PreparedRow) -> Value,
) -> Vec<(Value, Vec<&'a PreparedRow>)> {
    let mut groups: Vec<(Value, Vec<&PreparedRow>)> = Vec::new();
    for row in rows {
        let key = field(row);
        if key.is_null() {
            continue;
        }
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }
    groups
}

/// The anchor a windowed mark uses.
pub fn window_anchor(mark: &Mark) -> Anchor {
    mark.window.map(|(_, _, anchor)| anchor).unwrap_or_default()
}

/// The bin thresholds a mark uses.
pub fn thresholds(mark: &Mark) -> Thresholds {
    mark.bin
        .as_ref()
        .map(|bin| bin.thresholds)
        .unwrap_or_default()
}

/// The stack offset a mark uses.
pub fn stack_offset(mark: &Mark) -> StackOffset {
    mark.stack
        .as_ref()
        .map(|stack| stack.offset)
        .unwrap_or_default()
}
