//! Data transforms: Plot's `Plot.transform` functions.
//!
//! Plot applies transforms between the data and the marks — binning for
//! histograms and hexbins, stacking for areas and bars, grouping for
//! aggregation, windowing for moving averages, and normalization. Each
//! transform here reads a [`Dataset`] and returns a new one, so they compose
//! the way Plot's do.

use std::collections::BTreeMap;

use gpui::SharedString;

use crate::scale::dedup;
use crate::table::{Dataset, Table};
use crate::value::{Value, ValueKey};

/// Aggregate a field across rows sharing a group, Plot's `groupY` and
/// `groupX` with a reducer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reducer {
    /// The number of rows.
    #[default]
    Count,
    /// The sum of the values.
    Sum,
    /// The arithmetic mean, ignoring nulls.
    Mean,
    /// The smallest value.
    Min,
    /// The largest value.
    Max,
    /// The first value, ignoring nulls.
    First,
    /// The last value, ignoring nulls.
    Last,
    /// The median of the values.
    Median,
    /// The standard deviation of the values.
    Deviation,
    /// The sum of squared values, for a root-mean-square.
    SumOfSquares,
}

impl Reducer {
    /// Reduce `values` to one value.
    pub fn reduce(self, values: &[Value]) -> Value {
        let numbers: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
        match self {
            Reducer::Count => Value::Number(values.len() as f64),
            Reducer::Sum => Value::Number(numbers.iter().sum()),
            Reducer::Mean => {
                if numbers.is_empty() {
                    Value::Null
                } else {
                    Value::Number(numbers.iter().sum::<f64>() / numbers.len() as f64)
                }
            }
            Reducer::Min => numbers
                .iter()
                .copied()
                .reduce(f64::min)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Reducer::Max => numbers
                .iter()
                .copied()
                .reduce(f64::max)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Reducer::First => values
                .iter()
                .find(|v| !v.is_null())
                .cloned()
                .unwrap_or(Value::Null),
            Reducer::Last => values
                .iter()
                .rev()
                .find(|v| !v.is_null())
                .cloned()
                .unwrap_or(Value::Null),
            Reducer::Median => {
                if numbers.is_empty() {
                    return Value::Null;
                }
                let mut sorted = numbers.clone();
                sorted.sort_by(f64::total_cmp);
                let mid = sorted.len() / 2;
                Value::Number(if sorted.len().is_multiple_of(2) {
                    (sorted[mid - 1] + sorted[mid]) / 2.
                } else {
                    sorted[mid]
                })
            }
            Reducer::Deviation => {
                if numbers.len() < 2 {
                    return Value::Number(0.);
                }
                let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
                let variance = numbers.iter().map(|n| (n - mean).powi(2)).sum::<f64>()
                    / (numbers.len() - 1) as f64;
                Value::Number(variance.sqrt())
            }
            Reducer::SumOfSquares => Value::Number(numbers.iter().map(|n| n * n).sum()),
        }
    }
}

/// The reducer one grouped output applies, Plot's group output reducers.
///
/// Unlike [`Reducer`], which folds a single field, a group output also has the
/// group's records: `find` picks a row by a predicate and returns that row's
/// value, which is how a chart picks a series' reading by something other than
/// its own extent.
#[derive(Clone)]
pub enum GroupReducer {
    /// The number of rows in the group, Plot's `"count"`.
    Count,
    /// The number of distinct values, Plot's `"distinct"`.
    Distinct,
    /// The sum of the values, Plot's `"sum"`.
    Sum,
    /// The arithmetic mean, Plot's `"mean"`.
    Mean,
    /// The smallest value, Plot's `"min"`.
    Min,
    /// The largest value, Plot's `"max"`.
    Max,
    /// The median, Plot's `"median"`.
    Median,
    /// The standard deviation, Plot's `"deviation"`.
    Deviation,
    /// The group's first value, Plot's `"first"`.
    First,
    /// The group's last value, Plot's `"last"`.
    Last,
    /// The value at the first row the test accepts, Plot's `Plot.find`.
    Find(std::rc::Rc<dyn Fn(&Table) -> bool>),
}

impl GroupReducer {
    /// A reducer that returns the value of the first row matching `test`,
    /// Plot's `Plot.find`.
    pub fn find(test: impl Fn(&Table) -> bool + 'static) -> Self {
        Self::Find(std::rc::Rc::new(test))
    }

