//! Line mark: `marks/line.md`.

use crate::prelude::*;
use gpui_plot::transform::{Anchor, WindowReducer};

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "line", 1, e01),
        example("marks", "line", 2, e02),
        example("marks", "line", 3, e03),
        example("marks", "line", 4, e04),
        example("marks", "line", 5, e05),
        example("marks", "line", 6, e06),
        example("marks", "line", 7, e07),
        example("marks", "line", 8, e08),
        example("marks", "line", 9, e09),
        example("marks", "line", 10, e10),
        example("marks", "line", 11, e11),
        example("marks", "line", 12, e12),
        example("marks", "line", 13, e13),
        example("marks", "line", 14, e14),
        example("marks", "line", 15, e15),
        example("marks", "line", 16, e16),
        example("marks", "line", 17, e17),
        example("marks", "line", 18, e18),
    ]
}

/// The docs' `stocks`: AAPL, AMZN, GOOG and IBM's daily prices, each row
/// tagged with its `Symbol`.
fn stocks() -> Dataset {
    [
        ("AAPL", data::aapl()),
        ("AMZN", data::amzn()),
        ("GOOG", data::goog()),
        ("IBM", data::ibm()),
    ]
    .into_iter()
    .flat_map(|(symbol, data)| {
        data.rows()
            .iter()
            .map(|row| row.clone().with("Symbol", symbol))
            .collect::<Vec<_>>()
    })
    .collect()
}

/// The docs' `beagle`: `[lon, lat]` rows read with `d3.csvParseRows`, plus
/// each row's index for `(d, i) => i`.
fn beagle() -> Dataset {
    let rows = gpui_plot::io::rows(&data::text("beagle.csv"), b',');
    Dataset::new(rows.into_iter().enumerate().filter_map(|(i, row)| {
        let lon = row.first()?.parse::<f64>().ok()?;
        let lat = row.get(1)?.parse::<f64>().ok()?;
        Some(
            Table::empty()
                .with("0", lon)
                .with("1", lat)
                .with("i", i as f64),
        )
    }))
}

/// `d3.shuffle(data.slice())`, seeded so the port draws the same tangle every
/// time.
fn shuffled(data: Dataset, seed: u64) -> Dataset {
    let mut rows = data.rows().to_vec();
    let mut state = seed.max(1);
    for i in (1..rows.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        rows.swap(i, (state % (i as u64 + 1)) as usize);
    }
    Dataset::new(rows)
}

/// The month of a row's `Date`, from zero: `d.Date.getUTCMonth()`.
fn month(d: &Table) -> u32 {
    d.date("Date").map_or(0, |date| date.month0())
}

/// `Plot.line(aapl, {x: "Date", y: "Close"}).plot({y: {grid: true}})`
fn e01() -> Plot {
    line(data::aapl())
        .x("Date")
        .y("Close")
        .plot()
        .y(|y| y.grid(true))
}

/// `Plot.line(aapl.map((d) => [d.Date, d.Close])).plot()`
fn e02() -> Plot {
    let pairs = data::aapl()
        .rows()
        .iter()
        .map(|d| [d.value("Date"), d.value("Close")])
        .collect::<Vec<_>>();
    line(Dataset::tuples(pairs)).plot()
}

/// `Plot.lineY(d3.cumsum({length: 600}, d3.randomNormal())).plot()`
fn e03() -> Plot {
    let mut normal = data::random_normal(3);
    let mut sum = 0.;
    let walk: Vec<f64> = (0..600)
        .map(|_| {
            sum += normal();
            sum
        })
        .collect();
    line_y(walk).plot()
}

/// `Plot.lineY(d3.shuffle(aapl.slice()), {x: "Date", y: "Close"}).plot() // 🌶️`
fn e04() -> Plot {
    line_y(shuffled(data::aapl(), 42))
        .x("Date")
        .y("Close")
        .plot()
}

/// `Plot.lineY(d3.shuffle(aapl.slice()), {x: "Date", y: "Close", sort: "Date"}).plot()`
fn e05() -> Plot {
    line_y(shuffled(data::aapl(), 42))
        .x("Date")
        .y("Close")
        .sort("Date", false)
        .plot()
}

fn e06() -> Plot {
    Plot::plot()
        .x(|x| x.label("Distance from stage start (km)"))
        .y(|y| y.label("Elevation (m)").grid(true))
        .mark(rule_y([0.]))
        .mark(line(data::tdf()).x("distance").y("elevation"))
}

fn e07() -> Plot {
    let every_five: Dataset = data::driving()
        .rows()
        .iter()
        .filter(|d| d.num("year") as i64 % 5 == 0)
        .cloned()
        .collect();
    Plot::plot()
        .inset(10.)
        .grid(true)
        .x(|x| x.label("Miles driven (per person-year)"))
        .y(|y| y.label("Cost of gasoline ($ per gallon)"))
        .mark(
            line(data::driving())
                .x("miles")
                .y("gas")
                .curve(Curve::CatmullRom)
                .marker("true"),
        )
        .mark(
            text(every_five)
                .x("miles")
                .y("gas")
                .text(|d: &Table| Value::text(format!("{}", d.num("year"))))
                .dy(-8.),
        )
}

