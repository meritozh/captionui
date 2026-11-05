//! Pointer transform <VersionBadge version="0.6.7" />: `interactions/pointer.md`.
//!
//! There is no pointer transform: a mark wrapped in `Plot.pointer` draws only
//! the datum nearest the pointer, so until the pointer moves it draws
//! nothing. Those marks are left out below (noted where they were), and the
//! plot's built-in hover layer, on by default, stands in for them: it
//! highlights the nearest datum and shows a tooltip.
//!
//! `Plot.lineY` with an explicit `x` is ported as `line`: the library's
//! `line_y` ignores the `x` channel and spreads points by index.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("interactions", "pointer", 1, e01),
        example("interactions", "pointer", 2, e02),
        example("interactions", "pointer", 3, e03),
        example("interactions", "pointer", 4, e04),
        Example {
            height: 160.,
            ..example("interactions", "pointer", 5, e05)
        },
    ]
}

fn e01() -> Plot {
    Plot::plot()
        // Plot.dot(penguins, Plot.pointer({x: "culmen_length_mm", y:
        // "culmen_depth_mm", fill: "red", r: 8})): the hovered datum's
        // highlight.
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm"),
        )
}

/// `Plot.lineY(aapl, {x: "Date", y: "Close", tip: "x"}).plot()`
fn e02() -> Plot {
    line(data::aapl()).x("Date").y("Close").plot()
}

fn e03() -> Plot {
    Plot::plot().mark(rule_y([0.])).mark(
        line(data::bls_industry_unemployment())
            .x("date")
            .y("unemployed")
            .stroke("industry"),
    )
}

/// The bin reads only a field, not the docs' `x: (d) => Math.log10(d.Volume)`
/// accessor, so the log is precomputed into a `logVolume` field.
fn e04() -> Plot {
    let aapl: Dataset = data::aapl()
        .rows()
        .iter()
        .map(|d| {
            d.clone()
                .with("logVolume", Value::number(d.num("Volume").log10()))
        })
        .collect();
    Plot::plot()
        .x(|x| x.label("Daily volume (log₁₀)"))
        .mark(
            rect_y(aapl.clone())
                .x("logVolume")
                .bin(Thresholds::Count(40))
                .bin_output("y", GroupReducer::Count),
        )
        .mark(
            dot(aapl)
                .x("logVolume")
                .bin(Thresholds::Count(40))
                .bin_output("y", GroupReducer::Count)
                .stack_y()
                .stroke("red"),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .height(160.)
        .y(|y| y.axis(AxisAnchor::Right).grid(true).nice(true))
        .mark(line(data::aapl()).x("Date").y("Close"))
    // Plot.ruleX(aapl, Plot.pointerX({x: "Date", py: "Close", stroke: "red"})),
    // Plot.dot(aapl, Plot.pointerX({x: "Date", y: "Close", stroke: "red"})),
    // Plot.text(aapl, Plot.pointerX({px: "Date", py: "Close", dy: -17,
    //   frameAnchor: "top-left", text: …})): the pointer-driven marks, and
    // their `px`/`py` target channels, have no counterpart.
}
