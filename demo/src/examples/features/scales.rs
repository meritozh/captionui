//! Scales: `features/scales.md`.

use chrono::NaiveDate;

use crate::prelude::*;
use gpui_plot::schemes;
use gpui_plot::{Anchor, NumberChannel, WindowReducer};

pub fn examples() -> Vec<Example> {
    let builds: [fn() -> Plot; 36] = [
        e01, e02, e03, e04, e05, e06, e07, e08, e09, e10, e11, e12, e13, e14, e15, e16, e17, e18,
        e19, e20, e21, e22, e23, e24, e25, e26, e27, e28, e29, e30, e31, e32, e33, e34, e35, e36,
    ];
    let heights = [
        (32, 25. * 27.),
        (33, 25. * 11.),
        (34, 25. * 2.),
        (35, 25. * 10.),
    ];
    builds
        .into_iter()
        .enumerate()
        .map(|(i, build)| {
            let example = example("features", "scales", i + 1, build);
            match heights.iter().find(|(index, _)| *index == i + 1) {
                Some(&(_, height)) => Example { height, ..example },
                None => example,
            }
        })
        .collect()
}

fn date(y: i32, m: u32, d: u32) -> Value {
    Value::date(NaiveDate::from_ymd_opt(y, m, d).expect("a valid date"))
}

/// The letters `"ABCDEFGHIJ"`, which Plot iterates by character.
fn letters() -> Vec<Value> {
    "ABCDEFGHIJ"
        .chars()
        .map(|c| Value::text(c.to_string()))
        .collect()
}

/// `d3.range(n)`.
fn range(n: usize) -> Dataset {
    Dataset::values_of((0..n).map(|i| i as f64))
}

/// `Plot.lineY(gistemp, {x: "Date", y: "Anomaly"}).plot()`
fn e01() -> Plot {
    line_y(data::gistemp()).x("Date").y("Anomaly").plot()
}

/// `Plot.ruleX(gistemp, {x: "Date", stroke: "Anomaly"}).plot()`
fn e02() -> Plot {
    rule_x(data::gistemp()).x("Date").stroke("Anomaly").plot()
}

fn e03() -> Plot {
    Plot::plot()
        .mark(rule_y([0.]))
        .mark(line_y(data::goog()).x("Date").y("Close").stroke("red"))
        .mark(line_y(data::aapl()).x("Date").y("Close").stroke("blue"))
}

/// `Plot.plot({x: {domain: [0, 100], grid: true}})`
fn e04() -> Plot {
    Plot::plot().x(|x| x.domain([0., 100.]).grid(true))
}

/// `Plot.plot({x: {domain: [100, 0], grid: true}})`
fn e05() -> Plot {
    Plot::plot().x(|x| x.domain([100., 0.]).grid(true))
}

/// `Plot.plot({x: {domain: [0, 100], reverse: true, grid: true}})`
fn e06() -> Plot {
    Plot::plot().x(|x| x.domain([0., 100.]).reverse(true).grid(true))
}

/// `Plot.plot({x: {domain: [new Date("2021-01-01"), new Date("2022-01-01")], grid: true}})`
fn e07() -> Plot {
    Plot::plot().x(|x| x.domain([date(2021, 1, 1), date(2022, 1, 1)]).grid(true))
}

/// `Plot.plot({x: {type: "utc", domain: [1609459200000, 1640995200000], grid: true}})`
fn e08() -> Plot {
    Plot::plot().x(|x| {
        x.type_name("utc")
            .domain([
                Value::date_from_millis(1_609_459_200_000),
                Value::date_from_millis(1_640_995_200_000),
            ])
            .grid(true)
    })
}

/// `new Date(2021, 0, 1)` is local time in the docs; here it is UTC.
fn e09() -> Plot {
    Plot::plot().x(|x| {
        x.type_name("time")
            .domain([date(2021, 1, 1), date(2022, 1, 1)])
            .grid(true)
    })
}

/// `Plot.plot({x: {domain: [1992, 2003], grid: true}})`
fn e10() -> Plot {
    Plot::plot().x(|x| x.domain([1992., 2003.]).grid(true))
}

