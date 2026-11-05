//! Interactions: `features/interactions.md`.

use crate::prelude::*;
use gpui_plot::Interaction;

pub fn examples() -> Vec<Example> {
    vec![
        example("features", "interactions", 1, e01),
        example("features", "interactions", 2, e02),
    ]
}

/// `tip: true` with extra `channels: {name, sport}`: the plot's tooltip is
/// on by default; the extra channels have no counterpart, so the tip shows
/// the mark's own channels and the name as its title.
fn e01() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .stroke("sex")
        .tip_title("name")
        .plot()
}

/// `Plot.crosshair(olympians, {x: "weight", y: "height"})`: the plot's
/// pointer layer with its crosshair and no highlight.
fn e02() -> Plot {
    Plot::plot()
        .interaction(Interaction {
            enabled: true,
            crosshair: true,
            highlight: false,
        })
        .mark(dot(data::olympians()).x("weight").y("height").stroke("sex"))
}
