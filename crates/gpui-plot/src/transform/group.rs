//! The group transform, Plot's `Plot.groupX` and `Plot.groupY`.

use crate::table::{Dataset, Table};
use crate::transform::reduce::Reducer;
use crate::value::{Value, ValueKey};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::tests::rows;

    #[test]
    fn group_reduces_per_category() {
        let grouped = group(&rows(), "group", "v", Reducer::Sum, "sum");
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.rows()[0].value("sum"), Value::Number(4.));
        assert_eq!(grouped.rows()[1].value("sum"), Value::Number(5.));

        let counted = group(&rows(), "group", "v", Reducer::Count, "count");
        assert_eq!(counted.rows()[0].value("count"), Value::Number(2.));
    }
}
