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

/// A raster's `{width, height}` grid of values as one record per pixel: the
/// pixel's center `x` and `y` across `[x1, x2]` and `[y1, y2]`, and its value
/// under [`gpui_plot::table::VALUE`]. Plot's raster mark takes the flat array
/// and its dimensions directly; this library's raster paints a cell per
/// positioned record.
fn pixels(
    width: usize,
    height: usize,
    values: impl IntoIterator<Item = f64>,
    [x1, x2]: [f64; 2],
    [y1, y2]: [f64; 2],
) -> Dataset {
    let (dx, dy) = ((x2 - x1) / width as f64, (y2 - y1) / height as f64);
    Dataset::new(values.into_iter().enumerate().map(|(i, value)| {
        Table::empty()
            .with("x", x1 + ((i % width) as f64 + 0.5) * dx)
            .with("y", y1 + ((i / width) as f64 + 0.5) * dy)
            .with(gpui_plot::table::VALUE, value)
    }))
}

/// A raster's `fill: (x, y) => …` sampled at the centers of a `width` by
/// `height` grid over `[x1, x2]` by `[y1, y2]`; Plot samples it once per
/// screen pixel.
fn sampled(
    width: usize,
    height: usize,
    xs: [f64; 2],
    ys: [f64; 2],
    fill: impl Fn(f64, f64) -> f64,
) -> Dataset {
    let (dx, dy) = (
        (xs[1] - xs[0]) / width as f64,
        (ys[1] - ys[0]) / height as f64,
    );
    let values = (0..width * height).map(|i| {
        let x = xs[0] + ((i % width) as f64 + 0.5) * dx;
        let y = ys[0] + ((i / width) as f64 + 0.5) * dy;
        fill(x, y)
    });
    pixels(width, height, values.collect::<Vec<_>>(), xs, ys)
}

/// The docs' `volcano`: Maunga Whau's elevation on an 87 × 61 grid.
fn volcano() -> (usize, usize, Vec<f64>) {
    let json = data::json("volcano.json");
    let width = json["width"].as_u64().unwrap_or(0) as usize;
    let height = json["height"].as_u64().unwrap_or(0) as usize;
    let values = json["values"]
        .as_array()
        .map(|values| values.iter().filter_map(|v| v.as_f64()).collect())
        .unwrap_or_default();
    (width, height, values)
}

/// The docs' `grid`: `d3.cross(d3.range(10), d3.range(10), (x, y) => x * y)`.
fn grid_values() -> Vec<f64> {
    (0..10)
        .flat_map(|x| (0..10).map(move |y| (x * y) as f64))
        .collect()
}

/// The `grid` as text records: `Plot.identity` and `(d, i) => i % width + 0.5`.
fn grid_text() -> TextMark {
    text(pixels(10, 10, grid_values(), [0., 10.], [0., 10.]))
        .x("x")
        .y("y")
        .text(|d: &Table| Value::text(format!("{}", d.num(gpui_plot::table::VALUE))))
}

/// The docs' `vapor`: NASA's water vapor for November 2022, 360 × 180, with
/// 99999 as missing.
fn vapor() -> Vec<f64> {
    gpui_plot::io::rows(
        &data::text("MYDAL2_M_SKY_WV_2022-11-01_rgb_360x180.csv"),
        b',',
    )
    .into_iter()
    .flatten()
    .map(|x| {
        if x == "99999.0" {
            f64::NAN
        } else {
            x.trim().parse().unwrap_or(f64::NAN)
        }
    })
    .collect()
}

fn ca55_raster() -> RasterMark {
    raster(data::ca55())
        .x("LONGITUDE")
        .y("LATITUDE")
        .fill("MAG_IGRF90")
}

fn e01() -> Plot {
    let (width, height, values) = volcano();
    Plot::plot()
        .color(|c| c.label("Elevation (m)").legend(true))
        .mark(
            raster(pixels(
                width,
                height,
                values,
                [0., width as f64],
                [0., height as f64],
            ))
            .x("x")
            .y("y")
            .fill(ColorChannel::identity()),
        )
}

fn e02() -> Plot {
    Plot::plot()
        .grid(true)
        .x(|x| x.domain([0., 10.]).label("column"))
        .y(|y| y.domain([0., 10.]).label("row"))
        .mark(grid_text().fill(ColorChannel::identity()))
}

/// `imageRendering: "pixelated"` has no counterpart: the cells are flat.
fn e03() -> Plot {
    Plot::plot()
        .mark(
            raster(pixels(10, 10, grid_values(), [0., 10.], [0., 10.]))
                .x("x")
                .y("y")
                .fill(ColorChannel::identity()),
        )
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

/// `interpolate: "nearest"` has no counterpart: each sample paints one cell.
fn e06() -> Plot {
    ca55_raster().plot().color(|c| c.type_name("diverging"))
}

/// `interpolate: "barycentric"` has no counterpart.
fn e07() -> Plot {
    ca55_raster().plot().color(|c| c.type_name("diverging"))
}

/// `interpolate: "random-walk"` has no counterpart.
fn e08() -> Plot {
    ca55_raster().plot().color(|c| c.type_name("diverging"))
}

/// `interpolate: "random-walk"` and `blur: 5` have no counterpart.
fn e09() -> Plot {
    ca55_raster().plot().color(|c| c.type_name("diverging"))
}

/// `interpolate: "random-walk"` has no counterpart.
fn e10() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(
            raster(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fill("species"),
        )
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm"),
        )
}

/// `Plot.raster({fill: mandelbrot, x1: -2, x2: 1, y1: -1.164, y2: 1.164}).plot({aspectRatio: 1})`
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
    raster(sampled(300, 233, [-2., 1.], [-1.164, 1.164], mandelbrot))
        .x("x")
        .y("y")
        .fill(ColorChannel::identity())
        .plot()
        .aspect_ratio(1.)
}

/// `Plot.raster({x1: -1, x2: 1, y1: -1, y2: 1, fill: (x, y) => Math.atan2(y, x)}).plot()`
fn e12() -> Plot {
    raster(sampled(160, 100, [-1., 1.], [-1., 1.], |x, y| y.atan2(x)))
        .x("x")
        .y("y")
        .fill(ColorChannel::identity())
        .plot()
}

/// Without `projection: "equal-earth"`, `clip: "sphere"` and the sphere, the
/// water vapor is drawn on linear lon/lat axes.
fn e13() -> Plot {
    Plot::plot()
        .color(|c| {
            c.scheme("BuPu")
                .domain([0., 6.])
                .legend(true)
                .label("Water vapor (cm)")
        })
        .mark(
            raster(pixels(360, 180, vapor(), [-180., 180.], [90., -90.]))
                .x("x")
                .y("y")
                .fill(ColorChannel::identity()),
        )
}
