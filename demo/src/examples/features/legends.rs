//! Legends: `features/legends.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("features", "legends", 1, e01),
        example("features", "legends", 2, e02),
        example("features", "legends", 3, e03),
    ]
}

fn e01() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(dot(data::olympians()).x("weight").y("height").stroke("sex"))
}

fn e02() -> Plot {
    Plot::plot()
        .color(|c| c.scheme("BuRd").legend(true))
        .mark(rule_y([0.]))
        .mark(
            dot(data::gistemp())
                .x("Date")
                .y("Anomaly")
                .stroke("Anomaly"),
        )
}

fn e03() -> Plot {
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
