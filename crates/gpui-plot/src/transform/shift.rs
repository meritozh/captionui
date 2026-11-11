//! The shift transform, Plot's `shiftX` and `shiftY`.

use crate::table::Dataset;
use crate::value::Value;

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
