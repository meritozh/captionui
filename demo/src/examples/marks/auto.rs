//! Auto mark: `marks/auto.md`.
//!
//! `Plot.auto(data, {x, y, color})` is `Plot::auto(data, x, y, color)`. The
//! options it has no parameter for yet (`mark`, `size`, a `y` reducer or
//! `zero`, `fx`/`fy`) are noted where an example drops them.

use crate::prelude::*;
use gpui_plot::auto::{AutoChannel, AutoMark, AutoOptions};

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "auto", 1, e01),
        example("marks", "auto", 2, e02),
        example("marks", "auto", 3, e03),
        example("marks", "auto", 4, e04),
        example("marks", "auto", 5, e05),
        example("marks", "auto", 6, e06),
        example("marks", "auto", 7, e07),
        example("marks", "auto", 8, e08),
        example("marks", "auto", 9, e09),
        example("marks", "auto", 10, e10),
        example("marks", "auto", 11, e11),
        example("marks", "auto", 12, e12),
        example("marks", "auto", 13, e13),
        Example {
            height: 640.,
            ..example("marks", "auto", 14, e14)
        },
    ]
}

/// `Plot.auto(penguins, {x: "body_mass_g", y: "flipper_length_mm"}).plot()`
fn e01() -> Plot {
    Plot::auto(
        data::penguins(),
        Some("body_mass_g"),
        Some("flipper_length_mm"),
        None,
    )
}

/// `Plot.auto(aapl, {x: "Date", y: "Close"}).plot()`
fn e02() -> Plot {
    Plot::auto(data::aapl(), Some("Date"), Some("Close"), None)
}

/// `Plot.auto(olympians, {x: "weight"}).plot()`
fn e03() -> Plot {
    Plot::auto(data::olympians(), Some("weight"), None, None)
}

/// `Plot.auto(penguins, {x: "island"}).plot()`
fn e04() -> Plot {
    Plot::auto(data::penguins(), Some("island"), None, None)
}

/// `Plot.auto(penguins, {x: "body_mass_g", y: {reduce: null}}).plot()`
fn e05() -> Plot {
    Plot::auto_with(
        data::penguins(),
        AutoOptions {
            x: AutoChannel::field("body_mass_g"),
            y: AutoChannel::default().reduce(None),
            ..AutoOptions::default()
        },
    )
}

/// `Plot.auto(olympians, {x: "weight", y: "height", color: "count"}).plot()`
fn e06() -> Plot {
    Plot::auto(
        data::olympians(),
        Some("weight"),
        Some("height"),
        Some("count"),
    )
}

/// `Plot.auto(olympians, {x: "weight", y: "sex", color: "count"}).plot()`
fn e07() -> Plot {
    Plot::auto_with(
        data::olympians(),
        AutoOptions {
            x: AutoChannel::field("weight"),
            y: AutoChannel::field("sex"),
            color: AutoChannel::field("count"),
            ..AutoOptions::default()
        },
    )
}

/// `Plot.auto(penguins, {y: "island"}).plot()`
fn e08() -> Plot {
    Plot::auto(data::penguins(), None, Some("island"), None)
}

/// `Plot.auto(industries, {x: "date", y: "unemployed", color: "industry"}).plot()`
fn e09() -> Plot {
    Plot::auto(
        data::bls_industry_unemployment(),
        Some("date"),
        Some("unemployed"),
        Some("industry"),
    )
}

/// `Plot.auto(industries, {x: "date", y: "unemployed", color: "industry", mark: "area"}).plot()`
fn e10() -> Plot {
    // `mark: "area"` is not an option yet.
    Plot::auto(
        data::bls_industry_unemployment(),
        Some("date"),
        Some("unemployed"),
        Some("industry"),
    )
}

/// `Plot.auto(olympians, {x: "date_of_birth", y: {value: "height", reduce: "mean"}, color: "sex", mark: "line"})`
fn e11() -> Plot {
    Plot::auto_with(
        data::olympians(),
        AutoOptions {
            x: AutoChannel::field("date_of_birth"),
            y: AutoChannel::field("height").reduce(Some(GroupReducer::Mean)),
            color: AutoChannel::field("sex"),
            mark: Some(AutoMark::Line),
            ..AutoOptions::default()
        },
    )
    .color(|c| c.legend(true))
}

/// `Plot.auto(industries, {x: "date", y: {value: "unemployed", zero: true}, color: "industry"}).plot()`
fn e12() -> Plot {
    // `zero: true` on the y dimension, as the y scale's zero.
    Plot::auto(
        data::bls_industry_unemployment(),
        Some("date"),
        Some("unemployed"),
        Some("industry"),
    )
    .y(|y| y.zero(true))
}

/// `Plot.auto(aapl, {x: "Date", y: "Close", size: "Volume"}).plot()`
fn e13() -> Plot {
    // `size: "Volume"` is not an option yet.
    Plot::auto(data::aapl(), Some("Date"), Some("Close"), None)
}

fn e14() -> Plot {
    // `fx: "island", fy: "species"`, as the plot's facets.
    Plot::auto(
        data::penguins(),
        Some("body_mass_g"),
        Some("culmen_length_mm"),
        None,
    )
    .fx("island")
    .fy("species")
    .grid(true)
    .x(|x| x.ticks(5))
    .margin_right(70.)
}
