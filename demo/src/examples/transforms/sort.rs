//! Sort transform: `transforms/sort.md`.

use super::centroid::{albers_usa, geo_centroid, geo_mesh, map_plot, us};
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

/// Each county's population by its five-digit FIPS id, from
/// `us-county-population.csv` read as text (its `state` and `county` codes
/// keep their leading zeros, as the docs' untyped `d3.csv` does).
fn county_population() -> Vec<(String, f64)> {
    let text = data::text("us-county-population.csv");
    let rows = gpui_plot::io::rows(&text, b',');
    let Some((header, rows)) = rows.split_first() else {
        return Vec::new();
    };
    let column = |name: &str| header.iter().position(|h| h == name);
    let (Some(state), Some(county), Some(population)) =
        (column("state"), column("county"), column("population"))
    else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            let id = format!("{}{}", row.get(state)?, row.get(county)?);
            Some((id, row.get(population)?.parse().ok()?))
        })
        .collect()
}

/// Counties as dots sized by population, the largest drawn first:
/// `Plot.dot(counties, Plot.geoCentroid({r: (d) => d.properties.population, …, sort: {channel: "-r"}}))`.
///
/// Gap: as on `centroid`, the map is projected by hand; sorting by a
/// channel (`{channel: "-r"}`) is a sort on the population field here.
fn e01() -> Plot {
    let us = us();
    let population = county_population();
    let counties = Dataset::new(us.features("counties").iter().filter_map(|county| {
        let (x, y) = geo_centroid(county, albers_usa)?;
        let people = population
            .iter()
            .find(|(id, _)| *id == county.id)
            .map(|(_, p)| Value::Number(*p))
            .unwrap_or(Value::Null);
        Some(
            Table::empty()
                .with("x", Value::Number(x))
                .with("y", Value::Number(y))
                .with("population", people),
        )
    }));
    map_plot()
        .mark(geo_mesh(&us.mesh("states", false), albers_usa).stroke_opacity(0.4))
        .mark(
            dot(counties)
                .x("x")
                .y("y")
                .r("population")
                .fill("currentColor")
                .stroke("white")
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
