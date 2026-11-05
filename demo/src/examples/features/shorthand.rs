//! Shorthand: `features/shorthand.md`.

use crate::prelude::*;
use gpui_plot::Dodge;

pub fn examples() -> Vec<Example> {
    let builds: [fn() -> Plot; 20] = [
        e01, e02, e03, e04, e05, e06, e07, e08, e09, e10, e11, e12, e13, e14, e15, e16, e17, e18,
        e19, e20,
    ];
    builds
        .into_iter()
        .enumerate()
        .map(|(i, build)| example("features", "shorthand", i + 1, build))
        .collect()
}

/// The page's `numbers`: AAPL's closing prices, January–February 2018.
const NUMBERS: [f64; 40] = [
    170.16, 172.53, 172.54, 173.44, 174.35, 174.55, 173.16, 174.59, 176.18, 177.90, 176.15, 179.37,
    178.61, 177.30, 177.30, 177.25, 174.51, 172.00, 170.16, 165.53, 166.87, 167.17, 166.00, 159.10,
    154.83, 163.09, 160.29, 157.07, 158.50, 161.95, 163.04, 169.79, 172.36, 172.05, 172.83, 171.80,
    173.67, 176.35, 179.10, 179.26,
];

/// The same prices against their dates, as `[date, close]` tuples.
const TIME_SERIES: [(&str, f64); 40] = [
    ("2018-01-02", 170.160004),
    ("2018-01-03", 172.529999),
    ("2018-01-04", 172.539993),
    ("2018-01-05", 173.440002),
    ("2018-01-08", 174.350006),
    ("2018-01-09", 174.550003),
    ("2018-01-10", 173.160004),
    ("2018-01-11", 174.589996),
    ("2018-01-12", 176.179993),
    ("2018-01-16", 177.899994),
    ("2018-01-17", 176.149994),
    ("2018-01-18", 179.369995),
    ("2018-01-19", 178.610001),
    ("2018-01-22", 177.300003),
    ("2018-01-23", 177.300003),
    ("2018-01-24", 177.250000),
    ("2018-01-25", 174.509995),
    ("2018-01-26", 172.000000),
    ("2018-01-29", 170.160004),
    ("2018-01-30", 165.529999),
    ("2018-01-31", 166.869995),
    ("2018-02-01", 167.169998),
    ("2018-02-02", 166.000000),
    ("2018-02-05", 159.100006),
    ("2018-02-06", 154.830002),
    ("2018-02-07", 163.089996),
    ("2018-02-08", 160.289993),
    ("2018-02-09", 157.070007),
    ("2018-02-12", 158.500000),
    ("2018-02-13", 161.949997),
    ("2018-02-14", 163.039993),
    ("2018-02-15", 169.789993),
    ("2018-02-16", 172.360001),
    ("2018-02-20", 172.050003),
    ("2018-02-21", 172.830002),
    ("2018-02-22", 171.800003),
    ("2018-02-23", 173.669998),
    ("2018-02-26", 176.350006),
    ("2018-02-27", 179.100006),
    ("2018-02-28", 179.259995),
];

/// The page's `matrix`: pairs of names.
const MATRIX: [[&str; 2]; 13] = [
    ["Jacob", "Olivia"],
    ["Mia", "Noah"],
    ["Noah", "Ava"],
    ["Ava", "Mason"],
    ["Olivia", "Noah"],
    ["Jacob", "Emma"],
    ["Ava", "Noah"],
    ["Noah", "Jacob"],
    ["Olivia", "Ava"],
    ["Mason", "Emma"],
    ["Jacob", "Mia"],
    ["Mia", "Jacob"],
    ["Emma", "Jacob"],
];

/// The page's `gene`, a DNA sequence; Plot iterates a string by character.
const GENE: &str = "AAAAGAGTGAAGATGCTGGAGACGAGTGAAGCATTCACTTTAGGGAAAGCGAGGCAAGAGCGTTTCAGAAGACGAAACCTGGTAGGTGCACTCACCACAG";

fn numbers() -> Dataset {
    Dataset::values_of(NUMBERS)
}

fn time_series() -> Dataset {
    Dataset::tuples(TIME_SERIES.map(|(date, close)| {
        let date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("an ISO date");
        [Value::date(date), Value::Number(close)]
    }))
}

fn matrix() -> Dataset {
    Dataset::tuples(MATRIX)
}

fn gene() -> Dataset {
    Dataset::values_of(GENE.chars().map(|c| Value::text(c.to_string())))
}

/// `Plot.lineY(numbers).plot()`
fn e01() -> Plot {
    line_y(numbers()).plot()
}

/// `Plot.areaY(numbers).plot()`
fn e02() -> Plot {
    area_y(numbers()).plot()
}

/// `Plot.rectY(numbers).plot()`
fn e03() -> Plot {
    rect_y(numbers()).plot()
}

/// `Plot.barY(numbers).plot()`
fn e04() -> Plot {
    bar_y(numbers()).plot()
}

/// `Plot.cellX(numbers).plot()`
fn e05() -> Plot {
    cell_x(numbers()).plot()
}

/// `Plot.dotX(numbers).plot()`
fn e06() -> Plot {
    dot_x(numbers()).plot()
}

/// `Plot.ruleX(numbers).plot()`
fn e07() -> Plot {
    rule_x(numbers()).plot()
}

/// `Plot.tickX(numbers).plot()`
fn e08() -> Plot {
    tick_x(numbers()).plot()
}

/// `Plot.vectorX(numbers).plot()`
fn e09() -> Plot {
    vector_x(numbers()).plot()
}

/// `Plot.textX(numbers).plot()`
fn e10() -> Plot {
    text_x(numbers()).plot()
}

/// `Plot.boxX(numbers).plot()`
fn e11() -> Plot {
    box_x(numbers()).plot()
}

/// `Plot.rectY(numbers, Plot.binX()).plot()`
fn e12() -> Plot {
    rect_y(numbers()).bin(Thresholds::Auto).plot()
}

/// `Plot.barY(gene, Plot.groupX()).plot()`
fn e13() -> Plot {
    bar_y(gene()).group_x().plot()
}

/// `Plot.dotX(numbers, Plot.dodgeY()).plot()`
fn e14() -> Plot {
    dot_x(numbers()).dodge(Dodge::default()).plot()
}

/// `Plot.line(timeSeries).plot()`
fn e15() -> Plot {
    line(time_series()).plot()
}

/// `Plot.area(timeSeries).plot()`: there is no plain `area` mark, so this
/// is `areaY`, reading the tuples' `x` and `y`.
fn e16() -> Plot {
    area_y(time_series()).x("0").y("1").plot()
}

/// `Plot.dot(timeSeries).plot()`
fn e17() -> Plot {
    dot(time_series()).plot()
}

/// `Plot.vector(timeSeries).plot()`
fn e18() -> Plot {
    vector(time_series()).plot()
}

/// `Plot.text(timeSeries).plot()`
fn e19() -> Plot {
    text(time_series()).plot()
}

/// `Plot.cell(matrix).plot()`
fn e20() -> Plot {
    cell(matrix()).plot()
}
