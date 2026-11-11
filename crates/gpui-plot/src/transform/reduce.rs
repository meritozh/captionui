//! Reducers, Plot's `reduce` options, and the order statistics they read.

use crate::table::Table;
use crate::value::{Value, ValueKey};

/// Aggregate a field across rows sharing a group, Plot's `groupY` and
/// `groupX` with a reducer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reducer {
    /// The number of rows.
    #[default]
    Count,
    /// The sum of the values.
    Sum,
    /// The arithmetic mean, ignoring nulls.
    Mean,
    /// The smallest value.
    Min,
    /// The largest value.
    Max,
    /// The first value, ignoring nulls.
    First,
    /// The last value, ignoring nulls.
    Last,
    /// The median of the values.
    Median,
    /// The standard deviation of the values.
    Deviation,
    /// The sum of squared values, for a root-mean-square.
    SumOfSquares,
}

impl Reducer {
    /// Reduce `values` to one value.
    pub fn reduce(self, values: &[Value]) -> Value {
        // Only the numeric reducers read the values as numbers.
        let numbers = || -> Vec<f64> { values.iter().filter_map(Value::as_number).collect() };
        match self {
            Reducer::Count => Value::Number(values.len() as f64),
            Reducer::Sum => Value::Number(values.iter().filter_map(Value::as_number).sum()),
            Reducer::Mean => {
                let numbers = numbers();
                if numbers.is_empty() {
                    Value::Null
                } else {
                    Value::Number(numbers.iter().sum::<f64>() / numbers.len() as f64)
                }
            }
            // The least or greatest value itself, so dates stay dates.
            Reducer::Min => values
                .iter()
                .filter(|v| v.as_number().is_some_and(|n| !n.is_nan()))
                .min_by(|a, b| {
                    a.as_number()
                        .unwrap_or(0.)
                        .total_cmp(&b.as_number().unwrap_or(0.))
                })
                .cloned()
                .unwrap_or(Value::Null),
            Reducer::Max => values
                .iter()
                .filter(|v| v.as_number().is_some_and(|n| !n.is_nan()))
                .max_by(|a, b| {
                    a.as_number()
                        .unwrap_or(0.)
                        .total_cmp(&b.as_number().unwrap_or(0.))
                })
                .cloned()
                .unwrap_or(Value::Null),
            Reducer::First => values
                .iter()
                .find(|v| !v.is_null())
                .cloned()
                .unwrap_or(Value::Null),
            Reducer::Last => values
                .iter()
                .rev()
                .find(|v| !v.is_null())
                .cloned()
                .unwrap_or(Value::Null),
            Reducer::Median => OrderStatistics::default()
                .median(values)
                .map_or(Value::Null, Value::Number),
            Reducer::Deviation => {
                let numbers = numbers();
                if numbers.len() < 2 {
                    return Value::Number(0.);
                }
                let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
                let variance = numbers.iter().map(|n| (n - mean).powi(2)).sum::<f64>()
                    / (numbers.len() - 1) as f64;
                Value::Number(variance.sqrt())
            }
            Reducer::SumOfSquares => Value::Number(
                values
                    .iter()
                    .filter_map(Value::as_number)
                    .map(|n| n * n)
                    .sum(),
            ),
        }
    }
}

/// The reducer one grouped output applies, Plot's group output reducers.
///
/// Unlike [`Reducer`], which folds a single field, a group output also has the
/// group's records: `find` picks a row by a predicate and returns that row's
/// value, which is how a chart picks a series' reading by something other than
/// its own extent.
#[derive(Clone)]
pub enum GroupReducer {
    /// The number of rows in the group, Plot's `"count"`.
    Count,
    /// The number of distinct values, Plot's `"distinct"`.
    Distinct,
    /// The sum of the values, Plot's `"sum"`.
    Sum,
    /// The arithmetic mean, Plot's `"mean"`.
    Mean,
    /// The smallest value, Plot's `"min"`.
    Min,
    /// The largest value, Plot's `"max"`.
    Max,
    /// The median, Plot's `"median"`.
    Median,
    /// The standard deviation, Plot's `"deviation"`.
    Deviation,
    /// The group's first value, Plot's `"first"`.
    First,
    /// The group's last value, Plot's `"last"`.
    Last,
    /// The value at the first row the test accepts, Plot's `Plot.find`.
    Find(std::rc::Rc<dyn Fn(&Table) -> bool>),
    /// The most common value, Plot's `"mode"`.
    Mode,
    /// The group's share of all rows, Plot's `"proportion"` (and, since
    /// facets reduce on their own, `"proportion-facet"`).
    Proportion,
    /// The variance, Plot's `"variance"`.
    Variance,
    /// The `p`-quantile, Plot's `"p25"`, `"p75"` …
    Quantile(f64),
    /// The least value within 1.5 IQR below the first quartile, Plot's box
    /// `loqr1`: see [`whisker_low`](crate::mark::whisker_low).
    WhiskerLow,
    /// The greatest value within 1.5 IQR above the third quartile, Plot's
    /// box `hiqr2`: see [`whisker_high`](crate::mark::whisker_high).
    WhiskerHigh,
    /// A custom reducer over the group's values and records.
    #[allow(clippy::type_complexity)]
    Custom(std::rc::Rc<dyn Fn(&[Value], &[&Table]) -> Value>),
}

