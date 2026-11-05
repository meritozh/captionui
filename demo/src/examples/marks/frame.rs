//! Frame mark: `marks/frame.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "frame", 1, e01),
        example("marks", "frame", 2, e02),
        example("marks", "frame", 3, e03),
        example("marks", "frame", 4, e04),
        example("marks", "frame", 5, e05),
        example("marks", "frame", 6, e06),
    ]
}

/// `Plot.frame().plot({x: {domain: [0, 1], grid: true}})`
fn e01() -> Plot {
    frame().plot().x(|x| x.domain([0., 1.]).grid(true))
}

fn e02() -> Plot {
    // The docs' `framed` toggle, checked by default.
    let framed = true;
    let plot = Plot::plot().grid(true).inset(10.);
    let plot = if framed { plot.mark(frame()) } else { plot };
    plot.mark(
        dot(data::penguins())
            .x("culmen_length_mm")
            .y("culmen_depth_mm")
            .fill("#eee"),
    )
    .mark(
        dot(data::penguins())
            .x("culmen_length_mm")
            .y("culmen_depth_mm")
            .fx("species"),
    )
}

/// `Plot.frame({stroke: "red"}).plot({x: {domain: [0, 1], grid: true}})`
fn e03() -> Plot {
    frame()
        .stroke("red")
        .plot()
        .x(|x| x.domain([0., 1.]).grid(true))
}

fn e04() -> Plot {
    Plot::plot()
        .inset(30.)
        // `fill: 0`, a constant through the color scale.
        .mark(frame().fill(ColorChannel::number(|_| 0.)))
        .mark(
            density(data::faithful())
                .x("waiting")
                .y("eruptions")
                .fill("density"),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .margin_left(80.)
        .inset(10.)
        .mark(frame().fy(Value::text("Gentoo")))
        .mark(dot(data::penguins()).x("body_mass_g").fy("species"))
}

fn e06() -> Plot {
    Plot::plot()
        .x(|x| x.domain([0., 1.]).grid(true))
        .mark(frame().stroke("red").axis_anchor(AxisAnchor::Bottom))
}
