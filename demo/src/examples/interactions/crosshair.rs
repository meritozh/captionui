//! Crosshair mark <VersionBadge version="0.6.7" />: `interactions/crosshair.md`.
//!
//! There is no crosshair mark: the plot's built-in hover layer
//! ([`gpui_plot::Interaction`], on by default) draws a crosshair at the
//! datum nearest the pointer, so each `Plot.crosshair` maps onto it.
//!
//! `Plot.lineY` with an explicit `x` is ported as `line`: the library's
//! `line_y` ignores the `x` channel and spreads points by index.

use crate::prelude::*;
use gpui_plot::Interaction;

pub fn examples() -> Vec<Example> {
    vec![
        example("interactions", "crosshair", 1, e01),
        example("interactions", "crosshair", 2, e02),
        example("interactions", "crosshair", 3, e03),
        example("interactions", "crosshair", 4, e04),
    ]
}

/// The built-in crosshair, Plot's `Plot.crosshair(…)`.
fn crosshair() -> Interaction {
    Interaction {
        enabled: true,
        crosshair: true,
        highlight: true,
    }
}

fn e01() -> Plot {
    Plot::plot()
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .stroke("sex"),
        )
        // Plot.crosshair(penguins, {x: "culmen_length_mm", y: "culmen_depth_mm"})
        .interaction(crosshair())
}

fn e02() -> Plot {
    Plot::plot()
        .mark(line(data::aapl()).x("Date").y("Close"))
        // Plot.crosshairX(aapl, {x: "Date", y: "Close"})
        .interaction(crosshair())
}

fn e03() -> Plot {
    Plot::plot()
        .mark(tick_x(data::penguins()).x("body_mass_g"))
        // Plot.crosshairX(penguins, {x: "body_mass_g"})
        .interaction(crosshair())
}

fn e04() -> Plot {
    Plot::plot()
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .stroke("sex"),
        )
        // Plot.crosshair(penguins, {x: "culmen_length_mm", y:
        // "culmen_depth_mm", color: "sex", opacity: 0.5}): the built-in
        // crosshair takes no color or opacity.
        .interaction(crosshair())
}