impl GroupReducer {
    /// A reducer that returns the value of the first row matching `test`,
    /// Plot's `Plot.find`.
    pub fn find(test: impl Fn(&Table) -> bool + 'static) -> Self {
        Self::Find(std::rc::Rc::new(test))
    }

    /// A custom reducer, Plot's `{reduce: (values) => …}`.
    pub fn custom(reduce: impl Fn(&[Value], &[&Table]) -> Value + 'static) -> Self {
        Self::Custom(std::rc::Rc::new(reduce))
    }

    /// Plot's reducer names: `"count"`, `"sum"`, `"mean"`, `"p25"` …
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "count" => Self::Count,
            "distinct" => Self::Distinct,
            "sum" => Self::Sum,
            "mean" => Self::Mean,
            "min" => Self::Min,
            "max" => Self::Max,
            "median" => Self::Median,
            "deviation" => Self::Deviation,
            "variance" => Self::Variance,
            "first" => Self::First,
            "last" => Self::Last,
            "mode" => Self::Mode,
            "proportion" | "proportion-facet" => Self::Proportion,
            q if q.starts_with('p') => Self::Quantile(q[1..].parse::<f64>().ok()? / 100.),
            _ => return None,
        })
    }

    /// Whether the reducer reads the group's records, not just its values:
    /// only `find` and a custom reducer do, so the others can skip gathering
    /// them.
    pub(crate) fn reads_records(&self) -> bool {
        matches!(self, GroupReducer::Find(_) | GroupReducer::Custom(_))
    }

    /// Reduce one group: its values, and the records they came from.
    pub fn reduce(&self, values: &[Value], rows: &[&Table]) -> Value {
        self.reduce_with(values, &mut OrderStatistics::default(), rows)
    }

    /// [`GroupReducer::reduce`], reading the order of `values` through
    /// `order`, which the outputs reading one column share: a box's
    /// quartiles and whiskers gather the column's numbers once and select
    /// each quartile once. `order` must only ever be given these `values`.
    pub(crate) fn reduce_with(
        &self,
        values: &[Value],
        order: &mut OrderStatistics,
        rows: &[&Table],
    ) -> Value {
        let simple = match self {
            GroupReducer::Count => Some(Reducer::Count),
            GroupReducer::Sum => Some(Reducer::Sum),
            GroupReducer::Mean => Some(Reducer::Mean),
            GroupReducer::Min => Some(Reducer::Min),
            GroupReducer::Max => Some(Reducer::Max),
            GroupReducer::Deviation => Some(Reducer::Deviation),
            GroupReducer::First => Some(Reducer::First),
            GroupReducer::Last => Some(Reducer::Last),
            GroupReducer::Distinct
            | GroupReducer::Find(_)
            | GroupReducer::Mode
            | GroupReducer::Proportion
            | GroupReducer::Variance
            // These read the column's order, through `order`.
            | GroupReducer::Median
            | GroupReducer::Quantile(_)
            | GroupReducer::WhiskerLow
            | GroupReducer::WhiskerHigh
            | GroupReducer::Custom(_) => None,
        };
        if let Some(reducer) = simple {
            return reducer.reduce(values);
        }
        match self {
            GroupReducer::Distinct => {
                let mut seen: Vec<ValueKey> = Vec::new();
                for value in values {
                    let key = value.key();
                    if !seen.contains(&key) {
                        seen.push(key);
                    }
                }
                Value::Number(seen.len() as f64)
            }
            GroupReducer::Find(test) => values
                .iter()
                .zip(rows.iter())
                .find(|(_, row)| test(row))
                .map(|(value, _)| value.clone())
                .unwrap_or(Value::Null),
            GroupReducer::Mode => {
                let mut counts: Vec<(ValueKey, Value, usize)> = Vec::new();
                for value in values.iter().filter(|v| !v.is_null()) {
                    let key = value.key();
                    match counts.iter_mut().find(|(k, _, _)| *k == key) {
                        Some(entry) => entry.2 += 1,
                        None => counts.push((key, value.clone(), 1)),
                    }
                }
                counts
                    .into_iter()
                    .max_by_key(|(_, _, n)| *n)
                    .map(|(_, v, _)| v)
                    .unwrap_or(Value::Null)
            }
            GroupReducer::Proportion => Value::Number(values.len() as f64),
            GroupReducer::Variance => {
                let numbers: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
                if numbers.len() < 2 {
                    return Value::Null;
                }
                let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
                Value::Number(
                    numbers.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                        / (numbers.len() - 1) as f64,
                )
            }
            GroupReducer::Median => order.median(values).map_or(Value::Null, Value::Number),
            GroupReducer::Quantile(p) => order
                .quantile(values, *p)
                .map_or(Value::Null, Value::Number),
            GroupReducer::WhiskerLow => crate::mark::whisker_low_with(values, order),
            GroupReducer::WhiskerHigh => crate::mark::whisker_high_with(values, order),
            GroupReducer::Custom(f) => f(values, rows),
            _ => Value::Null,
        }
    }
}

