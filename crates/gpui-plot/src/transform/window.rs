//! The window transform, Plot's `Plot.windowY`: moving reductions over a series.

use crate::table::Dataset;
use crate::value::Value;

/// A window transform's reducer, Plot's `Plot.windowY` with a reducer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowReducer {
    /// A moving mean.
    #[default]
    Mean,
    /// A moving sum.
    Sum,
    /// A moving median.
    Median,
    /// The smallest value in the window.
    Min,
    /// The largest value in the window.
    Max,
    /// The cumulative mean.
    CumulativeMean,
    /// The difference between the current and previous value, Plot's `difference`.
    Difference,
    /// The value's change from the first value, Plot's `change`.
    Change,
    /// The percent change from the previous value.
    PercentChange,
    /// The sample standard deviation, Plot's `"deviation"`.
    Deviation,
    /// The sample variance, Plot's `"variance"`.
    Variance,
    /// The most common value, Plot's `"mode"`.
    Mode,
    /// The last value over the first, Plot's `"ratio"`.
    Ratio,
    /// The window's first value, Plot's `"first"`.
    First,
    /// The window's last value, Plot's `"last"`.
    Last,
    /// The window's `p`th percentile, Plot's `"p25"`, `"p75"` and so on.
    Percentile(u8),
}

/// A window transform over a numeric field, Plot's `Plot.windowY`.
///
/// `k` is the window width; `None` centers the window, and `anchor` decides
/// where a window sits when it cannot be centered (`center`, `start`, `end`).
pub fn window(
    data: &Dataset,
    field: &str,
    k: usize,
    reducer: WindowReducer,
    anchor: Anchor,
) -> Dataset {
    let numbers: Vec<Option<f64>> = data.values(field).iter().map(Value::as_number).collect();
    let output = window_values(&numbers, k, reducer, anchor, false);
    Dataset::new(
        data.rows()
            .iter()
            .cloned()
            .zip(output)
            .map(|(row, value)| row.with(field, value.map(Value::Number).unwrap_or(Value::Null))),
    )
}

