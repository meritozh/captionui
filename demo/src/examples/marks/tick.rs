//! Tick mark: `marks/tick.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "tick", 1, e01),
        example("marks", "tick", 2, e02),
        example("marks", "tick", 3, e03),
    ]
}

fn e01() -> Plot {
    let (stateage, ages) = data::stateage();
    Plot::plot()
        .x(|x| x.grid(true).label("Population (%)").percent(true))
        .y(|y| {
            y.domain(ages) // in age order
                .reverse(true)
                .label("Age (years)")
                .label_anchor(LabelAnchor::End)
        })
        .mark(rule_x([0.]))
        .mark(
            tick_x(stateage)
                .z("state")
                .x("population")
                .y("age")
                .normalize_x(NormalizeBasis::Sum),
        )
}

fn e02() -> Plot {
    Plot::plot()
        .x(|x| x.no_label())
        .y(|y| y.percent(true))
        .mark(
            bar_y(data::alphabet())
                .x("letter")
                .y("frequency")
                .fill_opacity(0.2),
        )
        .mark(tick_y(data::alphabet()).x("letter").y("frequency"))
        .mark(rule_y([0.]))
}

fn e03() -> Plot {
    Plot::plot().x(|x| x.domain([-4., 4.])).mark(
        tick_x(data::normal_samples(500, "x", 7))
            .x("x")
            .stroke_opacity(0.2),
    )
}
