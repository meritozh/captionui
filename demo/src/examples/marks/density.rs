//! Density mark <VersionBadge version="0.5.1" />: `marks/density.md`.

use super::geo;
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
const BANDWIDTH: f64 = 20.;
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
    // `thresholds: 4` has no port: the density mark always draws Plot's
    // default 20 thresholds.
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
                .stroke("blue"),
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
    // `bandwidth` has no port; the mark's fixed bandwidth is Plot's default
    // 20, the slider's initial value.
    let _ = BANDWIDTH;
    Plot::plot()
        .inset(20.)
        .mark(density(data::faithful()).x("waiting").y("eruptions"))
        .mark(dot(data::faithful()).x("waiting").y("eruptions"))
}

fn e03() -> Plot {
    // `thresholds` has no port; the mark's fixed 20 thresholds are Plot's
    // default, the slider's initial value.
    let _ = THRESHOLDS;
    Plot::plot()
        .inset(20.)
        .mark(density(data::faithful()).x("waiting").y("eruptions"))
        .mark(dot(data::faithful()).x("waiting").y("eruptions"))
}

fn e04() -> Plot {
    // `bandwidth: 10` and `thresholds: 4` have no port.
    Plot::plot()
        .height(100.)
        .inset(10.)
        .mark(
            density(data::faithful())
                .x("waiting")
                .stroke("blue")
                .stroke_width(0.25),
        )
        .mark(density(data::faithful()).x("waiting").stroke("blue"))
        .mark(
            dot(data::faithful())
                .x("waiting")
                .fill("currentColor")
                .r(1.5),
        )
}

fn e05() -> Plot {
    let us = geo::us();
    // `projection: "albers"` is the lower-48 equirectangular stand-in, with
    // the x and y scales pinned to it; `bandwidth: 10` has no port.
    geo::lower_48_scales(Plot::plot())
        .color(|c| c.scheme("YlGnBu"))
        .mark(
            density(data::walmarts())
                .x("longitude")
                .y("latitude")
                .fill("density"),
        )
        .mark(geo::statemesh(&us).stroke_opacity(0.3))
        .mark(geo::nation(&us))
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

fn e08() -> Plot {
    // `weight: (d) => d.sex === "FEMALE" ? 1 - skew : 1 + skew` has no port;
    // at the slider's initial skew of 0 every weight is 1, as here. The
    // dots' `strokeOpacity` channel is likewise 1 for every row.
    let opacity = (1. + SKEW) as f32;
    Plot::plot()
        .inset(10.)
        .color(|c| c.legend(true))
        .mark(
            density(sexed_penguins())
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
                .stroke_opacity(opacity),
        )
        .mark(frame())
}

fn e09() -> Plot {
    // One density per species, `fill: () => s` through the color scale.
    // `weight: (d) => d.species === s ? 1 : -1` and `thresholds: [0.05]`
    // have no port, so each draws the density of every penguin.
    let species = ["Adelie", "Chinstrap", "Gentoo"];
    let plot = Plot::plot().inset(10.).color(|c| c.legend(true));
    species
        .into_iter()
        .fold(plot, |plot, s| {
            plot.mark(
                density(data::penguins())
                    .x("flipper_length_mm")
                    .y("culmen_length_mm")
                    .fill(ColorChannel::value(move |_| Value::text(s)))
                    .fill_opacity(0.2),
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