    /// Reduce one group: its values, and the records they came from.
    pub fn reduce(&self, values: &[Value], rows: &[&Table]) -> Value {
        let simple = match self {
            GroupReducer::Count => Some(Reducer::Count),
            GroupReducer::Sum => Some(Reducer::Sum),
            GroupReducer::Mean => Some(Reducer::Mean),
            GroupReducer::Min => Some(Reducer::Min),
            GroupReducer::Max => Some(Reducer::Max),
            GroupReducer::Median => Some(Reducer::Median),
            GroupReducer::Deviation => Some(Reducer::Deviation),
            GroupReducer::First => Some(Reducer::First),
            GroupReducer::Last => Some(Reducer::Last),
            GroupReducer::Distinct | GroupReducer::Find(_) => None,
        };
        if let Some(reducer) = simple {
            return reducer.reduce(values);
        }
        match self {
            GroupReducer::Distinct => {
                let mut seen: Vec<ValueKey> = Vec::new();
                for value in values {
                    let key = value.key();
                    if !seen.contains(&key) {
                        seen.push(key);
                    }
                }
                Value::Number(seen.len() as f64)
            }
            GroupReducer::Find(test) => values
                .iter()
                .zip(rows.iter())
                .find(|(_, row)| test(row))
                .map(|(value, _)| value.clone())
                .unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }
}

impl std::fmt::Debug for GroupReducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            GroupReducer::Count => "Count",
            GroupReducer::Distinct => "Distinct",
            GroupReducer::Sum => "Sum",
            GroupReducer::Mean => "Mean",
            GroupReducer::Min => "Min",
            GroupReducer::Max => "Max",
            GroupReducer::Median => "Median",
            GroupReducer::Deviation => "Deviation",
            GroupReducer::First => "First",
            GroupReducer::Last => "Last",
            GroupReducer::Find(_) => "Find(..)",
        };
        f.write_str(name)
    }
}

/// Group rows by `key` and reduce each group's `value` into a new field.
///
/// This is Plot's `Plot.groupY`/`Plot.groupX` pair: the groups keep the order
/// they first appear in, so a categorical axis reads in data order.
pub fn group(data: &Dataset, key: &str, value: &str, reducer: Reducer, output: &str) -> Dataset {
    let mut groups: Vec<(ValueKey, Value, Vec<Value>)> = Vec::new();
    for row in data.rows() {
        let group_value = row.value(key);
        let group_key = group_value.key();
        match groups.iter_mut().find(|(k, _, _)| *k == group_key) {
            Some((_, _, values)) => values.push(row.value(value)),
            None => groups.push((group_key, group_value, vec![row.value(value)])),
        }
    }

    let rows: Vec<Table> = groups
        .into_iter()
        .map(|(_, group_value, values)| {
            let reduced = reducer.reduce(&values);
            Table::empty().with(key, group_value).with(output, reduced)
        })
        .collect();
    Dataset::new(rows)
}

/// How a [`bin`] computes its bins, Plot's `thresholds` option.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Thresholds {
    /// About this many evenly spaced bins; Plot's `thresholds` number.
    Count(usize),
    /// One bin per tick of the scale's nice ticks, Plot's `thresholds` with a
    /// tick generator.
    #[default]
    Auto,
    /// Bins of this fixed width, Plot's `interval`.
    Interval(f64),
}

/// A histogram bin: the interval `[x0, x1)` and the rows that fell in it.
#[derive(Clone, Debug)]
pub struct Bin {
    pub x0: f64,
    pub x1: f64,
    pub rows: Vec<usize>,
}

impl Bin {
    pub fn count(&self) -> f64 {
        self.rows.len() as f64
    }
}

/// Bin rows by a numeric field, Plot's `Plot.binX`/`Plot.binY`.
///
/// Returns the bins and the domain the bins span, so a mark can draw them
/// without recomputing the extent.
pub fn bin(data: &Dataset, field: &str, thresholds: Thresholds) -> (Vec<Bin>, [f64; 2]) {
    let values: Vec<(usize, f64)> = data
        .rows()
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            row.get(field)
                .and_then(Value::as_number)
                .map(|v| (index, v))
        })
        .collect();

    let Some(extent) = crate::scale::extent(values.iter().map(|(_, v)| Value::Number(*v))) else {
        return (Vec::new(), [0., 1.]);
    };

    let [lo, hi] = extent;
    let (lo, hi) = if lo == hi {
        (lo - 0.5, hi + 0.5)
    } else {
        (lo, hi)
    };
    let count = match thresholds {
        Thresholds::Count(count) => count.max(1),
        Thresholds::Auto => 10,
        Thresholds::Interval(width) => {
            (((hi - lo) / width.max(f64::MIN_POSITIVE)).ceil() as usize).max(1)
        }
    };
    let width = match thresholds {
        Thresholds::Interval(width) => width.max(f64::MIN_POSITIVE),
        _ => nice_bin_width((hi - lo) / count as f64),
    };

    let mut bins: Vec<Bin> = Vec::new();
    let mut edge = lo;
    while edge < hi {
        bins.push(Bin {
            x0: edge,
            x1: edge + width,
            rows: Vec::new(),
        });
        edge += width;
    }
    if bins.is_empty() {
        bins.push(Bin {
            x0: lo,
            x1: hi,
            rows: Vec::new(),
        });
    }

    for (index, value) in values {
        let slot = ((value - lo) / width).floor() as isize;
        let slot = slot.clamp(0, bins.len() as isize - 1) as usize;
        bins[slot].rows.push(index);
    }

    let domain = [bins[0].x0, bins[bins.len() - 1].x1];
    (bins, domain)
}

