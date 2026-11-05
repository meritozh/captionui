//! Projections: `features/projections.md`.
//!
//! The library projects only a `geo` mark's own rings, through an
//! equirectangular or Mercator view; there is no plot-level `projection`,
//! no orthographic, Albers or azimuthal projection, and no `graticule` or
//! `sphere` mark. These ports draw the same geography through an
//! equirectangular view of the region, with x/y scales pinned to that view
//! so dots and rules line up with the map, a frame for the sphere's outline
//! and rules every 10° for the graticule.

use crate::prelude::*;
use gpui_plot::Equirectangular;
use gpui_plot::Projection;

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

type Ring = Vec<(f64, f64)>;

/// The polygons of a TopoJSON object, as `topojson.feature` decodes them:
/// each arc's quantized deltas summed and scaled, the arcs of a ring
/// joined end to end (an arc index `~i` is arc `i` reversed).
fn topojson_rings(file: &str, object: &str) -> Vec<Ring> {
    let topology = data::json(file);
    let scale = &topology["transform"]["scale"];
    let translate = &topology["transform"]["translate"];
    let (kx, ky) = (scale[0].as_f64().unwrap(), scale[1].as_f64().unwrap());
    let (dx, dy) = (
        translate[0].as_f64().unwrap(),
        translate[1].as_f64().unwrap(),
    );
    let arcs: Vec<Ring> = topology["arcs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arc| {
            let (mut x, mut y) = (0., 0.);
            arc.as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    x += p[0].as_f64().unwrap();
                    y += p[1].as_f64().unwrap();
                    (x * kx + dx, y * ky + dy)
                })
                .collect()
        })
        .collect();
    let ring = |indexes: &serde_json::Value| -> Ring {
        let mut points: Ring = Vec::new();
        for index in indexes.as_array().unwrap() {
            let index = index.as_i64().unwrap();
            let mut arc = if index < 0 {
                let mut arc = arcs[(!index) as usize].clone();
                arc.reverse();
                arc
            } else {
                arcs[index as usize].clone()
            };
            if !points.is_empty() {
                arc.remove(0);
            }
            points.extend(arc);
        }
        points
    };
    let mut rings = Vec::new();
    for geometry in topology["objects"][object]["geometries"]
        .as_array()
        .unwrap()
    {
        match geometry["type"].as_str() {
            Some("Polygon") => {
                rings.extend(geometry["arcs"].as_array().unwrap().iter().map(ring));
            }
            Some("MultiPolygon") => {
                for polygon in geometry["arcs"].as_array().unwrap() {
                    rings.extend(polygon.as_array().unwrap().iter().map(ring));
                }
            }
            _ => {}
        }
    }
    rings
}

/// `topojson.feature(world, world.objects.land)`
fn land() -> Vec<Ring> {
    topojson_rings("countries-110m.json", "land")
}

/// `topojson.feature(us, us.objects.nation)`
fn nation() -> Vec<Ring> {
    topojson_rings("us-counties-10m.json", "nation")
}

/// `topojson.mesh(us, us.objects.states, (a, b) => a !== b)`: there is no
/// line geometry for a `geo` mark, so the states' outlines stand in.
fn statemesh() -> Vec<Ring> {
    topojson_rings("us-counties-10m.json", "states")
}

/// A `geo` mark over a list of rings, projected through `projection`. The
/// mark holds one empty record: a geo mark without rows resolves no colors.
fn geo_shapes(rings: Vec<Ring>, projection: &Projection) -> GeoMark {
    rings
        .into_iter()
        .fold(geo(Dataset::length(1)), |mark, ring| mark.ring(ring))
        .projection(projection.clone())
}

/// A lon/lat rectangle as an equirectangular view.
fn view(west: f64, east: f64, north: f64, south: f64) -> Projection {
    Projection::Equirectangular(Equirectangular {
        west,
        east,
        north,
        south,
    })
}

/// A plot whose x and y scales span `projection`'s view, without axes, so
/// marks positioned by lon/lat line up with the map.
fn map_plot(projection: &Projection) -> Plot {
    let Projection::Equirectangular(p) = projection else {
        unreachable!("the ports use equirectangular views")
    };
    let (west, east, north, south) = (p.west, p.east, p.north, p.south);
    Plot::plot()
        .no_axes()
        .x(move |x| x.domain([west, east]).nice(false).zero(false))
        .y(move |y| y.domain([south, north]).nice(false).zero(false))
}

/// Plot's `graticule`: a rule every 10° of longitude and latitude in view.
fn graticule(projection: &Projection) -> [Mark; 2] {
    let Projection::Equirectangular(p) = projection else {
        unreachable!("the ports use equirectangular views")
    };
    let steps = |lo: f64, hi: f64| -> Vec<f64> {
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        ((lo / 10.).ceil() as i32..=(hi / 10.).floor() as i32)
            .map(|i| i as f64 * 10.)
            .collect()
    };
    [
        rule_x(steps(p.west, p.east))
            .stroke("currentColor")
            .stroke_opacity(0.1)
            .into(),
        rule_y(steps(p.south, p.north))
            .stroke("currentColor")
            .stroke_opacity(0.1)
            .into(),
    ]
}

/// d3's `d3.geoCircle().center(center).radius(radius)()`: the ring of
/// points `radius` degrees from `center`, every 6°.
fn geo_circle(center: (f64, f64), radius: f64) -> Ring {
    let (lon0, lat0) = (center.0.to_radians(), center.1.to_radians());
    let r = radius.to_radians();
    (0..=60)
        .map(|i| {
            let bearing = (i as f64 * 6.).to_radians();
            let lat = (lat0.sin() * r.cos() + lat0.cos() * r.sin() * bearing.cos()).asin();
            let lon = lon0
                + (bearing.sin() * r.sin() * lat0.cos()).atan2(r.cos() - lat0.sin() * lat.sin());
            (lon.to_degrees(), lat.to_degrees())
        })
        .collect()
}