/// Plot's window `mapIndex` over one series: each value's window of `k`
/// starts `s` before it — `(k - 1) >> 1` for `middle`, 0 for `start`,
/// `k - 1` for `end` — and is truncated at the series' ends unless
/// `strict`, when a short window, or one holding a missing value, gives
/// none.
pub fn window_values(
    values: &[Option<f64>],
    k: usize,
    reducer: WindowReducer,
    anchor: Anchor,
    strict: bool,
) -> Vec<Option<f64>> {
    let n = values.len();
    let k = k.max(1);
    let s = match anchor {
        Anchor::Middle => (k - 1) >> 1,
        Anchor::Start => 0,
        Anchor::End => k - 1,
    };
    let mut output: Vec<Option<f64>> = vec![None; n];
    // d3's reducers skip missing values: Plot's `firstNumber`/`lastNumber`.
    let first_number =
        |slice: &[Option<f64>]| slice.iter().flatten().copied().find(|v| !v.is_nan());
    let last_number =
        |slice: &[Option<f64>]| slice.iter().rev().flatten().copied().find(|v| !v.is_nan());
    let reduce = |slice: &[Option<f64>]| -> Option<f64> {
        let defined: Vec<f64> = slice.iter().filter_map(|v| *v).filter(|v| !v.is_nan()).collect();
        match reducer {
            WindowReducer::Mean => {
                (!defined.is_empty()).then(|| defined.iter().sum::<f64>() / defined.len() as f64)
            }
            // A non-strict sum counts a missing value as zero.
            WindowReducer::Sum => Some(defined.iter().sum::<f64>()),
            WindowReducer::Median => d3_quantile(&defined, 0.5),
            WindowReducer::Percentile(p) => d3_quantile(&defined, f64::from(p) / 100.),
            WindowReducer::Min => defined.iter().copied().reduce(f64::min),
            WindowReducer::Max => defined.iter().copied().reduce(f64::max),
            WindowReducer::Variance => d3_variance(&defined),
            WindowReducer::Deviation => d3_variance(&defined).map(f64::sqrt),
            // d3's `mode`: the most frequent value, the first seen on a tie.
            WindowReducer::Mode => {
                let mut counts: Vec<(f64, usize)> = Vec::new();
                for v in &defined {
                    match counts.iter_mut().find(|(value, _)| value == v) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((*v, 1)),
                    }
                }
                let mut best: Option<(f64, usize)> = None;
                for (value, count) in counts {
                    if best.is_none_or(|(_, most)| count > most) {
                        best = Some((value, count));
                    }
                }
                best.map(|(value, _)| value)
            }
            WindowReducer::Difference => Some(last_number(slice)? - first_number(slice)?),
            WindowReducer::Ratio => Some(last_number(slice)? / first_number(slice)?),
            WindowReducer::First => first_number(slice),
            WindowReducer::Last => last_number(slice),
            _ => None,
        }
    };
    match reducer {
        // Not Plot window reducers: running reductions over the series.
        WindowReducer::CumulativeMean => {
            let (mut sum, mut count) = (0., 0usize);
            for (index, value) in values.iter().enumerate() {
                if let Some(v) = value {
                    sum += v;
                    count += 1;
                }
                output[index] = (count > 0).then(|| sum / count as f64);
            }
        }
        WindowReducer::Change => {
            let first = values.iter().find_map(|v| *v);
            for (index, value) in values.iter().enumerate() {
                output[index] = match (value, first) {
                    (Some(v), Some(f)) => Some(v - f),
                    _ => None,
                };
            }
        }
        WindowReducer::PercentChange => {
            for index in 1..n {
                output[index] = match (values[index], values[index - 1]) {
                    (Some(v), Some(p)) if p != 0. => Some((v - p) / p),
                    _ => None,
                };
            }
        }
        // Plot's strict `difference`, `ratio`, `first` and `last` read the
        // window's two ends, over the windows that fit, short one.
        WindowReducer::Difference
        | WindowReducer::Ratio
        | WindowReducer::First
        | WindowReducer::Last
            if strict =>
        {
            for i in 0..n.saturating_sub(k) {
                let (a, b) = (values[i], values[i + k - 1]);
                output[i + s] = match reducer {
                    WindowReducer::First => a,
                    WindowReducer::Last => b,
                    _ => match (a, b) {
                        (Some(a), Some(b)) if reducer == WindowReducer::Ratio => Some(b / a),
                        (Some(a), Some(b)) => Some(b - a),
                        _ => None,
                    },
                };
            }
        }
        // Plot's non-strict `difference` (and its kin) stops where the
        // window would start past `n - k`. Its loop also reaches windows
        // whose point `i + s` lies past the series' end (an `end` anchor's
        // last `k - 1`), where Plot's write lands on no index; those are
        // skipped here.
        WindowReducer::Difference
        | WindowReducer::Ratio
        | WindowReducer::First
        | WindowReducer::Last => {
            let end = (n as isize - k as isize + s as isize + 1).min(n as isize - s as isize);
            for i in -(s as isize)..end {
                let lo = i.max(0) as usize;
                let hi = ((i + k as isize).max(0) as usize).min(n);
                output[(i + s as isize) as usize] =
                    reduce(if lo < hi { &values[lo..hi] } else { &[] });
            }
        }
        _ => output = window_map(values, k, anchor, strict, reduce),
    }
    output
}

/// Plot's window over one series with a reducer of the window's values (its
/// `reduceArray`): each value's window of `k` starts `s` before it, as in
/// [`window_values`], and is truncated at the series' ends unless `strict`,
/// when only full windows of defined values give one.
pub fn window_map(
    values: &[Option<f64>],
    k: usize,
    anchor: Anchor,
    strict: bool,
    reduce: impl Fn(&[Option<f64>]) -> Option<f64>,
) -> Vec<Option<f64>> {
    let n = values.len();
    let k = k.max(1);
    let s = match anchor {
        Anchor::Middle => (k - 1) >> 1,
        Anchor::Start => 0,
        Anchor::End => k - 1,
    };
    let mut output: Vec<Option<f64>> = vec![None; n];
    if strict {
        for i in 0..(n + 1).saturating_sub(k) {
            let slice = &values[i..i + k];
            if slice.iter().all(|v| v.is_some_and(|v| !v.is_nan())) {
                output[i + s] = reduce(slice);
            }
        }
    } else {
        for (p, out) in output.iter_mut().enumerate() {
            let lo = p.saturating_sub(s);
            let hi = (p + k - s).min(n);
            *out = reduce(&values[lo..hi]);
        }
    }
    output
}

