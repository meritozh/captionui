//! The map transform, Plot's `Plot.map`, `Plot.mapX` and `Plot.mapY`.

use crate::table::{Dataset, Table};
use crate::transform::window::{Anchor, WindowReducer, d3_variance, window_map, window_values};
use crate::value::Value;

/// A map method, the mapper of Plot's `mapX`, `mapY` and `map`: it maps
/// each series of a channel's values (each `z` group of each facet) to as
/// many new values.
#[derive(Clone)]
pub enum Mapper {
    /// The running total, Plot's `"cumsum"`.
    Cumsum,
    /// Each value's rank in its series, from zero, Plot's `"rank"` (d3's
    /// `rank`: tied values share the lowest rank; a missing value has none).
    Rank,
    /// Each value's rank as a fraction of its series' defined values less
    /// one, Plot's `"quantile"`.
    Quantile,
    /// A moving window, Plot's `Plot.window({k, reduce, anchor, strict})`.
    Window {
        /// The window's size, Plot's `k`.
        k: usize,
        /// How the window's values reduce, Plot's `reduce`.
        reduce: WindowReducer,
        /// Where the window sits around its value, Plot's `anchor`.
        anchor: Anchor,
        /// Whether a short or incomplete window gives no value, Plot's
        /// `strict`.
        strict: bool,
    },
    /// A Bollinger band's edge, Plot's `Plot.bollinger({n, k, anchor,
    /// strict})`: the mean of the window of `n` values plus `k` sample
    /// standard deviations.
    Bollinger {
        /// The window's size, Plot's `n`.
        n: usize,
        /// How many deviations from the mean, Plot's `k`.
        k: f64,
        /// Where the window sits around its value, Plot's `anchor`.
        anchor: Anchor,
        /// Whether a short or incomplete window gives no value, Plot's
        /// `strict`.
        strict: bool,
    },
    /// A custom mapper over one series' values, Plot's `(values) => …`
    /// function or `{mapIndex}` object; it returns as many values.
    #[allow(clippy::type_complexity)]
    Function(std::rc::Rc<dyn Fn(&[Value]) -> Vec<Value>>),
}

impl std::fmt::Debug for Mapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mapper::Cumsum => f.write_str("Cumsum"),
            Mapper::Rank => f.write_str("Rank"),
            Mapper::Quantile => f.write_str("Quantile"),
            Mapper::Window {
                k,
                reduce,
                anchor,
                strict,
            } => f
                .debug_struct("Window")
                .field("k", k)
                .field("reduce", reduce)
                .field("anchor", anchor)
                .field("strict", strict)
                .finish(),
            Mapper::Bollinger {
                n,
                k,
                anchor,
                strict,
            } => f
                .debug_struct("Bollinger")
                .field("n", n)
                .field("k", k)
                .field("anchor", anchor)
                .field("strict", strict)
                .finish(),
            Mapper::Function(_) => f.write_str("Function(..)"),
        }
    }
}

impl Mapper {
    /// Plot's `Plot.window({k, reduce})`, anchored in the middle and not
    /// strict, Plot's defaults.
    pub fn window(k: usize, reduce: WindowReducer) -> Self {
        Mapper::Window {
            k,
            reduce,
            anchor: Anchor::Middle,
            strict: false,
        }
    }

    /// Plot's `Plot.bollinger({n, k})`, anchored at the end and strict,
    /// Plot's defaults (Plot's own default `n` is 20, and `k` 0).
    pub fn bollinger(n: usize, k: f64) -> Self {
        Mapper::Bollinger {
            n,
            k,
            anchor: Anchor::End,
            strict: true,
        }
    }

    /// A custom mapper, Plot's `{mapIndex(I, S, T)}` over one series.
    pub fn function(map: impl Fn(&[Value]) -> Vec<Value> + 'static) -> Self {
        Mapper::Function(std::rc::Rc::new(map))
    }

    /// Set a window's or band's `anchor`.
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        if let Mapper::Window { anchor: a, .. } | Mapper::Bollinger { anchor: a, .. } = &mut self {
            *a = anchor;
        }
        self
    }

    /// Set a window's or band's `strict`.
    pub fn strict(mut self, strict: bool) -> Self {
        if let Mapper::Window { strict: s, .. } | Mapper::Bollinger { strict: s, .. } = &mut self {
            *s = strict;
        }
        self
    }
}