fn nice_bin_width(rough: f64) -> f64 {
    if rough <= 0. || !rough.is_finite() {
        return 1.;
    }
    let exponent = rough.log10().floor();
    let base = 10f64.powf(exponent);
    let fraction = rough / base;
    let nice = if fraction <= 1. {
        1.
    } else if fraction <= 2. {
        2.
    } else if fraction <= 5. {
        5.
    } else {
        10.
    };
    nice * base
}

/// How stacked series are accumulated, Plot's `offset`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackOffset {
    /// Each series sits on the sum of the ones before it, from zero; d3's
    /// `stackOffsetNone`.
    #[default]
    Zero,
    /// Each series is normalized so the total is 1 at every x; d3's
    /// `stackOffsetExpand`, which Plot uses for percentage charts.
    Normalize,
    /// Series are centered on the baseline, showing a divergence around zero;
    /// d3's `stackOffsetSilhouette`.
    Center,
    /// Series overlap instead of stacking, Plot's `stack: false`.
    None,
}

/// One stacked series.
#[derive(Clone, Debug)]
pub struct StackedSeries {
    /// The series' key, in the order it appears in the data.
    pub key: SharedString,
    /// One point per x category: its lower and upper value.
    pub points: Vec<StackedPoint>,
}

/// A stacked point: the x category and the series' extent at it.
#[derive(Clone, Debug)]
pub struct StackedPoint {
    pub x: Value,
    pub y0: f64,
    pub y1: f64,
}

/// Stack a wide table: `x` names the category field and `keys` the fields that
/// become series, each field's value stacking at that x.
///
/// This is Plot's `Plot.stackY` over a wide input, where each numeric column is
/// a series.
pub fn stack_wide(
    data: &Dataset,
    x: &str,
    keys: &[&str],
    offset: StackOffset,
) -> Vec<StackedSeries> {
    let categories: Vec<Value> = dedup(data.rows().iter().map(|row| row.value(x).key()))
        .into_iter()
        .map(|key| key.to_value())
        .collect();

    let mut series: Vec<StackedSeries> = keys
        .iter()
        .map(|key| StackedSeries {
            key: (*key).into(),
            points: Vec::new(),
        })
        .collect();

    // A value per (series, category), summing rows that share both.
    let mut grid: Vec<Vec<f64>> = vec![vec![0.; categories.len()]; keys.len()];
    for row in data.rows() {
        let x_value = row.value(x);
        let Some(category) = categories.iter().position(|c| c.key() == x_value.key()) else {
            continue;
        };
        for (series_index, key) in keys.iter().enumerate() {
            if let Some(value) = row.number(key) {
                grid[series_index][category] += value;
            }
        }
    }

    let totals: Vec<f64> = (0..categories.len())
        .map(|category| grid.iter().map(|row| row[category]).sum())
        .collect();

    // The running total of the series drawn before this one, per category. It
    // lives outside the series loop: each series sits on everything stacked
    // under it, which is what stacking means.
    let mut lower = vec![0.; categories.len()];
    for (series_index, stacked) in series.iter_mut().enumerate() {
        for (category, category_value) in categories.iter().enumerate() {
            let value = grid[series_index][category];
            let (y0, y1) = match offset {
                StackOffset::Zero => (lower[category], lower[category] + value),
                StackOffset::Normalize => {
                    let total = totals[category];
                    let base = if total == 0. {
                        0.
                    } else {
                        lower[category] / total
                    };
                    let top = if total == 0. {
                        0.
                    } else {
                        (lower[category] + value) / total
                    };
                    (base, top)
                }
                StackOffset::Center => {
                    let total = totals[category];
                    let base = -total / 2. + lower[category];
                    (base, base + value)
                }
                StackOffset::None => (0., value),
            };
            lower[category] += value;
            stacked.points.push(StackedPoint {
                x: category_value.clone(),
                y0,
                y1,
            });
        }
    }

    series
}

