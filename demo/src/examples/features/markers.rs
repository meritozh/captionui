//! Markers: `features/markers.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![example("features", "markers", 1, e01)]
}

/// The page's marker control starts at `circle`.
fn e01() -> Plot {
    Plot::plot().mark(rule_y([0.])).mark(
        line_y(data::crimea())
            .x("date")
            .y("deaths")
            .stroke("cause")
            .marker("circle"),
    )
}
