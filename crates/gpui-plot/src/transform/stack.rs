//! The stack transform, Plot's `Plot.stackY`, over wide and long data.

use std::collections::BTreeMap;

use gpui::SharedString;

use crate::scale::dedup;
use crate::table::Dataset;
use crate::value::{Value, ValueKey};

/// How stacked series are accumulated, Plot's `offset`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackOffset {
    /// Each series sits on the sum of the ones before it, from zero; d3's
    /// `stackOffsetNone`.
    #[default]
    Zero,
    /// Each series is normalized so the total is 1 at every x; d3's
    /// `stackOffsetExpand`, which Plot uses for percentage charts.
    Normalize,
    /// Series are centered on the baseline, showing a divergence around zero;
    /// d3's `stackOffsetSilhouette`.
    Center,
    /// Series overlap instead of stacking, Plot's `stack: false`.
    None,
}

/// One stacked series.
#[derive(Clone, Debug)]
pub struct StackedSeries {
    /// The series' key, in the order it appears in the data.
    pub key: SharedString,
    /// One point per x category: its lower and upper value.
    pub points: Vec<StackedPoint>,
}

/// A stacked point: the x category and the series' extent at it.
#[derive(Clone, Debug)]
pub struct StackedPoint {
    pub x: Value,
    pub y0: f64,
    pub y1: f64,
}

/// Stack a wide table: `x` names the category field and `keys` the fields that
/// become series, each field's value stacking at that x.
///
/// This is Plot's `Plot.stackY` over a wide input, where each numeric column is
/// a series.
pub fn stack_wide(
    data: &Dataset,
    x: &str,
    keys: &[&str],
    offset: StackOffset,
) -> Vec<StackedSeries> {
    let categories: Vec<Value> = dedup(data.rows().iter().map(|row| row.value(x).key()))
        .into_iter()
        .map(|key| key.to_value())
        .collect();

    let mut series: Vec<StackedSeries> = keys
        .iter()
        .map(|key| StackedSeries {
            key: (*key).into(),
            points: Vec::new(),
        })
        .collect();

    // A value per (series, category), summing rows that share both.
    let mut grid: Vec<Vec<f64>> = vec![vec![0.; categories.len()]; keys.len()];
    for row in data.rows() {
        let x_value = row.value(x);
        let Some(category) = categories.iter().position(|c| c.key() == x_value.key()) else {
            continue;
        };
        for (series_index, key) in keys.iter().enumerate() {
            if let Some(value) = row.number(key) {
                grid[series_index][category] += value;
            }
        }
    }

    stack_grid(&mut series, &categories, &grid, offset);
    series
}

/// Stack a `series × category` grid of values into `series`' points, the one
/// stacking loop both [`stack_wide`] and [`stack_long`] share.
///
/// Like d3's `stackOffsetDiverging`, positive values stack up from zero and
/// negative ones down from it, so a negative segment never overlaps a
/// positive one. `Normalize` then maps each category's extent onto `[0, 1]`
/// (d3's `expand`) and `Center` centres it on zero (Plot's `center`).
fn stack_grid(
    series: &mut [StackedSeries],
    categories: &[Value],
    grid: &[Vec<f64>],
    offset: StackOffset,
) {
    let mut above = vec![0.; categories.len()];
    let mut below = vec![0.; categories.len()];
    let mut spans: Vec<Vec<(f64, f64)>> = Vec::with_capacity(series.len());
    for values in grid {
        let mut row = Vec::with_capacity(categories.len());
        for (category, &value) in values.iter().enumerate() {
            let value = if value.is_finite() { value } else { 0. };
            let span = match offset {
                StackOffset::None => (0., value),
                _ if value < 0. => {
                    let base = below[category];
                    below[category] += value;
                    (base, below[category])
                }
                _ => {
                    let base = above[category];
                    above[category] += value;
                    (base, above[category])
                }
            };
            row.push(span);
        }
        spans.push(row);
    }
    for (stacked, row) in series.iter_mut().zip(spans) {
        for (category, (category_value, (y0, y1))) in categories.iter().zip(row).enumerate() {
            let (lo, hi) = (below[category], above[category]);
            let (y0, y1) = match offset {
                StackOffset::Normalize => {
                    let extent = hi - lo;
                    if extent == 0. {
                        (0., 0.)
                    } else {
                        ((y0 - lo) / extent, (y1 - lo) / extent)
                    }
                }
                StackOffset::Center => {
                    let middle = (lo + hi) / 2.;
                    (y0 - middle, y1 - middle)
                }
                StackOffset::Zero | StackOffset::None => (y0, y1),
            };
            stacked.points.push(StackedPoint {
                x: category_value.clone(),
                y0,
                y1,
            });
        }
    }
}

/// Long-to-wide: Plot's `Plot.stackY` over long data, where one field names the
/// series and another holds its value.
pub fn stack_long(
    data: &Dataset,
    x: &str,
    series_field: &str,
    value_field: &str,
    offset: StackOffset,
) -> Vec<StackedSeries> {
    let mut series_keys: Vec<ValueKey> = Vec::new();
    let mut series_labels: Vec<SharedString> = Vec::new();
    let mut categories: Vec<ValueKey> = Vec::new();

    let mut grid: BTreeMap<(ValueKey, ValueKey), f64> = BTreeMap::new();
    for row in data.rows() {
        let x_value = row.value(x);
        let series_value = row.value(series_field);
        let Some(value) = row.number(value_field) else {
            continue;
        };
        if !categories.contains(&x_value.key()) {
            categories.push(x_value.key());
        }
        let series_key = series_value.key();
        if !series_keys.contains(&series_key) {
            series_keys.push(series_key.clone());
            series_labels.push(series_value.label());
        }
        *grid.entry((series_key, x_value.key())).or_insert(0.) += value;
    }

    let categories: Vec<Value> = categories.into_iter().map(|key| key.to_value()).collect();

    let mut series: Vec<StackedSeries> = series_labels
        .into_iter()
        .map(|key| StackedSeries {
            key,
            points: Vec::new(),
        })
        .collect();

    let grid: Vec<Vec<f64>> = series_keys
        .iter()
        .map(|key| {
            categories
                .iter()
                .map(|category| {
                    grid.get(&(key.clone(), category.key()))
                        .copied()
                        .unwrap_or(0.)
                })
                .collect()
        })
        .collect();

    stack_grid(&mut series, &categories, &grid, offset);
    series
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Table;

    #[test]
    fn stack_normalizes_to_one() {
        let data = Dataset::new([Table::empty()
            .with("x", Value::text("a"))
            .with("s1", Value::Number(1.))
            .with("s2", Value::Number(3.))]);
        let series = stack_wide(&data, "x", &["s1", "s2"], StackOffset::Normalize);
        assert_eq!(series.len(), 2);
        assert!((series[1].points[0].y1 - 1.).abs() < 1e-9);
        assert!((series[0].points[0].y1 - 0.25).abs() < 1e-9);
    }
}