impl std::fmt::Debug for GroupReducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            GroupReducer::Count => "Count",
            GroupReducer::Distinct => "Distinct",
            GroupReducer::Sum => "Sum",
            GroupReducer::Mean => "Mean",
            GroupReducer::Min => "Min",
            GroupReducer::Max => "Max",
            GroupReducer::Median => "Median",
            GroupReducer::Deviation => "Deviation",
            GroupReducer::First => "First",
            GroupReducer::Last => "Last",
            GroupReducer::Find(_) => "Find(..)",
            GroupReducer::Mode => "Mode",
            GroupReducer::Proportion => "Proportion",
            GroupReducer::Variance => "Variance",
            GroupReducer::Quantile(_) => "Quantile(..)",
            GroupReducer::WhiskerLow => "WhiskerLow",
            GroupReducer::WhiskerHigh => "WhiskerHigh",
            GroupReducer::Custom(_) => "Custom(..)",
        };
        f.write_str(name)
    }
}

/// The numbers of one column in ascending order, as d3's quantiles read
/// them, without sorting them unless that is the cheaper way.
///
/// The numbers are gathered once, on first use. Each order statistic asked
/// for selects it (`select_nth_unstable_by`), which leaves the numbers a
/// partly reordered copy of the same multiset; after a few selections the
/// numbers are sorted outright, and later statistics read them directly.
/// Either way the result is the statistic of the sorted numbers, bit for
/// bit: `total_cmp` orders every bit pattern apart (`-0` before `+0`, each
/// NaN by its bits), so the sorted sequence, and each of its elements, is
/// unique. Quantiles are kept by `p`, so a box's whiskers read its quartiles
/// once.
#[derive(Default)]
pub(crate) struct OrderStatistics {
    numbers: Option<Vec<f64>>,
    sorted: bool,
    selections: usize,
    quantiles: Vec<(u64, f64)>,
}

impl OrderStatistics {
    /// The selections after which sorting the numbers is cheaper than
    /// selecting again.
    const SELECTIONS_BEFORE_SORT: usize = 4;

    fn numbers(&mut self, values: &[Value]) -> &mut Vec<f64> {
        self.numbers
            .get_or_insert_with(|| values.iter().filter_map(Value::as_number).collect())
    }

    /// The number of numbers among `values`.
    fn len(&mut self, values: &[Value]) -> usize {
        self.numbers(values).len()
    }

    /// The `k`-th of the numbers in ascending order and, when `next`, the
    /// `k + 1`-th (else the `k`-th again).
    fn at(&mut self, values: &[Value], k: usize, next: bool) -> (f64, f64) {
        if !self.sorted && self.selections >= Self::SELECTIONS_BEFORE_SORT {
            self.numbers(values).sort_unstable_by(f64::total_cmp);
            self.sorted = true;
        }
        self.selections += 1;
        let sorted = self.sorted;
        let numbers = self.numbers(values);
        if sorted {
            return (numbers[k], if next { numbers[k + 1] } else { numbers[k] });
        }
        let (_, nth, above) = numbers.select_nth_unstable_by(k, f64::total_cmp);
        let nth = *nth;
        // Every number after the `k`-th is at least it, so the least of them
        // is the `k + 1`-th.
        let following = if next {
            above
                .iter()
                .copied()
                .min_by(f64::total_cmp)
                .expect("the k + 1-th number")
        } else {
            nth
        };
        (nth, following)
    }

