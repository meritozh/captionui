//! Hexbin transform: `transforms/hexbin.md`.
//!
//! Gap: Plot's hexbin is a transform with outputs (`fill`, `r`, `stroke`,
//! `text`, reducers like `mode` and `min`) feeding any mark; the library's
//! is a `hexbin` mark that fills each hexagon by its count in the theme's
//! sequential colors. Every example here draws that mark: outputs other than
//! `fill: "count"`, the `color.scheme`, and the dot/text/circle mark the
//! transform feeds are dropped.

use super::centroid::{albers, albers_plot, geo_mesh, outlines, us};
use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "hexbin", 1, e01),
        example("transforms", "hexbin", 2, e02),
        example("transforms", "hexbin", 3, e03),
        example("transforms", "hexbin", 4, e04),
        example("transforms", "hexbin", 5, e05),
        example("transforms", "hexbin", 6, e06),
        example("transforms", "hexbin", 7, e07),
        example("transforms", "hexbin", 8, e08),
        example("transforms", "hexbin", 9, e09),
        example("transforms", "hexbin", 10, e10),
        example("transforms", "hexbin", 11, e11),
    ]
}

/// The olympians' weight and height, hexbinned.
fn olympians_hexbin() -> HexbinMark {
    hexbin(data::olympians()).x("weight").y("height")
}

/// `Plot.dot(olympians, Plot.hexbin({fill: "count"}, {x: "weight", y: "height"}))`
fn e01() -> Plot {
    olympians_hexbin().plot().color(|c| c.scheme("YlGnBu"))
}

/// `Plot.dot(olympians, Plot.hexbin({r: "count"}, {x: "weight", y: "height", binWidth}))`,
/// with the docs' default `binWidth` of 20.
fn e02() -> Plot {
    olympians_hexbin().bin_width(20.).plot()
}

/// `Plot.hexbin({fill: "count", r: "count"}, {x: "weight", y: "height", stroke: "currentColor"})`
fn e03() -> Plot {
    olympians_hexbin()
        .stroke("currentColor")
        .plot()
        .color(|c| c.scheme("YlGnBu"))
}

/// `Plot.hexbin({fill: "mode", r: "count"}, {x: "weight", y: "height", fill: "sex"})`
fn e04() -> Plot {
    olympians_hexbin().fill("sex").plot()
}

/// `Plot.hexbin({stroke: "mode", r: "count"}, {x: "weight", y: "height", z: "sex", stroke: "sex"})`
fn e05() -> Plot {
    olympians_hexbin().z("sex").stroke("sex").plot()
}

/// `Plot.text(olympians, Plot.hexbin({text: "count"}, {x: "weight", y: "height"}))`
fn e06() -> Plot {
    olympians_hexbin().plot()
}

/// Walmart's openings over the US: `Plot.dot(walmarts, Plot.hexbin({r: "count", fill: "min"}, {x: "longitude", y: "latitude", fill: "date"}))`.
///
/// Gap: as on `centroid`, the map is projected by hand (d3's Albers) and
/// its meshes drawn as lines.
fn e07() -> Plot {
    let us = us();
    let walmarts = Dataset::new(data::walmarts().rows().iter().filter_map(|d| {
        let (x, y) = albers(d.number("longitude")?, d.number("latitude")?)?;
        Some(
            d.clone()
                .with("x", Value::Number(x))
                .with("y", Value::Number(y)),
        )
    }));
    albers_plot()
        .r(|r| r.range([0., 16.]))
        .color(|c| c.scheme("spectral").label("First year opened").legend(true))
        .mark(geo_mesh(&us.mesh("states", true), albers).stroke_opacity(0.5))
        .mark(geo_mesh(&outlines(&us.features("nation")), albers))
        .mark(hexbin(walmarts).x("x").y("y").fill("date"))
}

/// `Plot.hexgrid()` under `Plot.dot(olympians, Plot.hexbin({r: "count"}, {…, fill: "currentColor"}))`
fn e08() -> Plot {
    Plot::plot()
        .mark(hexgrid())
        .mark(olympians_hexbin().fill("currentColor"))
}

/// `Plot.circle(olympians, Plot.hexbin({r: "count"}, {x: "weight", y: "height"}))`
fn e09() -> Plot {
    olympians_hexbin().plot()
}

/// `Plot.dot(olympians, Plot.hexbin({fill: "count"}, …)).plot({inset: 10, …})`
fn e10() -> Plot {
    olympians_hexbin()
        .plot()
        .inset(10.)
        .color(|c| c.scheme("YlGnBu"))
}

/// The hexbin with explicit `Plot.axisX()` and `Plot.axisY()` marks.
fn e11() -> Plot {
    Plot::plot()
        .color(|c| c.scheme("YlGnBu"))
        .mark(olympians_hexbin())
        .mark(axis_x())
        .mark(axis_y())
}
