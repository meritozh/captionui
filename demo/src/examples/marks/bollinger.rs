//! Bollinger mark: `marks/bollinger.md`.

use chrono::NaiveDate;

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "bollinger", 1, e01),
        example("marks", "bollinger", 2, e02),
        example("marks", "bollinger", 3, e03),
        example("marks", "bollinger", 4, e04),
        example("marks", "bollinger", 5, e05),
    ]
}

/// The docs' `n` and `k` inputs, at their defaults.
const N: usize = 20;
const K: f32 = 2.;

/// `Plot.mapY(Plot.bollinger({n, k}), {y: "Close"})`: each close replaced by
/// the band's edge over the trailing `n` closes, the mean plus `k` standard
/// deviations, or missing until the window fills.
fn bollinger_band(data: &Dataset, n: usize, k: f64) -> Dataset {
    let closes: Vec<f64> = data.rows().iter().map(|d| d.num("Close")).collect();
    data.rows()
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let band = if i + 1 < n {
                f64::NAN
            } else {
                let window = &closes[i + 1 - n..=i];
                let mean = window.iter().sum::<f64>() / n as f64;
                // d3.deviation: the sample standard deviation.
                let variance =
                    window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
                mean + k * variance.sqrt()
            };
            d.clone().with("band", Value::number(band))
        })
        .collect()
}

fn date(year: i32, month: u32, day: u32) -> Value {
    Value::date(NaiveDate::from_ymd_opt(year, month, day).expect("a valid date"))
}

/// `Plot.bollingerY(aapl, {x: "Date", y: "Close", n, k}).plot()`
fn e01() -> Plot {
    bollinger_y(data::aapl())
        .x("Date")
        .y("Close")
        .bollinger(N, K)
        .plot()
}

fn e02() -> Plot {
    let aapl = data::aapl();
    // `Plot.mapY(Plot.bollinger(...))` has no map form: the bands are
    // computed into a field first.
    Plot::plot()
        .mark(
            line_y(bollinger_band(&aapl, 20, -2.))
                .x("Date")
                .y("band")
                .stroke("red"),
        )
        .mark(
            line_y(bollinger_band(&aapl, 20, 2.))
                .x("Date")
                .y("band")
                .stroke("green"),
        )
        .mark(line_y(aapl).x("Date").y("Close"))
}

fn e03() -> Plot {
    Plot::plot()
        .x(|x| x.domain([date(2014, 1, 1), date(2014, 6, 1)]))
        .y(|y| y.domain([68., 92.]).grid(true))
        .color(|c| c.domain([-1., 0., 1.]).colors(["red", "black", "green"]))
        .mark(
            bollinger_y(data::aapl())
                .x("Date")
                .y("Close")
                .stroke("none")
                .clip(true),
        )
        .mark(
            rule_x(data::aapl())
                .x("Date")
                .y1("Low")
                .y2("High")
                .stroke_width(1.)
                .clip(true),
        )
        .mark(
            rule_x(data::aapl())
                .x("Date")
                .y1("Open")
                .y2("Close")
                .stroke_width(3.)
                .stroke(ColorChannel::number(|d| {
                    let change = d.num("Close") - d.num("Open");
                    if change == 0. { 0. } else { change.signum() }
                }))
                .clip(true),
        )
}

/// `Plot.bollingerX(aapl, {y: "Date", x: "Close"}).plot()`
fn e04() -> Plot {
    bollinger_x(data::aapl()).y("Date").x("Close").plot()
}

/// `Plot.bollingerY(aapl.map((d) => d.Close)).plot()`
fn e05() -> Plot {
    // Plot's bollingerY defaults x to the index and y to the value; the port
    // spells both out over [index, close] tuples.
    let closes = Dataset::tuples(
        data::aapl()
            .rows()
            .iter()
            .enumerate()
            .map(|(i, d)| [i as f64, d.num("Close")]),
    );
    bollinger_y(closes).x("0").y("1").plot().no_labels()
}
