//! Delaunay marks <VersionBadge version="0.5.1" />: `marks/delaunay.md`.

use super::geo;
use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "delaunay", 1, e01),
        example("marks", "delaunay", 2, e02),
        example("marks", "delaunay", 3, e03),
        example("marks", "delaunay", 4, e04),
        example("marks", "delaunay", 5, e05),
        example("marks", "delaunay", 6, e06),
        example("marks", "delaunay", 7, e07),
    ]
}

fn e01() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(
            voronoi(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm")
                .fill("species")
                .fill_opacity(0.2)
                .stroke("var(--vp-c-bg)"),
        )
        .mark(frame())
        .mark(
            dot(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm")
                .fill("species"),
        )
}

fn e02() -> Plot {
    Plot::plot()
        .mark(
            voronoi_mesh(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm"),
        )
        .mark(
            dot(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm")
                .fill("species"),
        )
}

fn e03() -> Plot {
    Plot::plot()
        .mark(
            delaunay_mesh(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm")
                .z("species")
                .stroke("species")
                .stroke_opacity(0.5),
        )
        .mark(
            dot(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm")
                .fill("species"),
        )
}

fn e04() -> Plot {
    Plot::plot()
        .mark(
            hull(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm")
                .fill("species")
                .fill_opacity(0.2),
        )
        .mark(
            dot(data::penguins())
                .x("culmen_depth_mm")
                .y("culmen_length_mm")
                .stroke("species"),
        )
}

fn e05() -> Plot {
    Plot::plot().color(|c| c.legend(true)).mark(
        delaunay_link(data::penguins())
            .x("culmen_depth_mm")
            .y("culmen_length_mm")
            .stroke("body_mass_g")
            .stroke_width(1.5),
    )
}

fn e06() -> Plot {
    Plot::plot()
        .mark(voronoi(data::penguins()).x("body_mass_g").fill("species"))
        .mark(
            voronoi_mesh(data::penguins())
                .x("body_mass_g")
                .stroke("white")
                .stroke_opacity(1.),
        )
}

fn e07() -> Plot {
    let us = geo::us();
    // `projection: "albers"` is the lower-48 equirectangular stand-in, with
    // the x and y scales pinned to it so the dots and mesh land on the map.
    geo::lower_48_scales(Plot::plot())
        .mark(geo::nation(&us))
        .mark(
            dot(data::walmarts())
                .x("longitude")
                .y("latitude")
                .fill("currentColor")
                .r(1.),
        )
        .mark(voronoi_mesh(data::walmarts()).x("longitude").y("latitude"))
}
