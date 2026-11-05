//! Dot mark: `marks/dot.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "dot", 1, e01),
        example("marks", "dot", 2, e02),
        example("marks", "dot", 3, e03),
        example("marks", "dot", 4, e04),
        Example {
            height: 640.,
            ..example("marks", "dot", 5, e05)
        },
        example("marks", "dot", 6, e06),
        example("marks", "dot", 7, e07),
        Example {
            height: 660.,
            ..example("marks", "dot", 8, e08)
        },
        example("marks", "dot", 9, e09),
        example("marks", "dot", 10, e10),
        example("marks", "dot", 11, e11),
        example("marks", "dot", 12, e12),
    ]
}

/// `Plot.dot(cars, {x: "economy (mpg)", y: "power (hp)"}).plot({grid: true})`
fn e01() -> Plot {
    dot(data::cars())
        .x("economy (mpg)")
        .y("power (hp)")
        .plot()
        .grid(true)
}

fn e02() -> Plot {
    Plot::plot()
        .grid(true)
        .inset(10.)
        .x(|x| x.label("Fuel consumption (gallons per 100 miles)"))
        .y(|y| y.label("Horsepower"))
        .mark(
            dot(data::cars())
                .x(Channel::number(|d| 100. / d.num("economy (mpg)")))
                .y("power (hp)"),
        )
}

fn e03() -> Plot {
    Plot::plot()
        .y(|y| {
            y.grid(true)
                .tick_format("+f")
                .label("Surface temperature anomaly (°F)")
        })
        .color(|c| c.scheme("BuRd"))
        .mark(rule_y([0.]))
        .mark(
            dot(data::gistemp())
                .x("Date")
                .y("Anomaly")
                .stroke("Anomaly"),
        )
}

fn e04() -> Plot {
    Plot::plot()
        .grid(true)
        .x(|x| x.label("Daily change (%)").tick_format("+f").percent(true))
        .y(|y| y.type_name("log").label("Daily trading volume"))
        .mark(rule_x([0.]))
        .mark(
            dot(data::aapl())
                .x(Channel::number(|d| {
                    (d.num("Close") - d.num("Open")) / d.num("Open")
                }))
                .y("Volume")
                .r("Volume"),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .height(640.)
        .margin_left(60.)
        .grid(true)
        .x(|x| x.label("Carats"))
        .y(|y| y.label("Price ($)"))
        .r(|r| r.range([0., 20.]))
        .mark(
            dot(data::diamonds())
                .x("carat")
                .y("price")
                .bin(Thresholds::Count(100))
                .bin_output("r", GroupReducer::Count),
        )
}

/// `Plot.dot(alphabet, {x: "letter", r: "frequency"}).plot()`
fn e06() -> Plot {
    dot(data::alphabet()).x("letter").r("frequency").plot()
}

fn e07() -> Plot {
    Plot::plot()
        .x(|x| x.no_label().tick_padding(6.).tick_size(0.))
        .y(|y| y.percent(true))
        .mark(
            rule_x(data::alphabet())
                .x("letter")
                .y("frequency")
                .stroke_width(2.),
        )
        .mark(
            dot(data::alphabet())
                .x("letter")
                .y("frequency")
                .fill("currentColor")
                .r(4.),
        )
}

fn e08() -> Plot {
    let (stateage, ages) = data::stateage();
    let ages: Vec<Value> = ages.into_iter().map(Value::text).collect();
    // `xy`: each state's population share by age, Plot.normalizeX with the
    // sum basis, grouped by state.
    let xy = |mark: DotMark| {
        mark.x("population")
            .y("state")
            .z("state")
            .normalize_x(NormalizeBasis::Sum)
    };
    Plot::plot()
        .height(660.)
        .no_axes()
        .grid(true)
        .x(|x| {
            x.axis(AxisAnchor::Top)
                .label("Population (%)")
                .percent(true)
        })
        .color(|c| c.scheme("spectral").domain(ages).legend(true))
        .mark(rule_x([0.]))
        .mark(
            rule_y(stateage.clone())
                .x("population")
                .y("state")
                .z("state")
                .normalize_x(NormalizeBasis::Sum)
                .group_y()
                .group_output("x1", GroupReducer::Min)
                .group_output("x2", GroupReducer::Max)
                .sort_domain("y", "x1", false),
        )
        .mark(xy(dot(stateage.clone())).fill("age").tip_title("age"))
        .mark(
            text(stateage)
                .x("population")
                .y("state")
                .z("state")
                .normalize_x(NormalizeBasis::Sum)
                .select(Selector::Min("x".into()), Some("state"))
                .text_anchor(TextAnchor::End)
                .dx(-6.)
                .text("state"),
        )
}

fn e09() -> Plot {
    Plot::plot()
        .grid(true)
        .x(|x| x.label("Body mass (g)"))
        .y(|y| y.label("Flipper length (mm)"))
        .symbol(|s| s.legend(true))
        .mark(
            dot(data::penguins())
                .x("body_mass_g")
                .y("flipper_length_mm")
                .stroke("species")
                .symbol("species"),
        )
}

fn e10() -> Plot {
    dot_x([
        "circle", "cross", "diamond", "square", "star", "triangle", "wye",
    ])
    .fill("currentColor")
    .symbol(Channel::identity())
    .plot()
}

fn e11() -> Plot {
    dot_x([
        "circle",
        "plus",
        "times",
        "triangle2",
        "asterisk",
        "square2",
        "diamond2",
    ])
    .stroke("currentColor")
    .symbol(Channel::identity())
    .plot()
}

fn e12() -> Plot {
    Plot::plot()
        .aspect_ratio(1.)
        .x(|x| x.label("Age (years)"))
        .y(|y| {
            y.grid(true)
                .label("← Women · Men →")
                .label_anchor(LabelAnchor::Center)
                .tick_format(|v: &Value| format_number("", v.as_number().unwrap_or(0.).abs()))
        })
        .mark(
            dot(data::congress())
                .x(Channel::number(|d| {
                    2023.
                        - d.date("birthday")
                            .map(|b| b.year() as f64)
                            .unwrap_or(f64::NAN)
                }))
                .y(Channel::number(|d| {
                    if d.text("gender").is_some_and(|g| g == "M") {
                        1.
                    } else {
                        -1.
                    }
                }))
                .fill("gender")
                .tip_title("full_name")
                .stack_y2(),
        )
        .mark(rule_y([0.]))
}
