//! Map transform: `transforms/map.md`.
//!
//! Gap: the library's `lineY` always draws its rows at their index along x,
//! ignoring an explicit `x` channel, so the ports that give one use `line`,
//! which is what Plot's `lineY` with an `x` draws.

use crate::prelude::*;
use gpui_plot::{Anchor, Mapper, WindowReducer};

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
            // Plot's `y: d3.randomNormal()` is a function, so the axis has
            // no label.
            .y(Channel::number(|d| d.num("y")))
            .map_y(Mapper::Cumsum),
    )
}

/// Each day's volume quantile, raw and smoothed with `Plot.windowY(30, …)`:
/// `Plot.mapY("quantile", {x: "Date", y: "Volume"})`.
fn e02() -> Plot {
    Plot::plot()
        .mark(rule_y([0., 1.]))
        .mark(
            line(data::aapl())
                .x("Date")
                .y("Volume")
                .map_y(Mapper::Quantile)
                .stroke_opacity(0.2),
        )
        .mark(
            line(data::aapl())
                .x("Date")
                .y("Volume")
                .map_y(Mapper::Quantile)
                .window(30, WindowReducer::Mean, Anchor::Middle),
        )
}

/// AAPL's closing price with Bollinger bands: `Plot.map({y1: Plot.bollinger({n, k: -k}), y2: Plot.bollinger({n, k})}, …)`,
/// with the docs' default `n` of 20 and `k` of 2.
fn e03() -> Plot {
    let (n, k) = (20usize, 2.);
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(
            area_y(data::aapl())
                .x("Date")
                .y("Close")
                .map_channel("y1", Mapper::bollinger(n, -k))
                .map_channel("y2", Mapper::bollinger(n, k))
                .fill_opacity(0.2),
        )
        .mark(
            line(data::aapl())
                .x("Date")
                .y("Close")
                .map_channel("y", Mapper::bollinger(n, 0.))
                .stroke("blue"),
        )
        .mark(line(data::aapl()).x("Date").y("Close").stroke_width(1.))
}
