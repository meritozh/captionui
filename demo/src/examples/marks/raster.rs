//! Raster mark <VersionBadge version="0.6.2" />: `marks/raster.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    (1..=13)
        .map(|i| {
            example(
                "marks",
                "raster",
                i,
                [
                    e01, e02, e03, e04, e05, e06, e07, e08, e09, e10, e11, e12, e13,
                ][i - 1],
            )
        })
        .collect()
}

/// The docs' `volcano`: Maunga Whau's elevation, `{width: 87, height: 61, values}`.
fn volcano() -> (usize, usize, Dataset) {
    let json = data::json("volcano.json");
    let width = json["width"].as_u64().unwrap_or(87) as usize;
    let height = json["height"].as_u64().unwrap_or(61) as usize;
    let values: Vec<f64> = json["values"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| v.as_f64().unwrap_or(f64::NAN))
        .collect();
    (width, height, Dataset::values_of(values))
}

/// The docs' `grid`: `d3.cross(d3.range(10), d3.range(10), (x, y) => x * y)`,
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

/// The docs' `vapor`: NASA's water vapor for November 2022, 360 × 180, with
/// `99999.0` as missing.
fn vapor() -> Dataset {
    Dataset::values_of(
        gpui_plot::io::rows(
            &data::text("MYDAL2_M_SKY_WV_2022-11-01_rgb_360x180.csv"),
            b',',
        )
        .into_iter()
        .flatten()
        .map(|x| match x.trim() {
            "99999.0" => f64::NAN,
            x => x.parse().unwrap_or(f64::NAN),
        })
        .map(Value::number)
        .collect::<Vec<_>>(),
    )
}

