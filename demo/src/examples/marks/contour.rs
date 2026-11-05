//! Contour mark <VersionBadge version="0.6.2" />: `marks/contour.md`.
//!
//! Plot's contour reads a grid, `{width, height, values}`, or samples a
//! function over `x1..x2 × y1..y2`; this library's contour reads rows with
//! x and y channels, so each grid here is spelled as one row per cell, at the
//! cell's center, holding its value under [`gpui_plot::table::VALUE`] for
//! `Plot.identity`.

use std::f64::consts::PI;

use crate::prelude::*;
use gpui_plot::table::VALUE;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "contour", 1, e01),
        example("marks", "contour", 2, e02),
        example("marks", "contour", 3, e03),
        example("marks", "contour", 4, e04),
        example("marks", "contour", 5, e05),
        example("marks", "contour", 6, e06),
        example("marks", "contour", 7, e07),
        example("marks", "contour", 8, e08),
        Example {
            height: 640.,
            ..example("marks", "contour", 9, e09)
        },
    ]
}

/// A `width × height` grid of `values`, row-major, as one row per cell at
/// its center in grid coordinates: `x = i % width + 0.5`,
/// `y = floor(i / width) + 0.5`, Plot's default `x1: 0, y1: 0, x2: width,
/// y2: height`.
fn cells(width: usize, values: impl IntoIterator<Item = f64>) -> Dataset {
    values
        .into_iter()
        .enumerate()
        .map(|(i, value)| {
            Table::empty()
                .with("x", Value::Number((i % width) as f64 + 0.5))
                .with("y", Value::Number((i / width) as f64 + 0.5))
                .with(VALUE, Value::Number(value))
        })
        .collect()
}

/// `volcano.ts`: Maungawhau's elevation, an 87 × 61 grid.
fn volcano() -> Dataset {
    let volcano = data::json("volcano.json");
    let width = volcano["width"].as_u64().unwrap_or(87) as usize;
    let values = volcano["values"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|value| value.as_f64().unwrap_or(f64::NAN));
    cells(width, values)
}

/// The page's `grid`: `d3.cross(d3.range(10), d3.range(10), (x, y) => x * y)`.
fn grid() -> Dataset {
    cells(
        10,
        (0..10).flat_map(|x| (0..10).map(move |y| f64::from(x * y))),
    )
}

/// The page's `vapor`: a 360 × 180 grid of water vapor, `99999.0` for
/// missing, spanning longitude -180..180 and latitude 90..-90.
fn vapor() -> Dataset {
    data::text("MYDAL2_M_SKY_WV_2022-11-01_rgb_360x180.csv")
        .lines()
        .flat_map(|line| line.split(',').map(str::to_string).collect::<Vec<_>>())
        .enumerate()
        .map(|(i, text)| {
            let value = match text.trim() {
                "99999.0" => f64::NAN,
                text => text.parse().unwrap_or(f64::NAN),
            };
            Table::empty()
                .with("x", Value::Number(-180. + (i % 360) as f64 + 0.5))
                .with("y", Value::Number(90. - (i / 360) as f64 - 0.5))
                .with(VALUE, Value::Number(value))
        })
        .collect()
}

/// `Plot.contour(volcano.values, {width: volcano.width, height: volcano.height}).plot()`
fn e01() -> Plot {
    contour(volcano())
        .x("x")
        .y("y")
        .fill(ColorChannel::identity())
        .plot()
}

fn e02() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true).label("Elevation (m)"))
        .mark(
            contour(volcano())
                .x("x")
                .y("y")
                .fill(ColorChannel::identity())
                .stroke("black"),
        )
}

fn e03() -> Plot {
    Plot::plot()
        .grid(true)
        .x(|x| x.domain([0., 10.]).label("column"))
        .y(|y| y.domain([0., 10.]).label("row"))
        .mark(
            text(grid())
                .text(Channel::identity())
                .fill(ColorChannel::identity())
                .x("x")
                .y("y"),
        )
}

fn e04() -> Plot {
    Plot::plot()
        .mark(
            contour(grid())
                .x("x")
                .y("y")
                .fill(ColorChannel::identity())
                .interval(Interval::from(5.)),
        )
        .mark(
            text(grid())
                .text(Channel::identity())
                .fill("white")
                .x("x")
                .y("y"),
        )
}

/// `Plot.dot(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90"}).plot({color: {type: "diverging"}})`
fn e05() -> Plot {
    dot(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .fill("MAG_IGRF90")
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// `Plot.contour(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90"}).plot({color: {type: "diverging"}})`
fn e06() -> Plot {
    contour(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .fill("MAG_IGRF90")
        .plot()
        .color(|c| c.type_name("diverging"))
}

fn e07() -> Plot {
    // `blur: 4` has no port.
    contour(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .fill("MAG_IGRF90")
        .plot()
        .color(|c| c.type_name("diverging"))
}

fn e08() -> Plot {
    // The equal-earth projection, `clip: "sphere"`, `blur: 1` and
    // Plot.sphere have no port: the grid is drawn on plain lon/lat scales,
    // the frame standing in for the sphere.
    Plot::plot()
        .no_axes()
        .aspect_ratio(1.)
        .x(|x| x.domain([-180., 180.]))
        .y(|y| y.domain([-90., 90.]))
        .color(|c| {
            c.scheme("BuPu")
                .domain([0., 6.])
                .legend(true)
                .label("Water vapor (cm)")
        })
        .mark(
            contour(vapor())
                .x("x")
                .y("y")
                .fill(ColorChannel::identity())
                .stroke("black")
                .stroke_width(0.5)
                .clip(true),
        )
        .mark(frame().stroke("black"))
}

fn e09() -> Plot {
    // `fill: (x, y) => Math.sin(x) * Math.cos(y)` over `x1: 0, y1: 0,
    // x2: 6π, y2: 4π`: the contour cannot sample a function, so the function
    // is sampled here on a grid as fine as Plot's default (one sample per
    // 2 pixels of a ~640-pixel frame).
    let (nx, ny) = (160, 106);
    let (x2, y2) = (6. * PI, 4. * PI);
    let samples: Dataset = (0..ny)
        .flat_map(|j| (0..nx).map(move |i| (i, j)))
        .map(|(i, j)| {
            let x = (f64::from(i) + 0.5) * x2 / f64::from(nx);
            let y = (f64::from(j) + 0.5) * y2 / f64::from(ny);
            Table::empty()
                .with("x", Value::Number(x))
                .with("y", Value::Number(y))
        })
        .collect();
    Plot::plot()
        .aspect_ratio(1.)
        .x(|x| x.label("x"))
        .y(|y| y.label("y"))
        .color(|c| c.type_name("diverging").legend(true).label("sin(x) cos(y)"))
        .mark(
            contour(samples)
                .x("x")
                .y("y")
                .fill(ColorChannel::number(|d| {
                    d.num("x").sin() * d.num("y").cos()
                })),
        )
}