/// Long-to-wide: Plot's `Plot.stackY` over long data, where one field names the
/// series and another holds its value.
pub fn stack_long(
    data: &Dataset,
    x: &str,
    series_field: &str,
    value_field: &str,
    offset: StackOffset,
) -> Vec<StackedSeries> {
    let mut series_keys: Vec<ValueKey> = Vec::new();
    let mut series_labels: Vec<SharedString> = Vec::new();
    let mut categories: Vec<ValueKey> = Vec::new();

    let mut grid: BTreeMap<(ValueKey, ValueKey), f64> = BTreeMap::new();
    for row in data.rows() {
        let x_value = row.value(x);
        let series_value = row.value(series_field);
        let Some(value) = row.number(value_field) else {
            continue;
        };
        if !categories.contains(&x_value.key()) {
            categories.push(x_value.key());
        }
        let series_key = series_value.key();
        if !series_keys.contains(&series_key) {
            series_keys.push(series_key.clone());
            series_labels.push(series_value.label());
        }
        *grid.entry((series_key, x_value.key())).or_insert(0.) += value;
    }

    let categories: Vec<Value> = categories.into_iter().map(|key| key.to_value()).collect();

    let mut series: Vec<StackedSeries> = series_labels
        .into_iter()
        .map(|key| StackedSeries {
            key,
            points: Vec::new(),
        })
        .collect();

    let grid: Vec<Vec<f64>> = series_keys
        .iter()
        .map(|key| {
            categories
                .iter()
                .map(|category| {
                    grid.get(&(key.clone(), category.key()))
                        .copied()
                        .unwrap_or(0.)
                })
                .collect()
        })
        .collect();

    let totals: Vec<f64> = (0..categories.len())
        .map(|category| grid.iter().map(|row| row[category]).sum())
        .collect();

    // The running total of the series drawn before this one, per category. It
    // lives outside the series loop: each series sits on everything stacked
    // under it, which is what stacking means.
    let mut lower = vec![0.; categories.len()];
    for (series_index, stacked) in series.iter_mut().enumerate() {
        for (category, category_value) in categories.iter().enumerate() {
            let value = grid[series_index][category];
            let (y0, y1) = match offset {
                StackOffset::Zero => (lower[category], lower[category] + value),
                StackOffset::Normalize => {
                    let total = totals[category];
                    let base = if total == 0. {
                        0.
                    } else {
                        lower[category] / total
                    };
                    let top = if total == 0. {
                        0.
                    } else {
                        (lower[category] + value) / total
                    };
                    (base, top)
                }
                StackOffset::Center => {
                    let total = totals[category];
                    let base = -total / 2. + lower[category];
                    (base, base + value)
                }
                StackOffset::None => (0., value),
            };
            lower[category] += value;
            stacked.points.push(StackedPoint {
                x: category_value.clone(),
                y0,
                y1,
            });
        }
    }

    series
}

/// A window transform's reducer, Plot's `Plot.windowY` with a reducer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowReducer {
    /// A moving mean.
    #[default]
    Mean,
    /// A moving sum.
    Sum,
    /// A moving median.
    Median,
    /// The smallest value in the window.
    Min,
    /// The largest value in the window.
    Max,
    /// The cumulative mean.
    CumulativeMean,
    /// The difference between the current and previous value, Plot's `difference`.
    Difference,
    /// The value's change from the first value, Plot's `change`.
    Change,
    /// The percent change from the previous value.
    PercentChange,
}

/// A window transform over a numeric field, Plot's `Plot.windowY`.
///
/// `k` is the window width; `None` centers the window, and `anchor` decides
/// where a window sits when it cannot be centered (`center`, `start`, `end`).
pub fn window(
    data: &Dataset,
    field: &str,
    k: usize,
    reducer: WindowReducer,
    anchor: Anchor,
) -> Dataset {
    let values: Vec<Value> = data.values(field);
    let numbers: Vec<Option<f64>> = values.iter().map(Value::as_number).collect();
    let k = k.max(1);
    let mut output = Vec::with_capacity(numbers.len());

    for index in 0..numbers.len() {
        let (start, end) = window_bounds(index, numbers.len(), k, anchor);
        let slice: Vec<f64> = numbers[start..end].iter().filter_map(|v| *v).collect();
        let result = match reducer {
            WindowReducer::Mean => {
                (!slice.is_empty()).then(|| slice.iter().sum::<f64>() / slice.len() as f64)
            }
            WindowReducer::Sum => (!slice.is_empty()).then(|| slice.iter().sum::<f64>()),
            WindowReducer::Median => {
                if slice.is_empty() {
                    None
                } else {
                    let mut sorted = slice.clone();
                    sorted.sort_by(f64::total_cmp);
                    let mid = sorted.len() / 2;
                    Some(if sorted.len().is_multiple_of(2) {
                        (sorted[mid - 1] + sorted[mid]) / 2.
                    } else {
                        sorted[mid]
                    })
                }
            }
            WindowReducer::Min => slice.iter().copied().reduce(f64::min),
            WindowReducer::Max => slice.iter().copied().reduce(f64::max),
            WindowReducer::CumulativeMean => {
                let slice: Vec<f64> = numbers[..=index].iter().filter_map(|v| *v).collect();
                (!slice.is_empty()).then(|| slice.iter().sum::<f64>() / slice.len() as f64)
            }
            WindowReducer::Difference => {
                let current = numbers[index];
                let previous = index.checked_sub(1).and_then(|i| numbers[i]);
                match (current, previous) {
                    (Some(current), Some(previous)) => Some(current - previous),
                    _ => None,
                }
            }
            WindowReducer::Change => {
                let current = numbers[index];
                let first = numbers.iter().find_map(|v| *v);
                match (current, first) {
                    (Some(current), Some(first)) => Some(current - first),
                    _ => None,
                }
            }
            WindowReducer::PercentChange => {
                let current = numbers[index];
                let previous = index.checked_sub(1).and_then(|i| numbers[i]);
                match (current, previous) {
                    (Some(current), Some(previous)) if previous != 0. => {
                        Some((current - previous) / previous)
                    }
                    _ => None,
                }
            }
        };
        output.push(result.map(Value::Number).unwrap_or(Value::Null));
    }

    Dataset::new(
        data.rows()
            .iter()
            .cloned()
            .zip(output)
            .map(|(row, value)| row.with(field, value)),
    )
}

