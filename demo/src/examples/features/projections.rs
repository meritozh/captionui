//! Projections: `features/projections.md`.

use crate::prelude::*;
use gpui_plot::GeoProjection;
use gpui_plot::geo;

pub fn examples() -> Vec<Example> {
    vec![
        example("features", "projections", 1, e01),
        example("features", "projections", 2, e02),
        example("features", "projections", 3, e03),
        example("features", "projections", 4, e04),
        Example {
            height: 688.,
            ..example("features", "projections", 5, e05)
        },
        example("features", "projections", 6, e06),
        example("features", "projections", 7, e07),
    ]
}

/// The page's sliders, at their initial values.
const LONGITUDE: f64 = 90.;
const RADIUS: f64 = 30.;

fn projection(name: &str) -> GeoProjection {
    GeoProjection::named(name).expect("a projection Plot names")
}

/// `d3.geoCircle().center(center).radius(radius)()` away from the poles: the polygon of points
/// `radius` degrees from `center`, every 6°.
fn geo_circle(center: [f64; 2], radius: f64) -> Geometry {
    let (lon0, lat0) = (center[0].to_radians(), center[1].to_radians());
    let r = radius.to_radians();
    let ring = (0..=60)
        .map(|i| {
            let bearing = (f64::from(i) * 6.).to_radians();
            let lat = (lat0.sin() * r.cos() + lat0.cos() * r.sin() * bearing.cos()).asin();
            let lon = lon0
                + (bearing.sin() * r.sin() * lat0.cos()).atan2(r.cos() - lat0.sin() * lat.sin());
            [lon.to_degrees(), lat.to_degrees()]
        })
        .collect();
    Geometry::Polygon(vec![ring])
}

/// The docs' earthquakes are a live USGS feed with no local copy, so the
/// dots have no data. Gap: the `r` scale has no `transform` (Richter to
/// amplitude).
fn e01() -> Plot {
    let earthquakes = Dataset::new([]);
    Plot::plot()
        .projection(projection("orthographic").rotate([-LONGITUDE, -30., 0.]))
        .mark(geo(data::land()).fill("currentColor").fill_opacity(0.2))
        .mark(geo(geo::sphere()))
        .mark(
            dot(earthquakes)
                .x("longitude")
                .y("latitude")
                .r("magnitude")
                .stroke("red")
                .fill("red")
                .fill_opacity(0.2),
        )
}

/// The page's projection menu starts at `equirectangular`.
fn e02() -> Plot {
    Plot::plot()
        .projection("equirectangular")
        .mark(geo(geo::graticule()).stroke_opacity(0.1))
        .mark(geo(data::land()).fill("currentColor"))
        .mark(geo(geo::sphere()))
}

fn e03() -> Plot {
    Plot::plot()
        .projection("albers-usa")
        .mark(geo(data::nation()))
        .mark(geo(data::statemesh()).stroke_opacity(0.2))
}

fn e04() -> Plot {
    let circle = geo_circle([9., 34.], RADIUS);
    Plot::plot()
        .projection(
            projection("azimuthal-equidistant")
                .rotate([-9., -34., 0.])
                .domain(circle.clone())
                .inset(10.),
        )
        .mark(geo(geo::graticule()).stroke_opacity(0.1))
        .mark(geo(data::land()).fill("currentColor").fill_opacity(0.3))
        .mark(geo(circle).stroke("red").stroke_width(2.))
        .mark(frame())
}

/// Gap: a custom projection function is not supported, nor d3's
/// `clipAngle`. The docs' `d3.geoAzimuthalEquidistant().rotate([0, 90])
/// .scale(width)` puts a circle of half a radian around the pole at the
/// frame's edge, so fitting the domain to that circle gives the same scale
/// and translate; only the clip at 40° (just inside the corners) is lost.
fn e05() -> Plot {
    // That circle is the parallel half a radian from the south pole.
    let lat = -90. + 0.5_f64.to_degrees();
    let edge = Geometry::Polygon(vec![
        (0..=72)
            .map(|i| [-180. + 5. * f64::from(i), lat])
            .collect(),
    ]);
    Plot::plot()
        .width(688.)
        .height(688.)
        .projection(
            projection("azimuthal-equidistant")
                .rotate([0., 90., 0.])
                .domain(edge),
        )
        .mark(geo(geo::graticule()).stroke_opacity(0.1))
        .mark(geo(data::land()).fill("currentColor"))
        .mark(frame())
}

/// Gap: the identity projection ignores the scale its domain fits, so the
/// plan draws at one unit per pixel, offset, rather than filling the frame.
fn e06() -> Plot {
    let westport = geo::features(&data::json("westport-house.json"));
    let domain = Geometry::Collection(
        westport
            .rows()
            .iter()
            .filter_map(|d| d.value("geometry").as_geo().cloned())
            .collect(),
    );
    geo(westport)
        .plot()
        .projection(projection("identity").domain(domain))
}

fn e07() -> Plot {
    Plot::plot()
        .margin_left(0.)
        .margin_right(0.)
        .projection(crate::examples::albers())
        .fx_scale(|fx| {
            fx.interval("10 years")
                .tick_format(|v: &Value| {
                    v.as_date()
                        .map(|d| format!("{}’s", d.year()))
                        .unwrap_or_default()
                })
                .no_label()
        })
        .mark(geo(data::statemesh()).stroke_opacity(0.1))
        .mark(geo(data::nation()))
        .mark(
            dot(data::walmarts())
                .fx("date")
                .x("longitude")
                .y("latitude")
                .r(1.)
                .fill("currentColor"),
        )
}
