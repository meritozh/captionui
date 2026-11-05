//! Centroid transform: `transforms/centroid.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            height: 400.,
            ..example("transforms", "centroid", 1, e01)
        },
        example("transforms", "centroid", 2, e02),
        example("transforms", "centroid", 3, e03),
        example("transforms", "centroid", 4, e04),
        example("transforms", "centroid", 5, e05),
        example("transforms", "centroid", 6, e06),
    ]
}

/// This page's `statemesh`: `topojson.mesh(us, us.objects.states)`, every
/// state border once (the outer coast included).
fn statemesh() -> Geometry {
    data::us().mesh("states", None)
}

fn e01() -> Plot {
    Plot::plot()
        .projection("albers-usa")
        .mark(geo(statemesh()))
        .mark(
            text(data::states())
                .centroid()
                .text("name")
                .fill("currentColor")
                .stroke("var(--vp-c-bg)"),
        )
}

/// `Plot.voronoi(counties, Plot.centroid()).plot({projection: "albers"})`
fn e02() -> Plot {
    voronoi(data::counties())
        .centroid()
        .plot()
        .projection("albers")
}

fn e03() -> Plot {
    dot(data::counties())
        .centroid()
        .plot()
        .projection("albers-usa")
}

fn e04() -> Plot {
    dot(data::counties())
        .geo_centroid()
        .plot()
        .projection("albers-usa")
}

/// Gap: the hexbin is a mark, not a transform with outputs, so there is no
/// `r: "count"` dot; the hexbin mark fills each hexagon by its count. It
/// also ignores the plot's projection, so the centroids are pre-projected
/// (see [`crate::examples::pre_projected`]).
fn e05() -> Plot {
    let (counties, plot) = crate::examples::pre_projected(
        data::counties(),
        &crate::examples::albers(),
        crate::examples::geo_centroid,
    );
    plot.mark(hexbin(counties).x("x").y("y"))
}

/// Gap: `Plot.pointer` on a tip is an interaction; the snapshot shows no
/// pointer, so the tip draws nothing either way.
fn e06() -> Plot {
    Plot::plot()
        .projection("albers-usa")
        .mark(geo(statemesh()).stroke_opacity(0.2))
        .mark(geo(data::nation()))
        .mark(
            dot(data::states())
                .centroid()
                .fill("red")
                .stroke("var(--vp-c-bg-alt)"),
        )
}
