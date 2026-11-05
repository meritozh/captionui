//! Geo mark <VersionBadge version="0.6.1" />: `marks/geo.md`.
//!
//! The docs read their shapes from TopoJSON with `topojson.feature` and
//! `topojson.mesh`; [`Topology`] decodes their features the same files here, for this page
//! and the density and Delaunay pages that draw the same maps.

use std::f64::consts::PI;

use crate::prelude::*;
use gpui_plot::{Equirectangular, Projection};

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

/// A (lon, lat) path in degrees.
pub type Ring = Vec<(f64, f64)>;

/// One feature of a TopoJSON object, `topojson.feature`'s output: its id,
/// properties and polygons (each a list of rings).
pub struct Feature {
    pub id: Option<String>,
    pub properties: serde_json::Map<String, serde_json::Value>,
    pub polygons: Vec<Vec<Ring>>,
}

impl Feature {
    /// Every ring of every polygon.
    pub fn rings(&self) -> impl Iterator<Item = &Ring> {
        self.polygons.iter().flatten()
    }

    /// The feature's name property.
    pub fn name(&self) -> String {
        self.properties
            .get("name")
            .and_then(|name| name.as_str())
            .unwrap_or_default()
            .to_string()
    }
}

/// A decoded TopoJSON topology: its arcs in degrees, and its objects.
pub struct Topology {
    arcs: Vec<Ring>,
    objects: serde_json::Value,
}

