//! The normalize transform, Plot's `Plot.normalizeY`.

use crate::table::Dataset;
use crate::value::Value;

/// Normalize a numeric field to `0..=1` across the dataset, Plot's
/// `Plot.normalizeY` with `percent` off.
pub fn normalize(data: &Dataset, field: &str) -> Dataset {
    let extent = crate::scale::extent(data.values(field));
    let Some([lo, hi]) = extent else {
        return data.clone();
    };
    let span = hi - lo;
    Dataset::new(data.rows().iter().cloned().map(|row| {
        let normalized = row
            .number(field)
            .map(|value| if span == 0. { 0.5 } else { (value - lo) / span });
        row.with(field, normalized.map(Value::Number).unwrap_or(Value::Null))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::tests::rows;

    #[test]
    fn normalize_spans_zero_to_one() {
        let normalized = normalize(&rows(), "v");
        let values: Vec<f64> = normalized
            .values("v")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(values, vec![0., 0.5, 1.]);
    }
}
