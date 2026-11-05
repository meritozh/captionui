//! Interval transform: `transforms/interval.md`.
//!
//! Gap: a scale's `transform` function (`(d) => d / 1e6`) is missing, so the
//! ports divide the volume in the channel instead.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "interval", 1, e01),
        example("transforms", "interval", 2, e02),
        example("transforms", "interval", 3, e03),
    ]
}

/// `aapl.slice(-40)`: the last forty trading days.
fn last_40() -> Dataset {
    let aapl = data::aapl();
    let rows = aapl.rows();
    Dataset::new(rows[rows.len().saturating_sub(40)..].iter().cloned())
}

/// The volume in millions, the docs' `y: {transform: (d) => d / 1e6}`.
fn millions() -> Channel {
    Channel::number(|d| d.num("Volume") / 1e6)
}

/// `Plot.barY(aapl.slice(-40), {x: "Date", y: "Volume"})` on a band x.
fn e01() -> Plot {
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.type_name("band"))
        .y(|y| y.label("Daily trade volume (millions)"))
        .mark(bar_y(last_40()).x("Date").y(millions()))
        .mark(rule_y([0.]))
}

/// `Plot.rectY(aapl.slice(-40), {x: "Date", interval: "day", y: "Volume"})`
fn e02() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true).label("Daily trade volume (millions)"))
        .mark(
            rect_y(last_40())
                .x("Date")
                .interval(Interval::Day)
                .y(millions()),
        )
        .mark(rule_y([0.]))
}

/// `Plot.barY(aapl.slice(-40), {x: "Date", y: "Volume", interval: 5e6})`
///
/// Gap: an interval on a bar's value channel (bucketing `y` into
/// `[floor, floor + 5e6)` bars) is missing, as the library's interval
/// buckets the x channel; the port computes each bucket's `y1` and `y2`.
fn e03() -> Plot {
    let floor = |d: &Table| (d.num("Volume") / 5e6).floor() * 5.;
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.type_name("band"))
        .y(|y| y.grid(true).label("Daily trade volume (millions)"))
        .mark(
            bar_y(last_40())
                .x("Date")
                .y1(Channel::number(floor))
                .y2(Channel::number(move |d| floor(d) + 5.)),
        )
        .mark(rule_y([0.]))
}
