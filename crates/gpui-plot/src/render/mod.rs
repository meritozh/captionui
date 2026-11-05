//! Mark rendering: resolve a mark's channels against the data, then draw the
//! shapes Plot's mark types describe.
//!
//! Resolution and drawing are separate: [`prepare`] turns a [`Mark`] and a
//! [`Dataset`] into [`PreparedMark`]s, which hold plain values, and the frame
//! unions their values to build the scales. Drawing then only reads pixels.

pub(crate) mod basic;
pub mod color;
pub(crate) mod delaunay;
pub(crate) mod furniture;
pub(crate) mod radial;
pub mod stack;
pub(crate) mod stat;

use gpui::{Hsla, SharedString};

use crate::facet::Grid;
use crate::frame::Frame;
use crate::mark::{Axis, Channels, CrossAxis, MapReducer, Mark, MarkKind};
use crate::table::{Dataset, Table};
use crate::transform::{self, Anchor, Reducer, Selector, StackOffset, StackedSeries, Thresholds};
use crate::value::Value;

/// A mark whose channels have been resolved against the data.
#[derive(Clone, Debug)]
pub struct PreparedMark {
    pub mark: Mark,
    pub rows: Vec<PreparedRow>,
    /// Whether a dodge packed this mark's rows along x, so its offsets are
    /// vertical; meaningless when the mark does not dodge.
    pub dodge_along_x: bool,
}

/// One datum's resolved channels.
#[derive(Clone, Debug)]
pub struct PreparedRow {
    /// The original record, which the tooltip reads.
    pub row: Table,
    /// The index of the datum in the source data.
    pub index: usize,
    pub x: Value,
    pub y: Value,
    /// The x interval, when the mark draws one.
    pub x1: Option<Value>,
    pub x2: Option<Value>,
    /// The y interval, when the mark draws one.
    pub y1: Option<Value>,
    pub y2: Option<Value>,
    /// The series a mark groups by.
    pub z: Option<Value>,
    /// A hierarchy path, read for the `tree` mark.
    pub path: Option<SharedString>,
    /// The resolved radius of a dot.
    pub r: Option<f32>,
    /// The resolved symbol of a dot.
    pub symbol: Option<crate::style::Symbol>,
    /// The row's opacity from its `opacity` channel, when it has one.
    pub opacity: Option<f32>,
    /// The data value of a scaled `r` channel, before the radius scale.
    pub r_value: Option<f64>,
    /// The data value of a scaled `opacity` channel (or a bin's or group's
    /// `fillOpacity` output), before the opacity scale.
    pub opacity_value: Option<f64>,
    /// Where the row sits across the axis a dodge packs along, in pixels from
    /// that axis's start. Set by [`apply_dodge`] once the frame is known.
    pub dodge_offset: Option<f32>,
}

impl PreparedRow {
    /// The value the x channel reads, preferring the interval's start.
    pub fn x_value(&self) -> &Value {
        if !self.x.is_null() {
            &self.x
        } else {
            self.x1.as_ref().unwrap_or(&self.x)
        }
    }

    /// The value the y channel reads, preferring the interval's start.
    pub fn y_value(&self) -> &Value {
        if !self.y.is_null() {
            &self.y
        } else {
            self.y1.as_ref().unwrap_or(&self.y)
        }
    }

    /// The y interval's ends, `(start, end)`, when the mark draws one.
    pub fn y_interval(&self) -> Option<(Value, Value)> {
        let start = self
            .y1
            .clone()
            .or_else(|| (!self.y.is_null()).then(|| self.y.clone()))?;
        let end = self
            .y2
            .clone()
            .or_else(|| (!self.y.is_null()).then(|| self.y.clone()))?;
        Some((start, end))
    }

    /// The x interval's ends, `(start, end)`, when the mark draws one.
    pub fn x_interval(&self) -> Option<(Value, Value)> {
        let start = self
            .x1
            .clone()
            .or_else(|| (!self.x.is_null()).then(|| self.x.clone()))?;
        let end = self
            .x2
            .clone()
            .or_else(|| (!self.x.is_null()).then(|| self.x.clone()))?;
        Some((start, end))
    }
}

/// Resolve every mark against its data, for an unfaceted plot: the mark's
/// own data when it has some, else the plot's `data`.
pub fn prepare(data: &Dataset, marks: &[Mark]) -> Vec<PreparedMark> {
    marks
        .iter()
        .map(|mark| prepare_mark(mark.data.as_ref().unwrap_or(data), mark))
        .collect()
}

/// The facet channels a mark is split by: its own `fx`/`fy`, else — for a
/// mark reading the plot's data — the plot's facet fields. A mark with its
/// own data and no facet channels of its own is not faceted: Plot repeats it
/// in every facet (its `facet: "auto"`).
fn facet_channels(
    mark: &Mark,
    fx: Option<&str>,
    fy: Option<&str>,
) -> (Option<crate::table::Channel>, Option<crate::table::Channel>) {
    if mark.fx.is_some() || mark.fy.is_some() {
        return (mark.fx.clone(), mark.fy.clone());
    }
    if mark.data.is_some() {
        return (None, None);
    }
    let field = |name: Option<&str>| name.map(|name| crate::table::Channel::Field(name.into()));
    (field(fx), field(fy))
}

/// Resolve every mark against the part of its data that lands in each facet.
///
/// A mark's rows are split by its facet channels (see [`facet_channels`]),
/// and each facet's share then resolves on its own, so a stack, bin or
/// reducer summarizes within its facet; a mark with no facet channels draws
/// all its rows in every facet. The facets come back in the grid's order, one
/// list of marks per facet, with which facets hold any faceted rows; a figure
/// that is not faceted has one facet holding every row and no empty facets.
pub fn prepare_facets(
    data: &Dataset,
    marks: &[Mark],
    grid: &Grid,
    fx: Option<&str>,
    fy: Option<&str>,
) -> (Vec<Vec<PreparedMark>>, Vec<bool>) {
    if !grid.is_faceted() {
        return (vec![prepare(data, marks)], Vec::new());
    }
    let marks = pin_bin_edges(data, marks);
    let cells = grid.cells.len();
    let mut prepared: Vec<Vec<PreparedMark>> = vec![Vec::with_capacity(marks.len()); cells];
    let mut populated = vec![false; cells];
    // A furniture mark faceted by constants reads one placeholder record,
    // which lands in the facet the constants name.
    let placeholder = Dataset::new([Table::empty()]);
    for mark in &marks {
        let source = if mark.kind.is_furniture() && mark.data.is_none() {
            &placeholder
        } else {
            mark.data.as_ref().unwrap_or(data)
        };
        let (mark_fx, mark_fy) = facet_channels(mark, fx, fy);
        if mark_fx.is_none() && mark_fy.is_none() {
            let resolved = prepare_mark(source, mark);
            for cell in prepared.iter_mut() {
                cell.push(resolved.clone());
            }
            continue;
        }
        let groups = grid.partition_channels(source, mark_fx.as_ref(), mark_fy.as_ref());
        for (cell, rows) in groups.into_iter().enumerate() {
            populated[cell] |= !rows.is_empty();
            prepared[cell].push(prepare_mark(&Dataset::new(rows), mark));
        }
    }
    (prepared, populated)
}

/// A mark over plain values or tuples reads them by default, as Plot's marks
/// do: `Plot.ruleY([0])` puts `y` on the value, `Plot.dotX(numbers)` puts
/// `x` on it, and `Plot.line([[0, 1], [1, 2]])` reads `x` and `y` from each
/// pair.
fn with_value_defaults(mark: &Mark, data: &Dataset) -> Option<Mark> {
    use crate::table::Channel;
    let channels = &mark.channels;
    let unset_x = channels.x.is_none() && channels.x1.is_none() && channels.x2.is_none();
    let unset_y = channels.y.is_none() && channels.y1.is_none() && channels.y2.is_none();
    if data.is_values() {
        let category = mark.kind.category();
        let mut mark = mark.clone();
        match category.single_axis {
            Some(Axis::X) if unset_x => mark.channels.x = Some(Channel::identity()),
            Some(Axis::Y) if unset_y => mark.channels.y = Some(Channel::identity()),
            _ => {
                // The marks without a single axis that Plot reads a value
                // with: a rule or tick across one axis, a bar's length.
                match mark.kind {
                    MarkKind::RuleX
                    | MarkKind::TickX
                    | MarkKind::BarX
                    | MarkKind::GridX
                    | MarkKind::AxisX
                        if unset_x =>
                    {
                        mark.channels.x = Some(Channel::identity())
                    }
                    MarkKind::RuleY
                    | MarkKind::TickY
                    | MarkKind::BarY
                    | MarkKind::GridY
                    | MarkKind::AxisY
                        if unset_y =>
                    {
                        mark.channels.y = Some(Channel::identity())
                    }
                    _ => return None,
                }
            }
        }
        return Some(mark);
    }
    if data.is_tuples() && (unset_x || unset_y) {
        let mut mark = mark.clone();
        if unset_x {
            mark.channels.x = Some(Channel::Field("0".into()));
        }
        if unset_y {
            mark.channels.y = Some(Channel::Field("1".into()));
        }
        return Some(mark);
    }
    None
}

