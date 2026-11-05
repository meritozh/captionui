//! Rect mark: `marks/rect.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "rect", 1, e01),
        example("marks", "rect", 2, e02),
        example("marks", "rect", 3, e03),
        example("marks", "rect", 4, e04),
        example("marks", "rect", 5, e05),
        example("marks", "rect", 6, e06),
        Example {
            height: 640.,
            ..example("marks", "rect", 7, e07)
        },
        example("marks", "rect", 8, e08),
        example("marks", "rect", 9, e09),
        example("marks", "rect", 10, e10),
        example("marks", "rect", 11, e11),
        example("marks", "rect", 12, e12),
    ]
}

/// The docs' `bins`: `d3.bin()` of a thousand seeded normal samples, with
/// d3's default Sturges thresholds (nice steps of 0.5).
fn bins() -> Dataset {
    let mut normal = data::random_normal(42);
    let samples: Vec<f64> = (0..1000).map(|_| normal()).collect();
    let step = 0.5;
    let lo = (samples.iter().cloned().fold(f64::INFINITY, f64::min) / step).floor() as i64;
    let hi = (samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max) / step).ceil() as i64;
    Dataset::new((lo..hi).map(|i| {
        let x0 = i as f64 * step;
        let x1 = x0 + step;
        let length = samples.iter().filter(|&&s| s >= x0 && s < x1).count();
        Table::empty()
            .with("x0", x0)
            .with("x1", x1)
            .with("length", length as f64)
    }))
}

fn e01() -> Plot {
    Plot::plot().projection("albers-usa").mark(
        rect(data::countyboxes())
            .x1("0") // or ([x1]) => x1
            .y1("1") // or ([, y1]) => y1
            .x2("2") // or ([,, x2]) => x2
            .y2("3") // or ([,,, y2]) => y2
            .stroke("currentColor"),
    )
}

/// `Plot.rectY(bins, {x1: "x0", x2: "x1", y: "length"}).plot({round: true})`
fn e02() -> Plot {
    rect_y(bins())
        .x1("x0")
        .x2("x1")
        .y("length")
        .plot()
        .x(|x| x.round(true))
        .y(|y| y.round(true))
}

/// `Plot.rectY(d3.range(1000).map(d3.randomNormal()), Plot.binX()).plot()`
fn e03() -> Plot {
    let mut normal = data::random_normal(3);
    let samples: Vec<f64> = (0..1000).map(|_| normal()).collect();
    rect_y(samples)
        .bin(Thresholds::Auto)
        .bin_output("y", GroupReducer::Count)
        .plot()
}

fn e04() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .fill("sex")
                .bin(Thresholds::Auto)
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

fn e05() -> Plot {
    Plot::plot()
        .x(|x| x.round(true))
        .y(|y| y.round(true))
        .color(|c| c.legend(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .fill("sex")
                .mix_blend_mode(BlendMode::Multiply)
                .bin(Thresholds::Auto)
                .bin_output("y2", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

fn e06() -> Plot {
    Plot::plot()
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .fy("sex")
                .bin(Thresholds::Auto)
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

fn e07() -> Plot {
    Plot::plot()
        .height(640.)
        .margin_left(60.)
        .color(|c| c.scheme("YlGnBu").type_name("symlog"))
        .mark(
            rect(data::diamonds())
                .x("carat")
                .y("price")
                .bin(Thresholds::Count(100))
                .bin_output("fill", GroupReducer::Count),
        )
}

/// `order: "HeadCount", reverse: true` sorts the rows before stacking them
/// in input order; `insetLeft`/`insetRight: 0.2` are an all-sides
/// `inset(0.2)`.
fn e08() -> Plot {
    // Gap: the stack has no field `order` ("HeadCount", reversed), so the
    // rows are sorted by it up front and stacked in input order.
    let mut sorted: Vec<Table> = data::povcalnet().rows().to_vec();
    sorted.sort_by(|a, b| b.num("HeadCount").total_cmp(&a.num("HeadCount")));
    Plot::plot()
        .x(|x| x.label("Population (millions)"))
        .y(|y| {
            y.percent(true)
                .label("Proportion living on less than $30 per day (%)")
        })
        .mark(
            rect_y(Dataset::new(sorted))
                .filter(|d| {
                    d.text("CoverageType")
                        .is_some_and(|t| t.as_ref() == "N" || t.as_ref() == "A")
                })
                .x("ReqYearPopulation")
                .y2("HeadCount") // y2 to avoid stacking by y
                .stack_x()
                .tip_title(|d: &Table| {
                    Value::text(format!(
                        "{}\n{:.1}%",
                        d.text("CountryName").map_or("", |s| s.as_ref()),
                        d.num("HeadCount") * 100.
                    ))
                })
                .inset(0.2),
        )
        .mark(rule_y([0.]))
}

fn e09() -> Plot {
    let year2015: Dataset = data::seattle()
        .rows()
        .iter()
        .filter(|d| d.date("date").is_some_and(|date| date.year() == 2015))
        .cloned()
        .collect();
    Plot::plot()
        .aspect_ratio(1.)
        .y(|y| {
            y.ticks(12).tick_format(|v: &Value| {
                // Plot.formatMonth("en", "narrow")
                // d3's formatMonth wraps month 12 back to January.
                let month = (v.as_number().unwrap_or(0.) as usize) % 12;
                "JFMAMJJASOND"
                    .chars()
                    .nth(month)
                    .map(String::from)
                    .unwrap_or_default()
            })
        })
        .mark(
            rect(year2015)
                .x(Channel::number(|d| {
                    d.date("date").map_or(f64::NAN, |date| date.day() as f64)
                }))
                .y(Channel::number(|d| {
                    d.date("date").map_or(f64::NAN, |date| date.month0() as f64)
                }))
                .interval(Interval::Step(1.))
                .fill("temp_max")
                .inset(0.5),
        )
}

/// The docs' slider `r` starts at 4.
fn e10() -> Plot {
    let r = 4.;
    Plot::plot()
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .corner_radius(r)
                .bin(Thresholds::Count(10))
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// Per-corner `ry2: 4, ry1: -4` has no counterpart: a uniform corner radius,
/// clipped to the frame.
fn e11() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .fill("sex")
                .corner_radius(4.)
                .clip(true)
                .bin(Thresholds::Auto)
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// Per-corner `rx1y2: 10, rx1y1: -10` has no counterpart: a uniform corner
/// radius, clipped to the frame.
fn e12() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .fill("sex")
                .corner_radius(10.)
                .clip(true)
                .bin(Thresholds::Auto)
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}
