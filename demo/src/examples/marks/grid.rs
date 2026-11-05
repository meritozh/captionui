//! Grid mark <VersionBadge version="0.6.3" />: `marks/grid.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "grid", 1, e01),
        example("marks", "grid", 2, e02),
        example("marks", "grid", 3, e03),
        example("marks", "grid", 4, e04),
    ]
}

fn e01() -> Plot {
    // The docs' `atop` toggle, checked by default.
    let atop = true;
    let plot = Plot::plot()
        .x(|x| x.axis(AxisAnchor::Top).percent(true).grid(!atop))
        .mark(
            bar_x(data::alphabet())
                .x("frequency")
                .y("letter")
                .sort_domain("y", "width", false),
        );
    let plot = if atop {
        plot.mark(
            grid_x()
                .tick_interval(Interval::from(1.))
                .stroke("var(--vp-c-bg)")
                .stroke_opacity(0.5),
        )
    } else {
        plot
    };
    plot.mark(rule_x([0.]))
}

/// `Plot.gridX().plot({x: {type: "linear"}})`
fn e02() -> Plot {
    grid_x().plot().x(|x| x.type_name("linear"))
}

/// `Plot.gridX(d3.range(101), {stroke: Plot.identity, strokeOpacity: 1}).plot()`
fn e03() -> Plot {
    grid_x()
        .data((0..101).map(f64::from).collect::<Vec<_>>())
        .stroke(ColorChannel::identity())
        .stroke_opacity(1.)
        .plot()
}

/// `Plot.gridX({strokeDasharray: "2", strokeOpacity: 1}).plot({x: {type: "linear"}})`
fn e04() -> Plot {
    grid_x()
        .dash([2.])
        .stroke_opacity(1.)
        .plot()
        .x(|x| x.type_name("linear"))
}
