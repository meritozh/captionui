//! A mark's `reduce`: the rows sharing a category reduced to one.

use crate::mark::{Mark, MarkKind};
use crate::render::PreparedRow;
use crate::transform::Reducer;
use crate::value::Value;

pub(super) fn reduce_rows(
    rows: Vec<PreparedRow>,
    mark: &Mark,
    reducer: Reducer,
) -> Vec<PreparedRow> {
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
