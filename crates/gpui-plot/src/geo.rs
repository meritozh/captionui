//! Geography for Plot's `geo` mark and `projection` option: GeoJSON
//! geometries, TopoJSON decoding (`topojson.feature` and `topojson.mesh`),
//! d3-geo's projections, and the spherical helpers Plot leans on
//! (`d3.geoCentroid`, `Plot.sphere`, `Plot.graticule`).
//!
//! A projection is d3's: a raw function from radians to a unit plane,
//! composed with a rotation, a clip (the antimeridian cut for most, a small
//! circle for orthographic and the azimuthals), and Plot's fit of the result
//! to the frame. Lines are resampled along great circles so long edges bend
//! as they should.

mod clip;

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

use serde_json::Value as Json;

use crate::table::{Dataset, Table};
use crate::value::Value;

const RADIANS: f64 = PI / 180.;
const DEGREES: f64 = 180. / PI;
/// Plot's default projection `precision`, pixels.
const PRECISION: f64 = 0.15;

/// A GeoJSON geometry, in degrees: `[longitude, latitude]` positions.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point([f64; 2]),
    MultiPoint(Vec<[f64; 2]>),
    LineString(Vec<[f64; 2]>),
    MultiLineString(Vec<Vec<[f64; 2]>>),
    /// Rings, the first the exterior.
    Polygon(Vec<Vec<[f64; 2]>>),
    MultiPolygon(Vec<Vec<Vec<[f64; 2]>>>),
    Collection(Vec<Geometry>),
    /// The whole globe, Plot's `sphere`.
    Sphere,
}

impl Geometry {
    /// Parse a GeoJSON geometry, feature or collection.
    pub fn from_json(json: &Json) -> Option<Self> {
        let kind = json.get("type")?.as_str()?;
        let position = |p: &Json| -> Option<[f64; 2]> {
            let a = p.as_array()?;
            Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
        };
        let line =
            |l: &Json| -> Option<Vec<[f64; 2]>> { l.as_array()?.iter().map(position).collect() };
        let lines =
            |l: &Json| -> Option<Vec<Vec<[f64; 2]>>> { l.as_array()?.iter().map(line).collect() };
        let coords = json.get("coordinates");
        Some(match kind {
            "Point" => Geometry::Point(position(coords?)?),
            "MultiPoint" => Geometry::MultiPoint(line(coords?)?),
            "LineString" => Geometry::LineString(line(coords?)?),
            "MultiLineString" => Geometry::MultiLineString(lines(coords?)?),
            "Polygon" => Geometry::Polygon(lines(coords?)?),
            "MultiPolygon" => Geometry::MultiPolygon(
                coords?
                    .as_array()?
                    .iter()
                    .map(lines)
                    .collect::<Option<_>>()?,
            ),
            "GeometryCollection" => Geometry::Collection(
                json.get("geometries")?
                    .as_array()?
                    .iter()
                    .filter_map(Geometry::from_json)
                    .collect(),
            ),
            "Feature" => Geometry::from_json(json.get("geometry")?)?,
            "FeatureCollection" => Geometry::Collection(
                json.get("features")?
                    .as_array()?
                    .iter()
                    .filter_map(Geometry::from_json)
                    .collect(),
            ),
            "Sphere" => Geometry::Sphere,
            _ => return None,
        })
    }

    /// Every position, for bounds.
    fn positions(&self, out: &mut Vec<[f64; 2]>) {
        match self {
            Geometry::Point(p) => out.push(*p),
            Geometry::MultiPoint(ps) | Geometry::LineString(ps) => out.extend(ps),
            Geometry::MultiLineString(ls) | Geometry::Polygon(ls) => {
                ls.iter().for_each(|l| out.extend(l))
            }
            Geometry::MultiPolygon(ps) => ps.iter().flatten().for_each(|l| out.extend(l)),
            Geometry::Collection(gs) => gs.iter().for_each(|g| g.positions(out)),
            Geometry::Sphere => {}
        }
    }

    /// Wrap the geometry as a [`Value`] for a record.
    pub fn into_value(self) -> Value {
        Value::Geo(Rc::new(self))
    }
}

