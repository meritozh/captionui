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
        .filter(|row| {
            row.value("date")
                .as_date()
                .is_some_and(|date| date.year() == 2020)
        })
        .cloned()
        .collect()
}

fn e01() -> Plot {
    difference_y(data::gistemp())
        .x("Date")
        .y("Anomaly")
        .positive_fill("red")
        .negative_fill("blue")
        .plot()
        .y(|y| y.grid(true))
}

fn e02() -> Plot {
    difference_y(data::gistemp())
        .x("Date")
        .y("Anomaly")
        .positive_fill("red")
        .negative_fill("blue")
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
    // Plot.windowY(14, Plot.groupX({y1: find SJ, y2: find SF}, {x: "date",
    // y: "tmin"})): the mark's window runs before its grouping, so the
    // grouping (one row per date with each station's `tmin`) and then the
    // 14-day centred mean over y1 and y2 are spelled out; `tip: true` has no
    // port.
    let mut days: std::collections::BTreeMap<Value, (Value, Value)> = Default::default();
    for d in temperature().rows() {
        let entry = days.entry(d.value("date")).or_insert((Value::Null, Value::Null));
        match d.text("station").as_ref().map(|s| s.as_ref()) {
            Some("SJ") if entry.0.is_null() => entry.0 = d.value("tmin"),
            Some("SF") if entry.1.is_null() => entry.1 = d.value("tmin"),
            _ => {}
        }
    }
    let grouped: Dataset = days
        .into_iter()
        .map(|(date, (sj, sf))| {
            Table::empty()
                .with("date", date)
                .with("SJ", sj)
                .with("SF", sf)
        })
        .collect();
    let smooth = |data: &Dataset, field: &str| {
        gpui_plot::transform::window(data, field, 14, WindowReducer::Mean, Anchor::Middle)
    };
    let smoothed = smooth(&smooth(&grouped, "SJ"), "SF");
    Plot::plot()
        .x(|x| x.tick_format("%b"))
        .y(|y| y.grid(true).label("tmin"))
        .mark(rule_y([32.]))
        .mark(difference_y(smoothed).x("date").y1("SJ").y2("SF"))
}

fn e05() -> Plot {
    // Plot.shiftX("+1 year", {x: "Date", y: "Close"}) yields x1 = Date + 1
    // year and x2 = Date, and promotes the x domain [first + 1 year, last];
    // the accessor and the domain spell those out.
    let aapl = data::aapl();
    let year_later = |value: &Value| match value {
        Value::Date(date) => date
            .checked_add_months(chrono::Months::new(12))
            .map_or(Value::Null, Value::Date),
        _ => Value::Null,
    };
    let dates: Vec<Value> = aapl.rows().iter().map(|d| d.value("Date")).collect();
    let first = dates.iter().filter(|d| !d.is_null()).min().cloned();
    let last = dates.iter().filter(|d| !d.is_null()).max().cloned();
    let domain: Vec<Value> = first
        .map(|first| year_later(&first))
        .into_iter()
        .chain(last)
        .collect();
    difference_y(aapl)
        .x1(move |d: &Table| year_later(&d.value("Date")))
        .x2("Date")
        .y("Close")
        .plot()
        .x(|x| x.domain(domain))
        .y(|y| y.grid(true))
}
