//! Hexgrid mark <VersionBadge version="0.5.0" />: `marks/hexgrid.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![example("marks", "hexgrid", 1, e01)]
}

fn e01() -> Plot {
    // Plot.dot(penguins, Plot.hexbin({r: "count"}, {...})): the hexbin
    // transform feeding a dot's radius has no port; the hexbin mark bins the
    // same lattice instead.
    Plot::plot().mark(hexgrid()).mark(
        hexbin(data::penguins())
            .x("culmen_length_mm")
            .y("culmen_depth_mm")
            .fill("currentColor"),
    )
}
