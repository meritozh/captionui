//! Interval transform: `transforms/interval.md`.

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

/// The docs' `y: {transform: (d) => d / 1e6, label: …}`: the volume in
/// millions.
fn millions(y: gpui_plot::ScaleOptions) -> gpui_plot::ScaleOptions {
    y.transform(|d| d / 1e6)
        .label("Daily trade volume (millions)")
}

/// `Plot.barY(aapl.slice(-40), {x: "Date", y: "Volume"})` on a band x.
fn e01() -> Plot {
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.type_name("band"))
        .y(millions)
        .mark(bar_y(last_40()).x("Date").y("Volume"))
        .mark(rule_y([0.]))
}

/// `Plot.rectY(aapl.slice(-40), {x: "Date", interval: "day", y: "Volume"})`
fn e02() -> Plot {
    Plot::plot()
        .y(|y| millions(y.grid(true)))
        .mark(
            rect_y(last_40())
                .x("Date")
                .interval(Interval::Day)
                .y("Volume"),
        )
        .mark(rule_y([0.]))
}

/// `Plot.barY(aapl.slice(-40), {x: "Date", y: "Volume", interval: 5e6})`
///
/// Gap: an interval on a bar's value channel (bucketing `y` into
/// `[floor, floor + 5e6)` bars) is missing, as the library's interval
/// buckets the x channel; the port computes each bucket's `y1` and `y2`.
fn e03() -> Plot {
    let floor = |d: &Table| (d.num("Volume") / 5e6).floor() * 5e6;
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.type_name("band"))
        .y(|y| millions(y.grid(true)))
        .mark(
            bar_y(last_40())
                .x("Date")
                .y1(Channel::number(floor))
                .y2(Channel::number(move |d| floor(d) + 5e6)),
        )
        .mark(rule_y([0.]))
}