/// `Plot.raster(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90", interpolate})`
fn ca55_raster(interpolate: &str) -> RasterMark {
    raster(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .fill("MAG_IGRF90")
        .interpolate(interpolate)
}

/// ```js
/// Plot.plot({
///   color: {label: "Elevation (m)", legend: true},
///   marks: [
///     Plot.raster(volcano.values, {width: volcano.width, height: volcano.height})
///   ]
/// })
/// ```
fn e01() -> Plot {
    let (width, height, values) = volcano();
    Plot::plot()
        .color(|c| c.label("Elevation (m)").legend(true))
        .mark(raster(values).grid(width, height))
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
fn e02() -> Plot {
    Plot::plot()
        .grid(true)
        .x(|x| x.domain([0., 10.]).label("column"))
        .y(|y| y.domain([0., 10.]).label("row"))
        .mark(grid_text().fill(ColorChannel::identity()))
}

/// ```js
/// Plot.plot({
///   marks: [
///     Plot.raster(grid.values, {
///       width: grid.width,
///       height: grid.height,
///       imageRendering: "pixelated" // to better show the grid
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
///
/// `imageRendering` has no port: the cells are painted flat, as pixelated.
fn e03() -> Plot {
    Plot::plot()
        .mark(raster(grid()).grid(10, 10))
        .mark(grid_text().fill("white"))
}

/// `Plot.dot(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90"}).plot({color: {type: "diverging"}})`
fn e04() -> Plot {
    dot(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .fill("MAG_IGRF90")
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// `Plot.line(ca55, {x: "LONGITUDE", y: "LATITUDE", stroke: "MAG_IGRF90", z: "LINE_NUMB-SEG"}).plot({color: {type: "diverging"}})`
fn e05() -> Plot {
    line(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .stroke("MAG_IGRF90")
        .z("LINE_NUMB-SEG")
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// `Plot.raster(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90", interpolate: "nearest"}).plot({color: {type: "diverging"}})`
fn e06() -> Plot {
    ca55_raster("nearest")
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// `Plot.raster(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90", interpolate: "barycentric"}).plot({color: {type: "diverging"}})`
fn e07() -> Plot {
    ca55_raster("barycentric")
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// `Plot.raster(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90", interpolate: "random-walk"}).plot({color: {type: "diverging"}})`
fn e08() -> Plot {
    ca55_raster("random-walk")
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// `Plot.raster(ca55, {x: "LONGITUDE", y: "LATITUDE", fill: "MAG_IGRF90", interpolate: "random-walk", blur: 5}).plot({color: {type: "diverging"}})`
fn e09() -> Plot {
    ca55_raster("random-walk")
        .blur(5.)
        .plot()
        .color(|c| c.type_name("diverging"))
}

/// ```js
/// Plot.plot({
///   color: {legend: true},
///   marks: [
///     Plot.raster(penguins, {x: "culmen_length_mm", y: "culmen_depth_mm", fill: "species", interpolate: "random-walk"}),
///     Plot.dot(penguins, {x: "culmen_length_mm", y: "culmen_depth_mm"})
///   ]
/// })
/// ```
fn e10() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(
            raster(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fill("species")
                .interpolate("random-walk"),
        )
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm"),
        )
}

/// `Plot.raster({fill: mandelbrot, x1: -2, x2: 1, y1: -1.164, y2: 1.164}).plot({aspectRatio: 1})`
///
/// ```js
/// function mandelbrot(x, y) {
///   for (let n = 0, zr = 0, zi = 0; n < 80; ++n) {
///     [zr, zi] = [zr * zr - zi * zi + x, 2 * zr * zi + y];
///     if (zr * zr + zi * zi > 4) return n;
///   }
/// }
/// ```
fn e11() -> Plot {
    fn mandelbrot(x: f64, y: f64) -> f64 {
        let (mut zr, mut zi) = (0., 0.);
        for n in 0..80 {
            (zr, zi) = (zr * zr - zi * zi + x, 2. * zr * zi + y);
            if zr * zr + zi * zi > 4. {
                return n as f64;
            }
        }
        f64::NAN // undefined
    }
    raster(Dataset::default())
        .sample(mandelbrot)
        .x1(Value::Number(-2.))
        .x2(Value::Number(1.))
        .y1(Value::Number(-1.164))
        .y2(Value::Number(1.164))
        .plot()
        .aspect_ratio(1.)
}

/// `Plot.raster({x1: -1, x2: 1, y1: -1, y2: 1, fill: (x, y) => Math.atan2(y, x)}).plot()`
fn e12() -> Plot {
    raster(Dataset::default())
        .x1(Value::Number(-1.))
        .x2(Value::Number(1.))
        .y1(Value::Number(-1.))
        .y2(Value::Number(1.))
        .sample(|x, y| y.atan2(x))
        .plot()
}

/// ```js
/// Plot.plot({
///   projection: "equal-earth",
///   color: {scheme: "BuPu", domain: [0, 6], legend: true, label: "Water vapor (cm)"},
///   marks: [
///     Plot.raster(vapor, {
///       fill: Plot.identity,
///       width: 360,
///       height: 180,
///       x1: -180,
///       y1: 90,
///       x2: 180,
///       y2: -90,
///       interpolate: "barycentric",
///       clip: "sphere"
///     }),
///     Plot.sphere({stroke: "black"})
///   ]
/// })
/// ```
///
/// `clip: "sphere"` has no port; `interpolate` on a dense grid does nothing
/// here, as the grid is already full.
fn e13() -> Plot {
    Plot::plot()
        .projection("equal-earth")
        .color(|c| {
            c.scheme("BuPu")
                .domain([0., 6.])
                .legend(true)
                .label("Water vapor (cm)")
        })
        .mark(
            raster(vapor())
                .fill(ColorChannel::identity())
                .grid(360, 180)
                .x1(Value::Number(-180.))
                .y1(Value::Number(90.))
                .x2(Value::Number(180.))
                .y2(Value::Number(-90.))
                .interpolate("barycentric"),
        )
        .mark(geo(gpui_plot::geo::sphere()).stroke("black"))
}
