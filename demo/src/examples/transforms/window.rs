//! Window transform: `transforms/window.md`.
//!
//! Gap: the library's `lineY` always draws its rows at their index along x,
//! ignoring an explicit `x` channel, so the ports that give one use `line`,
//! which is what Plot's `lineY` with an `x` draws.

use crate::prelude::*;
use gpui_plot::{Anchor, WindowReducer};

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "window", 1, e01),
        example("transforms", "window", 2, e02),
        example("transforms", "window", 3, e03),
        example("transforms", "window", 4, e04),
        example("transforms", "window", 5, e05),
    ]
}

/// The temperature axis the San Francisco charts share.
fn temperature_axis(y: gpui_plot::ScaleOptions) -> gpui_plot::ScaleOptions {
    y.grid(true).label("Temperature (°F)")
}

/// The daily range with each bound smoothed: `Plot.windowY(k, …)`, with the
/// docs' default `k` of 7.
fn e01() -> Plot {
    let k = 7;
    Plot::plot()
        .y(temperature_axis)
        .mark(
            area_y(data::sftemp())
                .x("date")
                .y1("low")
                .y2("high")
                .fill_opacity(0.3),
        )
        .mark(
            line(data::sftemp())
                .x("date")
                .y("low")
                .stroke("blue")
                .window(k, WindowReducer::Mean, Anchor::Middle),
        )
        .mark(
            line(data::sftemp())
                .x("date")
                .y("high")
                .stroke("red")
                .window(k, WindowReducer::Mean, Anchor::Middle),
        )
}

/// `Plot.windowY({k: 28, anchor}, {x: "date", y: "high"})`, with the docs'
/// default `anchor` of `"end"`.
fn e02() -> Plot {
    Plot::plot()
        .y(temperature_axis)
        .mark(line(data::sftemp()).x("date").y("high").stroke_opacity(0.3))
        .mark(
            line(data::sftemp())
                .x("date")
                .y("high")
                .window(28, WindowReducer::Mean, Anchor::End),
        )
}

/// `Plot.windowY({k: 28, anchor: "end", strict}, {x: "date", y: "low"})`,
/// with the docs' default `strict` of `true`.
fn e03() -> Plot {
    Plot::plot()
        .y(temperature_axis)
        .mark(line(data::sftemp()).x("date").y("low").stroke_opacity(0.3))
        .mark(
            line(data::sftemp())
                .x("date")
                .y("low")
                .window(28, WindowReducer::Mean, Anchor::End)
                .window_strict(true),
        )
}

/// The 28-day minimum, maximum and median of the lows.
fn e04() -> Plot {
    let smoothed = |reducer: WindowReducer, stroke: &str| {
        let mark = line(data::sftemp())
            .x("date")
            .y("low")
            .window(28, reducer, Anchor::Middle);
        if stroke.is_empty() {
            mark
        } else {
            mark.stroke(stroke)
        }
    };
    Plot::plot()
        .y(temperature_axis)
        .mark(line(data::sftemp()).x("date").y("low").stroke_opacity(0.3))
        .mark(smoothed(WindowReducer::Min, "blue"))
        .mark(smoothed(WindowReducer::Max, "red"))
        .mark(smoothed(WindowReducer::Median, ""))
}

/// Each metro division's unemployment, colored by its month-over-month
/// change: `Plot.map({stroke: Plot.window({k: 2, reduce: "difference"})}, {…, z: "division", stroke: "unemployment"})`.
///
/// Gap: `Plot.map` with a window mapper on a color channel, and a window
/// restarting per `z` series, are missing; each division's difference is
/// computed up front into a `change` field the stroke reads.
fn e05() -> Plot {
    let bls = data::bls_metro_unemployment();
    // Plot's `window({k: 2, reduce: "difference"})` anchors in the middle,
    // `(k - 1) >> 1 = 0` before each value: a value's change is the next
    // value less it, and each division's last value has none.
    let rows = bls.rows();
    let changed = Dataset::new(rows.iter().enumerate().map(|(i, d)| {
        let division = d.value("division");
        let next = rows[i + 1..].iter().find(|n| n.value("division") == division);
        let change = match next {
            Some(next) => Value::number(next.num("unemployment") - d.num("unemployment")),
            None => Value::Null,
        };
        d.clone().with("change", change)
    }));
    Plot::plot()
        .y(|y| y.grid(true))
        .color(|c| c.scheme("BuYlRd").domain([-0.5, 0.5]))
        .mark(rule_y([0.]))
        .mark(
            line(changed)
                .x("date")
                .y("unemployment")
                .z("division")
                .stroke("change"),
        )
}
