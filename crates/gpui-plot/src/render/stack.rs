//! Plot's stack and normalize transforms over prepared rows.
//!
//! `stackY` groups the rows by x (and stacks within each group in series
//! order), turning each row's `y` into an interval `[y1, y2]`: positive
//! values stack up from zero and negative ones down. Its output `y` is the
//! interval's middle, or its start or end for `stackY1` and `stackY2`. The
//! bar, area and rect marks stack implicitly, as Plot's do. `normalizeX`
//! divides each series by a basis.

use crate::mark::{Axis, Mark, MarkKind};
use crate::render::PreparedRow;
use crate::value::{Value, ValueKey};
use rustc_hash::FxHashMap;

/// Which value a stack outputs as the row's position, Plot's `stackY`,
/// `stackY1` and `stackY2`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackAnchor {
    #[default]
    Middle,
    Start,
    End,
}

/// The order series stack in, Plot's stack `order`.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum StackOrder {
    /// The series' first appearance in the data, Plot's default.
    #[default]
    Input,
    /// By the series' value, Plot's `"value"`: smallest total first.
    Value,
    /// By the series' total, `"sum"`.
    Sum,
    /// By where each series peaks, `"appearance"`.
    Appearance,
    /// Inside-out, `"inside-out"`, for streamgraphs.
    InsideOut,
    /// Alphabetically by `z`, `"z"`.
    Z,
    /// An explicit order of `z` values.
    Explicit(Vec<Value>),
}

/// Plot's stack `offset`.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Offset {
    /// Stack up from zero, Plot's default.
    #[default]
    Zero,
    /// Divide each stack by its total so it spans `[0, 1]`.
    Normalize,
    /// Center each stack on zero.
    Center,
    /// Center and minimize the weighted change in slope, a streamgraph.
    Wiggle,
    /// Plot's custom `offset` function, `(stacks, X1, X2, Z) => …`, which
    /// shifts the stacked intervals in place (the docs' Likert offset).
    Custom(CustomOffset),
}

/// The signature of a custom stack offset: the facet's stacks (each the row
/// indices sharing a position, in stack order), the rows' interval starts
/// and ends to rewrite, and the rows' `z` values.
pub type OffsetFn = dyn Fn(&[Vec<usize>], &mut [f64], &mut [f64], &[Value]);

/// A custom stack offset, compared by identity.
#[derive(Clone)]
pub struct CustomOffset(pub std::rc::Rc<OffsetFn>);

impl Offset {
    /// A custom offset, Plot's `offset: (facets, X1, X2, Z) => …`, called
    /// with each facet's stacks.
    pub fn custom(
        offset: impl Fn(&[Vec<usize>], &mut [f64], &mut [f64], &[Value]) + 'static,
    ) -> Self {
        Self::Custom(CustomOffset(std::rc::Rc::new(offset)))
    }
}

impl std::fmt::Debug for CustomOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomOffset")
    }
}

impl PartialEq for CustomOffset {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }
}

/// One stack transform's options.
#[derive(Clone, Debug, PartialEq)]
pub struct StackSpec {
    /// The axis the values stack along: `Y` for `stackY`.
    pub axis: Axis,
    pub anchor: StackAnchor,
    pub offset: Offset,
    pub order: StackOrder,
    pub reverse: bool,
    /// Keep the row's value channel as it was: Plot's implicit stack on a
    /// bar or area sets only the interval the mark draws, and the tooltip
    /// and grouping read the value itself.
    pub keep_value: bool,
}

impl StackSpec {
    pub fn new(axis: Axis) -> Self {
        Self {
            axis,
            anchor: StackAnchor::Middle,
            offset: Offset::Zero,
            order: StackOrder::Input,
            reverse: false,
            keep_value: false,
        }
    }
}

/// The stack a mark applies: its explicit one, or Plot's implicit stack for
/// a bar, area or rect that names a single value channel on its length axis.
pub fn effective_stack(mark: &Mark) -> Option<StackSpec> {
    if let Some(spec) = &mark.stack_spec {
        return Some(spec.clone());
    }
    // The legacy `stack` options run their own transform.
    if mark.stack.is_some() || mark.bin.is_some() {
        return None;
    }
    let c = &mark.channels;
    // A group or an interval that outputs the mark's interval channels
    // leaves nothing to stack, as in Plot, where the stack only fills in a
    // missing `y1`/`y2`.
    let outputs = |a: &str, b: &str| {
        mark.group
            .as_ref()
            .is_some_and(|g| g.outputs.iter().any(|o| o.name == a || o.name == b))
            || mark.interval.is_some()
    };
    // A transform's `y` output is the value to stack, as a `y` channel is.
    let y_out = mark.group.as_ref().is_some_and(|g| {
        g.outputs.iter().any(|o| o.name == "y")
            || (g.outputs.is_empty() && g.axis == Some(Axis::X) && !g.both)
    });
    let x_out = mark.group.as_ref().is_some_and(|g| {
        g.outputs.iter().any(|o| o.name == "x")
            || (g.outputs.is_empty() && g.axis == Some(Axis::Y) && !g.both)
    });
    match mark.kind {
        MarkKind::BarY | MarkKind::AreaY | MarkKind::RectY | MarkKind::WaffleY
            if (c.y.is_some() || y_out)
                && c.y1.is_none()
                && c.y2.is_none()
                && !outputs("y1", "y2") =>
        {
            Some(StackSpec {
                keep_value: true,
                ..StackSpec::new(Axis::Y)
            })
        }
        MarkKind::BarX | MarkKind::AreaX | MarkKind::RectX | MarkKind::WaffleX
            if (c.x.is_some() || x_out)
                && c.x1.is_none()
                && c.x2.is_none()
                && !outputs("x1", "x2") =>
        {
            Some(StackSpec {
                keep_value: true,
                ..StackSpec::new(Axis::X)
            })
        }
        _ => None,
    }
}

