//! Shift transform: `transforms/shift.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![example("transforms", "shift", 1, e01)]
}

/// `Plot.differenceY(aapl, Plot.shiftX(`${shift} days`, {x: "Date", y: "Close"}))`,
/// with the docs' default `shift` of 365.
///
/// Gap: `shiftX` producing an `x1`/`x2` pair the difference mark compares
/// across is missing — the library's `shift` writes one shifted field and
/// its difference mark compares two values on the same row. The port pairs
/// each day's close with the close `shift` days before it, interpolated
/// between trading days, which is the comparison Plot draws.
fn e01() -> Plot {
    let shift = 365;
    let aapl = data::aapl();
    let points: Vec<(i64, f64)> = aapl
        .rows()
        .iter()
        .filter_map(|d| Some((d.date("Date")?.timestamp(), d.number("Close")?)))
        .collect();
    let at = |t: i64| -> Option<f64> {
        let i = points.partition_point(|(time, _)| *time < t);
        if i == 0 || i >= points.len() {
            return None;
        }
        let ((t0, v0), (t1, v1)) = (points[i - 1], points[i]);
        let f = (t - t0) as f64 / (t1 - t0).max(1) as f64;
        Some(v0 + (v1 - v0) * f)
    };
    let paired = Dataset::new(aapl.rows().iter().filter_map(|d| {
        let t = d.date("Date")?.timestamp();
        let before = at(t - shift * 86_400)?;
        Some(d.clone().with("before", Value::Number(before)))
    }));
    difference_y(paired)
        .x("Date")
        .y1("before")
        .y2("Close")
        .plot()
        .y(|y| y.grid(true))
}