fn prepare_mark(data: &Dataset, mark: &Mark) -> PreparedMark {
    let defaulted = with_value_defaults(mark, data);
    let mark = defaulted.as_ref().unwrap_or(mark);
    // A furniture mark draws the plot's axes, grid or frame rather than data,
    // so it reads no channels and resolves no rows.
    // A furniture mark without data reads one placeholder record, which its
    // constant channels (`axisY({x: 0})`, `gridX({stroke: "white"})`) read.
    if mark.kind.is_furniture() && mark.data.is_none() && data.len() != 1 {
        let placeholder = Dataset::new([Table::empty()]);
        return prepare_mark(&placeholder, mark);
    }
    let channels = &mark.channels;

    // The data transforms rewrite the records themselves, so they run before
    // any row is resolved: `select` drops rows, and `map`/`shift` add fields
    // the channels may read. Plot applies them to the mark's data in the same
    // order, and a mark's `shuffle` is only a reordering of what is left.
    let mut source = data.clone();
    if let Some(keep) = &mark.filter {
        source = Dataset::new(source.rows().iter().filter(|row| (keep.0)(row)).cloned());
    }
    if let Some(config) = mark.select.as_ref().filter(|config| config.axis.is_none()) {
        let group = config.group.as_deref().map(|field| field.to_string());
        let selector = match (&config.selector, config.axis) {
            // A select over an axis reads that axis's channel, which may be a
            // field name or an accessor; only the field form can be expressed
            // here, and an accessor's extreme falls back to the first row.
            (Selector::Min(_), Some(Axis::X)) => Selector::Min("x".into()),
            (Selector::Max(_), Some(Axis::X)) => Selector::Max("x".into()),
            (Selector::Min(_), Some(Axis::Y)) => Selector::Min("y".into()),
            (Selector::Max(_), Some(Axis::Y)) => Selector::Max("y".into()),
            (selector, _) => selector.clone(),
        };
        source = transform::select(
            &source,
            &transform::Select {
                selector,
                group: group.map(Into::into),
            },
        );
    }
    if let Some(config) = &mark.map {
        let rows = source.rows();
        let mut running: Vec<(String, f64, f64, usize)> = Vec::new();
        let mut output: Vec<Value> = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let key = config
                .group
                .as_ref()
                .map(|field| row.value(field).label().to_string())
                .unwrap_or_default();
            let value = row.value(&config.input).as_number().unwrap_or(f64::NAN);
            let slot = match running.iter().position(|(existing, ..)| *existing == key) {
                Some(index) => index,
                None => {
                    running.push((key, 0., f64::NAN, 0));
                    running.len() - 1
                }
            };
            let (_, total, extreme, count) = &mut running[slot];
            *count += 1;
            let mapped = match config.reducer {
                MapReducer::Rank => Value::Number((*count - 1) as f64),
                MapReducer::Sum => {
                    *total += value;
                    Value::Number(*total)
                }
                MapReducer::Min => {
                    *extreme = if extreme.is_nan() {
                        value
                    } else {
                        extreme.min(value)
                    };
                    Value::Number(*extreme)
                }
                MapReducer::Max => {
                    *extreme = if extreme.is_nan() {
                        value
                    } else {
                        extreme.max(value)
                    };
                    Value::Number(*extreme)
                }
                MapReducer::Mean => {
                    *total += value;
                    Value::Number(*total / *count as f64)
                }
            };
            output.push(mapped);
        }
        source = Dataset::new(
            rows.iter()
                .cloned()
                .zip(output)
                .map(|(row, value)| row.with(&config.output, value)),
        );
    }
    if let Some(config) = &mark.shift {
        source = transform::shift(&source, &config.input, &config.output, config.offset);
    }
    if let Some(seed) = mark.shuffle {
        source = transform::shuffle(&source, seed);
    }

    let mut rows: Vec<PreparedRow> = source
        .rows()
        .iter()
        .enumerate()
        .map(|(index, row)| resolve_row(row, index, channels, mark))
        .collect();

    // A mark's own ordering decides where its shapes land, and a line's order
    // is its path, so sorting happens before anything positional.
    apply_sort(&mut rows, mark);

    // Windows and normalization rewrite the value the mark draws, so they run
    // before the shapes read it.
    apply_window(data, mark, &mut rows);
    if mark.normalize {
        normalize_rows(&mut rows);
    }
    if let Some((axis, basis)) = mark.normalize_by {
        stack::normalize(&mut rows, axis, basis);
    }
    // A select over an axis (`selectMinX`) reads the mark's resolved channel,
    // after the transforms that rewrite it, per `z` series (or group field).
    if let Some(config) = mark.select.as_ref().filter(|config| config.axis.is_some()) {
        rows = select_rows(rows, config);
    }

    // A grouping collapses the rows sharing a category into one row each,
    // Plot's `groupX`/`groupY`/`groupZ`.
    if let Some(config) = &mark.group {
        rows = collapse_groups(rows, mark, config);
    }

    // A reducer collapses the rows sharing a category, Plot's `groupX`. A
    // binned mark reduces each bin instead, so its reducer goes to the bin.
    if let Some(reducer) = mark.reduce
        && mark.bin.is_none()
    {
        rows = reduce_rows(rows, mark, reducer);
    }

    // Binning replaces the rows with one row per bin.
    if mark.bin.is_some() {
        rows = bin_rows(mark, &rows);
    }

    // Stacking rewrites the y interval of every row.
    if mark.stack.is_some() {
        rows = stack_rows(rows, mark);
    }
    if let Some(spec) = stack::effective_stack(mark) {
        stack::stack(&mut rows, &spec);
    }

    // Statistical marks summarize their category themselves, so they keep the
    // raw rows and group at draw time.

    PreparedMark {
        mark: mark.clone(),
        rows,
        dodge_along_x: false,
    }
}

/// Keep the row of each group with the least or greatest value on the
/// selector's axis, Plot's `selectMinX` and its siblings.
fn select_rows(rows: Vec<PreparedRow>, config: &crate::mark::SelectConfig) -> Vec<PreparedRow> {
    let along_x = config.axis == Some(Axis::X);
    let want_max = matches!(config.selector, Selector::Max(_));
    let mut best: Vec<(crate::value::ValueKey, usize, f64)> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let group = match &config.group {
            Some(field) => row.row.value(field).key(),
            None => row
                .z
                .as_ref()
                .map(Value::key)
                .unwrap_or_else(|| Value::Null.key()),
        };
        let Some(v) = (if along_x {
            row.x_value()
        } else {
            row.y_value()
        })
        .as_number() else {
            continue;
        };
        match best.iter_mut().find(|(g, _, _)| *g == group) {
            Some(entry) => {
                if (want_max && v > entry.2) || (!want_max && v < entry.2) {
                    entry.1 = i;
                    entry.2 = v;
                }
            }
            None => best.push((group, i, v)),
        }
    }
    let keep: std::collections::HashSet<usize> = best.into_iter().map(|(_, i, _)| i).collect();
    rows.into_iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, row)| row)
        .collect()
}

/// Put every scaled `r` and `opacity` value through Plot's scales: a sqrt
/// radius scale over `[0, max]` whose first quartile is three pixels (capped
/// so the largest dot is 30), and a linear opacity scale from `[0, max]` to
/// `[0, 1]`. Both span every facet, so sizes compare across the figure.
pub fn apply_quantity_scales(
    facets: &mut [Vec<PreparedMark>],
    r: &crate::options::ScaleOptions,
    opacity: &crate::options::ScaleOptions,
) {
    let values =
        |facets: &[Vec<PreparedMark>], read: fn(&PreparedRow) -> Option<f64>| -> Vec<f64> {
            facets
                .iter()
                .flatten()
                .flat_map(|mark| mark.rows.iter().filter_map(read))
                .filter(|v| v.is_finite())
                .collect()
        };
    let radii = values(facets, |row| row.r_value);
    if !radii.is_empty() {
        let max = radii.iter().copied().fold(0., f64::max);
        let [d0, d1] = r.numeric_domain().unwrap_or([0., max]);
        let range = match &r.range {
            Some(range) if range.len() >= 2 => [range[0], range[1]],
            _ => {
                // Plot's `inferRadialRange`: the median of the marks' first
                // quartiles of positive values maps to three pixels.
                let mut positive: Vec<f64> = radii.iter().copied().filter(|v| *v > 0.).collect();
                positive.sort_by(f64::total_cmp);
                let q25 = positive
                    .get(((positive.len().max(1) - 1) as f64 * 0.25).round() as usize)
                    .copied()
                    .unwrap_or(1.)
                    .max(f64::MIN_POSITIVE);
                let range = [
                    3. * (d0 / q25).max(0.).sqrt(),
                    3. * (d1 / q25).max(0.).sqrt(),
                ];
                let k = 30. / range[1].max(f64::MIN_POSITIVE);
                if k < 1. {
                    [range[0] * k, range[1] * k]
                } else {
                    range
                }
            }
        };
        let kind = r.r#type.unwrap_or(crate::options::ScaleType::Sqrt);
        let t = |v: f64| match kind {
            crate::options::ScaleType::Linear => v,
            crate::options::ScaleType::Pow => v.signum() * v.abs().powf(r.exponent.unwrap_or(1.)),
            crate::options::ScaleType::Log => v.max(f64::MIN_POSITIVE).ln(),
            _ => v.signum() * v.abs().sqrt(),
        };
        let (t0, t1) = (t(d0), t(d1));
        for row in facets
            .iter_mut()
            .flatten()
            .flat_map(|mark| mark.rows.iter_mut())
        {
            if let Some(value) = row.r_value {
                let ratio = if t1 == t0 {
                    1.
                } else {
                    (t(value) - t0) / (t1 - t0)
                };
                row.r = Some((range[0] + (range[1] - range[0]) * ratio).max(0.) as f32);
            }
        }
    }
    let opacities = values(facets, |row| row.opacity_value);
    if !opacities.is_empty() {
        let max = opacities.iter().copied().fold(0., f64::max);
        let [d0, d1] = opacity.numeric_domain().unwrap_or([0., max]);
        let range = match &opacity.range {
            Some(range) if range.len() >= 2 => [range[0], range[1]],
            _ => [0., 1.],
        };
        for row in facets
            .iter_mut()
            .flatten()
            .flat_map(|mark| mark.rows.iter_mut())
        {
            if let Some(value) = row.opacity_value {
                let ratio = if d1 == d0 {
                    1.
                } else {
                    (value - d0) / (d1 - d0)
                };
                row.opacity = Some((range[0] + (range[1] - range[0]) * ratio).clamp(0., 1.) as f32);
            }
        }
    }
}

