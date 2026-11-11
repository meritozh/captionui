//! Plot's basic transforms over the rows: sort, filter, reverse and shuffle.

use crate::table::{Dataset, Table};
use crate::value::Value;

/// Sort rows by a field, Plot's `Plot.sort`.
pub fn sort_by(data: &Dataset, field: &str, descending: bool) -> Dataset {
    let mut rows: Vec<Table> = data.rows().to_vec();
    rows.sort_by(|a, b| {
        let ordering = a.value(field).cmp(&b.value(field));
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    Dataset::new(rows)
}

/// Keep rows whose `field` satisfies `predicate`, Plot's `Plot.filter`.
pub fn filter(data: &Dataset, field: &str, predicate: impl Fn(&Value) -> bool) -> Dataset {
    Dataset::new(
        data.rows()
            .iter()
            .filter(|row| predicate(&row.value(field)))
            .cloned(),
    )
}

/// Reverse row order, Plot's `Plot.reverse`.
pub fn reverse(data: &Dataset) -> Dataset {
    Dataset::new(data.rows().iter().rev().cloned())
}

/// Shuffle the rows with a fixed seed, Plot's `shuffle`.
///
/// The seed makes the order reproducible: a shuffled chart looks the same on
/// every run, which a random order would not.
pub fn shuffle(data: &Dataset, seed: u64) -> Dataset {
    let mut rows: Vec<Table> = data.rows().to_vec();
    let mut state = seed | 1;
    for index in (1..rows.len()).rev() {
        // xorshift64, so the order is deterministic without a dependency.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        rows.swap(index, (state % (index as u64 + 1)) as usize);
    }
    Dataset::new(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::tests::rows;

    #[test]
    fn sort_and_filter() {
        let sorted = sort_by(&rows(), "v", true);
        assert_eq!(sorted.rows()[0].value("v"), Value::Number(5.));
        let filtered = filter(&rows(), "v", |v| v.as_number().is_some_and(|n| n > 2.));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn shuffle_is_deterministic_for_a_seed() {
        let data = Dataset::new(
            (0..20).map(|index| Table::empty().with("i", Value::Number(index as f64))),
        );
        let once = shuffle(&data, 7);
        let twice = shuffle(&data, 7);
        assert_eq!(once.values("i"), twice.values("i"));
        let order: Vec<f64> = once
            .values("i")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(order.len(), 20);
        assert_ne!(order, (0..20).map(|i| i as f64).collect::<Vec<_>>());
    }
}
