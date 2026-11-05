//! Difference mark <VersionBadge version="0.6.12" pr="1896" />: `marks/difference.md`.

use crate::prelude::*;
use gpui_plot::{Anchor, WindowReducer};

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "difference", 1, e01),
        example("marks", "difference", 2, e02),
        example("marks", "difference", 3, e03),
        example("marks", "difference", 4, e04),
        example("marks", "difference", 5, e05),
    ]
}

/// The docs' `temperature`: the 2020 rows of the San Francisco and San Jose
/// temperatures.
fn temperature() -> Dataset {
    data::sf_sj_temperatures()
        .rows()
        .iter()
        .filter(|row| row.value("date").as_date().is_some_and(|date| date.year() == 2020))
        .cloned()
        .collect()
}

fn e01() -> Plot {
    // `positiveFill: "red", negativeFill: "blue", tip: true` have no port.
    difference_y(data::gistemp())
        .x("Date")
        .y("Anomaly")
        .plot()
        .y(|y| y.grid(true))
}

fn e02() -> Plot {
    // `positiveFill: "red", negativeFill: "blue", tip: true` have no port.
    difference_y(data::gistemp())
        .x("Date")
        .y("Anomaly")
        .window(12 * 2, WindowReducer::Mean, Anchor::Middle)
        .plot()
        .y(|y| y.grid(true))
}

fn e03() -> Plot {
    Plot::plot()
        .x(|x| x.tick_format("%b"))
        .y(|y| y.grid(true).label("Travelers"))
        .mark(
            axis_y()
                .axis_label("Travelers per day (thousands, 2020 vs. 2019)")
                .tick_format(|v: &Value| format_number(",", v.as_number().unwrap_or(0.) / 1000.)),
        )
        .mark(rule_y([0.]))
        .mark(
            // `tip: {format: {x: "%B %-d"}}` has no port.
            difference_y(data::tsa()).x("Date").y1("2019").y2("2020"),
        )
}

fn e04() -> Plot {
    Plot::plot()
        .x(|x| x.tick_format("%b"))
        .y(|y| y.grid(true))
        .mark(rule_y([32.]))
        .mark(
            // Plot.windowY(14, Plot.groupX({y1: find SJ, y2: find SF}, …)):
            // the window here would run before the grouping, over both
            // stations' rows, so it is left out; `tip: true` has no port.
            difference_y(temperature())
                .x("date")
                .y("tmin")
                .group_x()
                .group_output(
                    "y1",
                    GroupReducer::find(|d| d.text("station").is_some_and(|s| s == "SJ")),
                )
                .group_output(
                    "y2",
                    GroupReducer::find(|d| d.text("station").is_some_and(|s| s == "SF")),
                ),
        )
}

fn e05() -> Plot {
    // Plot.shiftX("+1 year", …) yields x1 = Date and x2 = Date + 1 year; the
    // shift here writes the moved date into `Date2` (an offset in days).
    difference_y(data::aapl())
        .shift("Date", "Date2", 365.)
        .x1("Date")
        .x2("Date2")
        .y("Close")
        .plot()
        .y(|y| y.grid(true))
}
