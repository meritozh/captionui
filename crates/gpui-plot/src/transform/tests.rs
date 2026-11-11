//! Tests spanning several transforms, and the rows fixture they share.

use super::*;
use crate::table::{Dataset, Table};
use crate::value::Value;

pub(super) fn rows() -> Dataset {
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