impl Topology {
    /// Read and decode a TopoJSON file in `demo/data/`.
    pub fn load(file: &str) -> Self {
        let json = data::json(file);
        let transform = &json["transform"];
        let scale = |i: usize| transform["scale"][i].as_f64().unwrap_or(1.);
        let translate = |i: usize| transform["translate"][i].as_f64().unwrap_or(0.);
        let quantized = transform.is_object();
        let arcs = json["arcs"]
            .as_array()
            .map(|arcs| {
                arcs.iter()
                    .map(|arc| {
                        // Quantized arcs are delta-encoded: each position is
                        // the running sum of the deltas before it.
                        let (mut x, mut y) = (0., 0.);
                        arc.as_array()
                            .into_iter()
                            .flatten()
                            .map(|position| {
                                let dx = position[0].as_f64().unwrap_or(0.);
                                let dy = position[1].as_f64().unwrap_or(0.);
                                if quantized {
                                    x += dx;
                                    y += dy;
                                    (x * scale(0) + translate(0), y * scale(1) + translate(1))
                                } else {
                                    (dx, dy)
                                }
                            })
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            arcs,
            objects: json["objects"].clone(),
        }
    }

    /// The points of arc `index`, reversed for a negative (`~i`) index.
    fn arc(&self, index: i64) -> Ring {
        if index >= 0 {
            self.arcs[index as usize].clone()
        } else {
            let mut arc = self.arcs[(!index) as usize].clone();
            arc.reverse();
            arc
        }
    }

    /// A ring stitched from its arcs, each arc after the first dropping the
    /// point it shares with the one before.
    fn ring(&self, arcs: &serde_json::Value) -> Ring {
        let mut ring = Ring::new();
        for (k, index) in arcs.as_array().into_iter().flatten().enumerate() {
            let arc = self.arc(index.as_i64().unwrap_or(0));
            ring.extend(arc.into_iter().skip(usize::from(k > 0)));
        }
        ring
    }

    fn geometries(&self, object: &str) -> Vec<serde_json::Value> {
        let object = &self.objects[object];
        match object["type"].as_str() {
            Some("GeometryCollection") => object["geometries"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            _ => vec![object.clone()],
        }
    }

    /// `topojson.feature(topology, topology.objects[object])`, its features.
    pub fn feature(&self, object: &str) -> Vec<Feature> {
        self.geometries(object)
            .iter()
            .map(|geometry| {
                let polygon = |arcs: &serde_json::Value| -> Vec<Ring> {
                    arcs.as_array()
                        .into_iter()
                        .flatten()
                        .map(|ring| self.ring(ring))
                        .collect()
                };
                let arcs = &geometry["arcs"];
                let polygons = match geometry["type"].as_str() {
                    Some("Polygon") => vec![polygon(arcs)],
                    Some("MultiPolygon") => {
                        arcs.as_array().into_iter().flatten().map(polygon).collect()
                    }
                    _ => Vec::new(),
                };
                Feature {
                    id: geometry["id"].as_str().map(str::to_string),
                    properties: geometry["properties"]
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                    polygons,
                }
            })
            .collect()
    }
}

/// The view the docs' `albers` and `albers-usa` maps show, the lower 48
/// states, as a lon/lat rectangle.
const LOWER_48: Equirectangular = Equirectangular {
    west: -125.,
    east: -66.5,
    north: 49.6,
    south: 24.3,
};

/// The stand-in for Plot's `albers` / `albers-usa` projections: an
/// equirectangular view of the lower 48.
pub fn lower_48() -> Projection {
    Projection::Equirectangular(LOWER_48)
}

/// Pin the plot's x and y scales to [`lower_48`]'s rectangle, so marks that
/// read longitude and latitude through the scales (Plot projects them) land
/// where the geo marks draw. The aspect ratio scales a degree of longitude
/// by the cosine of the view's middle latitude, roughly the conic shape.
pub fn lower_48_scales(plot: Plot) -> Plot {
    plot.no_axes()
        .aspect_ratio(1.27)
        .x(|x| x.domain([LOWER_48.west, LOWER_48.east]))
        .y(|y| y.domain([LOWER_48.south, LOWER_48.north]))
}

/// Pin the plot's x and y scales to the whole globe, the equirectangular
/// projection's default view, a degree as wide as it is tall, so a plot of
/// geo marks alone has a frame.
fn world_scales(plot: Plot) -> Plot {
    plot.no_axes()
        .aspect_ratio(1.)
        .x(|x| x.domain([-180., 180.]))
        .y(|y| y.domain([-90., 90.]))
}

/// Whether a ring has a point inside [`lower_48`]: rings wholly outside it
/// (Alaska, Hawaii, the territories) would be clamped onto its edges.
pub fn in_lower_48(ring: &Ring) -> bool {
    ring.iter().any(|(lon, lat)| {
        (LOWER_48.west..=LOWER_48.east).contains(lon)
            && (LOWER_48.south..=LOWER_48.north).contains(lat)
    })
}

/// A geo mark over every ring of `features` that `keep` accepts.
pub fn geo_rings<'a>(
    mark: GeoMark,
    rings: impl IntoIterator<Item = &'a Ring>,
    keep: impl Fn(&Ring) -> bool,
) -> GeoMark {
    rings
        .into_iter()
        .filter(|ring| keep(ring))
        .fold(mark, |mark, ring| mark.ring(ring.clone()))
}

/// The docs' `us`: `us-counties-10m.json`.
pub fn us() -> Topology {
    Topology::load("us-counties-10m.json")
}

/// `nation` as a geo mark in the lower-48 view.
pub fn nation(us: &Topology) -> GeoMark {
    let nation = us.feature("nation");
    geo_rings(
        geo(Vec::<Table>::new()).projection(lower_48()),
        nation.iter().flat_map(Feature::rings),
        in_lower_48,
    )
}

/// `statemesh`, the interior state borders, as a geo mark in the lower-48
/// view. The geo mark fills and closes what it draws, and has no open
/// line strings, so this draws the states' outlines in place of
/// `topojson.mesh`'s interior arcs.
pub fn statemesh(us: &Topology) -> GeoMark {
    let states = us.feature("states");
    geo_rings(
        geo(Vec::<Table>::new()).projection(lower_48()),
        states.iter().flat_map(Feature::rings),
        in_lower_48,
    )
}

/// `topojson.feature(world, world.objects.land)`, as a geo mark.
fn land() -> GeoMark {
    let world = Topology::load("countries-110m.json");
    let land = world.feature("land");
    geo_rings(
        geo(Vec::<Table>::new()),
        land.iter().flat_map(Feature::rings),
        |_| true,
    )
}

fn e01() -> Plot {
    // counties with their unemployment rate joined by id.
    let rates: std::collections::HashMap<String, f64> = data::us_county_unemployment()
        .rows()
        .iter()
        .filter_map(|row| {
            let id = match row.value("id") {
                Value::Number(n) => format!("{:05}", n as i64),
                other => other.as_text()?.to_string(),
            };
            Some((id, row.number("rate")?))
        })
        .collect();
    let counties = us().feature("counties");
    let rows: Vec<Table> = counties
        .iter()
        .map(|county| {
            let rate = county
                .id
                .as_ref()
                .and_then(|id| rates.get(id))
                .copied()
                .map_or(Value::Null, Value::Number);
            Table::empty()
                .with("name", Value::text(county.name()))
                .with("unemployment", rate)
        })
        .collect();
    // The geo mark has one shape list and one fill, not a feature per row,
    // so every county takes the first row's color; `title`/`tip` have no
    // port; `albers-usa` is the lower-48 equirectangular stand-in.
    lower_48_scales(Plot::plot())
        .color(|c| {
            c.type_name("quantile")
                .n(9)
                .scheme("blues")
                .label("Unemployment (%)")
                .legend(true)
        })
        .mark(geo_rings(
            geo(rows)
                .projection(lower_48())
                .fill("unemployment")
                .tip_title("name"),
            counties.iter().flat_map(Feature::rings),
            in_lower_48,
        ))
}

fn e02() -> Plot {
    // `r: {transform: (r) => 10 ** r}`, Plot.sphere() and the USGS
    // earthquake feed (fetched live by the docs, not in demo/data) have no
    // port; the frame stands in for the sphere, which an equirectangular
    // projection draws as a rectangle.
    world_scales(Plot::plot())
        .mark(land().fill("currentColor").fill_opacity(0.2))
        .mark(frame())
}

fn e03() -> Plot {
    // The orthographic projection, Plot.sphere and Plot.graticule have no
    // port: an equirectangular sphere is the frame, and its graticule the
    // 10° meridians and parallels, drawn here as rules.
    world_scales(Plot::plot())
        .inset(2.)
        .mark(frame().fill("var(--vp-c-bg-alt)").stroke("currentColor"))
        .mark(
            rule_x((-18..=18).map(|i| f64::from(i) * 10.).collect::<Vec<_>>())
                .stroke_opacity(0.3),
        )
        .mark(
            rule_y((-8..=8).map(|i| f64::from(i) * 10.).collect::<Vec<_>>())
                .stroke_opacity(0.3),
        )
}

/// `d3.geoCircle().center(center).radius(radius)`: the ring of points
/// `radius` degrees from `center` on the sphere.
fn geo_circle(center: (f64, f64), radius: f64) -> Ring {
    let (lon0, lat0) = (center.0.to_radians(), center.1.to_radians());
    let r = radius.to_radians();
    (0..=72)
        .map(|step| {
            let bearing = 2. * PI * f64::from(step) / 72.;
            let lat = (lat0.sin() * r.cos() + lat0.cos() * r.sin() * bearing.cos()).asin();
            let lon = lon0
                + (bearing.sin() * r.sin() * lat0.cos()).atan2(r.cos() - lat0.sin() * lat.sin());
            let lon = (lon.to_degrees() + 180.).rem_euclid(360.) - 180.;
            (lon, lat.to_degrees())
        })
        .collect()
}

fn e04() -> Plot {
    // The equal-earth projection and its rotation, the color scale's
    // `transform`, and Plot.sphere have no port. The geo mark draws one
    // shape list per mark, so each circle is its own mark, stroked by its
    // radius in km through the color scale.
    let radii: Vec<f64> = [0.5, 179.5]
        .into_iter()
        .chain((10..=170).step_by(10).map(f64::from))
        .collect();
    let plot = world_scales(Plot::plot())
        .color(|c| {
            c.legend(true)
                .label("Distance from Tonga (km)")
                .zero(true)
        })
        .mark(land());
    radii
        .into_iter()
        .fold(plot, |plot, r| {
            plot.mark(
                geo(Vec::<Table>::new())
                    .ring(geo_circle((-175.38, -20.57), r))
                    .fill("none")
                    .stroke(ColorChannel::number(move |_| 111.2 * r))
                    .stroke_width(2.),
            )
        })
        .mark(frame())
}

fn e05() -> Plot {
    let us = us();
    let states = us.feature("states");
    // Plot.centroid: each state's centroid, of its largest ring.
    let centroids: Vec<Table> = states
        .iter()
        .filter_map(|state| {
            let ring = state.rings().max_by_key(|ring| ring.len())?;
            let (lon, lat) = gpui_plot::centroid(ring)?;
            Some(
                Table::empty()
                    .with("name", Value::text(state.name()))
                    .with("x", Value::Number(lon))
                    .with("y", Value::Number(lat)),
            )
        })
        .collect();
    // `tip: true, title: "name"` has no port.
    lower_48_scales(Plot::plot())
        .mark(geo_rings(
            geo(Vec::<Table>::new())
                .projection(lower_48())
                .stroke_opacity(0.1),
            states.iter().flat_map(Feature::rings),
            in_lower_48,
        ))
        .mark(nation(&us))
        .mark(
            dot(centroids)
                .x("x")
                .y("y")
                .fill("red")
                .stroke("var(--vp-c-bg-alt)"),
        )
}

fn e06() -> Plot {
    let us = us();
    // The walmarts as Point features: the geo mark reads a point's
    // longitude and latitude from its x and y.
    lower_48_scales(Plot::plot())
        .margin(0.)
        .padding(0.)
        .mark(statemesh(&us).stroke_opacity(0.2))
        .mark(nation(&us))
        .mark(
            geo(data::walmarts())
                .projection(lower_48())
                .x("longitude")
                .y("latitude")
                // `fy: {interval: "10 years"}`: the fy scale has no
                // interval, so the channel floors each date to its decade.
                .fy(|d: &Table| {
                    d.date("date")
                        .and_then(|date| {
                            chrono::NaiveDate::from_ymd_opt(date.year() / 10 * 10, 1, 1)
                        })
                        .map_or(Value::Null, Value::date)
                })
                .r(1.5)
                .fill("blue")
                .tip_title("date"),
        )
        // Plot.axisFy({frameAnchor: "top", dy: 30, tickFormat: (d) => `${year}’s`})
        // has no port: there is no fy axis mark.
}