/// `Plot.plot({x: {type: "log", domain: [1e0, 1e5], grid: true}})`
fn e11() -> Plot {
    Plot::plot().x(|x| x.type_name("log").domain([1e0, 1e5]).grid(true))
}

/// `Plot.plot({x: {type: "log", domain: [1e0, 1e5], tickFormat: ",", grid: true}})`
fn e12() -> Plot {
    Plot::plot().x(|x| {
        x.type_name("log")
            .domain([1e0, 1e5])
            .tick_format(",")
            .grid(true)
    })
}

/// `Plot.plot({x: {type: "log", base: 2, domain: [1e0, 1e4], ticks: 20, grid: true}})`
fn e13() -> Plot {
    Plot::plot().x(|x| {
        x.type_name("log")
            .base(2.)
            .domain([1e0, 1e4])
            .ticks(20)
            .grid(true)
    })
}

/// `Plot.plot({x: {type: "symlog", domain: [-10, 10], grid: true}})`
fn e14() -> Plot {
    Plot::plot().x(|x| x.type_name("symlog").domain([-10., 10.]).grid(true))
}

/// `Plot.plot({x: {type: "sqrt", domain: [0, 100], grid: true}})`
fn e15() -> Plot {
    Plot::plot().x(|x| x.type_name("sqrt").domain([0., 100.]).grid(true))
}

/// `Plot.plot({x: {type: "pow", exponent: 1 / 3, domain: [0, 100], grid: true}})`
fn e16() -> Plot {
    Plot::plot().x(|x| {
        x.type_name("pow")
            .exponent(1. / 3.)
            .domain([0., 100.])
            .grid(true)
    })
}

/// `Plot.plot({x: {type: "point", domain: "ABCDEFGHIJ", grid: true}})`
fn e17() -> Plot {
    Plot::plot().x(|x| x.type_name("point").domain(letters()).grid(true))
}

fn e18() -> Plot {
    cell(letters())
        .x(Channel::identity())
        .stroke("currentColor")
        .stroke_opacity(0.1)
        .plot()
        .x(|x| x.type_name("band").domain(letters()))
}

/// The page's sliders start at `padding: 0.1` and `align: 0.5`.
fn e19() -> Plot {
    let (padding, align) = (0.1, 0.5);
    Plot::plot()
        .grid(true)
        .margin_top(0.5)
        .x(|x| x.padding(padding).align(align).round(false))
        .mark(frame().stroke_opacity(0.3))
        .mark(
            tick_x(letters())
                .x(Channel::identity())
                .stroke("currentColor"),
        )
}

fn e20() -> Plot {
    let (padding, align) = (0.1, 0.5);
    Plot::plot()
        .grid(true)
        .margin_top(0.5)
        .x(|x| x.padding(padding).align(align).round(false))
        .mark(frame().stroke_opacity(0.3))
        .mark(
            cell(letters())
                .x(Channel::identity())
                .stroke("currentColor"),
        )
}

/// `Plot.plot({x: {type: "band", domain: d3.range(1992, 2003), grid: true}})`
fn e21() -> Plot {
    Plot::plot().x(|x| {
        x.type_name("band")
            .domain((1992..2003).map(|y| y as f64))
            .grid(true)
    })
}

/// A ramp of `d3.range(40)` through the color scale.
fn ramp(plot: Plot) -> Plot {
    plot.no_axes().padding(0.).mark(
        cell(range(40))
            .x(Channel::identity())
            .fill(ColorChannel::value(|d| d.value(gpui_plot::table::VALUE)))
            .inset(-0.5),
    )
}

/// The page's sequential scheme control starts at `turbo`.
fn e22() -> Plot {
    ramp(Plot::plot().color(|c| c.scheme("turbo")))
}

/// The page's interpolator control starts at `rgb`: a linear ramp from
/// steelblue to orange (d3's default interpolation is RGB).
fn e23() -> Plot {
    ramp(Plot::plot().color(|c| c.type_name("linear").colors(["steelblue", "orange"])))
}

/// The page's diverging scheme control starts at `rdbu`.
fn e24() -> Plot {
    ramp(Plot::plot().color(|c| c.type_name("linear").scheme("rdbu")))
}

