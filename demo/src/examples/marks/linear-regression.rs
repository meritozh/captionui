//! Linear regression mark: `marks/linear-regression.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "linear-regression", 1, e01),
        example("marks", "linear-regression", 2, e02),
        example("marks", "linear-regression", 3, e03),
        example("marks", "linear-regression", 4, e04),
    ]
}

fn e01() -> Plot {
    Plot::plot()
        .mark(dot(data::cars()).x("weight (lb)").y("power (hp)"))
        .mark(
            linear_regression_y(data::cars())
                .x("weight (lb)")
                .y("power (hp)")
                .stroke("red"),
        )
}

/// The docs' slider `m` starts at zero, so `cars.slice(0, m)` is empty: the
/// faded dots alone.
fn e02() -> Plot {
    let m = 0;
    let first: Dataset = data::cars().rows().iter().take(m).cloned().collect();
    Plot::plot()
        .mark(
            dot(data::cars())
                .x("weight (lb)")
                .y("power (hp)")
                .fill("currentColor")
                .fill_opacity(0.2),
        )
        .mark(dot(first.clone()).x("weight (lb)").y("power (hp)"))
        .mark(
            linear_regression_y(first)
                .x("weight (lb)")
                .y("power (hp)")
                .stroke("red"),
        )
}

fn e03() -> Plot {
    Plot::plot()
        .grid(true)
        .color(|c| c.legend(true))
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fill("species"),
        )
        .mark(
            linear_regression_y(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .stroke("species"),
        )
        .mark(
            linear_regression_y(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm"),
        )
}

fn e04() -> Plot {
    Plot::plot()
        .mark(
            dot(data::cars())
                .x("weight (lb)")
                .y("power (hp)")
                .stroke_opacity(0.5)
                .r(2.),
        )
        .mark(
            linear_regression_y(data::cars())
                .x("weight (lb)")
                .y("power (hp)")
                .stroke("steelblue"),
        )
        .mark(
            linear_regression_x(data::cars())
                .x("weight (lb)")
                .y("power (hp)")
                .stroke("orange"),
        )
}
