//! Rule mark: `marks/rule.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    (1..=8)
        .map(|i| {
            example(
                "marks",
                "rule",
                i,
                [e01, e02, e03, e04, e05, e06, e07, e08][i - 1],
            )
        })
        .collect()
}

fn e01() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(rule_y([0.]).stroke("red"))
        .mark(line(data::aapl()).x("Date").y("Close"))
}

fn e02() -> Plot {
    Plot::plot()
        .y(|y| {
            y.type_name("log")
                .grid(true)
                .label("Change in price (%)")
                .tick_format(|v: &Value| {
                    let d = v.as_number().unwrap_or(1.);
                    format!("{:+}", ((d - 1.) * 100.).round() as i64)
                })
        })
        .mark(rule_y([1.]).stroke("red"))
        .mark(
            line(data::aapl())
                .x("Date")
                .y("Close")
                .normalize_y(NormalizeBasis::First),
        )
}

fn e03() -> Plot {
    Plot::plot().x(|x| x.domain([-4., 4.])).mark(
        rule_x(data::normal_samples(500, "x", 7))
            .x("x")
            .stroke_opacity(0.2),
    )
}

fn e04() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true).label("Temperature (°C)"))
        .color(|c| c.scheme("BuRd"))
        .mark(rule_y([0.]))
        .mark(
            rule_x(data::seattle())
                .x("date")
                .y1("temp_min")
                .y2("temp_max")
                .stroke("temp_min"),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .inset(6.)
        .no_labels()
        .y(|y| y.grid(true).label("Stock price ($)"))
        .color(|c| c.type_name("threshold").colors(["red", "green"]))
        .mark(rule_x(data::aapl()).x("Date").y1("Low").y2("High"))
        .mark(
            rule_x(data::aapl())
                .x("Date")
                .y1("Open")
                .y2("Close")
                .stroke(ColorChannel::number(|d| d.num("Close") - d.num("Open")))
                .stroke_width(4.),
        )
}

fn e06() -> Plot {
    Plot::plot()
        .mark(
            rule_x(data::simpsons())
                .x("season")
                .y("imdb_rating")
                .group_x()
                .group_output("y1", GroupReducer::Min)
                .group_output("y2", GroupReducer::Max),
        )
        .mark(
            dot(data::simpsons())
                .x("season")
                .y("imdb_rating")
                .fill("currentColor")
                .stroke("var(--vp-c-bg)"),
        )
        .mark(
            line_y(data::simpsons())
                .x("season")
                .y("imdb_rating")
                .stroke("red")
                .group_x()
                .group_output("y", GroupReducer::Median),
        )
}

fn e07() -> Plot {
    Plot::plot()
        .x(|x| x.no_label())
        .y(|y| y.percent(true))
        .mark(
            bar_y(data::alphabet())
                .x("letter")
                .y("frequency")
                .fill("blue"),
        )
        .mark(
            rule_x(data::alphabet())
                .x("letter")
                .y1(Channel::number(|d| d.num("frequency") * 0.9))
                .y2(Channel::number(|d| d.num("frequency") * 1.1))
                .marker("tick"),
        )
        .mark(rule_y([0.]))
}

fn e08() -> Plot {
    Plot::plot()
        .x(|x| x.no_label().tick_padding(6.).tick_size(0.))
        .y(|y| y.percent(true))
        .mark(
            rule_x(data::alphabet())
                .x("letter")
                .y("frequency")
                .stroke_width(2.)
                .marker_end("dot"),
        )
}