fn e25() -> Plot {
    Plot::plot()
        .grid(true)
        .color(|c| c.type_name("diverging").scheme("BuRd"))
        .mark(rule_y([0.]))
        .mark(
            dot(data::gistemp())
                .x("Date")
                .y("Anomaly")
                .stroke("Anomaly"),
        )
}

/// The page's ordinal scheme control starts at `Observable10`.
fn e26() -> Plot {
    Plot::plot()
        .color(|c| c.type_name("ordinal").scheme("Observable10"))
        .mark(
            cell(letters())
                .x(Channel::identity())
                .fill(ColorChannel::value(|d| d.value(gpui_plot::table::VALUE))),
        )
}

/// The page's radius slider starts at 8.
fn e27() -> Plot {
    let radius = 8.;
    Plot::plot().r(|r| r.range([0., radius])).mark(
        dot(Dataset::values_of((1..11).map(|i| i as f64)))
            .x(Channel::identity())
            .r(NumberChannel::scaled(|d| d.num(gpui_plot::table::VALUE)))
            .fill("currentColor"),
    )
}

/// `y: {transform: (f) => (f - 32) * (5 / 9)}` converts °F to °C.
fn e28() -> Plot {
    Plot::plot()
        .y(|y| {
            y.grid(true)
                .label("Temperature (°C)")
                .transform(|f| (f - 32.) * (5. / 9.))
        })
        .mark(rule_y([32.]))
        .mark(line_y(data::sftemp()).x("date").y("high").window(
            7,
            WindowReducer::Mean,
            Anchor::Middle,
        ))
}

/// `Plot.binX({y: "proportion", fill: "x"}, …)`: there is no `proportion`
/// reducer nor a bin's `x` as a reducer here, so the bins count and each
/// takes its mean anomaly as its fill.
fn e29() -> Plot {
    Plot::plot()
        .y(|y| y.percent(true))
        .color(|c| c.scheme("BuRd"))
        .mark(
            rect_y(data::gistemp())
                .x("Anomaly")
                .fill("Anomaly")
                .bin(Thresholds::Auto)
                .bin_output("y", GroupReducer::Count)
                .bin_output("fill", GroupReducer::Mean),
        )
        .mark(rule_y([0.]))
}

/// The page's `intervaled` toggle starts on.
fn e30() -> Plot {
    let intervaled = true;
    let aapl = data::aapl();
    let last40 = Dataset::new(aapl.rows()[aapl.len().saturating_sub(40)..].to_vec());
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| {
            let x = x.tick_rotate(-90.).no_label();
            if intervaled {
                x.interval(Interval::Day)
            } else {
                x
            }
        })
        .y(|y| {
            y.transform(|d| d / 1e6)
                .label("Daily trade volume (millions)")
        })
        .mark(bar_y(last40).x("Date").y("Volume"))
        .mark(rule_y([0.]))
}

/// `Plot.plot({x: {domain: [new Date("1880-01-01"), new Date("2016-11-01")]}})`
fn e31() -> Plot {
    Plot::plot().x(|x| x.domain([date(1880, 1, 1), date(2016, 11, 1)]))
}

/// A d3 interpolator, from the library's sampled scheme stops.
fn interpolator(name: &str) -> Option<impl Fn(f64) -> String> {
    let stops = schemes::CONTINUOUS
        .iter()
        .find(|(scheme, _, _)| scheme.eq_ignore_ascii_case(name))?
        .2;
    Some(move |t: f64| {
        let at = t.clamp(0., 1.) * (schemes::STOPS - 1) as f64;
        let (i, f) = (at.floor() as usize, at.fract());
        let (a, b) = (stops[i], stops[(i + 1).min(schemes::STOPS - 1)]);
        let channel = |shift: u32| {
            let (a, b) = (((a >> shift) & 0xff) as f64, ((b >> shift) & 0xff) as f64);
            (a + (b - a) * f).round() as u8
        };
        format!("rgb({}, {}, {})", channel(16), channel(8), channel(0))
    })
}

/// `d3.ticks(0, 1, n)`: the nice steps from 0 to 1. Both the docs' counts,
/// 20 and 30, round to a step of 0.05.
fn unit_ticks(n: usize) -> Vec<f64> {
    debug_assert!((20..=30).contains(&n));
    (0..=20).map(|i| i as f64 * 0.05).collect()
}