/// Where a window sits when it cannot be centered on its point, Plot's
/// `anchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// Keep the point in the middle of the window. (Default.)
    #[default]
    Middle,
    /// The window starts at the point.
    Start,
    /// The window ends at the point.
    End,
}

fn window_bounds(index: usize, len: usize, k: usize, anchor: Anchor) -> (usize, usize) {
    // A centered window reaches `left` back and `right` forward, so a window of
    // three takes one neighbor on each side and one at the ends takes only what
    // the data has — d3's partial rolling window.
    let left = k / 2;
    let right = k.saturating_sub(left + 1);
    let (start, end) = match anchor {
        Anchor::Middle => (index.saturating_sub(left), (index + right + 1).min(len)),
        Anchor::Start => (index, (index + k).min(len)),
        Anchor::End => (index.saturating_sub(k.saturating_sub(1)), index + 1),
    };
    (start, end)
}

/// Normalize a numeric field to `0..=1` across the dataset, Plot's
/// `Plot.normalizeY` with `percent` off.
pub fn normalize(data: &Dataset, field: &str) -> Dataset {
    let extent = crate::scale::extent(data.values(field));
    let Some([lo, hi]) = extent else {
        return data.clone();
    };
    let span = hi - lo;
    Dataset::new(data.rows().iter().cloned().map(|row| {
        let normalized = row
            .number(field)
            .map(|value| if span == 0. { 0.5 } else { (value - lo) / span });
        row.with(field, normalized.map(Value::Number).unwrap_or(Value::Null))
    }))
}

