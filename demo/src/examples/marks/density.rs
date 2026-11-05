//! Density mark <VersionBadge version="0.5.1" />: `marks/density.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "density", 1, e01),
        example("marks", "density", 2, e02),
        example("marks", "density", 3, e03),
        Example {
            height: 120.,
            ..example("marks", "density", 4, e04)
        },
        example("marks", "density", 5, e05),
        example("marks", "density", 6, e06),
        example("marks", "density", 7, e07),
        example("marks", "density", 8, e08),
        example("marks", "density", 9, e09),
    ]
}

/// The docs' sliders, at their initial values.
const SKEW: f64 = 0.;
const BANDWIDTH: f32 = 20.;
const THRESHOLDS: usize = 20;

/// `penguins.filter((d) => d.sex)`.
fn sexed_penguins() -> Dataset {
    data::penguins()
        .rows()
        .iter()
        .filter(|row| row.text("sex").is_some_and(|sex| !sex.is_empty()))
        .cloned()
        .collect()
}

fn e01() -> Plot {
    Plot::plot()
        .inset(10.)
        .mark(
            density(data::faithful())
                .x("waiting")
                .y("eruptions")
                .stroke("blue")
                .stroke_width(0.25),
        )
        .mark(
            density(data::faithful())
                .x("waiting")
                .y("eruptions")
                .stroke("blue")
                .thresholds(4),
        )
        .mark(
            dot(data::faithful())
                .x("waiting")
                .y("eruptions")
                .fill("currentColor")
                .r(1.5),
        )
}

fn e02() -> Plot {
    Plot::plot()
        .inset(20.)
        .mark(
            density(data::faithful())
                .x("waiting")
                .y("eruptions")
                .bandwidth(BANDWIDTH),
        )
        .mark(dot(data::faithful()).x("waiting").y("eruptions"))
}

fn e03() -> Plot {
    Plot::plot()
        .inset(20.)
        .mark(
            density(data::faithful())
                .x("waiting")
                .y("eruptions")
                .thresholds(THRESHOLDS),
        )
        .mark(dot(data::faithful()).x("waiting").y("eruptions"))
}

fn e04() -> Plot {
    Plot::plot()
        .height(100.)
        .inset(10.)
        .mark(
            density(data::faithful())
                .x("waiting")
                .stroke("blue")
                .stroke_width(0.25)
                .bandwidth(10.),
        )
        .mark(
            density(data::faithful())
                .x("waiting")
                .stroke("blue")
                .thresholds(4)
                .bandwidth(10.),
        )
        .mark(
            dot(data::faithful())
                .x("waiting")
                .fill("currentColor")
                .r(1.5),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .projection("albers")
        .color(|c| c.scheme("YlGnBu"))
        .mark(
            density(data::walmarts())
                .x("longitude")
                .y("latitude")
                .bandwidth(10.)
                .fill("density"),
        )
        .mark(geo(data::statemesh()).stroke_opacity(0.3))
        .mark(geo(data::nation()))
        .mark(
            dot(data::walmarts())
                .x("longitude")
                .y("latitude")
                .r(1.)
                .fill("currentColor"),
        )
}

fn e06() -> Plot {
    Plot::plot()
        .inset(10.)
        .grid(true)
        .x(|x| x.type_name("log"))
        .y(|y| y.type_name("log"))
        .mark(
            density(data::diamonds())
                .x("carat")
                .y("price")
                .stroke("density"),
        )
}

fn e07() -> Plot {
    Plot::plot()
        .mark(
            density(data::penguins())
                .fx("island")
                .x("flipper_length_mm")
                .y("culmen_length_mm")
                .stroke("density")
                .clip(true),
        )
        .mark(frame())
}

/// `(d) => d.sex === "FEMALE" ? 1 - skew : 1 + skew`.
fn skewed(d: &Table) -> f64 {
    if d.value("sex").label() == "FEMALE" {
        1. - SKEW
    } else {
        1. + SKEW
    }
}

fn e08() -> Plot {
    // Gap: `strokeOpacity` takes only a constant, not the docs' channel; at
    // the slider's initial skew of 0 the channel is 1 for every row, as here.
    Plot::plot()
        .inset(10.)
        .color(|c| c.legend(true))
        .mark(
            density(sexed_penguins())
                .weight(skewed)
                .x("flipper_length_mm")
                .y("culmen_length_mm")
                .stroke_opacity(0.5)
                .clip(true),
        )
        .mark(
            dot(sexed_penguins())
                .x("flipper_length_mm")
                .y("culmen_length_mm")
                .stroke("sex")
                .stroke_opacity((1. + SKEW) as f32),
        )
        .mark(frame())
}

fn e09() -> Plot {
    let species = ["Adelie", "Chinstrap", "Gentoo"];
    let plot = Plot::plot().inset(10.).color(|c| c.legend(true));
    species
        .into_iter()
        .fold(plot, |plot, s| {
            plot.mark(
                density(data::penguins())
                    .x("flipper_length_mm")
                    .y("culmen_length_mm")
                    .weight(move |d| {
                        if d.value("species").label() == s {
                            1.
                        } else {
                            -1.
                        }
                    })
                    .fill(ColorChannel::value(move |_| Value::text(s)))
                    .fill_opacity(0.2)
                    .threshold_values([0.05]),
            )
        })
        .mark(
            dot(data::penguins())
                .x("flipper_length_mm")
                .y("culmen_length_mm")
                .stroke("species"),
        )
        .mark(frame())
}
