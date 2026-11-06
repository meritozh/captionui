//! Contour mark <VersionBadge version="0.6.2" />: `marks/contour.md`.

use std::f64::consts::PI;

use crate::prelude::*;

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

/// `volcano.ts`: Maungawhau's elevation, `{width: 87, height: 61, values}`.
fn volcano() -> (usize, usize, Dataset) {
    let volcano = data::json("volcano.json");
    let width = volcano["width"].as_u64().unwrap_or(87) as usize;
    let height = volcano["height"].as_u64().unwrap_or(61) as usize;
    let values: Vec<f64> = volcano["values"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|value| value.as_f64().unwrap_or(f64::NAN))
        .collect();
    (width, height, Dataset::values_of(values))
}

/// The page's `grid`: `d3.cross(d3.range(10), d3.range(10), (x, y) => x * y)`,
/// 10 × 10. Each record also holds its index, for `(d, i) => …`.
fn grid() -> Dataset {
    Dataset::new(
        (0..10)
            .flat_map(|x| (0..10).map(move |y| f64::from(x * y)))
            .enumerate()
            .map(|(i, value)| {
                Table::empty()
                    .with(gpui_plot::table::VALUE, value)
                    .with("index", i as f64)
            }),
    )
}

/// `x: (d, i) => i % grid.width + 0.5, y: (d, i) => Math.floor(i / grid.width) + 0.5`
fn grid_text() -> TextMark {
    text(grid())
        .text(Channel::identity())
        .x(Channel::number(|d| d.num("index") % 10. + 0.5))
        .y(Channel::number(|d| (d.num("index") / 10.).floor() + 0.5))
}

/// The page's `vapor`: a 360 × 180 grid of water vapor, `99999.0` for missing.
fn vapor() -> Dataset {
    data::cached("contour vapor", || {
        Dataset::values_of(
            data::text("MYDAL2_M_SKY_WV_2022-11-01_rgb_360x180.csv")
                .lines()
                .flat_map(|line| line.split(',').map(str::to_string).collect::<Vec<_>>())
                .map(|text| match text.trim() {
                    "99999.0" => f64::NAN,
                    text => text.parse().unwrap_or(f64::NAN),
                })
                .map(Value::number)
                .collect::<Vec<_>>(),
        )
    })
}

/// `Plot.contour(volcano.values, {width: volcano.width, height: volcano.height}).plot()`
fn e01() -> Plot {
    let (width, height, values) = volcano();
    contour(values).grid(width, height).plot()
}

/// ```js
/// Plot.plot({
///   color: {legend: true, label: "Elevation (m)"},
///   marks: [
///     Plot.contour(volcano.values, {
///       width: volcano.width,
///       height: volcano.height,
///       fill: Plot.identity,
///       stroke: "black"
///     })
///   ]
/// })
/// ```
fn e02() -> Plot {
    let (width, height, values) = volcano();
    Plot::plot()
        .color(|c| c.legend(true).label("Elevation (m)"))
        .mark(
            contour(values)
                .grid(width, height)
                .fill(ColorChannel::identity())
                .stroke("black"),
        )
}

/// ```js
/// Plot.plot({
///   grid: true,
///   x: {domain: [0, grid.width], label: "column"},
///   y: {domain: [0, grid.height], label: "row"},
///   marks: [
///     Plot.text(grid.values, {
///       text: Plot.identity,
///       fill: Plot.identity,
///       x: (d, i) => i % grid.width + 0.5,
///       y: (d, i) => Math.floor(i / grid.width) + 0.5
///     })
///   ]
/// })
/// ```
fn e03() -> Plot {
    Plot::plot()
        .grid(true)
        .x(|x| x.domain([0., 10.]).label("column"))
        .y(|y| y.domain([0., 10.]).label("row"))
        .mark(grid_text().fill(ColorChannel::identity()))
}

/// ```js
/// Plot.plot({
///   marks: [
///     Plot.contour(grid.values, {
///       width: grid.width,
///       height: grid.height,
///       fill: Plot.identity,
///       interval: 5
///     }),
///     Plot.text(grid.values, {
///       text: Plot.identity,
///       fill: "white",
///       x: (d, i) => i % grid.width + 0.5,
///       y: (d, i) => Math.floor(i / grid.width) + 0.5
///     })
///   ]
/// })
/// ```
fn e04() -> Plot {
    Plot::plot()
        .mark(
            contour(grid())
                .grid(10, 10)
                .fill(ColorChannel::identity())
                .contour_interval(5.),
        )
        .mark(grid_text().fill("white"))
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

/// `Plot.contour(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90", blur: 4}).plot({color: {type: "diverging"}})`
fn e07() -> Plot {
    contour(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .fill("MAG_IGRF90")
        .blur(4.)
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// ```js
/// Plot.plot({
///   projection: "equal-earth",
///   color: {scheme: "BuPu", domain: [0, 6], legend: true, label: "Water vapor (cm)"},
///   marks: [
///     Plot.contour(vapor, {
///       fill: Plot.identity,
///       width: 360,
///       height: 180,
///       x1: -180,
///       y1: 90,
///       x2: 180,
///       y2: -90,
///       blur: 1,
///       stroke: "black",
///       strokeWidth: 0.5,
///       clip: "sphere"
///     }),
///     Plot.sphere({stroke: "black"})
///   ]
/// })
/// ```
///
/// The contours are traced on the grid and then projected, so they stay
/// within the sphere that `clip: "sphere"` clips to.
fn e08() -> Plot {
    Plot::plot()
        .projection("equal-earth")
        .color(|c| {
            c.scheme("BuPu")
                .domain([0., 6.])
                .legend(true)
                .label("Water vapor (cm)")
        })
        .mark(
            contour(vapor())
                .fill(ColorChannel::identity())
                .grid(360, 180)
                .x1(Value::Number(-180.))
                .y1(Value::Number(90.))
                .x2(Value::Number(180.))
                .y2(Value::Number(-90.))
                .blur(1.)
                .stroke("black")
                .stroke_width(0.5)
                .clip_sphere(),
        )
        .mark(geo(gpui_plot::geo::sphere()).stroke("black"))
}

/// ```js
/// Plot.plot({
///   aspectRatio: 1,
///   x: {tickSpacing: 80, label: "x"},
///   y: {tickSpacing: 80, label: "y"},
///   color: {type: "diverging", legend: true, label: "sin(x) cos(y)"},
///   marks: [
///     Plot.contour({
///       fill: (x, y) => Math.sin(x) * Math.cos(y),
///       x1: 0,
///       y1: 0,
///       x2: 6 * Math.PI,
///       y2: 4 * Math.PI
///     })
///   ]
/// })
/// ```
///
/// `tickSpacing` has no port. `.sample` sets the value, and
/// `fill: Plot.identity` fills the isobands with it, as Plot's `fill: (x, y) => …` does.
fn e09() -> Plot {
    Plot::plot()
        .aspect_ratio(1.)
        .x(|x| x.label("x"))
        .y(|y| y.label("y"))
        .color(|c| c.type_name("diverging").legend(true).label("sin(x) cos(y)"))
        .mark(
            contour(Dataset::default())
                .sample(|x, y| x.sin() * y.cos())
                .fill(ColorChannel::identity())
                .x1(Value::Number(0.))
                .y1(Value::Number(0.))
                .x2(Value::Number(6. * PI))
                .y2(Value::Number(4. * PI)),
        )
}
