//! Centroid transform: `transforms/centroid.md`.
//!
//! Gap: the library has no TopoJSON reader, no Albers projections and no
//! `centroid`/`geoCentroid` transforms, and its `geo` mark only projects
//! equirectangular or Mercator. So this file decodes the docs'
//! `us-counties-10m.json` itself, projects it with d3's Albers (and the
//! Albers USA composite) into plain x/y, and draws the meshes as `line` marks
//! and the centroids as ordinary rows. The other transform pages that draw
//! the US (hexbin, sort) share these helpers.

use std::rc::Rc;

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            height: 400.,
            ..example("transforms", "centroid", 1, e01)
        },
        example("transforms", "centroid", 2, e02),
        example("transforms", "centroid", 3, e03),
        example("transforms", "centroid", 4, e04),
        example("transforms", "centroid", 5, e05),
        example("transforms", "centroid", 6, e06),
    ]
}

/// A TopoJSON feature: its id, its name, and its rings in lon/lat degrees.
pub struct Feature {
    pub id: String,
    pub name: String,
    pub polygons: Vec<Vec<Vec<(f64, f64)>>>,
}

/// The decoded `us-counties-10m.json`: its arcs in lon/lat, and its objects.
pub struct Us {
    arcs: Vec<Vec<(f64, f64)>>,
    json: serde_json::Value,
}

thread_local! {
    static US: Rc<Us> = Rc::new(Us::load());
}

/// The US atlas, decoded once per thread.
pub fn us() -> Rc<Us> {
    US.with(Rc::clone)
}