/// Stack the rows: each row's value along `spec.axis` becomes an interval,
/// stacked with the other rows that share its position on the other axis.
pub fn stack(rows: &mut [PreparedRow], spec: &StackSpec) {
    let along_y = spec.axis == Axis::Y;
    let value_of = |row: &PreparedRow| -> f64 {
        // The value to stack: the channel itself, Plot's `y` for stackY. A
        // derived `[0, y]` interval stands for the same value.
        let value = if along_y { &row.y } else { &row.x };
        let value = if value.is_null() {
            if along_y {
                row.y2.as_ref()
            } else {
                row.x2.as_ref()
            }
            .unwrap_or(value)
        } else {
            value
        };
        value.as_number().unwrap_or(f64::NAN)
    };
    let index_of = |row: &PreparedRow| -> ValueKey {
        if along_y {
            row.x_value().key()
        } else {
            row.y_value().key()
        }
    };
    let values: Vec<f64> = rows.iter().map(value_of).collect();

    // The series in first-appearance order, and each row's series.
    let mut series: Vec<ValueKey> = Vec::new();
    let mut series_slots: FxHashMap<ValueKey, usize> = FxHashMap::default();
    let row_series: Vec<usize> = rows
        .iter()
        .map(|row| {
            let key = row
                .z
                .as_ref()
                .map(Value::key)
                .unwrap_or_else(|| Value::Null.key());
            match series_slots.get(&key) {
                Some(&slot) => slot,
                None => {
                    series_slots.insert(key.clone(), series.len());
                    series.push(key);
                    series.len() - 1
                }
            }
        })
        .collect();
    // Each series' total, its finite values summed in row order (from the
    // float sum's own zero, so an all `-0` series totals what `sum` gives).
    let totals = || -> Vec<f64> {
        let mut totals = vec![std::iter::empty::<f64>().sum::<f64>(); series.len()];
        for (&slot, &v) in row_series.iter().zip(&values) {
            if v.is_finite() {
                totals[slot] += v;
            }
        }
        totals
    };

    // The series order, as slots into `series`. Each key below is computed
    // once per series; the sorts are stable, so ties keep appearance order.
    let mut order: Vec<usize> = (0..series.len()).collect();
    match &spec.order {
        StackOrder::Input => {}
        StackOrder::Z => order.sort_by(|a, b| series[*a].cmp(&series[*b])),
        StackOrder::Value | StackOrder::Sum => {
            let totals = totals();
            order.sort_by(|a, b| totals[*a].total_cmp(&totals[*b]));
        }
        StackOrder::Appearance | StackOrder::InsideOut => {
            // Where each series peaks, by its index's position in the data:
            // the first row holding its largest value.
            let mut peaks = vec![(f64::NEG_INFINITY, usize::MAX); series.len()];
            for (i, (&slot, &v)) in row_series.iter().zip(&values).enumerate() {
                if v > peaks[slot].0 {
                    peaks[slot] = (v, i);
                }
            }
            order.sort_by_key(|slot| peaks[*slot].1);
            if spec.order == StackOrder::InsideOut {
                // d3's insideOut: alternate the series onto the top and the
                // bottom, the heaviest nearest the middle.
                let totals = totals();
                let (mut top, mut bottom) = (Vec::new(), Vec::new());
                let (mut top_sum, mut bottom_sum) = (0., 0.);
                for &slot in order.iter() {
                    if top_sum < bottom_sum {
                        top_sum += totals[slot];
                        top.push(slot);
                    } else {
                        bottom_sum += totals[slot];
                        bottom.push(slot);
                    }
                }
                bottom.reverse();
                order = bottom.into_iter().chain(top).collect();
            }
        }
        StackOrder::Explicit(explicit) => {
            // Each key's first position in the explicit order; a series it
            // leaves out sorts last.
            let mut positions: FxHashMap<ValueKey, usize> = FxHashMap::default();
            for (position, value) in explicit.iter().enumerate() {
                positions.entry(value.key()).or_insert(position);
            }
            let ranks: Vec<usize> = series
                .iter()
                .map(|key| positions.get(key).copied().unwrap_or(usize::MAX))
                .collect();
            order.sort_by_key(|slot| ranks[*slot]);
        }
    }
    if spec.reverse {
        order.reverse();
    }
    // Each row's rank, its series' place in the order.
    let mut series_rank = vec![0; series.len()];
    for (rank, &slot) in order.iter().enumerate() {
        series_rank[slot] = rank;
    }
    let ranks: Vec<usize> = row_series.iter().map(|slot| series_rank[*slot]).collect();

    // Group the rows by their index, by first appearance, keeping each group
    // in series order.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_slots: FxHashMap<ValueKey, usize> = FxHashMap::default();
    for (i, row) in rows.iter().enumerate() {
        let group = *group_slots.entry(index_of(row)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(i);
    }
    let mut intervals: Vec<(f64, f64)> = vec![(f64::NAN, f64::NAN); rows.len()];
    for members in groups.iter_mut() {
        members.sort_by_key(|i| ranks[*i]);
        let (mut positive, mut negative) = (0.0_f64, 0.0_f64);
        for &i in members.iter() {
            let v = values[i];
            if !v.is_finite() {
                continue;
            }
            if v >= 0. {
                intervals[i] = (positive, positive + v);
                positive += v;
            } else {
                intervals[i] = (negative, negative + v);
                negative += v;
            }
        }
        match spec.offset {
            Offset::Zero | Offset::Custom(_) => {}
            Offset::Normalize => {
                let span = positive - negative;
                if span > 0. {
                    for &i in members.iter() {
                        let (a, b) = intervals[i];
                        intervals[i] = ((a - negative) / span, (b - negative) / span);
                    }
                }
            }
            Offset::Center | Offset::Wiggle => {
                let mid = (positive + negative) / 2.;
                for &i in members.iter() {
                    let (a, b) = intervals[i];
                    intervals[i] = (a - mid, b - mid);
                }
            }
        }
    }
    if spec.offset == Offset::Wiggle {
        wiggle(&groups, &mut intervals, &ranks);
    }
    if let Offset::Custom(CustomOffset(offset)) = &spec.offset {
        let (mut x1, mut x2): (Vec<f64>, Vec<f64>) = intervals.iter().copied().unzip();
        let z: Vec<Value> = rows
            .iter()
            .map(|row| row.z.clone().unwrap_or(Value::Null))
            .collect();
        offset(&groups, &mut x1, &mut x2, &z);
        intervals = x1.into_iter().zip(x2).collect();
    }

    for (row, (a, b)) in rows.iter_mut().zip(intervals) {
        if !a.is_finite() {
            continue;
        }
        let position = match spec.anchor {
            StackAnchor::Middle => (a + b) / 2.,
            StackAnchor::Start => a,
            StackAnchor::End => b,
        };
        if along_y {
            row.y1 = Some(Value::Number(a));
            row.y2 = Some(Value::Number(b));
            if !spec.keep_value {
                row.y = Value::Number(position);
            }
        } else {
            row.x1 = Some(Value::Number(a));
            row.x2 = Some(Value::Number(b));
            if !spec.keep_value {
                row.x = Value::Number(position);
            }
        }
    }
}

/// d3's `stackOffsetWiggle`, after centering: shift each stack so the
/// weighted slope of its layers is as flat as can be, then lift the whole
/// streamgraph so its lowest point sits at zero. `ranks` holds each row's
/// series rank.
fn wiggle(groups: &[Vec<usize>], intervals: &mut [(f64, f64)], ranks: &[usize]) {
    if groups.len() < 2 {
        return;
    }
    let n = ranks.iter().copied().max().unwrap_or(0) + 1;
    // Each layer's thickness at a group, by series rank: its rows' widths
    // summed in the group's order, from the float sum's own zero.
    let thickness = |group: &[usize], intervals: &[(f64, f64)], out: &mut Vec<f64>| {
        out.clear();
        out.resize(n, std::iter::empty::<f64>().sum::<f64>());
        for &i in group {
            out[ranks[i]] += (intervals[i].1 - intervals[i].0).abs();
        }
    };
    let mut y = 0.0_f64;
    // The first stack starts at zero.
    let base = |group: &[usize], intervals: &[(f64, f64)]| -> f64 {
        group
            .iter()
            .map(|i| intervals[*i].0.min(intervals[*i].1))
            .fold(f64::INFINITY, f64::min)
    };
    let shift = |group: &[usize], intervals: &mut [(f64, f64)], by: f64| {
        for &i in group {
            intervals[i].0 += by;
            intervals[i].1 += by;
        }
    };
    let first = base(&groups[0], intervals);
    shift(&groups[0], intervals, -first);
    let (mut previous_layers, mut current_layers) = (Vec::new(), Vec::new());
    for j in 1..groups.len() {
        let (previous, current) = (&groups[j - 1], &groups[j]);
        // The previous stack is weighed again rather than carried over from
        // the last step: its shift moved its ends, which can change the
        // float difference between them.
        thickness(previous, intervals, &mut previous_layers);
        thickness(current, intervals, &mut current_layers);
        let (mut s1, mut s2) = (0.0_f64, 0.0_f64);
        for i in 0..n {
            let si = current_layers[i];
            let sij0 = previous_layers[i];
            let mut s3 = (si - sij0) / 2.;
            for k in 0..i {
                s3 += current_layers[k] - previous_layers[k];
            }
            s1 += si;
            s2 += s3 * si;
        }
        if s1 != 0. {
            y -= s2 / s1;
        }
        let current_base = base(current, intervals);
        shift(current, intervals, y - current_base);
    }
    let lowest = intervals
        .iter()
        .filter(|(a, _)| a.is_finite())
        .map(|(a, b)| a.min(*b))
        .fold(f64::INFINITY, f64::min);
    if lowest.is_finite() {
        for interval in intervals.iter_mut() {
            interval.0 -= lowest;
            interval.1 -= lowest;
        }
    }
}

/// Plot's normalize `basis`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormalizeBasis {
    /// Divide by the series' first value, Plot's default.
    #[default]
    First,
    Last,
    Min,
    Max,
    Mean,
    Median,
    Sum,
    /// Map the series' extent to `[0, 1]`.
    Extent,
    /// Subtract the mean and divide by the deviation.
    Deviation,
}

