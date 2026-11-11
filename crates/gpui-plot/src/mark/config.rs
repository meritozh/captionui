//! The transform settings a mark carries: the stack, bin, group, sort, select,
//! map and shift configurations, domain sorts, density options, row filters and
//! `Axis`.

use gpui::SharedString;

use crate::transform::{GroupReducer, Selector, StackOffset, Thresholds};

/// How a mark's data is stacked, Plot's `stack` option.
#[derive(Clone, Debug, PartialEq)]
pub struct StackConfig {
    /// Which offset the stack uses.
    pub offset: StackOffset,
    /// The field naming the series when the data is long, Plot's `z`.
    pub series_field: Option<SharedString>,
    /// The value fields to stack when the data is wide, one per series: Plot's
    /// `Plot.stackY({y1: "a", y2: "b"})` names its series as channels.
    pub fields: Vec<SharedString>,
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            offset: StackOffset::Zero,
            series_field: None,
            fields: Vec::new(),
        }
    }
}

/// How a mark's data is binned, Plot's `bin`, `binX` and `binY`.
///
/// The axes binned follow the mark: `rectY` bins x (`binX`), `rectX` bins y
/// (`binY`), and a `rect` naming both x and y bins both (`bin`). Each bin is
/// formed within its `z` series, so `fill: "sex"` bins each sex apart.
#[derive(Clone, Debug)]
pub struct BinConfig {
    pub thresholds: Thresholds,
    /// The outputs, Plot's bin outputs `{y: "count"}` or
    /// `{fillOpacity: "count"}`. With none, a one-dimensional bin counts into
    /// its length channel (`y` for `binX`, `x` for `binY`).
    pub outputs: Vec<GroupOutput>,
    /// Edges fixed ahead of binning, `[x, y]`: a faceted plot computes them
    /// from the whole data so every facet's bins line up. `None` computes
    /// them from the rows being binned.
    pub edges: [Option<crate::transform::BinEdges>; 2],
    /// Plot's `cumulative`: positive, each bin also holds every bin before
    /// it; negative, every bin after it (the complementary distribution);
    /// zero, a plain histogram.
    pub cumulative: i8,
    /// Plot's `filter: null`: keep the empty bins rather than dropping them,
    /// so a line or area over the bins falls to zero between the data.
    pub keep_empty: bool,
}

impl Default for BinConfig {
    fn default() -> Self {
        Self {
            thresholds: Thresholds::Auto,
            outputs: Vec::new(),
            edges: [None, None],
            cumulative: 0,
            keep_empty: false,
        }
    }
}

/// How a mark groups its rows, Plot's `groupX`, `groupY` and `groupZ`.
///
/// A group collapses the rows that share a category into one row per
/// category, each output channel reduced over the rows that formed it. Plot
/// splits the work between the axis the group reads — `groupX` groups by x,
/// `groupY` by y, `groupZ` by `z` alone — and the other channels it carries
/// along.
#[derive(Clone, Debug)]
pub struct GroupConfig {
    /// The axis the groups read, or `None` for `groupZ`, which groups by the
    /// `z` channel alone.
    pub axis: Option<Axis>,
    /// The outputs: which channel each group's reduction lands in, and how.
    /// Empty means Plot's default, a count into the length channel.
    pub outputs: Vec<GroupOutput>,
    /// Plot's `group`: group by x and y together.
    pub both: bool,
}

/// One grouped output channel: a field or channel name and its reducer.
///
/// Plot names outputs by channel — `{y: "count", fill: "mean"}` — and the
/// reducer reads that channel's own input unless it names one. `"count"`
/// takes no input at all, and a reducer that needs a value falls back to the
/// channel it writes, so `{y: "mean"}` averages the y values.
#[derive(Clone, Debug)]
pub struct GroupOutput {
    /// The channel the reduced value lands in, Plot's output name: `"x"`,
    /// `"y"`, `"x1"`, `"y2"`, `"fill"`, or a field name.
    pub name: SharedString,
    /// How the group is reduced.
    pub reducer: GroupReducer,
    /// The field the reducer reads, when it differs from the output's own
    /// channel; Plot's `{y: {value: "v", reduce: "mean"}}`.
    pub input: Option<SharedString>,
}

impl GroupOutput {
    /// An output writing `name`, reduced with `reducer`, reading that channel.
    pub fn new(name: impl Into<SharedString>, reducer: GroupReducer) -> Self {
        Self {
            name: name.into(),
            reducer,
            input: None,
        }
    }

    /// Read a field other than the output's own channel.
    pub fn input(mut self, field: impl Into<SharedString>) -> Self {
        self.input = Some(field.into());
        self
    }
}