/// An orthographic globe rotated to the page's `longitude` (90): drawn as
/// the whole world, equirectangular. The docs' earthquakes are a live USGS
/// feed with no local copy, so the dots have no data; the `r` scale's
/// `transform: (d) => Math.pow(10, d)` has no counterpart either.
fn e01() -> Plot {
    let projection = Projection::equirectangular();
    let earthquakes = Dataset::new([]);
    map_plot(&projection)
        .height(320.)
        .mark(
            geo_shapes(land(), &projection)
                .fill("currentColor")
                .fill_opacity(0.2),
        )
        .mark(frame())
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

/// The page's projection control starts at `equirectangular`.
fn e02() -> Plot {
    let projection = Projection::equirectangular();
    let [meridians, parallels] = graticule(&projection);
    // Plot sizes a projected plot to the projection's aspect ratio, 2:1.
    map_plot(&projection)
        .height(320.)
        .mark(meridians)
        .mark(parallels)
        .mark(geo_shapes(land(), &projection).fill("currentColor"))
        .mark(frame())
}

/// `projection: "albers-usa"`: the lower 48 states, equirectangular.
fn e03() -> Plot {
    let projection = view(-125., -66., 50., 24.);
    map_plot(&projection)
        .height(400.)
        .mark(
            geo_shapes(nation(), &projection)
                .fill("none")
                .stroke("currentColor"),
        )
        .mark(
            geo_shapes(statemesh(), &projection)
                .fill("none")
                .stroke("currentColor")
                .stroke_opacity(0.2),
        )
}

/// An azimuthal-equidistant view fit to a 30° circle around (9°, 34°):
/// here the circle's bounding box, equirectangular.
fn e04() -> Plot {
    let circle = geo_circle((9., 34.), 30.);
    let (mut west, mut east, mut north, mut south) = (180f64, -180f64, -90f64, 90f64);
    for &(lon, lat) in &circle {
        west = west.min(lon);
        east = east.max(lon);
        north = north.max(lat);
        south = south.min(lat);
    }
    let projection = view(west, east, north, south);
    let [meridians, parallels] = graticule(&projection);
    map_plot(&projection)
        .inset(10.)
        .mark(meridians)
        .mark(parallels)
        .mark(
            geo_shapes(land(), &projection)
                .fill("currentColor")
                .fill_opacity(0.3),
        )
        .mark(
            geo_shapes(vec![circle], &projection)
                .fill("none")
                .stroke("red")
                .stroke_width(2.),
        )
        .mark(frame())
}

/// A polar azimuthal-equidistant view (rotated to the south pole) clipped
/// at 40° from it: here the band south of 50°S, equirectangular.
fn e05() -> Plot {
    let projection = view(-180., 180., -50., -90.);
    let [meridians, parallels] = graticule(&projection);
    map_plot(&projection)
        .height(688.)
        .mark(meridians)
        .mark(parallels)
        .mark(geo_shapes(land(), &projection).fill("currentColor"))
        .mark(frame())
}

/// `projection: {type: "identity", domain: westport}`: the floor plan's
/// planar coordinates, y down, fit to the frame. Plot sizes the plot to the
/// domain's aspect ratio; here the height is set from it.
fn e06() -> Plot {
    let text = data::text("westport-house.json");
    let rings = gpui_plot::projection::geojson_rings(&text).unwrap_or_default();
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in rings.iter().flatten() {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let projection = view(x0, x1, y0, y1);
    geo(Dataset::length(1))
        .geojson(&text)
        .projection(projection)
        .fill("none")
        .stroke("currentColor")
        .plot()
        .no_axes()
        .height((640. * (y1 - y0) / (x1 - x0)) as f32)
}

/// Every fourth point of each ring: the 10m outlines drawn once per facet
/// overflow the renderer's frame buffer otherwise.
fn coarse(rings: Vec<Ring>) -> Vec<Ring> {
    rings
        .into_iter()
        .map(|ring| ring.into_iter().step_by(4).collect::<Ring>())
        .filter(|ring| ring.len() >= 3)
        .collect()
}

/// Walmart's openings by decade over the lower 48, equirectangular. A facet
/// scale's `interval` doesn't bucket its values here (each date would be its
/// own facet), so the `fx` channel floors each date to its decade, as
/// `fx: {interval: "10 years"}` does.
fn e07() -> Plot {
    let projection = view(-125., -66., 50., 24.);
    // Plot sizes the facets to the projection's aspect ratio.
    map_plot(&projection)
        .height(100.)
        .margin_left(0.)
        .margin_right(0.)
        .fx_scale(|fx| {
            fx.tick_format(|v: &Value| {
                v.as_date()
                    .map(|d| format!("{}’s", d.year()))
                    .unwrap_or_default()
            })
            .no_label()
        })
        .mark(
            geo_shapes(coarse(statemesh()), &projection)
                .fill("none")
                .stroke("currentColor")
                .stroke_opacity(0.1),
        )
        .mark(
            geo_shapes(coarse(nation()), &projection)
                .fill("none")
                .stroke("currentColor"),
        )
        .mark(
            dot(data::walmarts())
                .fx(|d: &Table| {
                    d.date("date")
                        .and_then(|date| {
                            chrono::NaiveDate::from_ymd_opt(date.year() / 10 * 10, 1, 1)
                        })
                        .map(Value::date)
                        .unwrap_or(Value::Null)
                })
                .x("longitude")
                .y("latitude")
                .r(1.)
                .fill("currentColor"),
        )
}