/// Plot's `normalizeX`/`normalizeY`: divide each `z` series' values on `axis`
/// by the series' basis.
pub fn normalize(rows: &mut [PreparedRow], axis: Axis, basis: NormalizeBasis) {
    let along_x = axis == Axis::X;
    // Each series' rows, by first appearance, in row order.
    let mut series: Vec<Vec<usize>> = Vec::new();
    let mut slots: FxHashMap<ValueKey, usize> = FxHashMap::default();
    for (i, row) in rows.iter().enumerate() {
        let key = row
            .z
            .as_ref()
            .map(Value::key)
            .unwrap_or_else(|| Value::Null.key());
        let slot = *slots.entry(key).or_insert_with(|| {
            series.push(Vec::new());
            series.len() - 1
        });
        series[slot].push(i);
    }
    // Plot's normalizeY maps each of y, y1 and y2 on its own basis; a mark
    // with a value normalizes that (and drops its interval), one with only
    // an interval (`binX({y2: "count"})`) normalizes the interval's ends. An
    // all-zero end is the bar's baseline and stays put.
    let has_value = rows
        .iter()
        .any(|row| !if along_x { &row.x } else { &row.y }.is_null());
    if has_value {
        normalize_field(rows, &series, along_x, basis, 0);
    } else {
        normalize_field(rows, &series, along_x, basis, 1);
        normalize_field(rows, &series, along_x, basis, 2);
    }
}

