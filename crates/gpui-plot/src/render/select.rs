//! Plot's `select` over a mark's resolved rows, and a box's outliers.

use crate::mark::Axis;
use crate::render::PreparedRow;
use crate::render::resolve::GroupRow;
use crate::transform::{self, Selector};
use crate::value::Value;

/// Keep each `z` group's rows beyond its whiskers, Plot's box outliers.
pub(super) fn outlier_rows(mut rows: Vec<PreparedRow>, axis: Axis) -> Vec<PreparedRow> {
    // Kept in place, so only the outliers move; `retain` visits the rows in
    // order, as the flags list them.
    let mut keep = outlier_flags(rows.iter(), axis).into_iter();
    rows.retain(|_| keep.next().expect("a flag per row"));
    rows
}

/// Which of `rows` lie beyond their `z` group's whiskers, in row order.
pub(super) fn outlier_flags<R: GroupRow>(rows: impl Iterator<Item = R>, axis: Axis) -> Vec<bool> {
    let read = |row: &R| -> Value {
        if axis == Axis::X {
            GroupRow::x_value(row).clone()
        } else {
            GroupRow::y_value(row).clone()
        }
    };
    // Each `z` group's first row's `z` and values, in row order, and each
    // row's group and number. A row whose `z` equals the `z` of the previous
    // row's group is in that group, since equal values have equal keys; any
    // other asks the map.
    let mut groups: Vec<(Option<Value>, Vec<Value>)> = Vec::new();
    let mut slots: rustc_hash::FxHashMap<crate::value::ValueKey, usize> = Default::default();
    let mut members: Vec<(usize, Option<f64>)> = Vec::with_capacity(rows.size_hint().0);
    let mut last: Option<usize> = None;
    for row in rows {
        let group = match last {
            Some(group) if groups[group].0.as_ref() == row.z() => group,
            _ => {
                let key = row.z().map(Value::key).unwrap_or_else(|| Value::Null.key());
                *slots.entry(key).or_insert_with(|| {
                    groups.push((row.z().cloned(), Vec::new()));
                    groups.len() - 1
                })
            }
        };
        last = Some(group);
        let value = read(&row);
        members.push((group, value.as_number()));
        groups[group].1.push(value);
    }
    let bounds: Vec<(f64, f64)> = groups
        .into_iter()
        .map(|(_, values)| {
            let mut order = transform::OrderStatistics::default();
            let lo = crate::mark::whisker_low_with(&values, &mut order)
                .as_number()
                .unwrap_or(f64::NEG_INFINITY);
            let hi = crate::mark::whisker_high_with(&values, &mut order)
                .as_number()
                .unwrap_or(f64::INFINITY);
            (lo, hi)
        })
        .collect();
    members
        .into_iter()
        .map(|(group, value)| {
            let (lo, hi) = bounds[group];
            value.is_some_and(|v| v < lo || v > hi)
        })
        .collect()
}

/// Keep the row of each group with the least or greatest value on the
/// selector's axis, Plot's `selectMinX` and its siblings.
pub(super) fn select_rows(
    rows: Vec<PreparedRow>,
    config: &crate::mark::SelectConfig,
) -> Vec<PreparedRow> {
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