/// Sort rows by a field, Plot's `Plot.sort`.
pub fn sort_by(data: &Dataset, field: &str, descending: bool) -> Dataset {
    let mut rows: Vec<Table> = data.rows().to_vec();
    rows.sort_by(|a, b| {
        let ordering = a.value(field).cmp(&b.value(field));
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    Dataset::new(rows)
}

/// Keep rows whose `field` satisfies `predicate`, Plot's `Plot.filter`.
pub fn filter(data: &Dataset, field: &str, predicate: impl Fn(&Value) -> bool) -> Dataset {
    Dataset::new(
        data.rows()
            .iter()
            .filter(|row| predicate(&row.value(field)))
            .cloned(),
    )
}

/// Reverse row order, Plot's `Plot.reverse`.
pub fn reverse(data: &Dataset) -> Dataset {
    Dataset::new(data.rows().iter().rev().cloned())
}

/// Where a dodge places its points relative to the axis it packs along.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DodgeAnchor {
    /// Pack from the low end of the axis. (Default for `dodge_x`.)
    #[default]
    Start,
    /// Pack symmetrically around the axis's middle.
    Middle,
    /// Pack from the high end of the axis.
    End,
}

/// A dodge transform's configuration.
#[derive(Clone, Copy, Debug)]
pub struct Dodge {
    /// Where the points pack from.
    pub anchor: DodgeAnchor,
    /// The radius of each point, in the units of the packed axis. Plot's
    /// default is the mark's dot radius.
    pub radius: f32,
    /// The gap between neighbouring points, Plot's `padding`.
    pub padding: f32,
}

impl Default for Dodge {
    fn default() -> Self {
        Self {
            anchor: DodgeAnchor::Start,
            radius: 3.,
            padding: 1.,
        }
    }
}

/// Pack one-dimensional points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
///
/// Given the position `field` along the axis the data varies on, this computes
/// the *other* axis so that no two points overlap: each point is placed at the
/// nearest free offset from the baseline where a circle of `radius` fits.
/// The result is written back under `output`, in the packed axis's units.
///
/// Plot runs this in pixel space, against the resolved scale; here the caller
/// passes positions already projected, so the units are pixels throughout.
pub fn dodge(
    data: &Dataset,
    positions: &[Option<f32>],
    output: &str,
    config: Dodge,
    extent: [f32; 2],
) -> Dataset {
    let mut placed: Vec<f32> = vec![f32::NAN; data.rows().len()];

    // Points already placed, as (low, high, offset) along the varying axis.
    let mut occupied: Vec<(f32, f32, f32)> = Vec::new();
    let (start, end) = (extent[0].min(extent[1]), extent[0].max(extent[1]));

    for (index, position) in positions.iter().enumerate() {
        let Some(position) = *position else {
            continue;
        };
        let radius = config.radius;
        let padding = config.padding;
        let low = position - radius;
        let high = position + radius;

        // Candidate offsets: the baseline first, then the two places a circle
        // can sit tangent to each already-placed circle that could overlap.
        let mut candidates = vec![0.0_f32];
        let mut neighbours = Vec::new();
        for &(other_low, other_high, other_offset) in &occupied {
            if other_high + padding < low - padding || other_low - padding > high + padding {
                continue;
            }
            let centre = (other_low + other_high) / 2.;
            let dx = position - centre;
            let dr = 2. * radius + padding;
            if dr * dr < dx * dx {
                continue;
            }
            let dy = (dr * dr - dx * dx).sqrt();
            candidates.push(other_offset - dy);
            candidates.push(other_offset + dy);
            neighbours.push((centre, other_offset));
        }

        // Nearest first, so the packed shape stays compact.
        candidates.sort_by(|a, b| a.abs().total_cmp(&b.abs()));

        // Take the first candidate that does not overlap anything placed.
        let needed = 2. * radius + padding;
        let mut chosen = candidates[0];
        'outer: for &candidate in &candidates {
            for &(centre, other_offset) in &neighbours {
                let gap = (candidate - other_offset).abs();
                let across = (position - centre).abs();
                if (gap * gap + across * across).sqrt() < needed - 1e-3 {
                    continue 'outer;
                }
            }
            chosen = candidate;
            break;
        }

        placed[index] = chosen;
        occupied.push((low, high, chosen));
    }

    // The packed offsets are already positions on the axis: the caller passes
    // the extent in the axis's own coordinates, and `Start` packs from its low
    // end, `End` from its high end, `Middle` outward from its centre.
    let rows: Vec<Table> = data
        .rows()
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let offset = placed[index];
            if offset.is_nan() {
                return row.clone();
            }
            let value = match config.anchor {
                DodgeAnchor::Start => start + offset,
                DodgeAnchor::End => end - offset,
                DodgeAnchor::Middle => (start + end) / 2. + offset,
            };
            row.clone().with(output, Value::Number(value as f64))
        })
        .collect();

    Dataset::new(rows)
}

/// Which row of a group a [`Select`] keeps, Plot's `select` options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    /// The first row, Plot's `"first"`.
    First,
    /// The last row, Plot's `"last"`.
    Last,
    /// The row with the smallest value of a field, Plot's `{field: "min"}`.
    Min(SharedString),
    /// The row with the largest value of a field, Plot's `{field: "max"}`.
    Max(SharedString),
}

/// A [`select`] configuration: which row to keep and how to group.
#[derive(Clone, Debug)]
pub struct Select {
    /// Which row of each group survives.
    pub selector: Selector,
    /// The field the groups are formed on, when the data is long; Plot's `z`.
    pub group: Option<SharedString>,
}

/// Keep one row per group, Plot's `selectFirst`, `selectLast` and
/// `selectMinX`/`selectMaxX` (and their y variants).
///
/// Every other row is dropped, which is what lets a mark draw a single
/// labelled point per series — the extreme reading of a line, the first
/// sample of a run — rather than every row the source data holds.
pub fn select(data: &Dataset, config: &Select) -> Dataset {
    // Without a group the whole dataset is one group, which is what makes
    // `selectMinX` pick the single extreme of a series.
    let mut groups: Vec<(ValueKey, Vec<&Table>)> = Vec::new();
    for row in data.rows() {
        let key = config
            .group
            .as_ref()
            .map(|field| row.value(field).key())
            .unwrap_or_else(|| Value::Null.key());
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }

    let kept: Vec<Table> = groups
        .into_iter()
        .filter_map(|(_, members)| {
            let chosen = match &config.selector {
                Selector::First => members.first(),
                Selector::Last => members.last(),
                // `min_by` keeps the first of equal values, which is Plot's
                // `least`: a tie goes to the earlier datum.
                Selector::Min(field) => members
                    .iter()
                    .filter(|row| row.number(field).is_some())
                    .min_by(|a, b| {
                        a.number(field)
                            .unwrap_or(f64::INFINITY)
                            .total_cmp(&b.number(field).unwrap_or(f64::INFINITY))
                    }),
                Selector::Max(field) => members
                    .iter()
                    .filter(|row| row.number(field).is_some())
                    .max_by(|a, b| {
                        a.number(field)
                            .unwrap_or(f64::NEG_INFINITY)
                            .total_cmp(&b.number(field).unwrap_or(f64::NEG_INFINITY))
                    }),
            }?;
            Some((*chosen).clone())
        })
        .collect();
    Dataset::new(kept)
}

