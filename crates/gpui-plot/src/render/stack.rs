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
        wiggle(rows, &groups, &mut intervals, &rank);
    }
    if let Offset::Custom(CustomOffset(offset)) = &spec.offset {
        let stacks: Vec<Vec<usize>> = groups.iter().map(|(_, members)| members.clone()).collect();
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
fn wiggle(
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
    let mut series: Vec<(ValueKey, Vec<usize>)> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let key = row
            .z
            .as_ref()
            .map(Value::key)
            .unwrap_or_else(|| Value::Null.key());
        match series.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(i),
            None => series.push((key, vec![i])),
        }
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
    series: &[(ValueKey, Vec<usize>)],
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
    for (_, members) in series {
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
