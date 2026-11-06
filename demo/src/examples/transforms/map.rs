//! Map transform: `transforms/map.md`.
//!
//! Gap: the library's `lineY` always draws its rows at their index along x,
//! ignoring an explicit `x` channel, so the ports that give one use `line`,
//! which is what Plot's `lineY` with an `x` draws.

use crate::prelude::*;
use gpui_plot::{Anchor, WindowReducer};

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "map", 1, e01),
        example("transforms", "map", 2, e02),
        example("transforms", "map", 3, e03),
    ]
}

/// `Plot.lineY({length: 600}, Plot.mapY("cumsum", {y: d3.randomNormal()}))`
fn e01() -> Plot {
    Plot::plot().mark(rule_y([0.])).mark(
        line_y(data::normal_samples(600, "y", 42))
            .map("y", "cumsum", MapReducer::Sum)
            // Plot's `y: d3.randomNormal()` is a function, so the axis has
            // no label.
            .y(Channel::number(|d| d.num("cumsum"))),
    )
}

/// The AAPL data with each day's volume quantile, Plot's
/// `Plot.mapY("quantile", {y: "Volume"})`: the volume's rank over the
/// series, as a fraction of its length.
fn aapl_volume_quantile() -> Dataset {
    let aapl = data::aapl();
    let mut sorted: Vec<f64> = aapl
        .rows()
        .iter()
        .filter_map(|d| d.number("Volume"))
        .collect();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len().max(2) as f64;
    Dataset::new(aapl.rows().iter().map(|d| {
        let quantile = d.number("Volume").map(|v| {
            // d3's quantile rank: where the value falls among the sorted values.
            let rank = sorted.partition_point(|x| *x < v);
            rank as f64 / (n - 1.)
        });
        d.clone().with(
            "quantile",
            quantile.map(Value::Number).unwrap_or(Value::Null),
        )
    }))
}

/// The volume's quantile, raw and smoothed with `Plot.windowY(30, …)`.
///
/// Gap: the map transform's `quantile` mapper is missing (the library's
/// `Rank` is the datum's position in the series); the quantile is computed
/// up front and the window reads it.
fn e02() -> Plot {
    Plot::plot()
        .mark(rule_y([0., 1.]))
        .mark(
            line(aapl_volume_quantile())
                .x("Date")
                .y("quantile")
                .stroke_opacity(0.2),
        )
        .mark(line(aapl_volume_quantile()).x("Date").y("quantile").window(
            30,
            WindowReducer::Mean,
            Anchor::Middle,
        ))
}

/// AAPL's closing price with Bollinger bands: `Plot.map({y1: Plot.bollinger({n, k: -k}), y2: Plot.bollinger({n, k})}, …)`,
/// with the docs' default `n` of 20 and `k` of 2.
///
/// Gap: `Plot.map` with a `Plot.bollinger` mapper is missing; the bands are
/// computed up front over the trailing `n` days.
fn e03() -> Plot {
    let (n, k) = (20usize, 2.);
    let aapl = data::aapl();
    let closes: Vec<f64> = aapl.rows().iter().map(|d| d.num("Close")).collect();
    let banded = Dataset::new(aapl.rows().iter().enumerate().map(|(i, d)| {
        let mut row = d.clone();
        if i + 1 >= n {
            let window = &closes[i + 1 - n..=i];
            let mean = window.iter().sum::<f64>() / n as f64;
            // d3's `deviation`: the sample standard deviation.
            let variance = window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
            let deviation = variance.sqrt();
            row = row
                .with("mean", Value::Number(mean))
                .with("lower", Value::Number(mean - k * deviation))
                .with("upper", Value::Number(mean + k * deviation));
        }
        row
    }));
    // Plot's map outputs carry their input's label, so every channel here
    // reads "Close".
    Plot::plot()
        .y(|y| y.grid(true).label("Close"))
        .mark(
            area_y(banded.clone())
                .x("Date")
                .y1("lower")
                .y2("upper")
                .fill_opacity(0.2),
        )
        .mark(line(banded).x("Date").y("mean").stroke("blue"))
        .mark(line(data::aapl()).x("Date").y("Close").stroke_width(1.))
}
