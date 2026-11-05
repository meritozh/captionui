//! Select transform: `transforms/select.md`.

use super::normalize::stocks;
use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "select", 1, e01),
        example("transforms", "select", 2, e02),
        example("transforms", "select", 3, e03),
    ]
}

/// `Plot.text(aapl, Plot.selectLast({x: "Date", y: "Close", text: "Close", frameAnchor: "bottom", dy: -6}))`
fn e01() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(rule_y([0.]))
        .mark(line(data::aapl()).x("Date").y("Close"))
        .mark(
            text(data::aapl())
                .x("Date")
                .y("Close")
                .text("Close")
                .frame_anchor(FrameAnchor::Bottom)
                .dy(-6.)
                .select(Selector::Last, None::<&str>),
        )
}

/// The lowest and highest closes, `Plot.selectMinY` and `Plot.selectMaxY`.
fn e02() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(rule_y([0.]))
        .mark(line(data::aapl()).x("Date").y("Close"))
        .mark(
            text(data::aapl())
                .x("Date")
                .y("Close")
                .text("Close")
                .frame_anchor(FrameAnchor::Top)
                .dy(6.)
                .select(Selector::Min("y".into()), None::<&str>),
        )
        .mark(
            text(data::aapl())
                .x("Date")
                .y("Close")
                .text("Close")
                .frame_anchor(FrameAnchor::Bottom)
                .dy(-6.)
                .select(Selector::Max("y".into()), None::<&str>),
        )
}

/// `Plot.text(stocks, Plot.selectLast({x: "Date", y: "Close", z: "Symbol", text: "Symbol", textAnchor: "start", dx: 3}))`
fn e03() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(rule_y([0.]))
        .mark(line(stocks()).x("Date").y("Close").stroke("Symbol"))
        .mark(
            text(stocks())
                .x("Date")
                .y("Close")
                .z("Symbol")
                .text("Symbol")
                .text_anchor(TextAnchor::Start)
                .dx(3.)
                .select(Selector::Last, Some("Symbol")),
        )
}
