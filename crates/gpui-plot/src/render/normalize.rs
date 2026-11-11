//! A mark's `normalize`: its y values rescaled to the unit interval.

use crate::render::PreparedRow;
use crate::value::Value;

pub(super) fn normalize_rows(rows: &mut [PreparedRow]) {
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
