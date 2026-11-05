//! Marks: `features/marks.md`.

use crate::prelude::*;
use gpui_plot::css;

pub fn examples() -> Vec<Example> {
    let builds: [fn() -> Plot; 24] = [
        e01, e02, e03, e04, e05, e06, e07, e08, e09, e10, e11, e12, e13, e14, e15, e16, e17, e18,
        e19, e20, e21, e22, e23, e24,
    ];
    builds
        .into_iter()
        .enumerate()
        .map(|(i, build)| example("features", "marks", i + 1, build))
        .collect()
}

/// The page's `linedata`: two sensors' readings by hour.
fn linedata() -> Dataset {
    let rows = [
        (0., 8., "A"),
        (0., 6., "B"),
        (1., 7., "A"),
        (1., 5., "B"),
        (2., 3., "A"),
        (2., 0., "B"),
        (3., 9., "A"),
        (3., 2., "B"),
    ];
    Dataset::new(rows.into_iter().map(|(hour, value, sensor)| {
        Table::empty()
            .with("hour", Value::Number(hour))
            .with("value", Value::Number(value))
            .with("sensor", Value::text(sensor))
    }))
}

/// The page's `timeseries`: world population by year, 2018 missing.
fn timeseries() -> Dataset {
    let rows = [
        (2014., 7295.290765),
        (2015., 7379.797139),
        (2016., 7464.022049),
        (2017., 7547.858925),
        (2019., 7713.468100),
        (2020., 7794.798739),
    ];
    Dataset::new(rows.into_iter().map(|(year, population)| {
        Table::empty()
            .with("year", Value::Number(year))
            .with("population", Value::Number(population))
    }))
}

/// `Plot.dot(gistemp, {x: "Date", y: "Anomaly"}).plot()`
fn e01() -> Plot {
    dot(data::gistemp()).x("Date").y("Anomaly").plot()
}

/// `Plot.lineY(gistemp, {x: "Date", y: "Anomaly"}).plot()`
fn e02() -> Plot {
    line_y(data::gistemp()).x("Date").y("Anomaly").plot()
}

/// `Plot.barX(alphabet, {x: "frequency", y: "letter"}).plot()`
fn e03() -> Plot {
    bar_x(data::alphabet()).x("frequency").y("letter").plot()
}

fn e04() -> Plot {
    Plot::plot()
        .mark(rule_y([0.]))
        .mark(area_y(data::aapl()).x("Date").y("Close").fill_opacity(0.2))
        .mark(line_y(data::aapl()).x("Date").y("Close"))
}

/// `[goog, aapl].map((stock) => Plot.lineY(stock, {x: "Date", y: "Close"}))`
fn e05() -> Plot {
    [data::goog(), data::aapl()]
        .into_iter()
        .fold(Plot::plot(), |plot, stock| {
            plot.mark(line_y(stock).x("Date").y("Close"))
        })
}

/// The docs fill the bars with an SVG `linearGradient` defined by a
/// function mark; there are no SVG defs or gradient paints here, so the bars
/// take the gradient's middle color.
fn e06() -> Plot {
    Plot::plot()
        .mark(
            bar_y(data::alphabet())
                .x("letter")
                .y("frequency")
                .fill("red"),
        )
        .mark(rule_y([0.]))
}

/// The page's `area` toggle starts on.
fn e07() -> Plot {
    let area = true;
    let mut plot = Plot::plot().mark(rule_y([0.]));
    if area {
        plot = plot.mark(area_y(data::aapl()).x("Date").y("Close").fill_opacity(0.2));
    }
    plot.mark(line_y(data::aapl()).x("Date").y("Close"))
}

fn e08() -> Plot {
    Plot::plot()
        .y(|y| y.type_name("log").domain([30., 300.]).grid(true))
        .mark(line_y(data::aapl()).x("Date").y("Close"))
}

/// `Plot.dot(aapl, {x: "Date", y: "Close"}).plot()`
fn e09() -> Plot {
    dot(data::aapl()).x("Date").y("Close").plot()
}

/// `Plot.lineY(aapl, {x: "Date", y: "Close"}).plot()`
fn e10() -> Plot {
    line_y(data::aapl()).x("Date").y("Close").plot()
}

/// `Plot.lineY(bls, {x: "date", y: "unemployment", z: "division"}).plot()`
fn e11() -> Plot {
    line_y(data::bls_metro_unemployment())
        .x("date")
        .y("unemployment")
        .z("division")
        .plot()
}

/// `Plot.lineY(linedata, {x: "hour", y: "value", stroke: "sensor"}).plot()`
fn e12() -> Plot {
    line_y(linedata())
        .x("hour")
        .y("value")
        .stroke("sensor")
        .plot()
}

fn e13() -> Plot {
    line_y(linedata())
        .x(|d: &Table| d.value("hour"))
        .y(|d: &Table| d.value("value"))
        .stroke(ColorChannel::value(|d| d.value("sensor")))
        .plot()
}

/// `Plot.lineY(["please", "don’t", "do", "this"]).plot()`
fn e14() -> Plot {
    line_y(["please", "don’t", "do", "this"]).plot()
}

fn e15() -> Plot {
    bar_y(timeseries())
        .x("year")
        .y("population")
        .plot()
        .x(|x| x.tick_format(""))
}

fn e16() -> Plot {
    rect_y(timeseries())
        .x("year")
        .y("population")
        .interval(Interval::Step(1.))
        .plot()
        .x(|x| x.tick_format(""))
}

fn e17() -> Plot {
    bar_y(timeseries())
        .x("year")
        .y("population")
        .plot()
        .x(|x| x.tick_format("").interval(1.))
}

/// `Plot.dot(gistemp, {x: "Date", y: "Anomaly", fill: "red"}).plot()`
fn e18() -> Plot {
    dot(data::gistemp())
        .x("Date")
        .y("Anomaly")
        .fill("red")
        .plot()
}

/// `Plot.lineY(gistemp.map((d) => d.Anomaly)).plot()`
fn e19() -> Plot {
    line_y(Dataset::values_of(data::gistemp().values("Anomaly"))).plot()
}

fn e20() -> Plot {
    bar_x(timeseries())
        .x("population")
        .y("year")
        .fill("purple")
        .plot()
        .y(|y| y.no_label().tick_format(""))
}

fn e21() -> Plot {
    bar_x(timeseries())
        .x("population")
        .y("year")
        .fill("year")
        .plot()
        .y(|y| y.no_label().tick_format(""))
}

fn e22() -> Plot {
    bar_x(timeseries())
        .x("population")
        .y("year")
        .fill(ColorChannel::number(|d| d.num("year")))
        .plot()
        .y(|y| y.no_label().tick_format(""))
}

/// `fill: (d) => d.year & 1 ? "red" : "currentColor"`
fn e23() -> Plot {
    let red = css::parse("red").expect("red is a color");
    bar_x(timeseries())
        .x("population")
        .y("year")
        .fill(move |d: &Table| {
            if d.num("year") as i64 & 1 == 1 {
                red
            } else {
                gpui_kit::Hsla::black()
            }
        })
        .plot()
        .y(|y| y.no_label().tick_format(""))
}

fn e24() -> Plot {
    bar_x(timeseries())
        .x("population")
        .y("year")
        .fill(ColorChannel::value(|d| {
            Value::text(if d.num("year") as i64 & 1 == 1 {
                "odd"
            } else {
                "even"
            })
        }))
        .plot()
        .y(|y| y.no_label().tick_format(""))
        .color(|c| c.legend(true))
}