/// Map one series of values, Plot's `mapIndex(I, S, T)`.
pub fn map_values(values: &[Value], mapper: &Mapper) -> Vec<Value> {
    // JavaScript's `+v`: a missing value is NaN.
    let numbers = || -> Vec<Option<f64>> {
        values
            .iter()
            .map(|v| v.as_number().filter(|n| !n.is_nan()))
            .collect()
    };
    let to_values = |out: Vec<Option<f64>>| -> Vec<Value> {
        out.into_iter()
            .map(|v| v.map_or(Value::Null, Value::number))
            .collect()
    };
    match mapper {
        // `sum += S[i]`: a null adds nothing; anything else that is not a
        // number poisons the rest of the sum.
        Mapper::Cumsum => {
            let mut sum = 0.;
            values
                .iter()
                .map(|v| {
                    sum += match v {
                        Value::Null => 0.,
                        v => v.as_number().unwrap_or(f64::NAN),
                    };
                    Value::number(sum)
                })
                .collect()
        }
        Mapper::Rank => to_values(d3_rank(values)),
        // `rank(I, f).map((r) => r / n)`, with `n` d3's `count` (the values
        // that are numbers) less one.
        Mapper::Quantile => {
            let n = values
                .iter()
                .filter(|v| v.as_number().is_some_and(|n| !n.is_nan()))
                .count() as f64
                - 1.;
            to_values(
                d3_rank(values)
                    .into_iter()
                    .map(|r| r.map(|r| r / n))
                    .collect(),
            )
        }
        Mapper::Window {
            k,
            reduce,
            anchor,
            strict,
        } => to_values(window_values(&numbers(), *k, *reduce, *anchor, *strict)),
        // `window({k: n, reduce: (Y) => mean(Y) + k * (deviation(Y) || 0)})`.
        Mapper::Bollinger {
            n,
            k,
            anchor,
            strict,
        } => to_values(window_map(&numbers(), *n, *anchor, *strict, |slice| {
            let defined: Vec<f64> = slice
                .iter()
                .flatten()
                .copied()
                .filter(|v| !v.is_nan())
                .collect();
            if defined.is_empty() {
                return None;
            }
            let mean = defined.iter().sum::<f64>() / defined.len() as f64;
            Some(mean + k * d3_variance(&defined).map_or(0., f64::sqrt))
        })),
        Mapper::Function(map) => {
            let mut out = map(values);
            out.resize(values.len(), Value::Null);
            out
        }
    }
}

/// d3's `rank`: each value's index in the ascending order of the defined
/// values, tied values sharing the lowest; a missing value has none.
fn d3_rank(values: &[Value]) -> Vec<Option<f64>> {
    let defined = |v: &Value| !v.is_null() && !matches!(v, Value::Number(n) if n.is_nan());
    let mut order: Vec<usize> = (0..values.len()).filter(|&i| defined(&values[i])).collect();
    order.sort_by(|&a, &b| values[a].cmp(&values[b]));
    let mut ranks = vec![None; values.len()];
    let mut previous: Option<(usize, usize)> = None;
    for (position, &index) in order.iter().enumerate() {
        let rank = match previous {
            Some((last, rank)) if values[last] == values[index] => rank,
            _ => position,
        };
        ranks[index] = Some(rank as f64);
        previous = Some((index, rank));
    }
    ranks
}

/// Write a computed value into a new field on every row, Plot's `mapY` and
/// `mapX`.
///
/// The mapper sees the whole series in order — every row's current value, its
/// index, and the rows — so a cumulative sum, a running maximum or a rank can
/// be built from the values before it. The output keeps the rows' order and
/// their original fields.
pub fn map_field(
    data: &Dataset,
    field: &str,
    output: &str,
    mapper: impl Fn(&Value, usize, &[Table]) -> Value,
) -> Dataset {
    let rows = data.rows();
    let mapped: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| mapper(&row.value(field), index, rows))
        .collect();
    Dataset::new(
        rows.iter()
            .cloned()
            .zip(mapped)
            .map(|(row, value)| row.with(output, value)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(values: &[Value]) -> Vec<Option<f64>> {
        values.iter().map(Value::as_number).collect()
    }

    #[test]
    fn map_rank_and_quantile_follow_d3() {
        // d3.rank([3, 1, null, 3, 2]) is [2, 0, NaN, 2, 1]; the quantile
        // divides by the four numbers less one.
        let values = [
            Value::Number(3.),
            Value::Number(1.),
            Value::Null,
            Value::Number(3.),
            Value::Number(2.),
        ];
        assert_eq!(
            numbers(&map_values(&values, &Mapper::Rank)),
            vec![Some(2.), Some(0.), None, Some(2.), Some(1.)]
        );
        assert_eq!(
            numbers(&map_values(&values, &Mapper::Quantile)),
            vec![Some(2. / 3.), Some(0.), None, Some(2. / 3.), Some(1. / 3.)]
        );
        assert_eq!(
            numbers(&map_values(&values, &Mapper::Cumsum)),
            vec![Some(3.), Some(4.), Some(4.), Some(7.), Some(9.)]
        );
    }

    #[test]
    fn map_bollinger_is_a_strict_trailing_window() {
        let values: Vec<Value> = [1., 2., 3., 4.].map(Value::Number).to_vec();
        // n = 3, k = 1: mean plus the sample deviation (1) of each window.
        assert_eq!(
            numbers(&map_values(&values, &Mapper::bollinger(3, 1.))),
            vec![None, None, Some(3.), Some(4.)]
        );
        assert_eq!(
            numbers(&map_values(
                &values,
                &Mapper::bollinger(3, 0.)
                    .anchor(Anchor::Middle)
                    .strict(false)
            )),
            vec![Some(1.5), Some(2.), Some(3.), Some(3.5)]
        );
    }
}