/// Write a computed value into a new field on every row, Plot's `mapY` and
/// `mapX`.
///
/// The mapper sees the whole series in order — every row's current value, its
/// index, and the rows — so a cumulative sum, a running maximum or a rank can
/// be built from the values before it. The output keeps the rows' order and
/// their original fields.
pub fn map_field(
    data: &Dataset,
    field: &str,
    output: &str,
    mapper: impl Fn(&Value, usize, &[Table]) -> Value,
) -> Dataset {
    let rows = data.rows();
    let mapped: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| mapper(&row.value(field), index, rows))
        .collect();
    Dataset::new(
        rows.iter()
            .cloned()
            .zip(mapped)
            .map(|(row, value)| row.with(output, value)),
    )
}

/// Shift a field by `offset`, Plot's `shiftX`/`shiftY`.
///
/// A numeric offset moves a number, and a date offset by days is what a
/// shift in time usually means: the shift's output keeps every row and its
/// order, and writes the moved value into a new field so the original stays
/// readable.
pub fn shift(data: &Dataset, field: &str, output: &str, offset: f64) -> Dataset {
    Dataset::new(data.rows().iter().cloned().map(|row| {
        let shifted = match row.value(field) {
            Value::Number(value) => Value::Number(value + offset),
            Value::Date(date) => Value::Date(date + chrono::TimeDelta::days(offset as i64)),
            _ => Value::Null,
        };
        row.with(output, shifted)
    }))
}

