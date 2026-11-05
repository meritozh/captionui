//! Hexbin transform: `transforms/hexbin.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    (1..=11)
        .map(|i| {
            let f = [e01, e02, e03, e04, e05, e06, e07, e08, e09, e10, e11][i - 1];
            example("transforms", "hexbin", i, f)
        })
        .collect()
}

use GroupReducer::{Count, Min, Mode};

/// `Plot.dot(olympians, Plot.hexbin({fill: "count"}, {x: "weight", y: "height"}))`
fn e01() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .hexbin_output("fill", Count)
        .plot()
        .color(|c| c.scheme("YlGnBu"))
}

/// `Plot.dot(olympians, Plot.hexbin({r: "count"}, {x: "weight", y: "height", binWidth}))`
fn e02() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .hexbin_output("r", Count)
        .hexbin_width(20.)
        .plot()
}

/// `Plot.hexbin({fill: "count", r: "count"}, {x: "weight", y: "height", stroke: "currentColor"})`
fn e03() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .stroke("currentColor")
        .hexbin_output("fill", Count)
        .hexbin_output("r", Count)
        .plot()
        .color(|c| c.scheme("YlGnBu"))
}

/// `Plot.hexbin({fill: "mode", r: "count"}, {x: "weight", y: "height", fill: "sex"})`
fn e04() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .fill("sex")
        .z(Value::Null)
        .hexbin_output("fill", Mode)
        .hexbin_output("r", Count)
        .plot()
}

/// `Plot.hexbin({stroke: "mode", r: "count"}, {x: "weight", y: "height", z: "sex", stroke: "sex"})`
fn e05() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .z("sex")
        .stroke("sex")
        .hexbin_output("stroke", Mode)
        .hexbin_output("r", Count)
        .plot()
}

/// `Plot.text(olympians, Plot.hexbin({text: "count"}, {x: "weight", y: "height"}))`
fn e06() -> Plot {
    text(data::olympians())
        .x("weight")
        .y("height")
        .hexbin_output("text", Count)
        .plot()
}

/// The walmarts by decade opened, hexbinned over Albers.
fn e07() -> Plot {
    Plot::plot()
        .projection("albers")
        .r(|r| r.range([0., 16.]))
        .color(|c| c.scheme("spectral").label("First year opened").legend(true))
        .mark(geo(data::statemesh()).stroke_opacity(0.5))
        .mark(geo(data::nation()))
        .mark(
            dot(data::walmarts())
                .x("longitude")
                .y("latitude")
                .fill("date")
                .hexbin_output("r", Count)
                .hexbin_output("fill", Min),
        )
}

/// `Plot.hexgrid()` under `Plot.dot(olympians, Plot.hexbin({r: "count"}, {…, fill: "currentColor"}))`
fn e08() -> Plot {
    Plot::plot().mark(hexgrid()).mark(
        dot(data::olympians())
            .x("weight")
            .y("height")
            .fill("currentColor")
            .hexbin_output("r", Count),
    )
}

/// `Plot.circle(olympians, Plot.hexbin({r: "count"}, {x: "weight", y: "height"}))`
fn e09() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .symbol("circle")
        .fill("currentColor")
        .hexbin_output("r", Count)
        .plot()
}

/// `{inset: 10}` over e01.
fn e10() -> Plot {
    e01().inset(10.)
}

/// e01 with explicit `Plot.axisX()` and `Plot.axisY()`.
fn e11() -> Plot {
    e01().mark(axis_x()).mark(axis_y())
}