    /// d3's R-7 `p`-quantile of the numbers, or `None` without any.
    pub(crate) fn quantile(&mut self, values: &[Value], p: f64) -> Option<f64> {
        if let Some(&(_, q)) = self.quantiles.iter().find(|(bits, _)| *bits == p.to_bits()) {
            return Some(q);
        }
        let len = self.len(values);
        if len == 0 {
            return None;
        }
        let h = (len - 1) as f64 * p;
        let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
        let (a, b) = self.at(values, lo, hi != lo);
        let q = a + (b - a) * (h - h.floor());
        self.quantiles.push((p.to_bits(), q));
        Some(q)
    }

    /// The median of the numbers, the middle one or the mean of the middle
    /// two, or `None` without any.
    pub(crate) fn median(&mut self, values: &[Value]) -> Option<f64> {
        let len = self.len(values);
        if len == 0 {
            return None;
        }
        let mid = len / 2;
        Some(if len.is_multiple_of(2) {
            let (a, b) = self.at(values, mid - 1, true);
            (a + b) / 2.
        } else {
            self.at(values, mid, false).0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reducers_ignore_nulls() {
        let values = vec![Value::Number(1.), Value::Null, Value::Number(3.)];
        assert_eq!(Reducer::Mean.reduce(&values), Value::Number(2.));
        assert_eq!(Reducer::Count.reduce(&values), Value::Number(3.));
        assert_eq!(Reducer::Median.reduce(&values), Value::Number(2.));
    }

    /// Columns with what a quantile must order exactly: nulls and text it
    /// skips, `-0` and `+0`, NaNs of both signs and other payloads, the
    /// infinities, dates and booleans it reads as numbers, and ties.
    fn awkward_columns() -> Vec<Vec<Value>> {
        let negative_nan = f64::from_bits(f64::NAN.to_bits() | (1 << 63));
        let payload_nan = f64::from_bits(f64::NAN.to_bits() | 1);
        let awkward = vec![
            Value::Number(3.),
            Value::Null,
            Value::Number(-0.),
            Value::Number(0.),
            Value::text("t"),
            Value::Number(f64::NAN),
            Value::Number(negative_nan),
            Value::Number(payload_nan),
            Value::Number(f64::INFINITY),
            Value::Number(f64::NEG_INFINITY),
            Value::date_from_millis(86_400_000),
            Value::Bool(true),
            Value::Number(3.),
            Value::Number(-2.5),
            Value::Number(1e300),
        ];
        let mut columns = vec![
            Vec::new(),
            vec![Value::Null, Value::text("only text")],
            vec![Value::Number(7.)],
            vec![Value::Number(-0.), Value::Number(0.)],
            awkward.clone(),
        ];
        // Longer columns, odd and even, finite and with the awkward values
        // spread through them.
        for len in [2usize, 9, 10, 101, 1000] {
            columns.push(
                (0..len)
                    .map(|i| Value::Number(((i * 7919) % 613) as f64 * 0.37 - 50.))
                    .collect(),
            );
            columns.push(
                (0..len)
                    .map(|i| match i % 5 {
                        0 => awkward[i % awkward.len()].clone(),
                        _ => Value::Number(((i * 104_729) % 97) as f64 / 3.),
                    })
                    .collect(),
            );
        }
        columns
    }

    /// d3's quantile over a fully sorted copy, the reading the order
    /// statistics must match bit for bit.
    fn sorted_quantile(values: &[Value], p: f64) -> Option<f64> {
        let mut sorted: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
        if sorted.is_empty() {
            return None;
        }
        sorted.sort_by(f64::total_cmp);
        let h = (sorted.len() - 1) as f64 * p;
        let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
        Some(sorted[lo] + (sorted[hi] - sorted[lo]) * (h - h.floor()))
    }

    fn sorted_median(values: &[Value]) -> Option<f64> {
        let mut sorted: Vec<f64> = values.iter().filter_map(Value::as_number).collect();
        if sorted.is_empty() {
            return None;
        }
        sorted.sort_by(f64::total_cmp);
        let mid = sorted.len() / 2;
        Some(if sorted.len().is_multiple_of(2) {
            (sorted[mid - 1] + sorted[mid]) / 2.
        } else {
            sorted[mid]
        })
    }

    fn bits(value: Option<f64>) -> Option<u64> {
        value.map(f64::to_bits)
    }

    #[test]
    fn order_statistics_match_a_full_sort_bit_for_bit() {
        let ps = [
            0., 0.25, 0.5, 0.75, 1., 0.1, 0.9, 0.333, 0.5, 0.25, 0.01, 0.99,
        ];
        for values in awkward_columns() {
            // One shared order across every statistic, as a group's outputs
            // share it: it selects, sorts after a few, and keeps quantiles.
            let mut shared = OrderStatistics::default();
            for (i, &p) in ps.iter().enumerate() {
                let expected = bits(sorted_quantile(&values, p));
                assert_eq!(
                    bits(shared.quantile(&values, p)),
                    expected,
                    "p{p} of {values:?}"
                );
                // A fresh order, which selects once.
                let mut fresh = OrderStatistics::default();
                assert_eq!(
                    bits(fresh.quantile(&values, p)),
                    expected,
                    "p{p} of {values:?}"
                );
                if i % 3 == 0 {
                    assert_eq!(bits(shared.median(&values)), bits(sorted_median(&values)));
                }
            }
            assert_eq!(
                bits(OrderStatistics::default().median(&values)),
                bits(sorted_median(&values))
            );
            let reduced = GroupReducer::Median.reduce(&values, &[]);
            assert_eq!(
                reduced.as_number().map(f64::to_bits),
                bits(sorted_median(&values))
            );
            assert_eq!(
                Reducer::Median
                    .reduce(&values)
                    .as_number()
                    .map(f64::to_bits),
                bits(sorted_median(&values))
            );
        }
    }

    /// Plot's box whiskers as they were written before the order statistics:
    /// the quartiles of a sorted copy, then the least (or greatest) number
    /// within 1.5 IQR, folded over the values in order.
    fn sorted_whisker(values: &[Value], low: bool) -> Value {
        let (Some(q1), Some(q3)) = (sorted_quantile(values, 0.25), sorted_quantile(values, 0.75))
        else {
            return Value::Null;
        };
        let numbers = values.iter().filter_map(Value::as_number);
        let found = if low {
            let lo = q1 * 2.5 - q3 * 1.5;
            numbers
                .filter(|v| *v >= lo)
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
        } else {
            let hi = q3 * 2.5 - q1 * 1.5;
            numbers
                .filter(|v| *v <= hi)
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
        };
        found.map(Value::Number).unwrap_or(Value::Null)
    }

    #[test]
    fn whisker_reducers_match_the_sorted_whiskers() {
        let same = |a: &Value, b: &Value| match (a, b) {
            (Value::Number(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
            _ => a == b,
        };
        for values in awkward_columns() {
            let low = sorted_whisker(&values, true);
            let high = sorted_whisker(&values, false);
            assert!(same(&crate::mark::whisker_low(&values), &low), "{values:?}");
            assert!(
                same(&crate::mark::whisker_high(&values), &high),
                "{values:?}"
            );
            assert!(same(&GroupReducer::WhiskerLow.reduce(&values, &[]), &low));
            assert!(same(&GroupReducer::WhiskerHigh.reduce(&values, &[]), &high));
            // A box's outputs over one column, sharing one order in the
            // order the parts ask: whiskers, then quartiles and median.
            let mut order = OrderStatistics::default();
            let reducers = [
                (GroupReducer::WhiskerLow, low.clone()),
                (GroupReducer::WhiskerHigh, high.clone()),
                (
                    GroupReducer::Quantile(0.25),
                    sorted_quantile(&values, 0.25).map_or(Value::Null, Value::Number),
                ),
                (
                    GroupReducer::Quantile(0.75),
                    sorted_quantile(&values, 0.75).map_or(Value::Null, Value::Number),
                ),
                (
                    GroupReducer::Quantile(0.5),
                    sorted_quantile(&values, 0.5).map_or(Value::Null, Value::Number),
                ),
                (
                    GroupReducer::Median,
                    sorted_median(&values).map_or(Value::Null, Value::Number),
                ),
                (GroupReducer::WhiskerLow, low.clone()),
            ];
            for (reducer, expected) in reducers {
                let reduced = reducer.reduce_with(&values, &mut order, &[]);
                assert!(same(&reduced, &expected), "{reducer:?} of {values:?}");
            }
        }
    }
}