/// Shuffle the rows with a fixed seed, Plot's `shuffle`.
///
/// The seed makes the order reproducible: a shuffled chart looks the same on
/// every run, which a random order would not.
pub fn shuffle(data: &Dataset, seed: u64) -> Dataset {
    let mut rows: Vec<Table> = data.rows().to_vec();
    let mut state = seed | 1;
    for index in (1..rows.len()).rev() {
        // xorshift64, so the order is deterministic without a dependency.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        rows.swap(index, (state % (index as u64 + 1)) as usize);
    }
    Dataset::new(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Dataset {
        Dataset::new([
            Table::empty()
                .with("group", Value::text("a"))
                .with("v", Value::Number(1.)),
            Table::empty()
                .with("group", Value::text("a"))
                .with("v", Value::Number(3.)),
            Table::empty()
                .with("group", Value::text("b"))
                .with("v", Value::Number(5.)),
        ])
    }

    #[test]
    fn group_reduces_per_category() {
        let grouped = group(&rows(), "group", "v", Reducer::Sum, "sum");
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.rows()[0].value("sum"), Value::Number(4.));
        assert_eq!(grouped.rows()[1].value("sum"), Value::Number(5.));

        let counted = group(&rows(), "group", "v", Reducer::Count, "count");
        assert_eq!(counted.rows()[0].value("count"), Value::Number(2.));
    }

    #[test]
    fn reducers_ignore_nulls() {
        let values = vec![Value::Number(1.), Value::Null, Value::Number(3.)];
        assert_eq!(Reducer::Mean.reduce(&values), Value::Number(2.));
        assert_eq!(Reducer::Count.reduce(&values), Value::Number(3.));
        assert_eq!(Reducer::Median.reduce(&values), Value::Number(2.));
    }

    #[test]
    fn bins_cover_the_extent() {
        let data = Dataset::new((0..10).map(|i| Table::empty().with("v", Value::Number(i as f64))));
        let (bins, domain) = bin(&data, "v", Thresholds::Count(5));
        assert_eq!(domain, [0., 10.]);
        assert_eq!(bins.iter().map(Bin::count).sum::<f64>(), 10.);
    }

    #[test]
    fn stack_normalizes_to_one() {
        let data = Dataset::new([Table::empty()
            .with("x", Value::text("a"))
            .with("s1", Value::Number(1.))
            .with("s2", Value::Number(3.))]);
        let series = stack_wide(&data, "x", &["s1", "s2"], StackOffset::Normalize);
        assert_eq!(series.len(), 2);
        assert!((series[1].points[0].y1 - 1.).abs() < 1e-9);
        assert!((series[0].points[0].y1 - 0.25).abs() < 1e-9);
    }

    #[test]
    fn window_mean_is_centered() {
        let data = Dataset::new((0..5).map(|i| Table::empty().with("v", Value::Number(i as f64))));
        let smoothed = window(&data, "v", 3, WindowReducer::Mean, Anchor::Middle);
        let values: Vec<f64> = smoothed
            .values("v")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(values, vec![0.5, 1., 2., 3., 3.5]);
    }

    #[test]
    fn normalize_spans_zero_to_one() {
        let normalized = normalize(&rows(), "v");
        let values: Vec<f64> = normalized
            .values("v")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(values, vec![0., 0.5, 1.]);
    }

    #[test]
    fn sort_and_filter() {
        let sorted = sort_by(&rows(), "v", true);
        assert_eq!(sorted.rows()[0].value("v"), Value::Number(5.));
        let filtered = filter(&rows(), "v", |v| v.as_number().is_some_and(|n| n > 2.));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn dodge_packs_overlapping_points_apart() {
        // Three points at the same x, with a radius of 4: they cannot all sit
        // on the baseline, so the dodge stacks them.
        let data = Dataset::new((0..3).map(|_| Table::empty().with("v", Value::Number(1.))));
        let packed = dodge(
            &data,
            &[Some(50.), Some(50.), Some(50.)],
            "dodge",
            Dodge {
                anchor: DodgeAnchor::Start,
                radius: 4.,
                padding: 1.,
            },
            [0., 100.],
        );
        let offsets: Vec<f64> = packed
            .values("dodge")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(offsets.len(), 3);
        // The dodge writes the cross-axis coordinate, not the along-axis one:
        // the first point keeps the baseline, and the others are pushed off it
        // by at least a diameter so that no two overlap.
        assert_eq!(offsets[0], 0.);
        for pair in offsets.windows(2) {
            assert!((pair[1] - pair[0]).abs() >= 8., "points {pair:?} overlap");
        }
    }

    #[test]
    fn dodge_leaves_separated_points_on_the_baseline() {
        let data = Dataset::new((0..3).map(|_| Table::empty()));
        let packed = dodge(
            &data,
            &[Some(10.), Some(50.), Some(90.)],
            "dodge",
            Dodge::default(),
            [0., 100.],
        );
        let offsets: Vec<f64> = packed
            .values("dodge")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        // Far enough apart that every point fits where it already is, so each
        // keeps the baseline of the axis it spreads along.
        assert_eq!(offsets, vec![0., 0., 0.]);
    }

    #[test]
    fn select_keeps_the_extreme_of_each_group() {
        let data = Dataset::new([
            Table::empty()
                .with("g", Value::text("a"))
                .with("v", Value::Number(3.)),
            Table::empty()
                .with("g", Value::text("a"))
                .with("v", Value::Number(9.)),
            Table::empty()
                .with("g", Value::text("b"))
                .with("v", Value::Number(1.)),
            Table::empty()
                .with("g", Value::text("b"))
                .with("v", Value::Number(4.)),
        ]);
        let selected = select(
            &data,
            &Select {
                selector: Selector::Max("v".into()),
                group: Some("g".into()),
            },
        );
        let values: Vec<f64> = selected
            .values("v")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(values, vec![9., 4.]);
        // Without a group the whole series is one group.
        let single = select(
            &data,
            &Select {
                selector: Selector::First,
                group: None,
            },
        );
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn map_folds_a_running_total_per_group() {
        let data = Dataset::new([
            Table::empty()
                .with("g", Value::text("a"))
                .with("v", Value::Number(1.)),
            Table::empty()
                .with("g", Value::text("b"))
                .with("v", Value::Number(2.)),
            Table::empty()
                .with("g", Value::text("a"))
                .with("v", Value::Number(3.)),
        ]);
        let mapped = map_field(&data, "v", "total", |value, _, _| {
            Value::Number(value.as_number().unwrap_or(0.))
        });
        assert_eq!(mapped.len(), 3);
        let shifted = shift(&data, "v", "moved", 10.);
        assert_eq!(
            shifted.values("moved"),
            vec![Value::Number(11.), Value::Number(12.), Value::Number(13.)]
        );
    }

    #[test]
    fn shuffle_is_deterministic_for_a_seed() {
        let data = Dataset::new(
            (0..20).map(|index| Table::empty().with("i", Value::Number(index as f64))),
        );
        let once = shuffle(&data, 7);
        let twice = shuffle(&data, 7);
        assert_eq!(once.values("i"), twice.values("i"));
        let order: Vec<f64> = once
            .values("i")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(order.len(), 20);
        assert_ne!(order, (0..20).map(|i| i as f64).collect::<Vec<_>>());
    }
}