fn e08() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true).label("Unemployment (%)"))
        .mark(rule_y([0.]))
        .mark(
            line(data::bls_metro_unemployment())
                .x("date")
                .y("unemployment")
                .z("division"),
        )
}

fn e09() -> Plot {
    Plot::plot()
        .y(|y| {
            y.type_name("log")
                .grid(true)
                .label("Change in price (%)")
                .tick_format(|v: &Value| {
                    let x = v.as_number().unwrap_or(1.);
                    format!("{:+}", ((x - 1.) * 100.).round() as i64)
                })
        })
        .mark(rule_y([1.]))
        .mark(
            line(stocks())
                .x("Date")
                .y("Close")
                .stroke("Symbol")
                .normalize_y(NormalizeBasis::First),
        )
        .mark(
            text(stocks())
                .x("Date")
                .y("Close")
                .z("Symbol")
                .text("Symbol")
                .text_anchor(TextAnchor::Start)
                .dx(3.)
                .normalize_y(NormalizeBasis::First)
                .select(Selector::Last, Some("Symbol")),
        )
}

fn e10() -> Plot {
    Plot::plot()
        .x(|x| x.no_label())
        .y(|y| y.grid(true).label("Unemployment (%)"))
        .mark(rule_y([0.]))
        .mark(
            line(data::bls_metro_unemployment())
                .x("date")
                .y("unemployment")
                .z("division")
                .stroke("unemployment"),
        )
}

/// `Plot.lineY(aapl, {x: "Date", y: "Close", stroke: "Close"}).plot({y: {grid: true}})`
fn e11() -> Plot {
    line_y(data::aapl())
        .x("Date")
        .y("Close")
        .stroke("Close")
        .plot()
        .y(|y| y.grid(true))
}

fn e12() -> Plot {
    let michigan = |d: &Table| d.text("division").is_some_and(|s| s.contains(", MI "));
    // `sort: {channel: "stroke"}`: draw the highlighted (true) series last.
    let mut rows = data::bls_metro_unemployment().rows().to_vec();
    rows.sort_by_key(|d| michigan(d));
    Plot::plot()
        .y(|y| y.grid(true).label("Unemployment (%)"))
        .color(|c| c.domain([false, true]).colors(["#ccc", "red"]))
        .mark(rule_y([0.]))
        .mark(
            line(Dataset::new(rows))
                .x("date")
                .y("unemployment")
                .z("division")
                .stroke(ColorChannel::value(move |d| Value::boolean(michigan(d)))),
        )
}

fn e13() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true).label("Temperature (°F)"))
        .mark(
            line(data::sftemp())
                .x("date")
                .y("low")
                .stroke("#4e79a7")
                .window(14, WindowReducer::Mean, Anchor::Middle),
        )
        .mark(
            line(data::sftemp())
                .x("date")
                .y("high")
                .stroke("#e15759")
                .window(14, WindowReducer::Mean, Anchor::Middle),
        )
        .mark(rule_y([32.])) // freezing
}

fn e14() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(line_y(data::aapl()).x("Date").y(Channel::number(|d| {
            if month(d) < 3 {
                f64::NAN
            } else {
                d.num("Close")
            }
        })))
}

fn e15() -> Plot {
    let after_march: Dataset = data::aapl()
        .rows()
        .iter()
        .filter(|d| month(d) >= 3)
        .cloned()
        .collect();
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(line_y(after_march).x("Date").y("Close").stroke_opacity(0.3))
        .mark(line_y(data::aapl()).x("Date").y(Channel::number(|d| {
            if month(d) < 3 {
                f64::NAN
            } else {
                d.num("Close")
            }
        })))
}

fn e16() -> Plot {
    let (stateage, ages) = data::stateage();
    Plot::plot()
        .x(|x| {
            x.domain(ages) // in age order
                .label("Age range (years)")
                .label_anchor(LabelAnchor::End)
                .label_arrow(true)
        })
        .y(|y| y.label("Population (%)").percent(true).grid(true))
        .mark(rule_y([0.]))
        .mark(
            line(stateage)
                .x("age")
                .y("population")
                .z("state")
                .stroke_width(1.)
                .normalize_y(NormalizeBasis::Sum),
        )
}

/// Without `projection: "equirectangular"` or the world's `land`, the
/// beagle's `[lon, lat]` track is drawn on linear x and y over the globe's
/// extent.
fn e17() -> Plot {
    Plot::plot()
        .x(|x| x.domain([-180., 180.]))
        .y(|y| y.domain([-90., 90.]))
        .mark(line(beagle()).stroke("red"))
        .mark(dot(Dataset::tuples([[-0.13, 51.5]])).fill("red")) // London
}

fn e18() -> Plot {
    Plot::plot()
        .x(|x| x.domain([-180., 180.]))
        .y(|y| y.domain([-90., 90.]))
        .mark(line(beagle()).stroke(ColorChannel::number(|d| d.num("i"))))
}
