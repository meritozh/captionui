//! A mark's `sort` over its resolved rows.

use crate::mark::Mark;
use crate::render::PreparedRow;
use crate::value::Value;

pub(super) fn apply_sort(rows: &mut Vec<PreparedRow>, mark: &Mark) {
    let Some(sort) = &mark.sort else {
        return;
    };
    // Each row's key is read once, then a stable sort of the indices by key
    // orders the rows as a stable sort of the rows would.
    let keys: Vec<Value> = if let Some(field) = &sort.field {
        rows.iter().map(|row| row.row.value(field)).collect()
    } else {
        // Sorting without a field sorts by the mark's own value: a bar chart's
        // length, which is Plot's default for a sorted bar chart.
        // The value is on x when the categories are on y (a `barX`), else y.
        let value_on_x = mark.kind.category().band_y;
        rows.iter()
            .map(|row| {
                if value_on_x {
                    row.x_value().clone()
                } else {
                    row.y_value().clone()
                }
            })
            .collect()
    };
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| {
        let ordering = keys[a].cmp(&keys[b]);
        if sort.descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    let mut taken: Vec<Option<PreparedRow>> = std::mem::take(rows).into_iter().map(Some).collect();
    rows.extend(order.into_iter().filter_map(|index| taken[index].take()));
    if sort.reverse {
        rows.reverse();
    }
}
