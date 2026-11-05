//! Stack transform: `transforms/stack.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "stack", 1, e01),
        example("transforms", "stack", 2, e02),
        example("transforms", "stack", 3, e03),
        example("transforms", "stack", 4, e04),
        example("transforms", "stack", 5, e05),
        example("transforms", "stack", 6, e06),
        example("transforms", "stack", 7, e07),
        example("transforms", "stack", 8, e08),
        example("transforms", "stack", 9, e09),
        Example {
            height: 640.,
            ..example("transforms", "stack", 10, e10)
        },
        Example {
            height: 200.,
            ..example("transforms", "stack", 11, e11)
        },
    ]
}

/// The RIAA revenue with each year's revenue in billions, the docs'
/// `y: {transform: (d) => d / 1000}`.
///
/// Gap: a scale's `transform` function is missing; the field is divided up
/// front.
fn riaa() -> Dataset {
    Dataset::new(data::riaa().rows().iter().map(|d| {
        let billions = d.number("revenue").map(|v| v / 1000.);
        d.clone().with(
            "revenue",
            billions.map(Value::Number).unwrap_or(Value::Null),
        )
    }))
}

/// The revenue axis the RIAA charts share.
fn revenue_axis(y: gpui_plot::ScaleOptions) -> gpui_plot::ScaleOptions {
    y.grid(true).label("Annual revenue (billions, adj.)")
}

/// `Plot.areaY(crimea, {x: "date", y: "deaths", fill: "cause"})`
fn e01() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(area_y(data::crimea()).x("date").y("deaths").fill("cause"))
        .mark(rule_y([0.]))
}

/// `Plot.rectY(crimea, {x: "date", y: "deaths", interval: "month", fill: "cause"})`
fn e02() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(
            rect_y(data::crimea())
                .x("date")
                .y("deaths")
                .interval(Interval::Month)
                .fill("cause"),
        )
        .mark(rule_y([0.]))
}

/// `Plot.barY(crimea, {x: "date", y: "deaths", fill: "cause"})` over a
/// monthly x, each tick its month's initial.
fn e03() -> Plot {
    Plot::plot()
        .x(|x| {
            x.interval(Interval::Month)
                .tick_format(|v: &Value| {
                    v.as_date()
                        .map(|d| format_time("%B", d).chars().take(1).collect())
                        .unwrap_or_default()
                })
                .no_label()
        })
        .y(|y| y.grid(true))
        .mark(bar_y(data::crimea()).x("date").y("deaths").fill("cause"))
        .mark(rule_y([0.]))
}

/// `Plot.barX(alphabet, Plot.stackX({x: "frequency", …}))` with each letter
/// labelled in its bar.
fn e04() -> Plot {
    Plot::plot()
        .x(|x| x.percent(true))
        .mark(
            bar_x(data::alphabet())
                .x("frequency")
                .fill_opacity(0.3)
                .inset(0.5)
                .stack_x(),
        )
        .mark(
            text_x(data::alphabet())
                .x("frequency")
                .text("letter")
                .inset(0.5)
                .stack_x(),
        )
        .mark(rule_x([0., 1.]))
}

/// `Plot.areaY(riaa, {x: "year", y: "revenue", z: "format", fill: "group", order})`,
/// with the docs' default `order` of `"appearance"`.
fn e05() -> Plot {
    Plot::plot()
        .y(revenue_axis)
        .color(|c| c.legend(true))
        .mark(
            area_y(riaa())
                .x("year")
                .y("revenue")
                .z("format")
                .fill("group")
                .stack_y()
                .stack_order(StackOrder::Appearance),
        )
        .mark(rule_y([0.]))
}

/// `Plot.stackY({order: "appearance", reverse}, …)`, with the docs' default
/// `reverse` of `true`.
fn e06() -> Plot {
    Plot::plot()
        .y(revenue_axis)
        .color(|c| c.legend(true))
        .mark(
            area_y(riaa())
                .x("year")
                .y("revenue")
                .z("format")
                .fill("group")
                .stack_y()
                .stack_order(StackOrder::Appearance)
                .stack_reverse(true),
        )
        .mark(rule_y([0.]))
}

/// `Plot.areaY(riaa, {…, order: "value"})`
fn e07() -> Plot {
    Plot::plot()
        .y(revenue_axis)
        .mark(
            area_y(riaa())
                .x("year")
                .y("revenue")
                .z("format")
                .fill("group")
                .stack_y()
                .stack_order(StackOrder::Value),
        )
        .mark(rule_y([0.]))
}