/// How a mark's data is ordered, Plot's `sort` option.
#[derive(Clone, Debug, Default)]
pub struct SortConfig {
    /// The field to sort by; `None` sorts by the mark's value.
    pub field: Option<SharedString>,
    /// Whether to sort descending.
    pub descending: bool,
    /// Plot's `reverse`, applied after sorting.
    pub reverse: bool,
}

/// One entry of Plot's mark `sort` option: order the `scale`'s domain by the
/// `by` channel's values, reduced per domain value (`max` by default).
#[derive(Clone, Debug)]
pub struct DomainSort {
    /// The scale whose domain is sorted: `"x"`, `"y"`, `"fx"`, `"fy"`,
    /// `"color"`.
    pub scale: SharedString,
    /// The channel to sort by: `"x"`, `"x1"`, `"y"`, `"r"`, `"length"` … or
    /// `"data"` for the input order.
    pub by: SharedString,
    pub reduce: GroupReducer,
    pub reverse: bool,
    /// Keep only the first `limit` values, Plot's `limit`.
    pub limit: Option<usize>,
}

/// Plot's density options.
#[derive(Clone, Default)]
pub struct DensitySpec {
    /// Plot's `bandwidth`, in pixels (20 by default).
    pub bandwidth: Option<f32>,
    /// Plot's `thresholds`: a count (20 by default).
    pub count: Option<usize>,
    /// Plot's explicit `thresholds`, in Plot's units (density × 100).
    pub values: Option<Vec<f64>>,
    /// Plot's `weight`.
    #[allow(clippy::type_complexity)]
    pub weight: Option<std::rc::Rc<dyn Fn(&crate::table::Table) -> f64>>,
}

impl std::fmt::Debug for DensitySpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DensitySpec")
            .field("bandwidth", &self.bandwidth)
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

/// A predicate over records, Plot's `filter` option.
#[derive(Clone)]
pub struct RowFilter(pub std::rc::Rc<dyn Fn(&crate::table::Table) -> bool>);

impl std::fmt::Debug for RowFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RowFilter(..)")
    }
}

/// A [`Mark::select`](crate::mark::Mark::select) configuration, Plot's `select`.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectConfig {
    /// Which row of each group the mark keeps.
    pub selector: Selector,
    /// The field that forms the groups, Plot's `z`.
    pub group: Option<SharedString>,
    /// The axis the selector reads, for `selectMinX`/`selectMaxX`: `None`
    /// picks the first or last row rather than an extreme.
    pub axis: Option<Axis>,
}

/// Which axis a select's extreme reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// The x channel.
    X,
    /// The y channel.
    Y,
}

/// A [`Mark::map`](crate::mark::Mark::map) configuration, Plot's `mapX`/`mapY`.
#[derive(Clone, Debug, PartialEq)]
pub struct MapConfig {
    /// The channel the mapper reads: `"x"`, `"y"` or a field name.
    pub input: SharedString,
    /// The field the mapped value is written to.
    pub output: SharedString,
    /// How the series is folded: the running total, the running extreme, the
    /// running mean or the datum's rank.
    pub reducer: MapReducer,
    /// The field that restarts the fold, Plot's `z`.
    pub group: Option<SharedString>,
}

/// How [`MapConfig`] folds a series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapReducer {
    /// The running total.
    Sum,
    /// The smallest value so far.
    Min,
    /// The largest value so far.
    Max,
    /// The mean of every value so far.
    Mean,
    /// The datum's position in the series, from zero.
    Rank,
}

/// The channels a [`Mapper`](crate::transform::Mapper) rewrites.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapTarget {
    /// Plot's `mapX`: each of `x`, `x1` and `x2` the mark has (`x`, the
    /// datum itself, when it has none).
    X,
    /// Plot's `mapY`: each of `y`, `y1` and `y2` the mark has.
    Y,
    /// One named channel, Plot's `map({name: mapper})`: a position (`x`,
    /// `x1`, `x2`, `y`, `y1`, `y2`), `fill`, `stroke`, `r` or `opacity`. An
    /// interval end the mark lacks reads its axis's channel, Plot's
    /// `maybeInput`.
    Channel(SharedString),
}

/// A [`Mark::shift`](crate::mark::Mark::shift) configuration, Plot's `shiftX`/`shiftY`.
#[derive(Clone, Debug, PartialEq)]
pub struct ShiftConfig {
    /// The channel the shift reads: `"x"`, `"y"` or a field name.
    pub input: SharedString,
    /// The field the shifted value is written to.
    pub output: SharedString,
    /// How far to move it: a number of units, or days for a date.
    pub offset: f64,
}