/// Put every symbol channel that names categories rather than symbols
/// through Plot's ordinal symbol scale: the sorted domain (or the `symbol`
/// options' domain) onto d3's fill symbols for a filled dot, its stroke
/// symbols otherwise.
pub fn apply_symbol_scale(
    facets: &mut [Vec<PreparedMark>],
    options: &crate::options::ScaleOptions,
) {
    use crate::style::{SYMBOLS_FILL, SYMBOLS_STROKE};
    let mut values: Vec<Value> = Vec::new();
    for mark in facets.iter().flatten() {
        if let Some(channel) = &mark.mark.channels.symbol {
            for row in &mark.rows {
                let value = channel.read(&row.row);
                if !value.is_null() && symbol_from_value(&value).is_none() {
                    values.push(value);
                }
            }
        }
    }
    if values.is_empty() {
        return;
    }
    let domain: Vec<crate::value::ValueKey> = match &options.domain {
        Some(domain) => domain.iter().map(Value::key).collect(),
        None => {
            let mut keys = crate::scale::dedup(values.iter().map(Value::key));
            keys.sort();
            keys
        }
    };
    for mark in facets.iter_mut().flatten() {
        let Some(channel) = mark.mark.channels.symbol.clone() else {
            continue;
        };
        if constant_symbol(&channel).is_some() {
            continue;
        }
        let range = if mark.mark.channels.fill.is_some() {
            SYMBOLS_FILL
        } else {
            SYMBOLS_STROKE
        };
        for row in mark.rows.iter_mut() {
            let value = channel.read(&row.row);
            if row.symbol.is_none()
                && let Some(index) = domain.iter().position(|key| *key == value.key())
            {
                row.symbol = Some(range[index % range.len()]);
            }
        }
    }
}

/// Pack a dodging mark's rows into a beeswarm, Plot's `dodgeX`/`dodgeY`.
///
/// The transform works in pixels, so it runs after the frame has resolved the
/// scales: each row's position on the axis the data varies on becomes an
/// offset across the other axis, which the mark then reads instead of its
/// own cross-axis value.
pub fn apply_dodge(prepared: &mut [PreparedMark], frame: &Frame) {
    for mark in prepared.iter_mut() {
        let Some(config) = mark.mark.dodge else {
            continue;
        };
        // The dodge packs along the axis the mark's data varies on, and writes
        // the offset across it: a `dotX` varies on x and spreads vertically,
        // a `dotY` varies on y and spreads horizontally.
        let along_x = mark.mark.kind.is_x_only();
        // The positions come from the axis the data varies on; the packed
        // offsets land on the *other* axis, so the extent is that one's.
        let (scale, extent) = if along_x {
            (&frame.x.scale, frame.plot.size.height.as_f32())
        } else {
            (&frame.y.scale, frame.plot.size.width.as_f32())
        };
        let positions: Vec<Option<f32>> = mark
            .rows
            .iter()
            .map(|row| {
                if along_x {
                    scale.position(row.x_value())
                } else {
                    scale.position(row.y_value())
                }
            })
            .collect();
        let extent = [0., extent];
        let packed = transform::dodge(
            &Dataset::new(mark.rows.iter().map(|row| row.row.clone())),
            &positions,
            "dodge",
            config,
            extent,
        );
        for (row, packed_row) in mark.rows.iter_mut().zip(packed.rows()) {
            row.dodge_offset = packed_row.number("dodge").map(|value| value as f32);
        }
        mark.dodge_along_x = along_x;
    }
}

/// Grow a mark's interval channels from its value channel, the way Plot's
/// `barY` grows from zero to `y` and its `rectY`/`cell` marks fill their
/// missing interval ends.
///
/// Plot derives these from the mark's own options, so the same derivation has
/// to run again on a grouped row: the grouping writes the reduced value into
/// `y` after the raw rows already had their intervals derived, and a bar with
/// a `y` but no `y1`/`y2` draws nothing.
fn derive_intervals(
    mark: &Mark,
    x: &Value,
    y: &Value,
    x1: &mut Option<Value>,
    x2: &mut Option<Value>,
    y1: &mut Option<Value>,
    y2: &mut Option<Value>,
) {
    let unset = |start: &Option<Value>, end: &Option<Value>| start.is_none() && end.is_none();
    // An interval buckets a singular date or number into a continuous extent:
    // `x` (or `y`) becomes the `[start, end)` that spans its bucket, so a
    // `rectY` over dates draws one bar per bucket. This runs first, before the
    // mark's own baseline derivation, and only when the mark names no interval
    // channels of its own.
    if let Some(interval) = mark.interval {
        // The interval buckets the mark's *primary* positional channel — x when
        // the mark reads an x, else y — so a `rectY` over dates buckets its
        // date axis and leaves the value axis to the baseline derivation below.
        // Bucketing both would turn the count into an extent too.
        if mark.channels.x.is_some() && unset(x1, x2) && !x.is_null() {
            if let Some((start, end)) = interval.extent(x) {
                *x1 = Some(interval_value(x, start));
                *x2 = Some(interval_value(x, end));
            }
        } else if mark.channels.x.is_none()
            && unset(y1, y2)
            && !y.is_null()
            && let Some((start, end)) = interval.extent(y)
        {
            *y1 = Some(interval_value(y, start));
            *y2 = Some(interval_value(y, end));
        }
    }
    // Plot's `maybeOptionalZero` for rules: a `ruleX` given a `y` spans from
    // zero to it, and one given only `y1` or `y2` spans from zero to that.
    let optional_zero = |v: &Value, start: &mut Option<Value>, end: &mut Option<Value>| {
        if !v.is_null() && start.is_none() && end.is_none() {
            *start = Some(Value::Number(0.));
            *end = Some(v.clone());
        } else if v.is_null() {
            match (start.is_some(), end.is_some()) {
                (true, false) => {
                    *end = start.take();
                    *start = Some(Value::Number(0.));
                }
                (false, true) => *start = Some(Value::Number(0.)),
                _ => {}
            }
        }
    };
    match mark.kind {
        MarkKind::RuleX => optional_zero(y, y1, y2),
        MarkKind::RuleY => optional_zero(x, x1, x2),
        // Plot's difference compares y with zero when given only y.
        MarkKind::DifferenceY if unset(y1, y2) && !y.is_null() => {
            *y1 = Some(Value::Number(0.));
            *y2 = Some(y.clone());
        }
        MarkKind::DifferenceX if unset(x1, x2) && !x.is_null() => {
            *x1 = Some(Value::Number(0.));
            *x2 = Some(x.clone());
        }
        _ => {}
    }
    match mark.kind {
        MarkKind::BarY | MarkKind::AreaY | MarkKind::WaffleY if unset(y1, y2) && !y.is_null() => {
            *y1 = Some(Value::Number(0.));
            *y2 = Some(y.clone());
        }
        MarkKind::BarX | MarkKind::AreaX | MarkKind::WaffleX if unset(x1, x2) && !x.is_null() => {
            *x1 = Some(Value::Number(0.));
            *x2 = Some(x.clone());
        }
        // `rectX`/`rectY` with one value channel is a bar: its interval runs
        // from the baseline to the value, which is how Plot's implicit stack
        // reads it.
        MarkKind::RectY if unset(y1, y2) && !y.is_null() => {
            *y1 = Some(Value::Number(0.));
            *y2 = Some(y.clone());
        }
        // `rectX` grows along x from the baseline, so its *value* is the x
        // interval's end and its cross axis is the index.
        MarkKind::RectX if unset(x1, x2) && !x.is_null() => {
            *x1 = Some(Value::Number(0.));
            *x2 = Some(x.clone());
        }
        MarkKind::Cell => {
            if x1.is_none() && !x.is_null() {
                *x1 = Some(x.clone());
            }
            if y1.is_none() && !y.is_null() {
                *y1 = Some(y.clone());
            }
        }
        _ => {}
    }
}

/// An interval boundary as a [`Value`], matching the singuar value it buckets:
/// a date stays a date (so the axis scales temporally and labels as calendar
/// dates), a number stays a number.
fn interval_value(source: &Value, millis_or_number: f64) -> Value {
    if source.as_date().is_some() {
        Value::date_from_millis(millis_or_number as i64)
    } else {
        Value::Number(millis_or_number)
    }
}

