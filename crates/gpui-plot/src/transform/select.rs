//! The select transform, Plot's `Plot.select`: one row per group.

use gpui::SharedString;

use crate::table::{Dataset, Table};
use crate::value::{Value, ValueKey};

/// Which row of a group a [`Select`] keeps, Plot's `select` options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    /// The first row, Plot's `"first"`.
    First,
    /// The last row, Plot's `"last"`.
    Last,
    /// The row with the smallest value of a field, Plot's `{field: "min"}`.
    Min(SharedString),
    /// The row with the largest value of a field, Plot's `{field: "max"}`.
    Max(SharedString),
}

/// A [`select`] configuration: which row to keep and how to group.
#[derive(Clone, Debug)]
pub struct Select {
    /// Which row of each group survives.
    pub selector: Selector,
    /// The field the groups are formed on, when the data is long; Plot's `z`.
    pub group: Option<SharedString>,
}

/// Keep one row per group, Plot's `selectFirst`, `selectLast` and
/// `selectMinX`/`selectMaxX` (and their y variants).
///
/// Every other row is dropped, which is what lets a mark draw a single
/// labelled point per series — the extreme reading of a line, the first
/// sample of a run — rather than every row the source data holds.
pub fn select(data: &Dataset, config: &Select) -> Dataset {
    // Without a group the whole dataset is one group, which is what makes
    // `selectMinX` pick the single extreme of a series.
    let mut groups: Vec<(ValueKey, Vec<&Table>)> = Vec::new();
    for row in data.rows() {
        let key = config
            .group
            .as_ref()
            .map(|field| row.value(field).key())
            .unwrap_or_else(|| Value::Null.key());
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }

    let kept: Vec<Table> = groups
        .into_iter()
        .filter_map(|(_, members)| {
            let chosen = match &config.selector {
                Selector::First => members.first(),
                Selector::Last => members.last(),
                // `min_by` keeps the first of equal values, which is Plot's
                // `least`: a tie goes to the earlier datum.
                Selector::Min(field) => members
                    .iter()
                    .filter(|row| row.number(field).is_some())
                    .min_by(|a, b| {
                        a.number(field)
                            .unwrap_or(f64::INFINITY)
                            .total_cmp(&b.number(field).unwrap_or(f64::INFINITY))
                    }),
                // `max_by` would keep the *last* of equal values; comparing
                // reversed under `min_by` keeps the first, Plot's `greatest`.
                Selector::Max(field) => members
                    .iter()
                    .filter(|row| row.number(field).is_some())
                    .min_by(|a, b| {
                        b.number(field)
                            .unwrap_or(f64::NEG_INFINITY)
                            .total_cmp(&a.number(field).unwrap_or(f64::NEG_INFINITY))
                    }),
            }?;
            Some((*chosen).clone())
        })
        .collect();
    Dataset::new(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_keeps_the_extreme_of_each_group() {
        let data = Dataset::new([
            Table::empty()
                .with("g", Value::text("a"))
                .with("v", Value::Number(3.)),
            Table::empty()
                .with("g", Value::text("a"))
                .with("v", Value::Number(9.)),
            Table::empty()
                .with("g", Value::text("b"))
                .with("v", Value::Number(1.)),
            Table::empty()
                .with("g", Value::text("b"))
                .with("v", Value::Number(4.)),
        ]);
        let selected = select(
            &data,
            &Select {
                selector: Selector::Max("v".into()),
                group: Some("g".into()),
            },
        );
        let values: Vec<f64> = selected
            .values("v")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(values, vec![9., 4.]);
        // Without a group the whole series is one group.
        let single = select(
            &data,
            &Select {
                selector: Selector::First,
                group: None,
            },
        );
        assert_eq!(single.len(), 1);
    }
}