/// One swatch row per scheme: `[t, name, color]` tuples.
fn swatches(rows: Vec<(f64, String, String)>) -> Dataset {
    Dataset::tuples(
        rows.into_iter()
            .map(|(t, name, color)| [Value::Number(t), Value::text(name), Value::text(color)]),
    )
}

/// The swatch figure the scheme tables share; Plot draws them 322 wide.
fn swatch_plot(rows: Vec<(f64, String, String)>, height: f32, inset: f32) -> Plot {
    Plot::plot()
        .height(height)
        .margin(0.)
        .margin_right(70.)
        .padding(0.)
        .x(|x| x.no_axis())
        .y(|y| y.axis(AxisAnchor::Right).tick_size(0.))
        .color(|c| c.type_name("identity"))
        .mark(cell(swatches(rows)).fill("2").inset(inset))
}

/// Continuous schemes sampled at `d3.ticks(0, 1, n)`; `reverse` reads the
/// scheme from 1 to 0, as the docs' `(t) => d3.interpolateRdBu(1 - t)`.
fn sampled(names: &[(&str, &str, bool)], n: usize) -> Vec<(f64, String, String)> {
    names
        .iter()
        .filter_map(|&(name, scheme, reverse)| {
            let i = interpolator(scheme)?;
            Some(
                unit_ticks(n)
                    .into_iter()
                    .map(|t| (t, name.to_string(), i(if reverse { 1. - t } else { t })))
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

fn e32() -> Plot {
    let names = [
        "Blues",
        "Greens",
        "Greys",
        "Purples",
        "Reds",
        "Oranges",
        "Turbo",
        "Viridis",
        "Magma",
        "Inferno",
        "Plasma",
        "Cividis",
        "Cubehelix",
        "Warm",
        "Cool",
        "BuGn",
        "BuPu",
        "GnBu",
        "OrRd",
        "PuBuGn",
        "PuBu",
        "PuRd",
        "RdPu",
        "YlGnBu",
        "YlGn",
        "YlOrBr",
        "YlOrRd",
    ]
    .map(|name| (name, name, false));
    swatch_plot(sampled(&names, 20), 25. * 27., 0.5)
}

fn e33() -> Plot {
    let names = [
        ("BrBG", "BrBG", false),
        ("PRGn", "PRGn", false),
        ("PiYG", "PiYG", false),
        ("PuOr", "PuOr", false),
        ("RdBu", "RdBu", false),
        ("RdGy", "RdGy", false),
        ("RdYlBu", "RdYlBu", false),
        ("RdYlGn", "RdYlGn", false),
        ("Spectral", "Spectral", false),
        ("BuRd", "RdBu", true),
        ("BuYlRd", "RdYlBu", true),
    ];
    swatch_plot(sampled(&names, 30), 25. * 11., 0.5)
}

fn e34() -> Plot {
    let names = [("rainbow", "Rainbow", false), ("sinebow", "Sinebow", false)];
    swatch_plot(sampled(&names, 30), 25. * 2., 0.5)
}

fn e35() -> Plot {
    let names = [
        "Accent",
        "Category10",
        "Dark2",
        "Observable10",
        "Paired",
        "Pastel1",
        "Pastel2",
        "Set1",
        "Set2",
        "Set3",
        "Tableau10",
    ];
    let rows = names
        .iter()
        .filter_map(|name| {
            let (_, colors) = schemes::CATEGORICAL.iter().find(|(n, _)| n == name)?;
            Some(
                colors
                    .iter()
                    .enumerate()
                    .map(move |(i, color)| (i as f64, name.to_string(), format!("#{color:06x}"))),
            )
        })
        .flatten()
        .collect();
    swatch_plot(rows, 25. * 10., 0.5)
}

/// `const color = Plot.scale({color: {scheme: "Tableau10", domain: ["apple",
/// "orange", "pear"]}})`, passed to the plot as its color options: there is
/// no standalone scale object, so the options are given directly.
fn e36() -> Plot {
    cell_x(["apple", "apple", "orange", "pear", "orange"])
        .plot()
        .color(|c| c.scheme("Tableau10").domain(["apple", "orange", "pear"]))
}