fn resolve_row(row: &Table, index: usize, channels: &Channels, mark: &Mark) -> PreparedRow {
    let read = |channel: &Option<crate::table::Channel>| {
        channel
            .as_ref()
            .map(|channel| channel.read(row))
            .unwrap_or(Value::Null)
    };

    let mut x = read(&channels.x);
    let mut y = read(&channels.y);
    let x1 = read(&channels.x1);
    let x2 = read(&channels.x2);
    let y1 = read(&channels.y1);
    let y2 = read(&channels.y2);

    let mut x1 = (!x1.is_null()).then_some(x1);
    let mut x2 = (!x2.is_null()).then_some(x2);
    let mut y1 = (!y1.is_null()).then_some(y1);
    let mut y2 = (!y2.is_null()).then_some(y2);
    derive_intervals(mark, &x, &y, &mut x1, &mut x2, &mut y1, &mut y2);

    // A one-dimensional mark has no second position channel. Plot fills it
    // with the datum's index for `lineX`/`rectX`, and leaves it unset for
    // `dotX`, whose dots all sit in the frame's middle.
    // A cross channel the mark names wins: `Plot.lineY(data, {x, y})`.
    if mark.kind.is_x_only() && mark.kind.cross_axis() == CrossAxis::Index && channels.y.is_none() {
        y = Value::Number(index as f64);
    } else if mark.kind.is_y_only()
        && mark.kind.cross_axis() == CrossAxis::Index
        && channels.x.is_none()
    {
        x = Value::Number(index as f64);
    }

    let z = channels
        .z
        .as_ref()
        .map(|channel| channel.read(row))
        .filter(|v| !v.is_null());

    PreparedRow {
        row: row.clone(),
        index,
        x,
        y,
        x1,
        x2,
        y1,
        y2,
        z,
        path: channels
            .path
            .as_ref()
            .map(|channel| channel.read(row))
            .filter(|v| !v.is_null())
            .map(|v| v.label()),
        r: channels
            .r
            .as_ref()
            .filter(|channel| !channel.is_scaled())
            .map(|channel| channel.read(row)),
        r_value: channels
            .r
            .as_ref()
            .and_then(|channel| channel.data_value(row)),
        opacity_value: channels
            .opacity
            .as_ref()
            .and_then(|channel| channel.data_value(row)),
        symbol: channels.symbol.as_ref().and_then(|channel| {
            constant_symbol(channel).or_else(|| symbol_from_value(&channel.read(row)))
        }),
        opacity: channels
            .opacity
            .as_ref()
            .filter(|channel| !channel.is_scaled())
            .map(|channel| channel.read(row)),
        dodge_offset: None,
    }
}

/// A symbol channel's value as a [`Symbol`](crate::style::Symbol): a name
/// (`"circle"`, `"square"`, …) or an index into the symbol set.
/// A symbol channel that names a symbol (`symbol: "square"`) is a constant,
/// as Plot reads it.
pub fn constant_symbol(channel: &crate::table::Channel) -> Option<crate::style::Symbol> {
    match channel {
        crate::table::Channel::Field(name) => crate::style::Symbol::parse(name),
        crate::table::Channel::Constant(value) => symbol_from_value(value),
        _ => None,
    }
}

fn symbol_from_value(value: &Value) -> Option<crate::style::Symbol> {
    match value {
        Value::Text(text) => crate::style::Symbol::parse(text),
        _ => None,
    }
}