/// d3's `quantile` over defined values: the R-7 interpolation of the
/// sorted values.
fn d3_quantile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if p <= 0. || n < 2 {
        return Some(sorted[0]);
    }
    if p >= 1. {
        return Some(sorted[n - 1]);
    }
    let i = (n - 1) as f64 * p;
    let i0 = i.floor() as usize;
    Some(sorted[i0] + (sorted[i0 + 1] - sorted[i0]) * (i - i0 as f64))
}

/// d3's `variance`: the sample variance, missing for fewer than two values.
pub(super) fn d3_variance(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    Some(values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64)
}

/// Where a window sits when it cannot be centered on its point, Plot's
/// `anchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// Keep the point in the middle of the window. (Default.)
    #[default]
    Middle,
    /// The window starts at the point.
    Start,
    /// The window ends at the point.
    End,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Table;

    #[test]
    fn window_mean_is_centered() {
        let data = Dataset::new((0..5).map(|i| Table::empty().with("v", Value::Number(i as f64))));
        let smoothed = window(&data, "v", 3, WindowReducer::Mean, Anchor::Middle);
        let values: Vec<f64> = smoothed
            .values("v")
            .iter()
            .filter_map(Value::as_number)
            .collect();
        assert_eq!(values, vec![0.5, 1., 2., 3., 3.5]);
    }

    #[test]
    fn window_reducers_match_plot() {
        let values: Vec<Option<f64>> = [1., 2., 4., 8.].map(Some).to_vec();
        let run = |reducer| window_values(&values, 2, reducer, Anchor::Middle, false);
        // Plot's non-strict difference and ratio stop one short of the end.
        assert_eq!(
            run(WindowReducer::Difference),
            vec![Some(1.), Some(2.), Some(4.), None]
        );
        assert_eq!(
            run(WindowReducer::Ratio),
            vec![Some(2.), Some(2.), Some(2.), None]
        );
        assert_eq!(
            run(WindowReducer::Last),
            vec![Some(2.), Some(4.), Some(8.), None]
        );
        assert_eq!(
            run(WindowReducer::Percentile(50)),
            vec![Some(1.5), Some(3.), Some(6.), Some(8.)]
        );
        assert_eq!(
            run(WindowReducer::Variance),
            vec![Some(0.5), Some(2.), Some(8.), None]
        );
    }

    /// An `end` anchor's non-strict difference (and kin) reads each window
    /// ending at its point, truncated at the start; Plot's loop also visits
    /// windows whose point lies past the series, which write nothing. Every
    /// length, width and anchor runs without reading or writing out of range.
    #[test]
    fn end_anchored_differences_stop_at_the_series_end() {
        let values: Vec<Option<f64>> = [1., 2., 4., 8., 16.].map(Some).to_vec();
        let end = |reducer| window_values(&values, 3, reducer, Anchor::End, false);
        assert_eq!(
            end(WindowReducer::Difference),
            vec![Some(0.), Some(1.), Some(3.), Some(6.), Some(12.)]
        );
        assert_eq!(
            end(WindowReducer::Ratio),
            vec![Some(1.), Some(2.), Some(4.), Some(4.), Some(4.)]
        );
        assert_eq!(
            end(WindowReducer::First),
            vec![Some(1.), Some(1.), Some(1.), Some(2.), Some(4.)]
        );
        assert_eq!(end(WindowReducer::Last), values);
        let reducers = [
            WindowReducer::Difference,
            WindowReducer::Ratio,
            WindowReducer::First,
            WindowReducer::Last,
        ];
        for n in 0..7 {
            let values: Vec<Option<f64>> = (0..n).map(|i| Some(i as f64 + 1.)).collect();
            for k in 0..6 {
                for anchor in [Anchor::Start, Anchor::Middle, Anchor::End] {
                    for strict in [false, true] {
                        for reducer in reducers {
                            let out = window_values(&values, k, reducer, anchor, strict);
                            assert_eq!(out.len(), n, "{n} {k} {anchor:?} {strict}");
                        }
                    }
                }
            }
        }
    }
}
