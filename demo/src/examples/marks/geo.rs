//! Geo mark <VersionBadge version="0.6.1" />: `marks/geo.md`.

use std::f64::consts::PI;

use crate::prelude::*;
use gpui_plot::GeoProjection;
use gpui_plot::geo;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "geo", 1, e01),
        example("marks", "geo", 2, e02),
        example("marks", "geo", 3, e03),
        example("marks", "geo", 4, e04),
        example("marks", "geo", 5, e05),
        Example {
            height: 1400.,
            ..example("marks", "geo", 6, e06)
        },
    ]
}

fn e01() -> Plot {
    Plot::plot()
        .projection("albers-usa")
        .color(|c| {
            c.type_name("quantile")
                .n(9)
                .scheme("blues")
                .label("Unemployment (%)")
                .legend(true)
        })
        .mark(
            geo(data::counties_unemployment())
                .fill("unemployment")
                .tip_title(|d: &Table| {
                    Value::text(format!(
                        "{} {}%",
                        d.value("name").label(),
                        d.value("unemployment").label()
                    ))
                }),
        )
}

fn e02() -> Plot {
    // The USGS earthquake feed is fetched live by the docs and is not in
    // demo/data, so only the land and the sphere are drawn.
    Plot::plot()
        .projection("equirectangular")
        .mark(geo(data::land()).fill("currentColor").fill_opacity(0.2))
        .mark(geo(geo::sphere()))
}

fn e03() -> Plot {
    Plot::plot()
        .inset(2.)
        .projection(
            GeoProjection::named("orthographic")
                .expect("a projection")
                .rotate([0., -30., 20.]),
        )
        .mark(
            geo(geo::sphere())
                .fill("var(--vp-c-bg-alt)")
                .stroke("currentColor"),
        )
        .mark(geo(geo::graticule()).stroke_opacity(0.3))
}

/// `d3.geoCircle().center(center).radius(radius)`: the polygon of points
/// `radius` degrees from `center` on the sphere.
fn geo_circle(center: [f64; 2], radius: f64) -> Geometry {
    let (lon0, lat0) = (center[0].to_radians(), center[1].to_radians());
    let r = radius.to_radians();
    let ring = (0..=72)
        .map(|step| {
            let bearing = 2. * PI * f64::from(step) / 72.;
            let lat = (lat0.sin() * r.cos() + lat0.cos() * r.sin() * bearing.cos()).asin();
            let lon = lon0
                + (bearing.sin() * r.sin() * lat0.cos()).atan2(r.cos() - lat0.sin() * lat.sin());
            [lon.to_degrees(), lat.to_degrees()]
        })
        .collect();
    Geometry::Polygon(vec![ring])
}

fn e04() -> Plot {
    let radii: Vec<f64> = [0.5, 179.5]
        .into_iter()
        .chain((10..=170).step_by(10).map(f64::from))
        .collect();
    // Gap: the color scale has no `transform` (degrees to km), so the stroke
    // channel converts each radius itself.
    Plot::plot()
        .projection(
            GeoProjection::named("equal-earth")
                .expect("a projection")
                .rotate([90., 0., 0.]),
        )
        .color(|c| c.legend(true).label("Distance from Tonga (km)").zero(true))
        .mark(geo(data::land()))
        .mark(
            geo(radii)
                .geometry(|d: &Table| {
                    geo_circle([-175.38, -20.57], d.number(gpui_plot::table::VALUE).unwrap_or(0.)).into_value()
                })
                .stroke(ColorChannel::number(|d: &Table| {
                    111.2 * d.number(gpui_plot::table::VALUE).unwrap_or(0.)
                }))
                .stroke_width(2.),
        )
        .mark(geo(geo::sphere()))
}

fn e05() -> Plot {
    Plot::plot()
        .projection("albers-usa")
        .mark(geo(data::states()).stroke_opacity(0.1).tip_title("name"))
        .mark(geo(data::nation()))
        .mark(
            dot(data::states())
                .centroid()
                .fill("red")
                .stroke("var(--vp-c-bg-alt)"),
        )
}

fn e06() -> Plot {
    // Gap: there is no `axisFy` mark, so the fy axis keeps its default
    // placement and dates (the docs label each facet "1960’s" at the top).
    Plot::plot()
        .margin(0.)
        .padding(0.)
        .projection(crate::examples::albers())
        .fy_scale(|fy| fy.interval("10 years"))
        .mark(geo(data::statemesh()).stroke_opacity(0.2))
        .mark(geo(data::nation()))
        .mark(
            geo(data::walmart_points())
                .fy("date")
                .r(1.5)
                .fill("blue")
                .tip_title("date"),
        )
}