fn apply_sort(rows: &mut [PreparedRow], mark: &Mark) {
    let Some(sort) = &mark.sort else {
        return;
    };
    if let Some(field) = &sort.field {
        rows.sort_by(|a, b| {
            let ordering = a.row.value(field).cmp(&b.row.value(field));
            if sort.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    } else {
        // Sorting without a field sorts by the mark's own value: a bar chart's
        // length, which is Plot's default for a sorted bar chart.
        // The value is on x when the categories are on y (a `barX`), else y.
        let value_on_x = mark.kind.category().band_y;
        let value = |row: &PreparedRow| {
            if value_on_x {
                row.x_value().clone()
            } else {
                row.y_value().clone()
            }
        };
        rows.sort_by(|a, b| {
            let ordering = value(a).cmp(&value(b));
            if sort.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }
    if sort.reverse {
        rows.reverse();
    }
}

fn apply_window(data: &Dataset, mark: &Mark, rows: &mut [PreparedRow]) {
    let Some((k, reducer, anchor)) = mark.window else {
        return;
    };
    // The window rewrites a field in place, so it runs over the whole dataset
    // and then the resolved rows read the rewritten value back.
    let field = match mark.channels.y.as_ref().or(mark.channels.x.as_ref()) {
        Some(crate::table::Channel::Field(field)) => field.clone(),
        _ => return,
    };
    let smoothed = transform::window(data, &field, k, reducer, anchor);
    for (prepared, row) in rows.iter_mut().zip(smoothed.rows()) {
        let value = row.value(&field);
        if mark.channels.y.as_ref().is_some_and(is_field(&field)) {
            prepared.y = value.clone();
        }
        if mark.channels.x.as_ref().is_some_and(is_field(&field)) {
            prepared.x = value;
        }
    }
}

fn is_field(field: &SharedString) -> impl Fn(&crate::table::Channel) -> bool + '_ {
    move |channel| matches!(channel, crate::table::Channel::Field(name) if name == field)
}

fn normalize_rows(rows: &mut [PreparedRow]) {
    let values: Vec<Value> = rows.iter().map(|row| row.y_value().clone()).collect();
    let Some([lo, hi]) = crate::scale::extent(values) else {
        return;
    };
    let span = hi - lo;
    for row in rows.iter_mut() {
        if let Some(value) = row.y_value().as_number() {
            let normalized = if span == 0. { 0.5 } else { (value - lo) / span };
            row.y = Value::Number(normalized);
            row.y1 = None;
            row.y2 = None;
        }
    }
}

/// The value a grouped output reduces, read from one member of the group.
///
/// Plot reads the channel the output writes unless the output names another
/// input, and `y1`/`y2` fall back to `y` when the mark names no interval —
/// Plot's `maybeInput` — so `{y1: "min"}` is the smallest y value even though
/// a bar's interval has already been grown from zero by the time it groups.
fn group_input(row: &PreparedRow, mark: &Mark, output: &crate::mark::GroupOutput) -> Value {
    let name = output.input.as_deref().unwrap_or(&output.name);
    match name {
        // An interval's end falls back to the channel it splits, the way
        // Plot's `maybeInput` does.
        "x1" | "x2" if mark.channels.x1.is_some() || mark.channels.x2.is_some() => row
            .x1
            .as_ref()
            .or(row.x2.as_ref())
            .unwrap_or(&row.x)
            .clone(),
        "y1" | "y2" if mark.channels.y1.is_some() || mark.channels.y2.is_some() => row
            .y1
            .as_ref()
            .or(row.y2.as_ref())
            .unwrap_or(&row.y)
            .clone(),
        "x1" | "x2" => row.x_value().clone(),
        "y1" | "y2" => row.y_value().clone(),
        "x" => row.x_value().clone(),
        "y" => row.y_value().clone(),
        "z" => row.z.clone().unwrap_or(Value::Null),
        // Any other name is a field on the record, which is where a grouped
        // color, label or tooltip value lands.
        field => row.row.value(field),
    }
}

/// Collapse the rows sharing a category, Plot's `groupX`/`groupY`/`groupZ`.
///
/// Every group becomes one row: its category channel keeps the key, its
/// outputs hold the reductions, and every other channel — the radius, the
/// symbol, the record the tooltip reads — carries over from the group's first
/// member. The groups keep the order they first appear in, so a categorical
/// axis reads in data order.
fn collapse_groups(
    rows: Vec<PreparedRow>,
    mark: &Mark,
    config: &crate::mark::GroupConfig,
) -> Vec<PreparedRow> {
    let key = |row: &PreparedRow| match config.axis {
        Some(Axis::X) => row.x_value().clone(),
        Some(Axis::Y) => row.y_value().clone(),
        // `groupZ` subdivides by the series, and a row with no `z` is its
        // own group rather than a missing one.
        None => row.z.clone().unwrap_or(Value::Null),
    };

    let mut groups: Vec<(Value, Vec<PreparedRow>)> = Vec::new();
    for row in rows {
        let key = key(&row);
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }

    groups
        .into_iter()
        .map(|(key, members)| {
            let first = members[0].clone();
            let mut grouped = PreparedRow {
                x: Value::Null,
                y: Value::Null,
                x1: None,
                x2: None,
                y1: None,
                y2: None,
                ..first
            };
            for output in &config.outputs {
                let values: Vec<Value> = members
                    .iter()
                    .map(|row| group_input(row, mark, output))
                    .collect();
                let records: Vec<&Table> = members.iter().map(|row| &row.row).collect();
                let reduced = output.reducer.reduce(&values, &records);
                match &*output.name {
                    "x" => grouped.x = reduced,
                    "y" => grouped.y = reduced,
                    "x1" => grouped.x1 = (!reduced.is_null()).then_some(reduced),
                    "x2" => grouped.x2 = (!reduced.is_null()).then_some(reduced),
                    "y1" => grouped.y1 = (!reduced.is_null()).then_some(reduced),
                    "y2" => grouped.y2 = (!reduced.is_null()).then_some(reduced),
                    "z" => grouped.z = (!reduced.is_null()).then_some(reduced),
                    // Anything else is a field the record gains, so a channel
                    // that reads it — a fill accessor, a text label — sees the
                    // reduced value.
                    field => grouped.row = grouped.row.with(field, reduced),
                }
            }
            // The category channel keeps the key when no output wrote it,
            // which is what makes a grouped dot sit at its own category.
            if grouped.x.is_null() && grouped.x1.is_none() {
                grouped.x = key.clone();
            }
            if grouped.y.is_null() && grouped.y1.is_none() {
                grouped.y = key;
            }
            // The grouping wrote the reduced value into a channel the mark
            // turns into an interval, so a bar grows from zero to the count
            // again. Plot derives the interval from the mark's options after
            // its transforms, not before.
            derive_intervals(
                mark,
                &grouped.x,
                &grouped.y,
                &mut grouped.x1,
                &mut grouped.x2,
                &mut grouped.y1,
                &mut grouped.y2,
            );
            grouped
        })
        .collect()
}

fn reduce_rows(rows: Vec<PreparedRow>, mark: &Mark, reducer: Reducer) -> Vec<PreparedRow> {
    // Grouping by x for a vertical mark and by y for a horizontal one mirrors
    // Plot's `groupX` and `groupY`.
    let group_by_y = matches!(
        mark.kind,
        MarkKind::BarX | MarkKind::AreaX | MarkKind::RuleY | MarkKind::TickY
    );

    let mut groups: Vec<(Value, Vec<PreparedRow>)> = Vec::new();
    for row in rows {
        let key = if group_by_y {
            row.y_value().clone()
        } else {
            row.x_value().clone()
        };
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }

    groups
        .into_iter()
        .map(|(key, members)| {
            let values: Vec<Value> = members
                .iter()
                .map(|row| {
                    if group_by_y {
                        row.x_value().clone()
                    } else {
                        row.y_value().clone()
                    }
                })
                .collect();
            let reduced = reducer.reduce(&values);
            let first = members[0].clone();
            if group_by_y {
                PreparedRow {
                    y: key,
                    x: reduced,
                    x1: None,
                    x2: None,
                    y1: None,
                    y2: None,
                    ..first
                }
            } else {
                PreparedRow {
                    x: key,
                    y: reduced,
                    x1: None,
                    x2: None,
                    y1: None,
                    y2: None,
                    ..first
                }
            }
        })
        .collect()
}

/// Which axes a binned mark bins, `(x, y)`: Plot's `binX`, `binY` or `bin`.
///
/// A mark with a fixed bin axis (`rectY` and `barY` bin x, `rectX` and
/// `barX` bin y) bins that one; a `rect` bins every axis it names a field on,
/// so `rect(x: "weight", y: "height")` is Plot's two-dimensional `bin`.
pub fn bin_axes(mark: &Mark) -> (bool, bool) {
    if mark.bin.is_none() {
        return (false, false);
    }
    match mark.kind.category().bins_on_x {
        Some(true) => (true, false),
        Some(false) => (false, true),
        None => {
            let named = |channel: &Option<crate::table::Channel>| {
                matches!(channel, Some(crate::table::Channel::Field(_)))
            };
            match (named(&mark.channels.x), named(&mark.channels.y)) {
                (_, false) => (true, false),
                (false, true) => (false, true),
                (true, true) => (true, true),
            }
        }
    }
}

/// The bin edges a mark lays over `rows` on each of its bin axes.
///
/// A faceted plot pins the edges from the whole data first (see
/// [`pin_bin_edges`]), the way Plot computes its thresholds once and then
/// bins each facet, so the facets' bins line up.
fn bin_edges(
    mark: &Mark,
    rows: &[Table],
) -> (Option<transform::BinEdges>, Option<transform::BinEdges>) {
    let Some(bin) = &mark.bin else {
        return (None, None);
    };
    let (x_channel, y_channel) = bin_channels(mark);
    let edges = |channel: Option<&crate::table::Channel>, pinned: Option<transform::BinEdges>| {
        pinned.or_else(|| {
            let channel = channel?;
            let values: Vec<f64> = rows
                .iter()
                .filter_map(|row| channel.read(row).as_number())
                .collect();
            transform::BinEdges::of(&values, bin.thresholds)
        })
    };
    (
        edges(x_channel, bin.edges[0]),
        edges(y_channel, bin.edges[1]),
    )
}

/// The channels a binned mark bins on x and y: [`bin_fields`]' choice, but
/// any channel form, so `x: (d) => Math.log(d.volume)` bins too.
fn bin_channels(
    mark: &Mark,
) -> (
    Option<&crate::table::Channel>,
    Option<&crate::table::Channel>,
) {
    let (x, y) = (mark.channels.x.as_ref(), mark.channels.y.as_ref());
    match bin_axes(mark) {
        (true, true) => (x, y),
        (true, false) => (x.or(y), None),
        (false, true) => (None, y.or(x)),
        (false, false) => (None, None),
    }
}

/// Fix every binned mark's edges over the whole data, before it is split
/// into facets.
fn pin_bin_edges(data: &Dataset, marks: &[Mark]) -> Vec<Mark> {
    marks
        .iter()
        .map(|mark| {
            let mut mark = mark.clone();
            if mark.bin.is_some() {
                let source = mark.data.clone().unwrap_or_else(|| data.clone());
                let (x, y) = bin_edges(&mark, source.rows());
                if let Some(bin) = mark.bin.as_mut() {
                    bin.edges = [x, y];
                }
            }
            mark
        })
        .collect()
}

/// One series' bins: the row indices in each `(x, y)` bin cell.
type BinCells = std::collections::BTreeMap<(usize, usize), Vec<usize>>;

/// Replace rows with one row per bin, Plot's `binX`, `binY` and `bin`.
///
/// Rows are binned within their `z` series, so `fill: "sex"` gives each sex
/// its own bins over shared edges. Each bin is reduced into the mark's bin
/// outputs — by default, a one-dimensional bin counts into its length
/// channel. A one-dimensional count is stacked across the series in each bin,
/// Plot's implicit `stackY` for `rectY`; an `opacity` or `fillOpacity` output
/// goes through Plot's default opacity scale, `[0, max] → [0, 1]`.
fn bin_rows(mark: &Mark, rows: &[PreparedRow]) -> Vec<PreparedRow> {
    let Some(bin) = &mark.bin else {
        return rows.to_vec();
    };
    let (bin_x, bin_y) = bin_axes(mark);
    let (x_channel, y_channel) = bin_channels(mark);
    let tables: Vec<Table> = rows.iter().map(|row| row.row.clone()).collect();
    let (x_edges, y_edges) = bin_edges(mark, &tables);
    if (bin_x && x_edges.is_none()) || (bin_y && y_edges.is_none()) {
        return Vec::new();
    }

    // The outputs: the mark's own, or a count into the length channel. A
    // mark-level `reduce` stays the length's reducer, as before bins had
    // outputs of their own.
    let length_name = if bin_x { "y" } else { "x" };
    let outputs: Vec<crate::mark::GroupOutput> = if bin.outputs.is_empty() {
        if bin_x && bin_y {
            vec![crate::mark::GroupOutput::new(
                "fillOpacity",
                transform::GroupReducer::Count,
            )]
        } else {
            let reducer = match mark.reduce {
                None | Some(Reducer::Count) => transform::GroupReducer::Count,
                Some(Reducer::Sum) => transform::GroupReducer::Sum,
                Some(Reducer::Mean) => transform::GroupReducer::Mean,
                Some(Reducer::Min) => transform::GroupReducer::Min,
                Some(Reducer::Max) => transform::GroupReducer::Max,
                Some(Reducer::Median) => transform::GroupReducer::Median,
                Some(_) => transform::GroupReducer::Count,
            };
            vec![crate::mark::GroupOutput::new(length_name, reducer)]
        }
    } else {
        bin.outputs.clone()
    };

    // Group the rows by series, then by bin cell. The series keep their
    // sorted order, the order the color scale and the stack use.
    let mut series: std::collections::BTreeMap<Option<crate::value::ValueKey>, BinCells> =
        std::collections::BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        let slot =
            |channel: Option<&crate::table::Channel>, edges: Option<transform::BinEdges>| match (
                channel, edges,
            ) {
                (Some(channel), Some(edges)) => {
                    let value = channel
                        .read(&row.row)
                        .as_number()
                        .filter(|v| v.is_finite())?;
                    Some(edges.slot(value))
                }
                _ => Some(0),
            };
        let (Some(xi), Some(yi)) = (slot(x_channel, x_edges), slot(y_channel, y_edges)) else {
            continue;
        };
        series
            .entry(row.z.as_ref().map(|z| z.key()))
            .or_default()
            .entry((xi, yi))
            .or_default()
            .push(index);
    }

    let mut binned: Vec<PreparedRow> = Vec::new();
    // The running top (and bottom, for negative values) of each
    // one-dimensional bin's stack.
    let mut stack_tops: std::collections::HashMap<usize, (f64, f64)> =
        std::collections::HashMap::new();
    for cells in series.values() {
        for (&(xi, yi), members) in cells {
            let mut row = rows[members[0]].clone();
            let records: Vec<&Table> = members.iter().map(|index| &rows[*index].row).collect();
            // A bin outputs its extent (`x1`, `x2`) and, for the marks that
            // read a point (a dot, a text), its midpoint as `x`.
            let point_mark = !matches!(
                mark.kind,
                MarkKind::Rect
                    | MarkKind::RectX
                    | MarkKind::RectY
                    | MarkKind::BarX
                    | MarkKind::BarY
                    | MarkKind::Cell
            );
            if let Some(edges) = x_edges.filter(|_| bin_x) {
                let (x0, x1) = edges.bounds(xi);
                row.x = if point_mark {
                    Value::Number((x0 + x1) / 2.)
                } else {
                    Value::Null
                };
                row.x1 = Some(Value::Number(x0));
                row.x2 = Some(Value::Number(x1));
            }
            if let Some(edges) = y_edges.filter(|_| bin_y) {
                let (y0, y1) = edges.bounds(yi);
                row.y = if point_mark {
                    Value::Number((y0 + y1) / 2.)
                } else {
                    Value::Null
                };
                row.y1 = Some(Value::Number(y0));
                row.y2 = Some(Value::Number(y1));
            }
            for output in &outputs {
                // A reducer reads its input field, else the channel it
                // outputs to (the mark's own `y` for `{y: "sum"}`), else a
                // field of its name.
                let channel = match output.name.as_ref() {
                    "y" if !bin_y => mark.channels.y.as_ref(),
                    "x" if !bin_x => mark.channels.x.as_ref(),
                    "fill" => None,
                    _ => None,
                };
                let input = output.input.clone().or_else(|| match output.name.as_ref() {
                    "x" | "y" => None,
                    name => Some(SharedString::from(name.to_string())),
                });
                let values: Vec<Value> = members
                    .iter()
                    .map(|index| {
                        let record = &rows[*index].row;
                        match (&input, channel) {
                            (Some(field), _) => record.value(field),
                            (None, Some(channel)) => channel.read(record),
                            (None, None) => Value::Null,
                        }
                    })
                    .collect();
                let reduced = output.reducer.reduce(&values, &records);
                let number = reduced.as_number().unwrap_or(0.);
                match output.name.as_ref() {
                    "y" if bin_x && !bin_y => {
                        let (up, down) = stack_tops.entry(xi).or_insert((0., 0.));
                        let base = if number < 0. { down } else { up };
                        row.y = Value::Number(number);
                        row.y1 = Some(Value::Number(*base));
                        row.y2 = Some(Value::Number(*base + number));
                        *base += number;
                    }
                    "x" if bin_y && !bin_x => {
                        let (up, down) = stack_tops.entry(yi).or_insert((0., 0.));
                        let base = if number < 0. { down } else { up };
                        row.x = Value::Number(number);
                        row.x1 = Some(Value::Number(*base));
                        row.x2 = Some(Value::Number(*base + number));
                        *base += number;
                    }
                    "y" => row.y = reduced.clone(),
                    "x" => row.x = reduced.clone(),
                    "fillOpacity" | "opacity" => row.opacity_value = Some(number),
                    "r" => row.r_value = Some(number),
                    _ => {}
                }
                row.row = row.row.clone().with(output.name.clone(), reduced);
            }
            binned.push(row);
        }
    }

    binned
}

/// Rewrite every row's y interval from the stacked series.
///
/// Plot stacks two shapes of data: *long* data, where one field names the
/// series (`stack_series("platform")`), and *wide* data, where the mark names
/// one value channel per series (`y1("tablet").y2("desktop")`). Both end in
/// the same series list.
fn stack_rows(rows: Vec<PreparedRow>, mark: &Mark) -> Vec<PreparedRow> {
    let Some(stack) = &mark.stack else {
        return rows;
    };

    // A horizontal mark puts its categories on y and its values on x; a
    // vertical one is the other way round.
    let group_by_y = mark.kind.category().groups_on_y;
    let (category_channel, value_channel) = if group_by_y {
        (&mark.channels.y, &mark.channels.x)
    } else {
        (&mark.channels.x, &mark.channels.y)
    };

    let field_name = |channel: &Option<crate::table::Channel>| match channel {
        Some(crate::table::Channel::Field(name)) => Some(name.clone()),
        _ => None,
    };

    let Some(category_field) = field_name(category_channel) else {
        return rows;
    };
    let data = Dataset::new(rows.iter().map(|row| row.row.clone()));

    let series = match &stack.series_field {
        // Long data: the field names the series, the value channel holds the
        // value.
        Some(field) => {
            let Some(value_field) = field_name(value_channel) else {
                return rows;
            };
            transform::stack_long(&data, &category_field, field, &value_field, stack.offset)
        }
        // Wide data: the caller names the value fields to stack.
        None if !stack.fields.is_empty() => {
            let keys: Vec<&str> = stack.fields.iter().map(|field| field.as_ref()).collect();
            transform::stack_wide(&data, &category_field, &keys, stack.offset)
        }
        None => return rows,
    };

    stack_rows_from_series(rows, mark, &series)
}

/// Apply a stacked series to the rows: the series' points replace the y
/// intervals, and the rows keep their x and original record.
fn stack_rows_from_series(
    rows: Vec<PreparedRow>,
    mark: &Mark,
    series: &[StackedSeries],
) -> Vec<PreparedRow> {
    let group_by_y = mark.kind.category().groups_on_y;
    let mut out: Vec<PreparedRow> = Vec::new();

    for stacked in series {
        for point in &stacked.points {
            // Find the row this point came from so the tooltip keeps its
            // record; a point per (series, category) has one.
            let source = rows
                .iter()
                .find(|row| {
                    let key = if group_by_y {
                        row.y_value()
                    } else {
                        row.x_value()
                    };
                    key.key() == point.x.key()
                        && row.z.as_ref().is_some_and(|z| z.label() == stacked.key)
                })
                .or_else(|| {
                    rows.iter().find(|row| {
                        let key = if group_by_y {
                            row.y_value()
                        } else {
                            row.x_value()
                        };
                        key.key() == point.x.key()
                    })
                })
                .cloned();

            let mut row = source.unwrap_or_else(|| PreparedRow {
                row: Table::empty(),
                index: 0,
                x: point.x.clone(),
                y: Value::Null,
                x1: None,
                x2: None,
                y1: None,
                y2: None,
                z: None,
                path: None,
                r: None,
                symbol: None,
                opacity: None,
                r_value: None,
                opacity_value: None,
                dodge_offset: None,
            });
            if group_by_y {
                row.x1 = Some(Value::Number(point.y0));
                row.x2 = Some(Value::Number(point.y1));
            } else {
                row.y1 = Some(Value::Number(point.y0));
                row.y2 = Some(Value::Number(point.y1));
            }
            row.x = point.x.clone();
            row.z = Some(Value::Text(stacked.key.clone()));
            out.push(row);
        }
    }

    // Keep the stacking order: the series' keys order the shapes, and within
    // each series the x order decides where they land.
    out
}

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

#[cfg(test)]
mod stack_tests {
    use super::*;
    use crate::mark::MarkKind;

    fn long_rows() -> Vec<Table> {
        vec![
            Table::empty()
                .with("m", Value::text("a"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("m", Value::text("a"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(30.)),
            Table::empty()
                .with("m", Value::text("b"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(20.)),
            Table::empty()
                .with("m", Value::text("b"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(20.)),
        ]
    }

    #[test]
    fn long_stack_rewrites_intervals() {
        let mark = Mark::new(MarkKind::BarY)
            .x("m")
            .y("v")
            .z("p")
            .stack(StackOffset::Normalize)
            .stack_series("p");
        let prepared = prepare(&Dataset::new(long_rows()), &[mark]);
        let rows = &prepared[0].rows;
        assert_eq!(rows.len(), 4, "one row per (series, category)");
        let a: Vec<(f64, f64)> = rows
            .iter()
            .filter(|row| row.x_value() == &Value::text("a"))
            .map(|row| {
                (
                    row.y1.as_ref().unwrap().as_number().unwrap(),
                    row.y2.as_ref().unwrap().as_number().unwrap(),
                )
            })
            .collect();
        assert_eq!(a, vec![(0., 0.25), (0.25, 1.)]);
    }

    #[test]
    fn wide_stack_reads_the_named_fields() {
        let rows = vec![
            Table::empty()
                .with("m", Value::text("a"))
                .with("x", Value::Number(1.))
                .with("y", Value::Number(3.)),
            Table::empty()
                .with("m", Value::text("b"))
                .with("x", Value::Number(2.))
                .with("y", Value::Number(4.)),
        ];
        let mark = Mark::new(MarkKind::AreaY)
            .x("m")
            .y("x")
            .stack(StackOffset::Zero)
            .stack_fields(["x", "y"]);
        let prepared = prepare(&Dataset::new(rows), &[mark]);
        assert_eq!(prepared[0].rows.len(), 4);
        let top = prepared[0]
            .rows
            .iter()
            .filter(|row| row.x_value() == &Value::text("a"))
            .map(|row| row.y2.as_ref().unwrap().as_number().unwrap())
            .fold(0.0_f64, f64::max);
        assert_eq!(top, 4.);
    }

    fn grouped_rows() -> Vec<Table> {
        vec![
            Table::empty()
                .with("c", Value::text("a"))
                .with("v", Value::Number(1.))
                .with("sex", Value::text("F")),
            Table::empty()
                .with("c", Value::text("a"))
                .with("v", Value::Number(3.))
                .with("sex", Value::text("M")),
            Table::empty()
                .with("c", Value::text("b"))
                .with("v", Value::Number(5.))
                .with("sex", Value::text("F")),
        ]
    }

    #[test]
    fn group_x_counts_rows_per_category() {
        let mark = Mark::new(MarkKind::BarY).x("c").y("v").group_x();
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        assert_eq!(rows.len(), 2, "one row per category");
        assert_eq!(rows[0].x_value(), &Value::text("a"));
        assert_eq!(rows[0].y_value(), &Value::Number(2.));
        assert_eq!(rows[1].x_value(), &Value::text("b"));
        assert_eq!(rows[1].y_value(), &Value::Number(1.));
    }

    #[test]
    fn a_grouped_bar_keeps_its_baseline() {
        // The grouping writes the count into `y` after the raw rows already
        // grew their interval from zero, so the interval has to be derived
        // again for the bar to have a length to draw.
        let mark = Mark::new(MarkKind::BarY).x("c").y("v").group_x();
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        assert_eq!(
            rows[0].y_interval(),
            Some((Value::Number(0.), Value::Number(2.)))
        );
        assert_eq!(
            rows[1].y_interval(),
            Some((Value::Number(0.), Value::Number(1.)))
        );
    }

    #[test]
    fn group_x_reduces_a_named_channel() {
        let mark = Mark::new(MarkKind::BarY)
            .x("c")
            .y("v")
            .group_x()
            .group_output("y", crate::transform::GroupReducer::Sum);
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        // The later output replaces the default count, and sums the y values.
        assert_eq!(rows[0].y_value(), &Value::Number(4.));
        assert_eq!(rows[1].y_value(), &Value::Number(5.));
    }

    #[test]
    fn find_picks_the_row_matching_its_test() {
        // Plot's example: a bar's two ends come from two rows of the same x,
        // chosen by a field rather than by an extreme.
        let mark = Mark::new(MarkKind::BarY)
            .x("c")
            .y("v")
            .group_x()
            .group_output(
                "y1",
                crate::transform::GroupReducer::find(|row| {
                    row.value("sex").label().as_ref() == "F"
                }),
            )
            .group_output(
                "y2",
                crate::transform::GroupReducer::find(|row| {
                    row.value("sex").label().as_ref() == "M"
                }),
            );
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        let a = rows
            .iter()
            .find(|row| row.x_value() == &Value::text("a"))
            .unwrap();
        assert_eq!(a.y1.as_ref().unwrap(), &Value::Number(1.));
        assert_eq!(a.y2.as_ref().unwrap(), &Value::Number(3.));
        // Category "b" has no M row, so its upper end is missing rather than
        // borrowing a neighbour's value.
        let b = rows
            .iter()
            .find(|row| row.x_value() == &Value::text("b"))
            .unwrap();
        assert_eq!(b.y1.as_ref().unwrap(), &Value::Number(5.));
        assert!(b.y2.is_none());
    }

    #[test]
    fn group_z_subdivides_by_series() {
        let mark = Mark::new(MarkKind::BarY).x("c").y("v").z("sex").group_z();
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        // One row per series, each holding the series' count.
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].z.as_ref().unwrap(), &Value::text("F"));
        assert_eq!(rows[0].row.number("fill"), Some(2.));
        assert_eq!(rows[1].z.as_ref().unwrap(), &Value::text("M"));
        assert_eq!(rows[1].row.number("fill"), Some(1.));
    }
}

#[cfg(test)]
mod facet_tests {
    use super::*;
    use crate::facet::Grid;
    use crate::mark::MarkKind;

    #[test]
    fn a_stack_summarizes_within_its_facet() {
        // Two facets, each with both series, so a whole-figure stack would
        // reach 100 in each and a per-facet one reaches the facet's own total.
        let rows = vec![
            Table::empty()
                .with("f", Value::text("one"))
                .with("m", Value::text("a"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("f", Value::text("one"))
                .with("m", Value::text("a"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(30.)),
            Table::empty()
                .with("f", Value::text("two"))
                .with("m", Value::text("a"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(20.)),
            Table::empty()
                .with("f", Value::text("two"))
                .with("m", Value::text("a"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(20.)),
        ];
        let mark = Mark::new(MarkKind::BarY)
            .x("m")
            .y("v")
            .z("p")
            .stack(StackOffset::Normalize)
            .stack_series("p");
        let grid = Grid::from_values(vec![Value::text("one"), Value::text("two")], Vec::new());
        let (facets, populated) =
            prepare_facets(&Dataset::new(rows), &[mark], &grid, Some("f"), None);
        assert_eq!(facets.len(), 2);
        assert_eq!(populated, vec![true, true]);
        for marks in &facets {
            let top = marks[0]
                .rows
                .iter()
                .map(|row| row.y2.as_ref().unwrap().as_number().unwrap())
                .fold(0.0_f64, f64::max);
            // Normalized within the facet: the two series add to one.
            assert_eq!(top, 1.);
        }
    }

    #[test]
    fn an_unfaceted_grid_prepares_every_row_once() {
        let rows = vec![Table::empty().with("x", Value::Number(1.))];
        let mark = Mark::new(MarkKind::Dot).x("x");
        let grid = Grid::from_values(Vec::new(), Vec::new());
        let (facets, populated) = prepare_facets(&Dataset::new(rows), &[mark], &grid, None, None);
        assert_eq!(facets.len(), 1);
        assert_eq!(facets[0][0].rows.len(), 1);
        assert!(populated.is_empty());
    }

    #[test]
    fn preparing_is_a_pure_function() {
        // Plot caches the prepared facets from prepaint and reuses them for
        // hover, tooltip and paint. That is only faithful if resolving the
        // same data twice yields identical rows — not just the same values,
        // but the same per-row records, so a hovered datum is placed exactly
        // where it was painted.
        let rows = vec![
            Table::empty()
                .with("f", Value::text("one"))
                .with("x", Value::Number(1.))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("f", Value::text("one"))
                .with("x", Value::Number(2.))
                .with("v", Value::Number(20.)),
        ];
        let mark = Mark::new(MarkKind::Dot).x("x").y("v");
        let grid = Grid::from_values(vec![Value::text("one")], Vec::new());
        let dataset = Dataset::new(rows);

        let (first, _) = prepare_facets(
            &dataset,
            std::slice::from_ref(&mark),
            &grid,
            Some("f"),
            None,
        );
        for _ in 0..2 {
            let (again, _) = prepare_facets(
                &dataset,
                std::slice::from_ref(&mark),
                &grid,
                Some("f"),
                None,
            );
            // PreparedMark is not PartialEq (its mark holds closures), so
            // compare the resolved rows: the shape is the position a hovered
            // datum is placed at, and it must be identical across calls.
            let describe = |facets: &[Vec<PreparedMark>]| {
                let cell = &facets[0][0];
                (
                    cell.mark.kind,
                    cell.rows
                        .iter()
                        .map(|row| {
                            (
                                row.x_value().clone(),
                                row.y_value().clone(),
                                row.index,
                                row.row.value("v"),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            };
            assert_eq!(
                describe(&first),
                describe(&again),
                "preparation must be stable across calls"
            );
        }
    }
}

/// The transforms' composition order in [`prepare_mark`], pinned so a future
/// rearrangement cannot silently reorder them.
///
/// The pipeline is `select/reduce` → `map` → `shift` → `shuffle` (record
/// rewrites, run on the cloned `Dataset` before any row resolves), then
/// `sort` → `window` → `normalize` → `group` → `reduce` → `bin` → `stack` (row
/// rewrites, run on the resolved rows). Each test below exercises one edge of
/// that order: if a transform moved, an observable value changes.
#[cfg(test)]
mod transform_order_tests {
    use super::*;
    use crate::mark::{MapReducer, Mark, MarkKind};
    use crate::transform::WindowReducer;

    fn nums(values: &[f64]) -> Vec<Table> {
        values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Table::empty()
                    .with("x", Value::Number(i as f64))
                    .with("g", Value::text(if i % 2 == 0 { "a" } else { "b" }))
                    .with("v", Value::Number(*v))
            })
            .collect()
    }

    #[test]
    fn map_runs_before_resolve() {
        // `map` folds `v` into a new `total` field; the y channel reads it, so
        // the field must exist before the row resolves. If resolve ran first,
        // the y channel would find no `total` and read nothing.
        let mark = Mark::new(MarkKind::BarY)
            .x("x")
            .y("total")
            .map("v", "total", MapReducer::Sum);
        let rows = &prepare(&Dataset::new(nums(&[1., 2., 3., 4.])), &[mark])[0].rows;
        assert_eq!(
            rows.iter().map(|r| r.y_value()).collect::<Vec<_>>(),
            vec![
                &Value::Number(1.),
                &Value::Number(3.),
                &Value::Number(6.),
                &Value::Number(10.)
            ]
        );
    }

    #[test]
    fn shift_runs_before_resolve() {
        let mark = Mark::new(MarkKind::BarY)
            .x("x")
            .y("shifted")
            .shift("v", "shifted", 10.);
        let rows = &prepare(&Dataset::new(nums(&[1., 2., 3.])), &[mark])[0].rows;
        assert_eq!(
            rows.iter().map(|r| r.y_value()).collect::<Vec<_>>(),
            vec![
                &Value::Number(11.),
                &Value::Number(12.),
                &Value::Number(13.)
            ]
        );
    }

    #[test]
    fn sort_runs_before_group() {
        // Sorting by the value, then grouping by x, must group in the sorted
        // (descending) order. If group ran first, the categories would appear
        // in their original order.
        let mark = Mark::new(MarkKind::BarY)
            .x("x")
            .y("v")
            .sort("v", true)
            .group_x();
        let rows = &prepare(&Dataset::new(nums(&[10., 1., 20., 2.])), &[mark])[0].rows;
        // Sorted descending: 20 (i=2), 10 (i=0), 2 (i=3), 1 (i=1). Grouped by
        // x, so the x values keep the sorted encounter order.
        assert_eq!(rows[0].x_value(), &Value::Number(2.));
        assert_eq!(rows[1].x_value(), &Value::Number(0.));
        assert_eq!(rows[2].x_value(), &Value::Number(3.));
        assert_eq!(rows[3].x_value(), &Value::Number(1.));
    }

    #[test]
    fn window_runs_before_draw() {
        // A window rewrites the y value in place; the first row of a backward
        // cumulative window is its own value only if the window ran before the
        // rows were read for placement.
        let mark = Mark::new(MarkKind::Line).x("x").y("v").window(
            2,
            WindowReducer::CumulativeMean,
            Anchor::Start,
        );
        let rows = &prepare(&Dataset::new(nums(&[2., 4., 6., 8.])), &[mark])[0].rows;
        // Cumulative means: 2, 3, 4, 5.
        assert_eq!(
            rows.iter().map(|r| r.y_value()).collect::<Vec<_>>(),
            vec![
                &Value::Number(2.),
                &Value::Number(3.),
                &Value::Number(4.),
                &Value::Number(5.)
            ]
        );
    }

    #[test]
    fn normalize_rescales_before_group_counts() {
        // `normalize` rescales y to [0, 1] and a `group` then bins by it. Three
        // equal rows normalize to the same point, so their group's count sees
        // the normalized value, not the raw one.
        let data = vec![
            Table::empty()
                .with("x", Value::Number(0.))
                .with("g", Value::text("a"))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("x", Value::Number(1.))
                .with("g", Value::text("a"))
                .with("v", Value::Number(20.)),
            Table::empty()
                .with("x", Value::Number(2.))
                .with("g", Value::text("a"))
                .with("v", Value::Number(0.)),
        ];
        let mark = Mark::new(MarkKind::BarY)
            .x("g")
            .y("v")
            .normalize()
            .group_x();
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        // All three share x="a", so one group. Its count is 3 regardless of
        // the normalized values; the important edge is that normalize ran on
        // the whole set first (span 20), not after grouping.
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].y_value(), &Value::Number(3.));
    }

    #[test]
    fn reduce_runs_after_resolve() {
        // `reduce` by sum collapses rows sharing a category (x here) into one
        // total. If reduce ran before the rows were resolved, the categories
        // wouldn't be available to group on.
        let data = vec![
            Table::empty()
                .with("x", Value::text("a"))
                .with("v", Value::Number(1.)),
            Table::empty()
                .with("x", Value::text("a"))
                .with("v", Value::Number(2.)),
            Table::empty()
                .with("x", Value::text("b"))
                .with("v", Value::Number(3.)),
            Table::empty()
                .with("x", Value::text("b"))
                .with("v", Value::Number(4.)),
        ];
        let mark = Mark::new(MarkKind::BarY).x("x").y("v").reduce(Reducer::Sum);
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        // One row per category, summed.
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].x_value(), &Value::text("a"));
        assert_eq!(rows[0].y_value(), &Value::Number(3.));
        assert_eq!(rows[1].x_value(), &Value::text("b"));
        assert_eq!(rows[1].y_value(), &Value::Number(7.));
    }
}

#[cfg(test)]
mod interval_tests {
    use super::*;
    use crate::mark::{Interval, Mark, MarkKind};

    #[test]
    fn a_numeric_step_buckets_x_into_an_interval() {
        let data = vec![
            Table::empty()
                .with("t", Value::Number(5.))
                .with("v", Value::Number(2.)),
        ];
        let mark = Mark::new(MarkKind::RectY)
            .x("t")
            .y("v")
            .interval(Interval::Step(10.));
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        // 5 floors to [0, 10); the value grows the y interval to [0, 2].
        assert_eq!(rows[0].x1, Some(Value::Number(0.)));
        assert_eq!(rows[0].x2, Some(Value::Number(10.)));
        assert_eq!(rows[0].y1, Some(Value::Number(0.)));
        assert_eq!(rows[0].y2, Some(Value::Number(2.)));
    }

    #[test]
    fn a_day_interval_buckets_dates_by_calendar_day() {
        let date = Value::date(chrono::NaiveDate::from_ymd_opt(2024, 3, 15).unwrap());
        let data = vec![Table::empty().with("t", date).with("v", Value::Number(4.))];
        let mark = Mark::new(MarkKind::RectY)
            .x("t")
            .y("v")
            .interval(Interval::Day);
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        let start = rows[0].x1.as_ref().unwrap().as_number().unwrap();
        let end = rows[0].x2.as_ref().unwrap().as_number().unwrap();
        // Midnight on the 15th to midnight on the 16th.
        let midnight = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2024, 3, 15, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis() as f64;
        assert_eq!(start, midnight);
        assert_eq!(end, midnight + 86_400_000.);
    }
}

#[cfg(test)]
mod interval_domain_tests {
    use super::*;
    use crate::mark::{Interval, Mark, MarkKind};

    #[test]
    fn month_interval_domain_spans_the_data() {
        let dates = (0..3)
            .map(|m| {
                Table::empty()
                    .with(
                        "d",
                        Value::date(chrono::NaiveDate::from_ymd_opt(2024, m + 1, 15).unwrap()),
                    )
                    .with("v", Value::Number(1.))
            })
            .collect::<Vec<_>>();
        let mark = Mark::new(MarkKind::RectY)
            .x("d")
            .y("v")
            .interval(Interval::Month);
        let prepared = prepare(&Dataset::new(dates), &[mark]);
        let xs = crate::render::basic::x_values(&prepared[0]);
        let first = xs[0].clone();
        let kinds: Vec<_> = xs.iter().map(|v| v.kind()).collect();
        assert!(
            kinds.iter().all(|k| *k == PlotKind::Date),
            "interval boundaries must stay dates: {kinds:?}"
        );
        assert_eq!(first.kind(), PlotKind::Date);
        // First boundary is Jan 1 2024, not 1980.
        let millis = first.as_date().unwrap().timestamp_millis();
        assert_eq!(
            millis,
            chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2024, 1, 1, 0, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );
    }

    use crate::value::PlotKind;
}

#[cfg(test)]
mod bin_tests {
    use super::*;
    use crate::mark::MarkKind;
    use crate::transform::GroupReducer;

    fn athletes() -> Dataset {
        // Two series over weights 40..=99, heights two values apart.
        Dataset::new((0..120).map(|i| {
            Table::empty()
                .with(
                    "sex",
                    Value::text(if i % 2 == 0 { "female" } else { "male" }),
                )
                .with("weight", Value::Number(40. + (i / 2) as f64))
                .with("height", Value::Number(if i % 4 < 2 { 1.6 } else { 1.9 }))
        }))
    }

    #[test]
    fn a_fill_field_bins_and_stacks_each_series() {
        let mark: Mark = Mark::new(MarkKind::RectY)
            .x("weight")
            .fill("sex")
            .bin_output("y", GroupReducer::Count);
        let prepared = prepare(&athletes(), &[mark]);
        let rows = &prepared[0].rows;
        // Both series share the bins' edges.
        let edges = |z: &str| -> Vec<(Value, Value)> {
            rows.iter()
                .filter(|row| row.z.as_ref().map(Value::label).as_deref() == Some(z))
                .map(|row| row.x_interval().unwrap())
                .collect()
        };
        assert_eq!(edges("female"), edges("male"));
        // Each bin's male rect stacks on its female rect, the counts summing
        // to every row.
        let total: f64 = rows.iter().filter_map(|row| row.y.as_number()).sum();
        assert_eq!(total, 120.);
        let top = rows
            .iter()
            .filter_map(|row| row.y2.as_ref()?.as_number())
            .fold(0., f64::max);
        let tallest_bin = rows
            .iter()
            .filter(|row| row.y1.as_ref().and_then(Value::as_number) == Some(0.))
            .count();
        assert!(top > 0. && tallest_bin > 0);
        for row in rows
            .iter()
            .filter(|row| row.z.as_ref().unwrap().label() == "male")
        {
            assert!(
                row.y1.as_ref().unwrap().as_number().unwrap() > 0.,
                "male stacks on female"
            );
        }
    }

    #[test]
    fn a_rect_naming_x_and_y_bins_both() {
        let mark: Mark = Mark::new(MarkKind::Rect)
            .x("weight")
            .y("height")
            .fill("sex")
            .bin_output("fillOpacity", GroupReducer::Count);
        assert_eq!(bin_axes(&mark), (true, true));
        let mut facets = vec![prepare(&athletes(), &[mark])];
        apply_quantity_scales(&mut facets, &Default::default(), &Default::default());
        let rows = &facets[0][0].rows;
        assert!(rows.iter().all(|row| row.x1.is_some() && row.y1.is_some()));
        // The opacity scale runs [0, max] → [0, 1].
        let max = rows.iter().filter_map(|row| row.opacity).fold(0., f32::max);
        assert_eq!(max, 1.);
        assert_eq!(
            rows.iter()
                .map(|row| row.row.number("fillOpacity").unwrap())
                .sum::<f64>(),
            120.
        );
    }

    #[test]
    fn auto_thresholds_follow_scotts_rule_on_nice_edges() {
        let values: Vec<f64> = (0..1000).map(|i| (i % 100) as f64 + 31.).collect();
        let edges = transform::BinEdges::of(&values, Thresholds::Auto).unwrap();
        // Scott: 3.49·σ·n^(-1/3) ≈ 10 → ten bins of a nice width 10 from 30.
        assert_eq!(edges.width, 10.);
        assert_eq!(edges.start, 30.);
        assert_eq!(edges.slot(130.), edges.count - 1);
    }

    #[test]
    fn a_color_field_implies_the_series() {
        let mark: Mark = Mark::new(MarkKind::Dot)
            .x("weight")
            .y("height")
            .stroke("sex");
        assert!(
            matches!(&mark.channels.z, Some(crate::table::Channel::Field(name)) if name == "sex")
        );
    }
}
