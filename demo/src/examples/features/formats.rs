//! Formats: `features/formats.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("features", "formats", 1, e01),
        example("features", "formats", 2, e02),
    ]
}

/// The integers `0..n`, d3's `d3.range(n)`.
fn range(n: usize) -> Dataset {
    Dataset::values_of((0..n).map(|i| i as f64))
}

/// Plot's `Plot.formatWeekday()`: `0` is Sunday.
fn format_weekday(v: &Value) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    v.as_number()
        .and_then(|i| DAYS.get(i as usize))
        .map(|d| d.to_string())
        .unwrap_or_default()
}

/// Plot's `Plot.formatMonth()`: `0` is January.
fn format_month(v: &Value) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    v.as_number()
        .and_then(|i| MONTHS.get(i as usize))
        .map(|m| m.to_string())
        .unwrap_or_default()
}

/// `Plot.textX(d3.range(7)).plot({x: {tickFormat: Plot.formatWeekday()}})`
fn e01() -> Plot {
    text_x(range(7))
        .text(Channel::identity())
        .plot()
        .x(|x| x.tick_format(format_weekday))
}

/// `Plot.textX(d3.range(12)).plot({x: {tickFormat: Plot.formatMonth(), ticks: 12}})`
fn e02() -> Plot {
    text_x(range(12))
        .text(Channel::identity())
        .plot()
        .x(|x| x.tick_format(format_month).ticks(12))
}