impl Us {
    fn load() -> Self {
        let json = data::json("us-counties-10m.json");
        let number = |v: &serde_json::Value| v.as_f64().unwrap_or(0.);
        let scale = &json["transform"]["scale"];
        let translate = &json["transform"]["translate"];
        let (kx, ky) = (number(&scale[0]), number(&scale[1]));
        let (tx, ty) = (number(&translate[0]), number(&translate[1]));
        // Quantized arcs are delta-encoded: each point is the running sum.
        let arcs = json["arcs"]
            .as_array()
            .map(|arcs| {
                arcs.iter()
                    .map(|arc| {
                        let (mut x, mut y) = (0., 0.);
                        arc.as_array()
                            .into_iter()
                            .flatten()
                            .map(|p| {
                                x += number(&p[0]);
                                y += number(&p[1]);
                                (x * kx + tx, y * ky + ty)
                            })
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { arcs, json }
    }

    /// An arc's points, reversed for a negative (`~i`) index.
    fn arc(&self, index: i64) -> Vec<(f64, f64)> {
        if index >= 0 {
            self.arcs[index as usize].clone()
        } else {
            let mut points = self.arcs[(!index) as usize].clone();
            points.reverse();
            points
        }
    }

    /// A ring stitched from its arcs.
    fn ring(&self, arcs: &serde_json::Value) -> Vec<(f64, f64)> {
        let mut ring: Vec<(f64, f64)> = Vec::new();
        for index in arcs.as_array().into_iter().flatten() {
            let points = self.arc(index.as_i64().unwrap_or(0));
            let skip = usize::from(!ring.is_empty());
            ring.extend(points.into_iter().skip(skip));
        }
        ring
    }

    fn geometries(&self, object: &str) -> &[serde_json::Value] {
        self.json["objects"][object]["geometries"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// `topojson.feature(us, us.objects[object]).features`.
    pub fn features(&self, object: &str) -> Vec<Feature> {
        self.geometries(object)
            .iter()
            .map(|geometry| {
                let polygons: Vec<Vec<Vec<(f64, f64)>>> = match geometry["type"].as_str() {
                    Some("Polygon") => vec![
                        geometry["arcs"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|ring| self.ring(ring))
                            .collect(),
                    ],
                    Some("MultiPolygon") => geometry["arcs"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|polygon| {
                            polygon
                                .as_array()
                                .into_iter()
                                .flatten()
                                .map(|ring| self.ring(ring))
                                .collect()
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                Feature {
                    id: geometry["id"].as_str().unwrap_or_default().to_string(),
                    name: geometry["properties"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    polygons,
                }
            })
            .collect()
    }

    /// `topojson.mesh(us, us.objects[object])`: every arc the object uses,
    /// once. With `interior`, only the arcs two geometries share, the docs'
    /// `(a, b) => a !== b` filter.
    pub fn mesh(&self, object: &str, interior: bool) -> Vec<Vec<(f64, f64)>> {
        let mut uses = vec![0usize; self.arcs.len()];
        fn walk(value: &serde_json::Value, uses: &mut [usize]) {
            match value {
                serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, uses)),
                serde_json::Value::Number(n) => {
                    let i = n.as_i64().unwrap_or(0);
                    let i = if i >= 0 { i as usize } else { (!i) as usize };
                    if let Some(count) = uses.get_mut(i) {
                        *count += 1;
                    }
                }
                _ => {}
            }
        }
        for geometry in self.geometries(object) {
            walk(&geometry["arcs"], &mut uses);
        }
        uses.iter()
            .enumerate()
            .filter(|(_, n)| if interior { **n >= 2 } else { **n >= 1 })
            .map(|(i, _)| self.arcs[i].clone())
            .collect()
    }
}

/// d3's conic equal-area projection, raw, in radians: `[x, y]` with y up.
fn conic_equal_area(parallels: (f64, f64), lon: f64, lat: f64) -> (f64, f64) {
    let (phi0, phi1) = (parallels.0.to_radians(), parallels.1.to_radians());
    let sy0 = phi0.sin();
    let n = (sy0 + phi1.sin()) / 2.;
    let c = 1. + sy0 * (2. * n - sy0);
    let r0 = c.sqrt() / n;
    let r = (c - 2. * n * lat.sin()).max(0.).sqrt() / n;
    let x = lon * n;
    (r * x.sin(), r0 - r * x.cos())
}

/// One of d3's conic equal-area projections, rotated and centered as d3's
/// `geoAlbers` and `geoAlbersUsa` configure them, scaled by `k`.
fn albers_part(
    rotate: f64,
    center: (f64, f64),
    parallels: (f64, f64),
    k: f64,
    lon: f64,
    lat: f64,
) -> (f64, f64) {
    let raw = |lon: f64, lat: f64| {
        let mut lambda = (lon + rotate).to_radians();
        // Wrap into [-π, π] as d3's rotation does.
        while lambda > std::f64::consts::PI {
            lambda -= std::f64::consts::TAU;
        }
        while lambda < -std::f64::consts::PI {
            lambda += std::f64::consts::TAU;
        }
        conic_equal_area(parallels, lambda, lat.to_radians())
    };
    // d3's `center` is in the rotated frame, so it is projected unrotated.
    let (cx, cy) = {
        let lambda = center.0.to_radians();
        conic_equal_area(parallels, lambda, center.1.to_radians())
    };
    let (x, y) = raw(lon, lat);
    ((x - cx) * k, (y - cy) * k)
}

/// d3's `geoAlbers()`: the lower 48's conic equal-area, in pixels at d3's
/// default scale with y up.
pub fn albers(lon: f64, lat: f64) -> Option<(f64, f64)> {
    Some(albers_part(
        96.,
        (-0.6, 38.7),
        (29.5, 45.5),
        1070.,
        lon,
        lat,
    ))
}

/// d3's `geoAlbersUsa()`: the lower 48, with Alaska and Hawaii moved and
/// scaled into insets below it; anything else (Puerto Rico, the territories)
/// projects to nothing, as d3's does.
pub fn albers_usa(lon: f64, lat: f64) -> Option<(f64, f64)> {
    let k = 1070.;
    if (50.0..=72.).contains(&lat) && !(-129.0..0.).contains(&lon) {
        let (x, y) = albers_part(154., (-2., 58.5), (55., 65.), 0.35 * k, lon, lat);
        return Some((x - 0.307 * k, y - 0.201 * k));
    }
    if (18.0..=23.).contains(&lat) && (-162.0..=-154.).contains(&lon) {
        let (x, y) = albers_part(157., (-3., 19.9), (8., 18.), k, lon, lat);
        return Some((x - 0.205 * k, y - 0.212 * k));
    }
    if (24.0..=50.).contains(&lat) && (-125.0..=-66.).contains(&lon) {
        return albers(lon, lat);
    }
    None
}

/// A projection, d3's `"albers"` or `"albers-usa"`.
pub type Project = fn(f64, f64) -> Option<(f64, f64)>;

/// A mesh's arcs as rows a `line` mark draws: `x`, `y` and the arc's `arc`
/// as its `z`, broken where the projection drops a point.
pub fn mesh_rows(arcs: &[Vec<(f64, f64)>], project: Project) -> Dataset {
    let mut rows = Vec::new();
    let mut piece = 0usize;
    for arc in arcs {
        piece += 1;
        for &(lon, lat) in arc {
            match project(lon, lat) {
                Some((x, y)) => rows.push(
                    Table::empty()
                        .with("x", Value::Number(x))
                        .with("y", Value::Number(y))
                        .with("arc", Value::Number(piece as f64)),
                ),
                None => piece += 1,
            }
        }
    }
    Dataset::new(rows)
}

/// The rings of every feature as mesh arcs, for `Plot.geo(nation)`.
pub fn outlines(features: &[Feature]) -> Vec<Vec<(f64, f64)>> {
    features
        .iter()
        .flat_map(|f| f.polygons.iter().flatten().cloned())
        .collect()
}

/// A `line` mark drawing a projected mesh, Plot's `Plot.geo(mesh)`.
pub fn geo_mesh(arcs: &[Vec<(f64, f64)>], project: Project) -> LineMark {
    line(mesh_rows(arcs, project))
        .x("x")
        .y("y")
        .z("arc")
        .stroke_width(1.)
        .clip(true)
}

/// Plot's planar `Plot.centroid()`: the area-weighted centroid of the
/// feature's projected polygons.
pub fn planar_centroid(feature: &Feature, project: Project) -> Option<(f64, f64)> {
    let (mut area, mut cx, mut cy) = (0., 0., 0.);
    let mut fallback: Vec<(f64, f64)> = Vec::new();
    for ring in feature.polygons.iter().flatten() {
        let points: Vec<(f64, f64)> = ring.iter().filter_map(|&(a, b)| project(a, b)).collect();
        fallback.extend(points.iter().copied());
        for pair in points.windows(2) {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            let cross = x0 * y1 - x1 * y0;
            area += cross;
            cx += (x0 + x1) * cross;
            cy += (y0 + y1) * cross;
        }
    }
    if area.abs() > 1e-9 {
        Some((cx / (3. * area), cy / (3. * area)))
    } else if !fallback.is_empty() {
        let n = fallback.len() as f64;
        Some((
            fallback.iter().map(|p| p.0).sum::<f64>() / n,
            fallback.iter().map(|p| p.1).sum::<f64>() / n,
        ))
    } else {
        None
    }
}

/// Plot's `Plot.geoCentroid()`: the spherical centroid, approximated as the
/// area-weighted centroid in lon/lat, then projected.
pub fn geo_centroid(feature: &Feature, project: Project) -> Option<(f64, f64)> {
    let identity: Project = |lon, lat| Some((lon, lat));
    let (lon, lat) = planar_centroid(feature, identity)?;
    project(lon, lat)
}

/// One row per feature at its centroid: `x`, `y`, `name` and `id`.
pub fn centroid_rows(
    features: &[Feature],
    centroid: impl Fn(&Feature) -> Option<(f64, f64)>,
) -> Dataset {
    Dataset::new(features.iter().filter_map(|feature| {
        let (x, y) = centroid(feature)?;
        Some(
            Table::empty()
                .with("x", Value::Number(x))
                .with("y", Value::Number(y))
                .with("name", Value::text(feature.name.clone()))
                .with("id", Value::text(feature.id.clone())),
        )
    }))
}

/// A projected map's frame: no axes, and an aspect ratio of one so the
/// projection is not stretched, as Plot sizes a projected plot.
pub fn map_plot() -> Plot {
    Plot::plot().no_axes().aspect_ratio(1.).margin(0.)
}

/// The frame of Plot's `projection: "albers"`: d3's default viewport, 960
/// pixels across, centered on the lower 48, so Alaska and Hawaii fall off
/// its edges.
pub fn albers_plot() -> Plot {
    map_plot().x_domain(-480., 480.).y_domain(-298., 298.)
}

/// `Plot.text(states, Plot.centroid({text: (d) => d.properties.name, fill: "currentColor", stroke: "white"}))`
fn e01() -> Plot {
    let us = us();
    let states = us.features("states");
    map_plot()
        .mark(geo_mesh(&us.mesh("states", false), albers_usa))
        .mark(
            text(centroid_rows(&states, |f| planar_centroid(f, albers_usa)))
                .x("x")
                .y("y")
                .text("name")
                .fill("currentColor")
                .stroke("white"),
        )
}

/// `Plot.voronoi(counties, Plot.centroid()).plot({projection: "albers"})`
///
/// Gap: the library's `voronoi` strokes nothing by default (Plot's strokes
/// `currentColor`), so the stroke is explicit.
fn e02() -> Plot {
    let counties = us().features("counties");
    albers_plot().mark(
        voronoi(centroid_rows(&counties, |f| planar_centroid(f, albers)))
            .x("x")
            .y("y")
            .stroke("currentColor"),
    )
}

/// `Plot.dot(counties, Plot.centroid()).plot({projection: "albers-usa"})`
fn e03() -> Plot {
    let counties = us().features("counties");
    map_plot().mark(
        dot(centroid_rows(&counties, |f| planar_centroid(f, albers_usa)))
            .x("x")
            .y("y"),
    )
}

/// `Plot.dot(counties, Plot.geoCentroid()).plot({projection: "albers-usa"})`
fn e04() -> Plot {
    let counties = us().features("counties");
    map_plot().mark(
        dot(centroid_rows(&counties, |f| geo_centroid(f, albers_usa)))
            .x("x")
            .y("y"),
    )
}

/// `Plot.dot(counties, Plot.hexbin({r: "count"}, Plot.geoCentroid())).plot({projection: "albers"})`
///
/// Gap: hexbin outputs (`r: "count"`) are missing; the library's `hexbin`
/// mark fills each hexagon by its count instead of sizing a dot.
fn e05() -> Plot {
    let counties = us().features("counties");
    albers_plot().mark(
        hexbin(centroid_rows(&counties, |f| geo_centroid(f, albers)))
            .x("x")
            .y("y"),
    )
}

/// The states' centroids over the nation, with a tip naming each state.
///
/// Gap: `Plot.pointer` on a tip is an interaction; the snapshot shows no
/// pointer, so the tip draws nothing either way.
fn e06() -> Plot {
    let us = us();
    let states = us.features("states");
    let centroids = centroid_rows(&states, |f| planar_centroid(f, albers_usa));
    map_plot()
        .mark(geo_mesh(&us.mesh("states", false), albers_usa).stroke_opacity(0.2))
        .mark(geo_mesh(&outlines(&us.features("nation")), albers_usa))
        .mark(dot(centroids).x("x").y("y").fill("red").stroke("white"))
}