/// GeoJSON features as records: each feature's geometry under `"geometry"`
/// (which the `geo` mark reads by default), its `id` under `"id"`, and its
/// properties under their own names, so `fill: "population"` works as
/// Plot's `fill: (d) => d.properties.population` does.
pub fn features(json: &Json) -> Dataset {
    let list: Vec<&Json> = match json.get("type").and_then(Json::as_str) {
        Some("FeatureCollection") => json["features"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default(),
        _ => vec![json],
    };
    Dataset::new(list.into_iter().filter_map(feature_record))
}

fn feature_record(feature: &Json) -> Option<Table> {
    let geometry = Geometry::from_json(feature)?;
    let mut row = Table::empty().with("geometry", geometry.into_value());
    if let Some(id) = feature.get("id") {
        row = row.with("id", json_value(id));
    }
    if let Some(props) = feature.get("properties").and_then(Json::as_object) {
        for (key, value) in props {
            row = row.with(key.clone(), json_value(value));
        }
    }
    Some(row)
}

/// A JSON scalar as a [`Value`].
pub fn json_value(json: &Json) -> Value {
    match json {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::number(n.as_f64().unwrap_or(f64::NAN)),
        Json::String(s) => crate::io::auto_type(s),
        other => Value::text(other.to_string()),
    }
}

/// A TopoJSON topology, decoded once so its objects can be read as features
/// or meshes, `topojson-client`'s `feature` and `mesh`.
pub struct Topology {
    arcs: Vec<Vec<[f64; 2]>>,
    objects: serde_json::Map<String, Json>,
}

impl Topology {
    /// Decode a topology, applying its quantization transform to the arcs.
    pub fn parse(json: &Json) -> Option<Self> {
        let transform = json.get("transform").map(|t| {
            let pair = |k: &str| {
                let a = &t[k];
                [a[0].as_f64().unwrap_or(1.), a[1].as_f64().unwrap_or(0.)]
            };
            (pair("scale"), pair("translate"))
        });
        let arcs = json
            .get("arcs")?
            .as_array()?
            .iter()
            .map(|arc| {
                let mut x = 0.;
                let mut y = 0.;
                arc.as_array()
                    .map(|points| {
                        points
                            .iter()
                            .map(|p| {
                                let (a, b) =
                                    (p[0].as_f64().unwrap_or(0.), p[1].as_f64().unwrap_or(0.));
                                match transform {
                                    Some((scale, translate)) => {
                                        x += a;
                                        y += b;
                                        [x * scale[0] + translate[0], y * scale[1] + translate[1]]
                                    }
                                    None => [a, b],
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();
        Some(Self {
            arcs,
            objects: json.get("objects")?.as_object()?.clone(),
        })
    }

    /// The points of arc `index`, reversed for a negative index (`~i`).
    fn arc(&self, index: i64, out: &mut Vec<[f64; 2]>) {
        let (arc, reverse) = if index < 0 {
            ((!index) as usize, true)
        } else {
            (index as usize, false)
        };
        let Some(points) = self.arcs.get(arc) else {
            return;
        };
        if !out.is_empty() {
            out.pop();
        }
        if reverse {
            out.extend(points.iter().rev());
        } else {
            out.extend(points.iter());
        }
    }

    fn line(&self, arcs: &Json) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        for index in arcs.as_array().into_iter().flatten() {
            self.arc(index.as_i64().unwrap_or(0), &mut out);
        }
        out
    }

    fn geometry(&self, object: &Json) -> Option<Geometry> {
        let arcs = object.get("arcs");
        let point = |p: &Json| [p[0].as_f64().unwrap_or(0.), p[1].as_f64().unwrap_or(0.)];
        Some(match object.get("type")?.as_str()? {
            "Point" => Geometry::Point(point(object.get("coordinates")?)),
            "MultiPoint" => Geometry::MultiPoint(
                object
                    .get("coordinates")?
                    .as_array()?
                    .iter()
                    .map(point)
                    .collect(),
            ),
            "LineString" => Geometry::LineString(self.line(arcs?)),
            "MultiLineString" => {
                Geometry::MultiLineString(arcs?.as_array()?.iter().map(|a| self.line(a)).collect())
            }
            "Polygon" => {
                Geometry::Polygon(arcs?.as_array()?.iter().map(|a| self.line(a)).collect())
            }
            "MultiPolygon" => Geometry::MultiPolygon(
                arcs?
                    .as_array()?
                    .iter()
                    .map(|p| {
                        p.as_array()
                            .into_iter()
                            .flatten()
                            .map(|a| self.line(a))
                            .collect()
                    })
                    .collect(),
            ),
            "GeometryCollection" => Geometry::Collection(
                object
                    .get("geometries")?
                    .as_array()?
                    .iter()
                    .filter_map(|g| self.geometry(g))
                    .collect(),
            ),
            _ => return None,
        })
    }

    /// `topojson.feature(topology, topology.objects[name])`: one record per
    /// geometry of a collection (or one for a lone geometry), as [`features`]
    /// lays them out.
    pub fn feature(&self, name: &str) -> Dataset {
        let Some(object) = self.objects.get(name) else {
            return Dataset::default();
        };
        let geometries: Vec<&Json> = match object.get("type").and_then(Json::as_str) {
            Some("GeometryCollection") => object["geometries"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default(),
            _ => vec![object],
        };
        Dataset::new(geometries.into_iter().filter_map(|g| {
            let geometry = self.geometry(g)?;
            let mut row = Table::empty().with("geometry", geometry.into_value());
            if let Some(id) = g.get("id") {
                // TopoJSON ids are FIPS codes: keep them as text, as d3 does.
                let id = match id {
                    Json::String(s) => Value::text(s.clone()),
                    other => json_value(other),
                };
                row = row.with("id", id);
            }
            if let Some(props) = g.get("properties").and_then(Json::as_object) {
                for (key, value) in props {
                    row = row.with(key.clone(), json_value(value));
                }
            }
            Some(row)
        }))
    }

    /// One object's geometry as a single collection, for `topojson.feature`
    /// on a lone object (`nation`).
    pub fn feature_geometry(&self, name: &str) -> Option<Geometry> {
        self.geometry(self.objects.get(name)?)
    }

    /// `topojson.mesh(topology, object, filter)`: the object's arcs as lines,
    /// each once. `filter(a, b)` sees the indices of the two geometries an arc
    /// separates (equal for an exterior arc); `None` keeps every arc.
    pub fn mesh(&self, name: &str, filter: Option<&dyn Fn(usize, usize) -> bool>) -> Geometry {
        let Some(object) = self.objects.get(name) else {
            return Geometry::MultiLineString(Vec::new());
        };
        // Each arc's geometries: which members of the collection use it.
        let mut users: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        let members: Vec<&Json> = match object.get("type").and_then(Json::as_str) {
            Some("GeometryCollection") => object["geometries"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default(),
            _ => vec![object],
        };
        fn walk(arcs: &Json, f: &mut dyn FnMut(usize)) {
            match arcs {
                Json::Array(items) => items.iter().for_each(|i| walk(i, f)),
                Json::Number(n) => {
                    let i = n.as_i64().unwrap_or(0);
                    f(if i < 0 { (!i) as usize } else { i as usize })
                }
                _ => {}
            }
        }
        for (g, member) in members.iter().enumerate() {
            if let Some(arcs) = member.get("arcs") {
                walk(arcs, &mut |arc| {
                    let list = users.entry(arc).or_default();
                    if list.last() != Some(&g) {
                        list.push(g);
                    }
                });
            }
        }
        let mut keys: Vec<usize> = users.keys().copied().collect();
        keys.sort_unstable();
        let lines = keys
            .into_iter()
            .filter(|arc| match filter {
                None => true,
                Some(f) => {
                    let u = &users[arc];
                    f(u[0], *u.last().unwrap_or(&u[0]))
                }
            })
            .filter_map(|arc| self.arcs.get(arc).cloned())
            .collect();
        Geometry::MultiLineString(lines)
    }
}

/// Plot's `graticule`: meridians every 10° and parallels every 10°, d3's
/// default `geoGraticule10` (meridians stop short of the poles except every
/// 90°).
pub fn graticule() -> Geometry {
    let mut lines = Vec::new();
    let step = |a: f64, b: f64, s: f64| -> Vec<f64> {
        let mut v = Vec::new();
        let mut x = a;
        while x <= b + 1e-9 {
            v.push(x);
            x += s;
        }
        v
    };
    for lon in step(-180., 170., 10.) {
        let extent = if (lon % 90.).abs() < 1e-9 { 90. } else { 80. };
        lines.push(
            step(-extent, extent, 2.5)
                .into_iter()
                .map(|lat| [lon, lat])
                .collect(),
        );
    }
    for lat in step(-80., 80., 10.) {
        lines.push(
            step(-180., 180., 2.5)
                .into_iter()
                .map(|lon| [lon, lat])
                .collect(),
        );
    }
    Geometry::MultiLineString(lines)
}

/// d3's `geoCentroid`: the spherical centroid of a geometry, in degrees.
/// Polygons weigh by area, lines by length, points equally — the highest
/// dimension present wins, as d3's does.
pub fn centroid(geometry: &Geometry) -> Option<[f64; 2]> {
    let mut points = [0.; 4];
    let mut lines = [0.; 4];
    let mut areas = [0.; 4];
    let cart = |p: [f64; 2]| {
        let (l, f) = (p[0] * RADIANS, p[1] * RADIANS);
        [f.cos() * l.cos(), f.cos() * l.sin(), f.sin()]
    };
    fn add(acc: &mut [f64; 4], v: [f64; 3], w: f64) {
        acc[0] += v[0] * w;
        acc[1] += v[1] * w;
        acc[2] += v[2] * w;
        acc[3] += w;
    }
    fn visit(
        g: &Geometry,
        cart: &dyn Fn([f64; 2]) -> [f64; 3],
        p: &mut [f64; 4],
        l: &mut [f64; 4],
        a: &mut [f64; 4],
    ) {
        // Each vertex's unit vector is computed once and carried to the
        // next segment, which starts where this one ends.
        let line = |ps: &[[f64; 2]], l: &mut [f64; 4]| {
            let Some((first, rest)) = ps.split_first() else {
                return;
            };
            let mut u = cart(*first);
            for p in rest {
                let v = cart(*p);
                let dot = (u[0] * v[0] + u[1] * v[1] + u[2] * v[2]).clamp(-1., 1.);
                let length = dot.acos();
                add(l, [u[0] + v[0], u[1] + v[1], u[2] + v[2]], length / 2.);
                u = v;
            }
        };
        // A ring's area-weighted centroid vector, by fanning triangles from
        // its first vertex (each weighs by its planar area on the unit
        // sphere), with the ring's signed total.
        let ring = |ps: &[[f64; 2]]| -> [f64; 4] {
            let mut acc = [0.; 4];
            if ps.len() < 3 {
                return acc;
            }
            let o = cart(ps[0]);
            let mut u = cart(ps[1]);
            for p in &ps[2..] {
                let v = cart(*p);
                let e1 = [u[0] - o[0], u[1] - o[1], u[2] - o[2]];
                let e2 = [v[0] - o[0], v[1] - o[1], v[2] - o[2]];
                let n = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                let c = [
                    (o[0] + u[0] + v[0]) / 3.,
                    (o[1] + u[1] + v[1]) / 3.,
                    (o[2] + u[2] + v[2]) / 3.,
                ];
                let area = (n[0] * c[0] + n[1] * c[1] + n[2] * c[2]) / 2.;
                add(&mut acc, c, area);
                u = v;
            }
            acc
        };
        // A polygon's rings weigh relative to its exterior ring's winding, so
        // holes subtract whichever way the file winds them.
        let polygon = |rings: &[Vec<[f64; 2]>], a: &mut [f64; 4]| {
            for (i, r) in rings.iter().enumerate() {
                let part = ring(r);
                // Normalize the ring's winding so its weight is its area's
                // magnitude: the exterior adds, a hole subtracts.
                let sign = if part[3] < 0. { -1. } else { 1. };
                let k = if i == 0 { sign } else { -sign };
                for j in 0..4 {
                    a[j] += part[j] * k;
                }
            }
        };
        match g {
            Geometry::Point(pt) => add(p, cart(*pt), 1.),
            Geometry::MultiPoint(ps) => ps.iter().for_each(|pt| add(p, cart(*pt), 1.)),
            Geometry::LineString(ps) => line(ps, l),
            Geometry::MultiLineString(ls) => ls.iter().for_each(|ps| line(ps, l)),
            Geometry::Polygon(rings) => polygon(rings, a),
            Geometry::MultiPolygon(polys) => polys.iter().for_each(|rings| polygon(rings, a)),
            Geometry::Collection(gs) => gs.iter().for_each(|g| visit(g, cart, p, l, a)),
            Geometry::Sphere => {}
        }
    }
    visit(geometry, &cart, &mut points, &mut lines, &mut areas);
    let pick = if areas[3].abs() > 1e-12 {
        areas
    } else if lines[3] > 1e-12 {
        lines
    } else {
        points
    };
    let m = (pick[0] * pick[0] + pick[1] * pick[1] + pick[2] * pick[2]).sqrt();
    if m < 1e-12 {
        return None;
    }
    Some([
        pick[1].atan2(pick[0]) * DEGREES,
        (pick[2] / m).clamp(-1., 1.).asin() * DEGREES,
    ])
}

/// A d3-geo projection's raw form: radians in, a unit plane out (y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Raw {
    Equirectangular,
    Mercator,
    TransverseMercator,
    Orthographic,
    Stereographic,
    Gnomonic,
    AzimuthalEqualArea,
    AzimuthalEquidistant,
    EqualEarth,
    /// A conic equal-area projection with standard parallels (radians).
    ConicEqualArea(f64, f64),
    ConicConformal(f64, f64),
    ConicEquidistant(f64, f64),
}

impl Raw {
    fn forward(self, l: f64, f: f64) -> Option<[f64; 2]> {
        Some(match self {
            Raw::Equirectangular => [l, f],
            Raw::Mercator => [l, ((FRAC_PI_2 + f) / 2.).tan().ln()],
            Raw::TransverseMercator => [((FRAC_PI_2 + f) / 2.).tan().ln(), -l],
            Raw::Orthographic => [f.cos() * l.sin(), f.sin()],
            Raw::Stereographic => {
                let k = 1. + f.cos() * l.cos();
                if k <= 1e-9 {
                    return None;
                }
                [f.cos() * l.sin() / k, f.sin() / k]
            }
            Raw::Gnomonic => {
                let k = f.cos() * l.cos();
                if k <= 1e-9 {
                    return None;
                }
                [f.cos() * l.sin() / k, f.sin() / k]
            }
            Raw::AzimuthalEqualArea => {
                let c = f.cos() * l.cos();
                let k = (2. / (1. + c)).sqrt();
                if !k.is_finite() {
                    return None;
                }
                [k * f.cos() * l.sin(), k * f.sin()]
            }
            Raw::AzimuthalEquidistant => {
                let c = (f.cos() * l.cos()).clamp(-1., 1.).acos();
                let k = if c.abs() < 1e-9 { 1. } else { c / c.sin() };
                [k * f.cos() * l.sin(), k * f.sin()]
            }
            Raw::EqualEarth => {
                let (a1, a2, a3, a4) = (1.340264, -0.081106, 0.000893, 0.003796);
                let m = 3f64.sqrt() / 2.;
                let t = (m * f.sin()).asin();
                let t2 = t * t;
                let t6 = t2 * t2 * t2;
                [
                    l * t.cos() / (m * (a1 + 3. * a2 * t2 + t6 * (7. * a3 + 9. * a4 * t2))),
                    t * (a1 + a2 * t2 + t6 * (a3 + a4 * t2)),
                ]
            }
            Raw::ConicEqualArea(y0, y1) => {
                let sy0 = y0.sin();
                let n = (sy0 + y1.sin()) / 2.;
                let c = 1. + sy0 * (2. * n - sy0);
                let r0 = c.sqrt() / n;
                let r = (c - 2. * n * f.sin()).max(0.).sqrt() / n;
                [r * (l * n).sin(), r0 - r * (l * n).cos()]
            }
            Raw::ConicConformal(y0, y1) => {
                let tany = |y: f64| ((FRAC_PI_2 + y) / 2.).tan();
                let cy0 = y0.cos();
                let n = if (y0 - y1).abs() < 1e-9 {
                    y0.sin()
                } else {
                    (cy0 / y1.cos()).ln() / (tany(y1) / tany(y0)).ln()
                };
                let fk = cy0 * tany(y0).powf(n) / n;
                let f = if fk > 0. {
                    f.max(-FRAC_PI_2 + 1e-6)
                } else {
                    f.min(FRAC_PI_2 - 1e-6)
                };
                let r = fk / tany(f).powf(n);
                [r * (n * l).sin(), fk - r * (n * l).cos()]
            }
            Raw::ConicEquidistant(y0, y1) => {
                let cy0 = y0.cos();
                let n = if (y0 - y1).abs() < 1e-9 {
                    y0.sin()
                } else {
                    (cy0 - y1.cos()) / (y1 - y0)
                };
                let g = cy0 / n + y0;
                let gy = g - f;
                let nx = n * l;
                [gy * nx.sin(), g - gy * nx.cos()]
            }
        })
    }

    /// d3's default `clipAngle` of an azimuthal projection, degrees, else
    /// `None` for an antimeridian cut.
    fn clip_angle(self) -> Option<f64> {
        match self {
            Raw::Orthographic => Some(90. + 1e-6),
            Raw::Stereographic => Some(142.),
            Raw::Gnomonic => Some(60.),
            Raw::AzimuthalEqualArea | Raw::AzimuthalEquidistant => Some(180. - 1e-3),
            _ => None,
        }
    }
}

/// A projection as Plot builds it: the raw projection, rotation, and Plot's
/// fit of its unit plane to the frame, d3's `scale` and `translate`.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectionKind {
    /// A d3 projection: its raw form and its `[kx, ky]` extent, Plot's
    /// `scaleProjection` constants.
    D3(Raw, [f64; 2]),
    /// d3's `geoAlbersUsa`: lower 48, Alaska and Hawaii insets.
    AlbersUsa,
    /// Plot's `identity`: the coordinates are already pixels.
    Identity,
    /// Plot's `reflect-y`: pixels, y flipped.
    ReflectY,
}

/// Plot's `projection` option.
#[derive(Clone, Debug, PartialEq)]
pub struct GeoProjection {
    pub kind: ProjectionKind,
    /// d3's `rotate`, degrees `[λ, φ, γ]`.
    pub rotate: [f64; 3],
    /// Plot's projection `domain`: a geometry the projection fits to the frame.
    pub domain: Option<Rc<Geometry>>,
    /// Plot's projection `inset`, pixels.
    pub inset: f32,
    /// Conic `parallels`, degrees.
    pub parallels: Option<[f64; 2]>,
    /// d3's `center`, degrees: the (rotated) point the projection places at
    /// the frame's middle.
    pub center: [f64; 2],
    /// Plot's numeric projection `clip`, d3's `clipAngle`, degrees: a
    /// small-circle clip in place of the projection's default.
    pub clip_angle: Option<f64>,
}

impl GeoProjection {
    /// A projection by Plot's name: `"albers-usa"`, `"equal-earth"`,
    /// `"orthographic"`, `"mercator"` …
    pub fn named(name: &str) -> Option<Self> {
        let deg = |a: f64, b: f64| (a * RADIANS, b * RADIANS);
        let (raw, k) = match name.to_ascii_lowercase().as_str() {
            "albers-usa" => {
                return Some(Self::new(ProjectionKind::AlbersUsa));
            }
            "identity" => return Some(Self::new(ProjectionKind::Identity)),
            "reflect-y" => return Some(Self::new(ProjectionKind::ReflectY)),
            "albers" => {
                let (a, b) = deg(29.5, 45.5);
                let mut p = Self::new(ProjectionKind::D3(
                    Raw::ConicEqualArea(a, b),
                    [0.7463, 0.4673],
                ));
                p.rotate = [96., 0., 0.];
                p.parallels = Some([29.5, 45.5]);
                p.center = [-0.6, 38.7];
                return Some(p);
            }
            "azimuthal-equal-area" => (Raw::AzimuthalEqualArea, [4., 4.]),
            "azimuthal-equidistant" => (Raw::AzimuthalEquidistant, [TAU, TAU]),
            "conic-conformal" => {
                let (a, b) = deg(30., 30.);
                (Raw::ConicConformal(a, b), [TAU, TAU])
            }
            "conic-equal-area" => {
                let (a, b) = deg(0., 60.);
                (Raw::ConicEqualArea(a, b), [6.1702, 2.9781])
            }
            "conic-equidistant" => {
                let (a, b) = deg(0., 60.);
                (Raw::ConicEquidistant(a, b), [7.312, 3.6282])
            }
            "equal-earth" => (Raw::EqualEarth, [5.4133, 2.6347]),
            "equirectangular" => (Raw::Equirectangular, [TAU, PI]),
            "gnomonic" => (Raw::Gnomonic, [3.4641, 3.4641]),
            "mercator" => (Raw::Mercator, [TAU, TAU]),
            "orthographic" => (Raw::Orthographic, [2., 2.]),
            "stereographic" => (Raw::Stereographic, [2., 2.]),
            "transverse-mercator" => (Raw::TransverseMercator, [TAU, TAU]),
            _ => return None,
        };
        let mut p = Self::new(ProjectionKind::D3(raw, k));
        // d3's conic defaults centre the projection on a mid-latitude.
        if matches!(raw, Raw::ConicEqualArea(..)) {
            p.center = [0., 33.6442];
        }
        Some(p)
    }

    /// d3's `center`, degrees.
    pub fn center(mut self, center: [f64; 2]) -> Self {
        self.center = center;
        self
    }

    fn new(kind: ProjectionKind) -> Self {
        Self {
            kind,
            rotate: [0., 0., 0.],
            domain: None,
            inset: 0.,
            parallels: None,
            center: [0., 0.],
            clip_angle: None,
        }
    }

    /// Plot's numeric projection `clip`, d3's `clipAngle`, degrees.
    pub fn clip_angle(mut self, angle: f64) -> Self {
        self.clip_angle = Some(angle);
        self
    }

    /// d3's `rotate`.
    pub fn rotate(mut self, rotate: [f64; 3]) -> Self {
        self.rotate = rotate;
        self
    }

    /// Plot's projection `domain`.
    pub fn domain(mut self, geometry: Geometry) -> Self {
        self.domain = Some(Rc::new(geometry));
        self
    }

    /// Plot's projection `inset`.
    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }

    /// A conic projection's `parallels`, degrees.
    pub fn parallels(mut self, parallels: [f64; 2]) -> Self {
        self.parallels = Some(parallels);
        if let ProjectionKind::D3(raw, k) = &mut self.kind {
            let (a, b) = (parallels[0] * RADIANS, parallels[1] * RADIANS);
            *raw = match *raw {
                Raw::ConicEqualArea(..) => Raw::ConicEqualArea(a, b),
                Raw::ConicConformal(..) => Raw::ConicConformal(a, b),
                Raw::ConicEquidistant(..) => Raw::ConicEquidistant(a, b),
                other => other,
            };
            let _ = k;
        }
        self
    }

    /// The aspect ratio (height / width) Plot gives the projection's frame.
    pub fn aspect_ratio(&self) -> Option<f64> {
        match &self.kind {
            ProjectionKind::D3(_, [kx, ky]) => Some(ky / kx),
            ProjectionKind::AlbersUsa => Some(0.4673 / 0.7463),
            _ => None,
        }
    }

    /// Fit the projection to a `width × height` frame, Plot's way: d3's
    /// default scale for the frame (or a fit to the domain), centred.
    pub fn fit(&self, width: f64, height: f64) -> Fitted {
        let inset = self.inset as f64;
        let (w, h) = ((width - 2. * inset).max(1.), (height - 2. * inset).max(1.));
        // Plot's `precision: 0.15`, d3's resampling threshold.
        let delta2 = PRECISION * PRECISION;
        let base = match &self.kind {
            ProjectionKind::D3(_, [kx, ky]) => Fitted {
                projection: self.clone(),
                scale: (w / kx).min(h / ky),
                translate: [w / 2. + inset, h / 2. + inset],
                delta2,
            },
            ProjectionKind::AlbersUsa => Fitted {
                projection: self.clone(),
                scale: (w / 0.7463).min(h / 0.4673),
                translate: [w / 2. + inset, h / 2. + inset],
                delta2,
            },
            _ => Fitted {
                projection: self.clone(),
                scale: 1.,
                translate: [inset, inset],
                delta2,
            },
        };
        let Some(domain) = &self.domain else {
            return base;
        };
        // Plot fits the domain's bounds, as the projection draws them before
        // its insets, into the frame, then scales and translates the
        // projection's output: the resampling happens before that scale.
        let t0 = [base.translate[0] - inset, base.translate[1] - inset];
        let unfit = Fitted {
            translate: t0,
            ..base.clone()
        };
        let Some([[x0, y0], [x1, y1]]) = unfit.bounds(domain) else {
            return base;
        };
        let k = (w / (x1 - x0)).min(h / (y1 - y0));
        if !(k > 0. && k.is_finite()) {
            return base;
        }
        Fitted {
            scale: k * base.scale,
            translate: [
                k * t0[0] + inset + (w - k * (x0 + x1)) / 2.,
                k * t0[1] + inset + (h - k * (y0 + y1)) / 2.,
            ],
            delta2: delta2 * k * k,
            ..base
        }
    }
}

/// A projection fit to a frame: it projects degrees to frame pixels.
#[derive(Clone, Debug)]
pub struct Fitted {
    pub projection: GeoProjection,
    pub scale: f64,
    pub translate: [f64; 2],
    /// d3's squared resampling precision, in these pixels.
    delta2: f64,
}

impl Fitted {
    fn rotate_point(&self, l: f64, f: f64) -> (f64, f64) {
        let [dl, df, dg] = self.projection.rotate.map(|d| d * RADIANS);
        // d3's rotateRadians: λ first, then φ and γ.
        let mut l = l + dl;
        if l > PI {
            l -= TAU;
        } else if l < -PI {
            l += TAU;
        }
        if df == 0. && dg == 0. {
            return (l, f);
        }
        let (cdf, sdf, cdg, sdg) = (df.cos(), df.sin(), dg.cos(), dg.sin());
        let cf = f.cos();
        let x = l.cos() * cf;
        let y = l.sin() * cf;
        let z = f.sin();
        let k = z * cdf + x * sdf;
        (
            (y * cdg - k * sdg).atan2(x * cdf - z * sdf),
            (k * cdg + y * sdg).clamp(-1., 1.).asin(),
        )
    }

    /// The projection's pre-clip: its `clipAngle`, else d3's default for
    /// the raw projection, else the antimeridian cut.
    fn clip(&self) -> Option<clip::Clip> {
        let ProjectionKind::D3(raw, _) = &self.projection.kind else {
            return None;
        };
        Some(match self.projection.clip_angle.or(raw.clip_angle()) {
            Some(angle) if angle != 0. => clip::Clip::Circle(angle * RADIANS),
            _ => clip::Clip::Antimeridian,
        })
    }

    /// The raw point for a degree position after rotation, or `None` when it
    /// falls outside the projection's clip.
    fn raw(&self, lon: f64, lat: f64) -> Option<[f64; 2]> {
        match &self.projection.kind {
            ProjectionKind::Identity => Some([lon, lat]),
            ProjectionKind::ReflectY => Some([lon, -lat]),
            ProjectionKind::AlbersUsa => albers_usa(lon, lat),
            ProjectionKind::D3(raw, _) => {
                let (l, f) = self.rotate_point(lon * RADIANS, lat * RADIANS);
                if !self.clip()?.visible(l, f) {
                    return None;
                }
                raw.forward(l, f)
            }
        }
    }

    /// Project a degree position to frame pixels (y down).
    pub fn project(&self, lon: f64, lat: f64) -> Option<[f64; 2]> {
        let [x, y] = self.raw(lon, lat)?;
        match self.projection.kind {
            // Plot's identity and reflect-y take a domain's fit: scale, then
            // translate.
            ProjectionKind::Identity | ProjectionKind::ReflectY => Some([
                x * self.scale + self.translate[0],
                y * self.scale + self.translate[1],
            ]),
            _ => Some(self.to_px([x, y])),
        }
    }

    /// The raw point d3's `center` puts at the translate: the centre
    /// projected without the rotation, as d3 does.
    fn center_offset(&self) -> [f64; 2] {
        match &self.projection.kind {
            ProjectionKind::D3(raw, _) => {
                let [cl, cf] = self.projection.center;
                if cl == 0. && cf == 0. {
                    return [0., 0.];
                }
                raw.forward(cl * RADIANS, cf * RADIANS).unwrap_or([0., 0.])
            }
            _ => [0., 0.],
        }
    }

    /// A raw (y-up) point in frame pixels, with d3's centre, scale and
    /// translate.
    fn to_px(&self, p: [f64; 2]) -> [f64; 2] {
        let [cx, cy] = self.center_offset();
        [
            self.translate[0] + (p[0] - cx) * self.scale,
            self.translate[1] - (p[1] - cy) * self.scale,
        ]
    }

    /// A rotated, clipped point (radians) in frame pixels: d3's
    /// `projectTransform`. Mercator's infinite poles are held to its square,
    /// as d3's Mercator clip extent holds them.
    fn transform(&self, l: f64, f: f64) -> [f64; 2] {
        let ProjectionKind::D3(raw, _) = &self.projection.kind else {
            return [f64::NAN; 2];
        };
        match raw.forward(l, f) {
            Some([x, y]) if matches!(raw, Raw::Mercator | Raw::TransverseMercator) => {
                self.to_px([x.clamp(-PI, PI), y.clamp(-PI, PI)])
            }
            Some(p) => self.to_px(p),
            None => [f64::NAN; 2],
        }
    }

    /// The projected paths of a geometry: closed rings (for fills) and open
    /// lines, in frame pixels, as d3's `geoPath` streams them: rotated,
    /// clipped at the antimeridian or the clip circle (polygons closed along
    /// the clip edge), then resampled adaptively to Plot's precision.
    pub fn paths(&self, geometry: &Geometry) -> Vec<(Vec<[f64; 2]>, bool)> {
        let mut out = Vec::new();
        match self.clip() {
            Some(clip) => self.stream(clip, geometry, &mut out),
            None => self.collect_planar(geometry, &mut out),
        }
        out
    }

    fn stream(&self, clip: clip::Clip, geometry: &Geometry, out: &mut Vec<(Vec<[f64; 2]>, bool)>) {
        let rotate = |ps: &[[f64; 2]]| -> Vec<[f64; 2]> {
            ps.iter()
                .map(|p| {
                    let (l, f) = self.rotate_point(p[0] * RADIANS, p[1] * RADIANS);
                    [l, f]
                })
                .collect()
        };
        // d3 streams a ring without its closing position.
        let open = |ring: &Vec<[f64; 2]>| -> Vec<[f64; 2]> {
            let n = ring.len();
            let closed = n > 1 && ring[0] == ring[n - 1];
            rotate(&ring[..if closed { n - 1 } else { n }])
        };
        let polygon = |rings: &[Vec<[f64; 2]>], out: &mut Vec<(Vec<[f64; 2]>, bool)>| {
            let rings: Vec<Vec<[f64; 2]>> = rings.iter().map(open).collect();
            clip.polygon(&rings, &mut Resample::new(self, true, out));
        };
        match geometry {
            Geometry::Point(_) | Geometry::MultiPoint(_) => {}
            Geometry::LineString(ps) => clip.line(&rotate(ps), &mut Resample::new(self, false, out)),
            Geometry::MultiLineString(ls) => {
                for l in ls {
                    clip.line(&rotate(l), &mut Resample::new(self, false, out));
                }
            }
            Geometry::Polygon(rings) => polygon(rings, out),
            Geometry::MultiPolygon(polys) => polys.iter().for_each(|rings| polygon(rings, out)),
            Geometry::Collection(gs) => gs.iter().for_each(|g| self.stream(clip, g, out)),
            Geometry::Sphere => clip.sphere(&mut Resample::new(self, true, out)),
        }
    }

    /// The paths of a planar projection (`identity`, `reflect-y`) or Albers
    /// USA: each position projected, lines broken where a point drops out.
    fn collect_planar(&self, geometry: &Geometry, out: &mut Vec<(Vec<[f64; 2]>, bool)>) {
        let line = |ps: &[[f64; 2]], closed: bool, out: &mut Vec<(Vec<[f64; 2]>, bool)>| {
            let mut current = Vec::new();
            for p in ps {
                match self.project(p[0], p[1]) {
                    Some(xy) => current.push(xy),
                    None if closed => {}
                    None => {
                        if current.len() >= 2 {
                            out.push((std::mem::take(&mut current), false));
                        }
                        current.clear();
                    }
                }
            }
            if current.len() >= if closed { 3 } else { 2 } {
                out.push((current, closed));
            }
        };
        match geometry {
            Geometry::Point(_) | Geometry::MultiPoint(_) | Geometry::Sphere => {}
            Geometry::LineString(ps) => line(ps, false, out),
            Geometry::MultiLineString(ls) => ls.iter().for_each(|l| line(l, false, out)),
            Geometry::Polygon(rings) => rings.iter().for_each(|r| line(r, true, out)),
            Geometry::MultiPolygon(polys) => polys.iter().flatten().for_each(|r| line(r, true, out)),
            Geometry::Collection(gs) => gs.iter().for_each(|g| self.collect_planar(g, out)),
        }
    }

    /// The globe's outline in pixels, d3's `geoPath` of the sphere: the clip
    /// circle of an azimuthal projection, else the antimeridian and poles.
    pub fn sphere(&self) -> Option<Vec<[f64; 2]>> {
        let clip = self.clip()?;
        let mut out = Vec::new();
        clip.sphere(&mut Resample::new(self, true, &mut out));
        out.into_iter().next().map(|(ring, _)| ring)
    }

    /// The projected bounds of a geometry, `[[x0, y0], [x1, y1]]`.
    pub fn bounds(&self, geometry: &Geometry) -> Option<[[f64; 2]; 2]> {
        let mut points: Vec<[f64; 2]> = Vec::new();
        for (path, _) in self.paths(geometry) {
            points.extend(path);
        }
        let mut positions = Vec::new();
        geometry.positions(&mut positions);
        points.extend(
            positions
                .into_iter()
                .filter_map(|p| self.project(p[0], p[1])),
        );
        if points.is_empty() {
            return None;
        }
        let mut b = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
        for [x, y] in points {
            b[0][0] = b[0][0].min(x);
            b[0][1] = b[0][1].min(y);
            b[1][0] = b[1][0].max(x);
            b[1][1] = b[1][1].max(y);
        }
        Some(b)
    }

    /// The projected (planar) centroid of a geometry, d3's `geoPath.centroid`.
    pub fn centroid(&self, geometry: &Geometry) -> Option<[f64; 2]> {
        let (mut cx, mut cy, mut area) = (0., 0., 0.);
        let (mut lx, mut ly, mut length) = (0., 0., 0.);
        for (path, closed) in self.paths(geometry) {
            if closed {
                for i in 0..path.len() {
                    let [x0, y0] = path[i];
                    let [x1, y1] = path[(i + 1) % path.len()];
                    let cross = x0 * y1 - x1 * y0;
                    area += cross;
                    cx += (x0 + x1) * cross;
                    cy += (y0 + y1) * cross;
                }
            } else {
                for w in path.windows(2) {
                    let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
                    lx += (w[0][0] + w[1][0]) / 2. * d;
                    ly += (w[0][1] + w[1][1]) / 2. * d;
                    length += d;
                }
            }
        }
        if area.abs() > 1e-9 {
            return Some([cx / (3. * area), cy / (3. * area)]);
        }
        if length > 0. {
            return Some([lx / length, ly / length]);
        }
        let mut positions = Vec::new();
        geometry.positions(&mut positions);
        let projected: Vec<[f64; 2]> = positions
            .into_iter()
            .filter_map(|p| self.project(p[0], p[1]))
            .collect();
        if projected.is_empty() {
            return None;
        }
        let n = projected.len() as f64;
        Some([
            projected.iter().map(|p| p[0]).sum::<f64>() / n,
            projected.iter().map(|p| p[1]).sum::<f64>() / n,
        ])
    }
}

/// d3's `resample` at the end of the projection stream: each clipped line
/// projected, with points added along great circles wherever the projected
/// path strays from the straight segment by more than the precision, then
/// collected as paths (closed when streamed as a polygon's rings).
struct Resample<'a> {
    fitted: &'a Fitted,
    ring: bool,
    out: &'a mut Vec<(Vec<[f64; 2]>, bool)>,
    current: Vec<[f64; 2]>,
    /// The previous point: pixels, longitude and unit vector.
    previous: Option<ResamplePoint>,
    first: Option<ResamplePoint>,
}

type ResamplePoint = ([f64; 2], f64, [f64; 3]);

impl<'a> Resample<'a> {
    fn new(fitted: &'a Fitted, ring: bool, out: &'a mut Vec<(Vec<[f64; 2]>, bool)>) -> Self {
        Self {
            fitted,
            ring,
            out,
            current: Vec::new(),
            previous: None,
            first: None,
        }
    }

    /// d3's `resampleLineTo`.
    fn line_to(&mut self, (p0, l0, c0): ResamplePoint, (p1, l1, c1): ResamplePoint, depth: u32) {
        // cos 30°, d3's minimum angular distance.
        const COS_MIN_DISTANCE: f64 = 0.866_025_403_784_438_6;
        let delta2 = self.fitted.delta2;
        let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
        let d2 = dx * dx + dy * dy;
        if !(d2 > 4. * delta2 && depth > 0) {
            return;
        }
        let depth = depth - 1;
        let (a, b, c) = (c0[0] + c1[0], c0[1] + c1[1], c0[2] + c1[2]);
        let m = (a * a + b * b + c * c).sqrt();
        let c = c / m;
        let phi2 = c.clamp(-1., 1.).asin();
        let lambda2 = if (c.abs() - 1.).abs() < 1e-6 || (l0 - l1).abs() < 1e-6 {
            (l0 + l1) / 2.
        } else {
            b.atan2(a)
        };
        let p2 = self.fitted.transform(lambda2, phi2);
        let (dx2, dy2) = (p2[0] - p0[0], p2[1] - p0[1]);
        let dz = dy * dx2 - dx * dy2;
        if dz * dz / d2 > delta2
            || ((dx * dx2 + dy * dy2) / d2 - 0.5).abs() > 0.3
            || c0[0] * c1[0] + c0[1] * c1[1] + c0[2] * c1[2] < COS_MIN_DISTANCE
        {
            let mid = (p2, lambda2, [a / m, b / m, c]);
            self.line_to((p0, l0, c0), mid, depth);
            self.current.push(p2);
            self.line_to(mid, (p1, l1, c1), depth);
        }
    }
}

impl clip::Sink for Resample<'_> {
    fn point(&mut self, l: f64, f: f64, _: u8) {
        let p = (self.fitted.transform(l, f), l, clip::cartesian([l, f]));
        if let Some(previous) = self.previous {
            self.line_to(previous, p, 16);
        }
        self.current.push(p.0);
        self.previous = Some(p);
        if self.first.is_none() {
            self.first = Some(p);
        }
    }

    fn line_start(&mut self) {
        self.current.clear();
        self.previous = None;
        self.first = None;
    }

    fn line_end(&mut self) {
        if self.ring
            && let (Some(previous), Some(first)) = (self.previous, self.first)
        {
            self.line_to(previous, first, 16);
        }
        let mut path = std::mem::take(&mut self.current);
        path.retain(|p| p[0].is_finite() && p[1].is_finite());
        if path.len() >= 2 {
            self.out.push((path, self.ring));
        }
        self.previous = None;
        self.first = None;
    }
}

/// d3's `geoAlbersUsa` at scale 1, translate 0, y up: the lower 48 on an
/// Albers conic, Alaska (at 0.35×) and Hawaii on conics of their own, each
/// offset and clipped as d3 composes them.
fn albers_usa(lon: f64, lat: f64) -> Option<[f64; 2]> {
    // One conic: rotated by `rotate`, centred so `center` (in the rotated
    // frame, as d3's `center` is) lands at the origin, in pixels y down.
    let conic = |rotate: f64,
                 center: [f64; 2],
                 parallels: [f64; 2],
                 scale: f64,
                 offset: [f64; 2]|
     -> [f64; 2] {
        let raw = Raw::ConicEqualArea(parallels[0] * RADIANS, parallels[1] * RADIANS);
        let mut l = (lon + rotate) * RADIANS;
        if l > PI {
            l -= TAU;
        } else if l < -PI {
            l += TAU;
        }
        let [x, y] = raw.forward(l, lat * RADIANS).unwrap_or([f64::NAN; 2]);
        let [cx, cy] = raw
            .forward(center[0] * RADIANS, center[1] * RADIANS)
            .unwrap_or([0., 0.]);
        [(x - cx) * scale + offset[0], -(y - cy) * scale + offset[1]]
    };
    let inside = |p: [f64; 2], x0: f64, y0: f64, x1: f64, y1: f64| {
        p[0] >= x0 && p[0] < x1 && p[1] >= y0 && p[1] < y1
    };
    let lower48 = conic(96., [-0.6, 38.7], [29.5, 45.5], 1., [0., 0.]);
    let alaska = conic(154., [-2., 58.5], [55., 65.], 0.35, [-0.307, 0.201]);
    let hawaii = conic(157., [-3., 19.9], [8., 18.], 1., [-0.205, 0.212]);
    let up = |p: [f64; 2]| Some([p[0], -p[1]]);
    if inside(lower48, -0.455, -0.238, 0.455, 0.238) {
        return up(lower48);
    }
    if inside(alaska, -0.425, 0.120, -0.214, 0.234) {
        return up(alaska);
    }
    if inside(hawaii, -0.214, 0.166, -0.115, 0.234) {
        return up(hawaii);
    }
    None
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn topojson_decodes_quantized_arcs() {
        let json: Json = serde_json::from_str(
            r#"{"type":"Topology","transform":{"scale":[1,1],"translate":[0,0]},
                "arcs":[[[0,0],[10,0],[0,10],[-10,0],[0,-10]]],
                "objects":{"box":{"type":"GeometryCollection","geometries":[{"type":"Polygon","arcs":[[0]],"id":"01"}]}}}"#,
        )
        .unwrap();
        let topology = Topology::parse(&json).unwrap();
        let features = topology.feature("box");
        assert_eq!(features.len(), 1);
        let geometry = features.rows()[0].value("geometry");
        let Some(Geometry::Polygon(rings)) = geometry.as_geo() else {
            panic!()
        };
        assert_eq!(
            rings[0],
            vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.], [0., 0.]]
        );
        assert_eq!(features.rows()[0].value("id"), Value::text("01"));
        let Geometry::MultiLineString(mesh) = topology.mesh("box", None) else {
            panic!()
        };
        assert_eq!(mesh.len(), 1);
    }

    #[test]
    fn projections_place_known_points() {
        let p = GeoProjection::named("equirectangular")
            .unwrap()
            .fit(640., 320.);
        let [x, y] = p.project(0., 0.).unwrap();
        assert!((x - 320.).abs() < 1e-6 && (y - 160.).abs() < 1e-6);
        let o = GeoProjection::named("orthographic")
            .unwrap()
            .fit(400., 400.);
        assert!(o.project(180., 0.).is_none(), "the far side is clipped");
        let usa = GeoProjection::named("albers-usa").unwrap().fit(975., 610.);
        let [x, y] = usa.project(-98., 39.).unwrap();
        assert!(
            x > 400. && x < 600. && y > 250. && y < 350.,
            "Kansas near the middle: {x},{y}"
        );
        assert!(usa.project(-150., 61.).is_some(), "Alaska is inset");
    }

    /// [`centroid`] as it was before each vertex's unit vector was carried
    /// to the next segment: every segment converts both its ends.
    fn centroid_converting_twice(geometry: &Geometry) -> Option<[f64; 2]> {
        let mut points = [0.; 4];
        let mut lines = [0.; 4];
        let mut areas = [0.; 4];
        let cart = |p: [f64; 2]| {
            let (l, f) = (p[0] * RADIANS, p[1] * RADIANS);
            [f.cos() * l.cos(), f.cos() * l.sin(), f.sin()]
        };
        fn add(acc: &mut [f64; 4], v: [f64; 3], w: f64) {
            acc[0] += v[0] * w;
            acc[1] += v[1] * w;
            acc[2] += v[2] * w;
            acc[3] += w;
        }
        fn visit(
            g: &Geometry,
            cart: &dyn Fn([f64; 2]) -> [f64; 3],
            p: &mut [f64; 4],
            l: &mut [f64; 4],
            a: &mut [f64; 4],
        ) {
            let line = |ps: &[[f64; 2]], l: &mut [f64; 4]| {
                for w in ps.windows(2) {
                    let (u, v) = (cart(w[0]), cart(w[1]));
                    let dot = (u[0] * v[0] + u[1] * v[1] + u[2] * v[2]).clamp(-1., 1.);
                    add(l, [u[0] + v[0], u[1] + v[1], u[2] + v[2]], dot.acos() / 2.);
                }
            };
            let ring = |ps: &[[f64; 2]]| -> [f64; 4] {
                let mut acc = [0.; 4];
                if ps.len() < 3 {
                    return acc;
                }
                let o = cart(ps[0]);
                for w in ps[1..].windows(2) {
                    let (u, v) = (cart(w[0]), cart(w[1]));
                    let e1 = [u[0] - o[0], u[1] - o[1], u[2] - o[2]];
                    let e2 = [v[0] - o[0], v[1] - o[1], v[2] - o[2]];
                    let n = [
                        e1[1] * e2[2] - e1[2] * e2[1],
                        e1[2] * e2[0] - e1[0] * e2[2],
                        e1[0] * e2[1] - e1[1] * e2[0],
                    ];
                    let c = [
                        (o[0] + u[0] + v[0]) / 3.,
                        (o[1] + u[1] + v[1]) / 3.,
                        (o[2] + u[2] + v[2]) / 3.,
                    ];
                    add(&mut acc, c, (n[0] * c[0] + n[1] * c[1] + n[2] * c[2]) / 2.);
                }
                acc
            };
            let polygon = |rings: &[Vec<[f64; 2]>], a: &mut [f64; 4]| {
                for (i, r) in rings.iter().enumerate() {
                    let part = ring(r);
                    let sign = if part[3] < 0. { -1. } else { 1. };
                    let k = if i == 0 { sign } else { -sign };
                    for j in 0..4 {
                        a[j] += part[j] * k;
                    }
                }
            };
            match g {
                Geometry::Point(pt) => add(p, cart(*pt), 1.),
                Geometry::MultiPoint(ps) => ps.iter().for_each(|pt| add(p, cart(*pt), 1.)),
                Geometry::LineString(ps) => line(ps, l),
                Geometry::MultiLineString(ls) => ls.iter().for_each(|ps| line(ps, l)),
                Geometry::Polygon(rings) => polygon(rings, a),
                Geometry::MultiPolygon(polys) => polys.iter().for_each(|rings| polygon(rings, a)),
                Geometry::Collection(gs) => gs.iter().for_each(|g| visit(g, cart, p, l, a)),
                Geometry::Sphere => {}
            }
        }
        visit(geometry, &cart, &mut points, &mut lines, &mut areas);
        let pick = if areas[3].abs() > 1e-12 {
            areas
        } else if lines[3] > 1e-12 {
            lines
        } else {
            points
        };
        let m = (pick[0] * pick[0] + pick[1] * pick[1] + pick[2] * pick[2]).sqrt();
        if m < 1e-12 {
            return None;
        }
        Some([
            pick[1].atan2(pick[0]) * DEGREES,
            (pick[2] / m).clamp(-1., 1.).asin() * DEGREES,
        ])
    }

    #[test]
    fn carried_unit_vectors_keep_centroids_bit_for_bit() {
        let mut seed = 5_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut path = |n: usize| -> Vec<[f64; 2]> {
            let (lon, lat) = (next() * 360. - 180., next() * 160. - 80.);
            (0..n)
                .map(|_| [lon + next() * 8. - 4., lat + next() * 8. - 4.])
                .collect()
        };
        let mut shapes = Vec::new();
        for n in [0, 1, 2, 3, 4, 7, 40] {
            shapes.push(Geometry::LineString(path(n)));
            shapes.push(Geometry::MultiLineString(vec![path(n), path(n + 1)]));
            shapes.push(Geometry::Polygon(vec![path(n), path(n / 2 + 1)]));
            shapes.push(Geometry::MultiPolygon(vec![
                vec![path(n + 3)],
                vec![path(n), path(3)],
            ]));
            shapes.push(Geometry::MultiPoint(path(n)));
            shapes.push(Geometry::Collection(vec![
                Geometry::Polygon(vec![path(n + 3)]),
                Geometry::LineString(path(n)),
                Geometry::Point(path(1)[0]),
            ]));
        }
        shapes.push(Geometry::Polygon(vec![vec![
            [0., 0.],
            [f64::NAN, 1.],
            [1., 1.],
            [0., 0.],
        ]]));
        shapes.push(Geometry::Sphere);
        let bits = |c: Option<[f64; 2]>| c.map(|[x, y]| [x.to_bits(), y.to_bits()]);
        for shape in &shapes {
            assert_eq!(
                bits(centroid(shape)),
                bits(centroid_converting_twice(shape)),
                "{shape:?}"
            );
        }
    }

    #[test]
    fn centroids_sit_inside_shapes() {
        let square = Geometry::Polygon(vec![vec![
            [0., 0.],
            [10., 0.],
            [10., 10.],
            [0., 10.],
            [0., 0.],
        ]]);
        let [lon, lat] = centroid(&square).unwrap();
        assert!(
            (lon - 5.).abs() < 0.2 && (lat - 5.).abs() < 0.2,
            "{lon},{lat}"
        );
    }
}

impl From<&str> for GeoProjection {
    fn from(name: &str) -> Self {
        GeoProjection::named(name).unwrap_or_else(|| GeoProjection::new(ProjectionKind::Identity))
    }
}

impl From<Geometry> for Dataset {
    /// A single geometry as one record, Plot's `Plot.geo(statemesh)`.
    fn from(geometry: Geometry) -> Self {
        Dataset::new([Table::empty().with("geometry", geometry.into_value())])
    }
}

/// Plot's `Plot.sphere()` data.
pub fn sphere() -> Dataset {
    Geometry::Sphere.into()
}