/// Normalize one of a channel's fields, `0` the value, `1` the interval's
/// start, `2` its end, per series.
fn normalize_field(
    rows: &mut [PreparedRow],
    series: &[Vec<usize>],
    along_x: bool,
    basis: NormalizeBasis,
    field: usize,
) {
    let slot = |row: &PreparedRow| -> Option<Value> {
        match (along_x, field) {
            (true, 0) => Some(row.x.clone()),
            (false, 0) => Some(row.y.clone()),
            (true, 1) => row.x1.clone(),
            (false, 1) => row.y1.clone(),
            (true, _) => row.x2.clone(),
            (false, _) => row.y2.clone(),
        }
    };
    let read =
        |row: &PreparedRow| -> f64 { slot(row).and_then(|v| v.as_number()).unwrap_or(f64::NAN) };
    for members in series {
        let values: Vec<f64> = members.iter().map(|i| read(&rows[*i])).collect();
        if field != 0 && values.iter().all(|v| *v == 0. || !v.is_finite()) {
            continue;
        }
        let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
        if finite.is_empty() {
            continue;
        }
        let sum: f64 = finite.iter().sum();
        let mean = sum / finite.len() as f64;
        let (lo, hi) = finite
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        let map: Box<dyn Fn(f64) -> f64> = match basis {
            NormalizeBasis::First => {
                let first = values.iter().copied().find(|v| v.is_finite()).unwrap_or(1.);
                Box::new(move |v| v / first)
            }
            NormalizeBasis::Last => {
                let last = values
                    .iter()
                    .rev()
                    .copied()
                    .find(|v| v.is_finite())
                    .unwrap_or(1.);
                Box::new(move |v| v / last)
            }
            NormalizeBasis::Min => Box::new(move |v| v / lo),
            NormalizeBasis::Max => Box::new(move |v| v / hi),
            NormalizeBasis::Mean => Box::new(move |v| v / mean),
            NormalizeBasis::Median => {
                let mut sorted = finite.clone();
                sorted.sort_by(f64::total_cmp);
                let m = sorted.len();
                let median = if m % 2 == 1 {
                    sorted[m / 2]
                } else {
                    (sorted[m / 2 - 1] + sorted[m / 2]) / 2.
                };
                Box::new(move |v| v / median)
            }
            NormalizeBasis::Sum => Box::new(move |v| v / sum),
            NormalizeBasis::Extent => {
                Box::new(move |v| if hi > lo { (v - lo) / (hi - lo) } else { 0. })
            }
            NormalizeBasis::Deviation => {
                let variance = finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                    / (finite.len().max(2) - 1) as f64;
                let deviation = variance.sqrt();
                Box::new(move |v| {
                    if deviation > 0. {
                        (v - mean) / deviation
                    } else {
                        0.
                    }
                })
            }
        };
        for &i in members {
            let v = read(&rows[i]);
            if !v.is_finite() {
                continue;
            }
            let normalized = Value::Number(map(v));
            let row = &mut rows[i];
            match (along_x, field) {
                (true, 0) => {
                    row.x = normalized;
                    row.x1 = None;
                    row.x2 = None;
                }
                (false, 0) => {
                    row.y = normalized;
                    row.y1 = None;
                    row.y2 = None;
                }
                (true, 1) => row.x1 = Some(normalized),
                (false, 1) => row.y1 = Some(normalized),
                (true, _) => row.x2 = Some(normalized),
                (false, _) => row.y2 = Some(normalized),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Table;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// The stack as it was before the series keys were precomputed: every
    /// key below is recomputed per comparison, and the groups and series are
    /// found by linear search.
    fn naive_stack(rows: &mut [PreparedRow], spec: &StackSpec) {
        let along_y = spec.axis == Axis::Y;
        let value_of = |row: &PreparedRow| -> f64 {
            // The value to stack: the channel itself, Plot's `y` for stackY. A
            // derived `[0, y]` interval stands for the same value.
            let value = if along_y { &row.y } else { &row.x };
            let value = if value.is_null() {
                if along_y {
                    row.y2.as_ref()
                } else {
                    row.x2.as_ref()
                }
                .unwrap_or(value)
            } else {
                value
            };
            value.as_number().unwrap_or(f64::NAN)
        };
        let index_of = |row: &PreparedRow| -> ValueKey {
            if along_y {
                row.x_value().key()
            } else {
                row.y_value().key()
            }
        };

        // The series order.
        let mut series: Vec<ValueKey> = Vec::new();
        for row in rows.iter() {
            let key = row
                .z
                .as_ref()
                .map(Value::key)
                .unwrap_or_else(|| Value::Null.key());
            if !series.contains(&key) {
                series.push(key);
            }
        }
        let total = |key: &ValueKey| -> f64 {
            rows.iter()
                .filter(|row| {
                    row.z
                        .as_ref()
                        .map(Value::key)
                        .unwrap_or_else(|| Value::Null.key())
                        == *key
                })
                .map(value_of)
                .filter(|v| v.is_finite())
                .sum()
        };
        match &spec.order {
            StackOrder::Input => {}
            StackOrder::Z => series.sort(),
            StackOrder::Value | StackOrder::Sum => {
                series.sort_by(|a, b| total(a).total_cmp(&total(b)));
            }
            StackOrder::Appearance | StackOrder::InsideOut => {
                // Where each series peaks, by its index's position in the data.
                let peak = |key: &ValueKey| -> usize {
                    let mut best = (f64::NEG_INFINITY, usize::MAX);
                    for (i, row) in rows.iter().enumerate() {
                        if row
                            .z
                            .as_ref()
                            .map(Value::key)
                            .unwrap_or_else(|| Value::Null.key())
                            == *key
                        {
                            let v = value_of(row);
                            if v > best.0 {
                                best = (v, i);
                            }
                        }
                    }
                    best.1
                };
                series.sort_by_key(peak);
                if spec.order == StackOrder::InsideOut {
                    // d3's insideOut: alternate the series onto the top and the
                    // bottom, the heaviest nearest the middle.
                    let (mut top, mut bottom) = (Vec::new(), Vec::new());
                    let (mut top_sum, mut bottom_sum) = (0., 0.);
                    for key in series.iter() {
                        if top_sum < bottom_sum {
                            top_sum += total(key);
                            top.push(key.clone());
                        } else {
                            bottom_sum += total(key);
                            bottom.push(key.clone());
                        }
                    }
                    bottom.reverse();
                    series = bottom.into_iter().chain(top).collect();
                }
            }
            StackOrder::Explicit(order) => {
                let rank = |key: &ValueKey| {
                    order
                        .iter()
                        .position(|v| v.key() == *key)
                        .unwrap_or(usize::MAX)
                };
                series.sort_by_key(rank);
            }
        }
        if spec.reverse {
            series.reverse();
        }
        let rank = |row: &PreparedRow| {
            let key = row
                .z
                .as_ref()
                .map(Value::key)
                .unwrap_or_else(|| Value::Null.key());
            series.iter().position(|k| *k == key).unwrap_or(0)
        };

        // Group the rows by their index, keeping each group in series order.
        let mut groups: Vec<(ValueKey, Vec<usize>)> = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let key = index_of(row);
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, members)) => members.push(i),
                None => groups.push((key, vec![i])),
            }
        }
        let mut intervals: Vec<(f64, f64)> = vec![(f64::NAN, f64::NAN); rows.len()];
        for (_, members) in groups.iter_mut() {
            members.sort_by_key(|i| rank(&rows[*i]));
            let (mut positive, mut negative) = (0.0_f64, 0.0_f64);
            for &i in members.iter() {
                let v = value_of(&rows[i]);
                if !v.is_finite() {
                    continue;
                }
                if v >= 0. {
                    intervals[i] = (positive, positive + v);
                    positive += v;
                } else {
                    intervals[i] = (negative, negative + v);
                    negative += v;
                }
            }
            match spec.offset {
                Offset::Zero | Offset::Custom(_) => {}
                Offset::Normalize => {
                    let span = positive - negative;
                    if span > 0. {
                        for &i in members.iter() {
                            let (a, b) = intervals[i];
                            intervals[i] = ((a - negative) / span, (b - negative) / span);
                        }
                    }
                }
                Offset::Center | Offset::Wiggle => {
                    let mid = (positive + negative) / 2.;
                    for &i in members.iter() {
                        let (a, b) = intervals[i];
                        intervals[i] = (a - mid, b - mid);
                    }
                }
            }
        }
        if spec.offset == Offset::Wiggle {
            naive_wiggle(rows, &groups, &mut intervals, &rank);
        }
        if let Offset::Custom(CustomOffset(offset)) = &spec.offset {
            let stacks: Vec<Vec<usize>> =
                groups.iter().map(|(_, members)| members.clone()).collect();
            let (mut x1, mut x2): (Vec<f64>, Vec<f64>) = intervals.iter().copied().unzip();
            let z: Vec<Value> = rows
                .iter()
                .map(|row| row.z.clone().unwrap_or(Value::Null))
                .collect();
            offset(&stacks, &mut x1, &mut x2, &z);
            intervals = x1.into_iter().zip(x2).collect();
        }

        for (row, (a, b)) in rows.iter_mut().zip(intervals) {
            if !a.is_finite() {
                continue;
            }
            let position = match spec.anchor {
                StackAnchor::Middle => (a + b) / 2.,
                StackAnchor::Start => a,
                StackAnchor::End => b,
            };
            if along_y {
                row.y1 = Some(Value::Number(a));
                row.y2 = Some(Value::Number(b));
                if !spec.keep_value {
                    row.y = Value::Number(position);
                }
            } else {
                row.x1 = Some(Value::Number(a));
                row.x2 = Some(Value::Number(b));
                if !spec.keep_value {
                    row.x = Value::Number(position);
                }
            }
        }
    }

    /// d3's `stackOffsetWiggle`, after centering: shift each stack so the
    /// weighted slope of its layers is as flat as can be, then lift the whole
    /// streamgraph so its lowest point sits at zero.
    fn naive_wiggle(
        rows: &[PreparedRow],
        groups: &[(ValueKey, Vec<usize>)],
        intervals: &mut [(f64, f64)],
        rank: &dyn Fn(&PreparedRow) -> usize,
    ) {
        if groups.len() < 2 {
            return;
        }
        // A layer's thickness at a group, by series rank.
        let thickness = |group: &[usize], series: usize, intervals: &[(f64, f64)]| -> f64 {
            group
                .iter()
                .filter(|i| rank(&rows[**i]) == series)
                .map(|i| (intervals[*i].1 - intervals[*i].0).abs())
                .sum()
        };
        let n = rows.iter().map(rank).max().unwrap_or(0) + 1;
        let mut y = 0.0_f64;
        // The first stack starts at zero.
        let base = |group: &[usize], intervals: &[(f64, f64)]| -> f64 {
            group
                .iter()
                .map(|i| intervals[*i].0.min(intervals[*i].1))
                .fold(f64::INFINITY, f64::min)
        };
        let shift = |group: &[usize], intervals: &mut [(f64, f64)], by: f64| {
            for &i in group {
                intervals[i].0 += by;
                intervals[i].1 += by;
            }
        };
        let first = base(&groups[0].1, intervals);
        shift(&groups[0].1, intervals, -first);
        for j in 1..groups.len() {
            let (previous, current) = (&groups[j - 1].1, &groups[j].1);
            let (mut s1, mut s2) = (0.0_f64, 0.0_f64);
            for i in 0..n {
                let si = thickness(current, i, intervals);
                let sij0 = thickness(previous, i, intervals);
                let mut s3 = (si - sij0) / 2.;
                for k in 0..i {
                    s3 += thickness(current, k, intervals) - thickness(previous, k, intervals);
                }
                s1 += si;
                s2 += s3 * si;
            }
            if s1 != 0. {
                y -= s2 / s1;
            }
            let current_base = base(current, intervals);
            shift(current, intervals, y - current_base);
        }
        let lowest = intervals
            .iter()
            .filter(|(a, _)| a.is_finite())
            .map(|(a, b)| a.min(*b))
            .fold(f64::INFINITY, f64::min);
        if lowest.is_finite() {
            for interval in intervals.iter_mut() {
                interval.0 -= lowest;
                interval.1 -= lowest;
            }
        }
    }

    fn row(z: Option<Value>, x: Value, y: Value) -> PreparedRow {
        PreparedRow {
            row: Table::new([("k", Value::Number(0.))]),
            index: 0,
            x,
            y,
            x1: None,
            x2: None,
            y1: None,
            y2: None,
            z,
            path: None,
            r: None,
            symbol: None,
            opacity: None,
            r_value: None,
            opacity_value: None,
            dodge_offset: None,
            pixel: None,
        }
    }

    /// Rows with awkward series keys (text, null, -0 and +0, a NaN, a
    /// boolean, a missing z), awkward index keys (text, -0 and +0, a null
    /// that falls back to the interval's start), and values that are
    /// negative, zero, -0, NaN, null (some falling back to the interval's
    /// end) and of very different magnitudes, so any change in the order
    /// of a float sum shows.
    fn awkward_rows(along_y: bool) -> Vec<PreparedRow> {
        let zs = [
            Some(Value::text("b")),
            None,
            Some(Value::text("a")),
            Some(Value::Number(-0.)),
            Some(Value::Number(0.)),
            Some(Value::Number(f64::NAN)),
            Some(Value::Bool(true)),
            Some(Value::Null),
            Some(Value::text("c")),
        ];
        let indexes = [
            Value::text("p"),
            Value::Number(-0.),
            Value::Number(0.),
            Value::text("q"),
            Value::Null,
            Value::Number(3.),
        ];
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        (0..160)
            .map(|i| {
                let r = next();
                let value = match r % 11 {
                    0 => Value::Null,
                    1 => Value::Number(f64::NAN),
                    2 => Value::Number(-0.),
                    3 => Value::Number(0.),
                    4 => Value::Number(-(((r >> 8) % 1000) as f64) * 0.1),
                    5 => Value::Number(1e16 + (r % 7) as f64),
                    6 => Value::Number(((r >> 8) % 997) as f64 / 7.),
                    _ => Value::Number(((r >> 8) % 100) as f64 * 0.3 + 0.1),
                };
                let index = indexes[(r >> 20) as usize % indexes.len()].clone();
                let z = zs[(i * 7 + (r >> 30) as usize) % zs.len()].clone();
                let mut row = if along_y {
                    row(z, index.clone(), value)
                } else {
                    row(z, value, index.clone())
                };
                // A null index reads the interval's start; a null value
                // stacks the interval's end.
                if index.is_null() {
                    let start = Some(Value::Number((r % 3) as f64));
                    if along_y {
                        row.x1 = start;
                    } else {
                        row.y1 = start;
                    }
                }
                if r % 13 == 0 {
                    let end = Some(Value::Number(((r >> 4) % 50) as f64 - 10.));
                    if along_y {
                        row.y = Value::Null;
                        row.y2 = end;
                    } else {
                        row.x = Value::Null;
                        row.x2 = end;
                    }
                }
                row
            })
            .collect()
    }

    /// A streamgraph's rows: many series over many positions, some
    /// positions missing a series, so wiggle weighs uneven stacks.
    fn stream_rows() -> Vec<PreparedRow> {
        let mut rows = Vec::new();
        for x in 0..40 {
            for s in 0..17 {
                if (x * 3 + s * 5) % 7 == 0 {
                    continue;
                }
                let v = ((x * 31 + s * 17) % 23) as f64 * 1.37 + (s as f64).sqrt();
                rows.push(row(
                    Some(Value::text(format!("s{}", (s * 5) % 17))),
                    Value::Number(x as f64),
                    Value::Number(v),
                ));
            }
        }
        rows
    }

    fn channels(rows: &[PreparedRow]) -> Vec<String> {
        rows.iter()
            .map(|r| format!("{:?}", (&r.x, &r.y, &r.x1, &r.x2, &r.y1, &r.y2)))
            .collect()
    }

    /// The stacks and z a custom offset was handed, and a shift it applies.
    fn recording_offset(log: Rc<RefCell<Vec<String>>>) -> Offset {
        Offset::custom(move |stacks, x1, x2, z| {
            log.borrow_mut().push(format!("{stacks:?} {z:?}"));
            for (n, stack) in stacks.iter().enumerate() {
                for &i in stack {
                    x1[i] -= n as f64 * 0.5;
                    x2[i] -= n as f64 * 0.5;
                }
            }
        })
    }

    #[test]
    fn stack_matches_the_naive_stack_for_every_order_and_offset() {
        let orders = [
            StackOrder::Input,
            StackOrder::Value,
            StackOrder::Sum,
            StackOrder::Appearance,
            StackOrder::InsideOut,
            StackOrder::Z,
            // Duplicates keep their first place; missing series sort last.
            StackOrder::Explicit(vec![
                Value::text("c"),
                Value::Number(0.),
                Value::text("a"),
                Value::text("c"),
                Value::Null,
                Value::text("absent"),
            ]),
            StackOrder::Explicit(vec![
                Value::text("s4"),
                Value::text("s9"),
                Value::text("s0"),
            ]),
        ];
        let mut cases = 0;
        for along_y in [true, false] {
            for data in [awkward_rows(along_y), stream_rows()] {
                if !along_y && data.len() == stream_rows().len() {
                    continue;
                }
                for order in &orders {
                    for offset_kind in 0..5 {
                        for reverse in [false, true] {
                            for anchor in
                                [StackAnchor::Middle, StackAnchor::Start, StackAnchor::End]
                            {
                                for keep_value in [false, true] {
                                    let (log_a, log_b) = Default::default();
                                    let offsets: [Offset; 2] = match offset_kind {
                                        0 => [Offset::Zero, Offset::Zero],
                                        1 => [Offset::Normalize, Offset::Normalize],
                                        2 => [Offset::Center, Offset::Center],
                                        3 => [Offset::Wiggle, Offset::Wiggle],
                                        _ => [
                                            recording_offset(Rc::clone(&log_a)),
                                            recording_offset(Rc::clone(&log_b)),
                                        ],
                                    };
                                    let [offset_a, offset_b] = offsets;
                                    let spec = |offset: Offset| StackSpec {
                                        axis: if along_y { Axis::Y } else { Axis::X },
                                        anchor,
                                        offset,
                                        order: order.clone(),
                                        reverse,
                                        keep_value,
                                    };
                                    let (mut fast, mut naive) = (data.clone(), data.clone());
                                    stack(&mut fast, &spec(offset_a));
                                    naive_stack(&mut naive, &spec(offset_b));
                                    assert_eq!(
                                        channels(&fast),
                                        channels(&naive),
                                        "{order:?} offset {offset_kind} reverse {reverse} y {along_y}"
                                    );
                                    assert_eq!(*log_a.borrow(), *log_b.borrow());
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(cases > 500);
    }

    #[test]
    fn empty_single_and_zero_total_stacks_match_the_naive_stack() {
        for data in [
            Vec::new(),
            vec![row(None, Value::Number(1.), Value::Number(-0.))],
            vec![
                row(None, Value::Number(1.), Value::Number(-0.)),
                row(None, Value::Number(1.), Value::Number(-0.)),
                row(None, Value::Number(2.), Value::Number(f64::NAN)),
            ],
            // Totals of +0 (`1 - 1`), -0 and nothing: `total_cmp` puts -0
            // (an empty float sum) below +0, and the -0 series' empty
            // interval lands below or above the +0 series' bar, so the zero
            // a total starts from shows.
            vec![
                row(Some(Value::text("c")), Value::Number(1.), Value::Number(1.)),
                row(
                    Some(Value::text("a")),
                    Value::Number(1.),
                    Value::Number(-0.),
                ),
                row(
                    Some(Value::text("b")),
                    Value::Number(1.),
                    Value::Number(f64::NAN),
                ),
                row(
                    Some(Value::text("c")),
                    Value::Number(2.),
                    Value::Number(-1.),
                ),
                row(Some(Value::text("d")), Value::Number(2.), Value::Number(1.)),
            ],
        ] {
            for order in [
                StackOrder::Value,
                StackOrder::InsideOut,
                StackOrder::Appearance,
            ] {
                for offset in [Offset::Zero, Offset::Wiggle, Offset::Normalize] {
                    let spec = StackSpec {
                        order: order.clone(),
                        offset,
                        ..StackSpec::new(Axis::Y)
                    };
                    let (mut fast, mut naive) = (data.clone(), data.clone());
                    stack(&mut fast, &spec);
                    naive_stack(&mut naive, &spec);
                    assert_eq!(channels(&fast), channels(&naive));
                }
            }
        }
    }
}