/// `Plot.areaY(riaa, {…, offset})`, with the docs' default `offset` of
/// `"wiggle"`.
fn e08() -> Plot {
    Plot::plot().y(revenue_axis).mark(
        area_y(riaa())
            .x("year")
            .y("revenue")
            .z("format")
            .fill("group")
            .stack_y()
            .stack_offset(Offset::Wiggle),
    )
}

/// `Plot.stackY({offset: "normalize", order: "group", reverse: true}, …)`
///
/// Gap: ordering a stack by another channel (`order: "group"`) is missing;
/// the port spells the formats out in their groups' order.
fn e09() -> Plot {
    let riaa = riaa();
    let mut formats: Vec<(Value, Value)> = Vec::new();
    for d in riaa.rows() {
        let format = d.value("format");
        if !formats.iter().any(|(f, _)| *f == format) {
            formats.push((format, d.value("group")));
        }
    }
    // A stable sort keeps each group's formats in their data order.
    formats.sort_by(|a, b| a.1.cmp(&b.1));
    Plot::plot()
        .y(|y| y.label("Annual revenue (%)").percent(true))
        .mark(
            area_y(riaa)
                .x("year")
                .y("revenue")
                .z("format")
                .fill("group")
                .stack_y()
                .stack_offset(Offset::Normalize)
                .stack_order(StackOrder::Explicit(
                    formats.into_iter().map(|(f, _)| f).collect(),
                ))
                .stack_reverse(true),
        )
        .mark(rule_y([0., 1.]))
}

/// Congress's members by age, men stacked up and women down:
/// `Plot.dot(congress, Plot.stackY2({x: (d) => 2023 - d.birthday.getUTCFullYear(), y: (d) => d.gender === "M" ? 1 : -1, …}))`.
fn e10() -> Plot {
    Plot::plot()
        .aspect_ratio(1.)
        .x(|x| x.label("Age (years)"))
        .y(|y| {
            y.grid(true)
                .label("← Women · Men →")
                .label_anchor(LabelAnchor::Center)
                .tick_format(|v: &Value| format!("{}", v.as_number().unwrap_or(0.).abs()))
        })
        .mark(
            dot(data::congress())
                .x(Channel::number(|d| {
                    2023. - d.date("birthday").map_or(f64::NAN, |b| b.year() as f64)
                }))
                .y(Channel::number(|d| {
                    if d.value("gender").label() == "M" {
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

/// The docs' `Likert` responses and their weights.
const LIKERT: [(&str, f64); 5] = [
    ("Strongly Disagree", -1.),
    ("Disagree", -1.),
    ("Neutral", 0.),
    ("Agree", 1.),
    ("Strongly Agree", 1.),
];

/// The survey as a diverging stacked bar per question, with the docs'
/// custom Likert stack `order` and `offset`.
fn e11() -> Plot {
    let order: Vec<Value> = LIKERT.iter().map(|(r, _)| Value::text(*r)).collect();
    // The docs' `Likert(responses).offset`: shift each stack so the
    // negative responses (and half the neutral ones) fall left of zero.
    let offset = Offset::custom(|stacks, x1, x2, z| {
        let weight = |z: &Value| {
            LIKERT
                .iter()
                .find(|(r, _)| *r == &*z.label())
                .map_or(0., |(_, w)| *w)
        };
        for stack in stacks {
            let k: f64 = stack
                .iter()
                .map(|&i| (x2[i] - x1[i]) * (1. - weight(&z[i])))
                .filter(|v| v.is_finite())
                .sum::<f64>()
                / 2.;
            for &i in stack {
                x1[i] -= k;
                x2[i] -= k;
            }
        }
    });
    Plot::plot()
        .x(|x| x.tick_format(|v: &Value| format!("{}", v.as_number().unwrap_or(0.).abs())))
        .color(|c| c.domain(order.clone()).scheme("RdBu").legend(true))
        .mark(
            bar_x(data::survey())
                .fy("Question")
                .fill("Response")
                .group_output("x", GroupReducer::Count)
                .group_z()
                .stack_order(StackOrder::Explicit(order))
                .stack_offset(offset),
        )
        .mark(rule_x([0.]))
}
