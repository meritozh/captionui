//! Sort transform: `transforms/sort.md`.

use crate::prelude::*;
use gpui_plot::{Dodge, DodgeAnchor};

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "sort", 1, e01),
        example("transforms", "sort", 2, e02),
        Example {
            height: 180.,
            ..example("transforms", "sort", 3, e03)
        },
    ]
}

/// Counties as dots sized by population, the largest drawn first:
/// `sort: {channel: "-r"}`.
///
/// Gap: a sort by channel is missing; sorting on the `population` field,
/// which the `r` channel reads, gives the same order.
fn e01() -> Plot {
    Plot::plot()
        .projection("albers-usa")
        .mark(geo(data::us().mesh("states", None)).stroke_opacity(0.4))
        .mark(
            dot(data::counties())
                .geo_centroid()
                .r(NumberChannel::scaled(|d| d.num("population")))
                .fill("currentColor")
                .stroke("var(--vp-c-bg)")
                .stroke_width(1.)
                .sort("population", true),
        )
}

/// Michigan's metro divisions drawn red, on top of the rest:
/// `sort: (d) => /, MI /.test(d.division)`.
///
/// Gap: a sort by an accessor is missing; the test is a field computed up
/// front, which the sort and the stroke both read.
fn e02() -> Plot {
    let bls = Dataset::new(data::bls_metro_unemployment().rows().iter().map(|d| {
        let michigan = d.value("division").label().contains(", MI ");
        d.clone().with("michigan", Value::Bool(michigan))
    }));
    Plot::plot()
        .y(|y| y.grid(true).label("Unemployment (%)"))
        .color(|c| c.domain([false, true]).colors(["#ccc", "red"]))
        .mark(rule_y([0.]))
        .mark(
            line(bls)
                .x("date")
                .y("unemployment")
                .z("division")
                .sort("michigan", false)
                .stroke("michigan"),
        )
}

/// `Plot.dotX(cars, Plot.dodgeY({x: "weight (lb)", title: "name", fill: "currentColor", sort: {channel: "x", order}}))`,
/// with the docs' default `order` of `"ascending"`.
fn e03() -> Plot {
    Plot::plot().height(180.).mark(
        dot_x(data::cars())
            .x("weight (lb)")
            .tip_title("name")
            .fill("currentColor")
            .sort("weight (lb)", false)
            .dodge(Dodge {
                anchor: DodgeAnchor::End,
                ..Dodge::default()
            }),
    )
}
