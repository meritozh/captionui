//! Geographic projections for the `geo` mark: map longitude/latitude onto the
//! plot.
//!
//! Plot's `geo` mark is built on d3-geo's spherical projections. This library
//! ships two projection families — the flat **equirectangular** (lon maps
//! linearly across, lat down) and the spherical **Mercator** (lat is warped by
//! `ln(tan(45°) + tan(lat/2))`, so distances distort toward the poles) — both
//! of which turn a degree longitude/latitude into a `[0, 1]` point the plot
//! places.

/// A geographic projection of a longitude/latitude pair.
///
/// `lon`/`lat` are degrees; `project` yields a `[0, 1]` point — x from the
/// view's west to its east, y from its north down to its south (north at the
/// top).
#[derive(Clone, Debug)]
pub enum Projection {
    /// A flat lon/lat view, [`Projection::equirectangular`].
    Equirectangular(Equirectangular),
    /// A spherical Mercator view of the world between two latitudes,
    /// [`Projection::mercator`].
    Mercator(Mercator),
}

/// A flat, equirectangular view of a longitude/latitude rectangle.
#[derive(Clone, Debug)]
pub struct Equirectangular {
    /// The west longitude of the view's left edge, degrees.
    pub west: f64,
    /// The east longitude of the view's right edge, degrees.
    pub east: f64,
    /// The north latitude of the view's top edge, degrees.
    pub north: f64,
    /// The south latitude of the view's bottom edge, degrees.
    pub south: f64,
}

/// A spherical Mercator view of the world between two latitudes.
#[derive(Clone, Debug)]
pub struct Mercator {
    /// The west longitude of the view's left edge, degrees.
    pub west: f64,
    /// The east longitude of the view's right edge, degrees.
    pub east: f64,
    /// The north latitude of the view's top edge, degrees.
    pub north: f64,
    /// The south latitude of the view's bottom edge, degrees.
    pub south: f64,
    /// The longitude the view's column is centred on, degrees (usually 0).
    pub center: f64,
}

impl Default for Projection {
    fn default() -> Self {
        Projection::Equirectangular(Equirectangular::default())
    }
}

impl Projection {
    /// An equirectangular view spanning the whole globe.
    pub fn equirectangular() -> Self {
        Projection::Equirectangular(Equirectangular::default())
    }

    /// A spherical Mercator view, fit to a longitude/latitude rectangle.
    ///
    /// A view whose `east` is west of its `west` crosses the antimeridian
    /// (`170..-170` is the 20° around 180°), so `east` is read a turn later.
    pub fn mercator(west: f64, east: f64, north: f64, south: f64) -> Self {
        let east = if east < west { east + 360. } else { east };
        Projection::Mercator(Mercator {
            west,
            east,
            north,
            south,
            center: (west + east) / 2.,
        })
    }

    /// Project a degree longitude/latitude into a `[0, 1]` frame point. A point
    /// inside the view projects inside `[0, 1]`; one outside is clamped to the
    /// edge so a shape clipped at the frame draws right up to it.
    pub fn project(&self, lon: f64, lat: f64) -> [f64; 2] {
        match self {
            Projection::Equirectangular(p) => {
                let x = (lon - p.west) / (p.east - p.west);
                let y = (p.north - lat) / (p.north - p.south);
                [clamp01(x), clamp01(y)]
            }
            Projection::Mercator(p) => {
                // Web/Mercator: x is linear in lon about the center; y is the
                // mercator latitude (inverse Gudermannian), mapped to fit the
                // view's own meridians.
                let lng = |deg: f64| deg.to_radians();
                // Mercator's y is infinite at the poles; clamp to the Web
                // Mercator limit so a view reaching ±90° still has a span.
                const LIMIT: f64 = 85.051_128_779_806_59;
                let merc = |lat_deg: f64| {
                    (std::f64::consts::FRAC_PI_4 + lat_deg.clamp(-LIMIT, LIMIT).to_radians() / 2.)
                        .tan()
                        .ln()
                };
                // Read the longitude within half a turn of the view's center,
                // so a view across the antimeridian sees -175° as 185°.
                let lon = p.center + (lon - p.center + 180.).rem_euclid(360.) - 180.;
                let north_y = merc(lat.max(p.south));
                let (north, south) = (merc(p.north), merc(p.south));
                let span = (north - south).max(f64::MIN_POSITIVE);
                let x = (lng(lon) - lng(p.center))
                    / (lng(p.east) - lng(p.center)).max(f64::MIN_POSITIVE)
                    * 0.5
                    + 0.5;
                let y = (north - north_y) / span;
                [clamp01(x), clamp01(y)]
            }
        }
    }
}

fn clamp01(v: f64) -> f64 {
    (v as f32).clamp(0., 1.) as f64
}

impl Default for Equirectangular {
    fn default() -> Self {
        Self {
            west: -180.,
            east: 180.,
            north: 90.,
            south: -90.,
        }
    }
}

/// The linear rings of a GeoJSON document, in `(lon, lat)` degree points.
///
/// This is the pieces of GeoJSON the `geo` mark can fill: every polygon found
/// in the document, each as one closed ring. It walks `Feature`s and
/// `GeometryCollection`s, and a `Polygon` yields its rings, a `MultiPolygon`
/// all of its polygons' rings. Points, lines and empty geometries yield nothing.
///
/// `text` is the GeoJSON source — a file or URL's contents a caller has already
/// read; nothing here fetches it.
pub fn geojson_rings(text: &str) -> Result<Vec<Vec<(f64, f64)>>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid GeoJSON: {e}"))?;
    let mut rings = Vec::new();
    collect_geometry(&value, &mut rings);
    Ok(rings)
}

/// Recurse a GeoJSON value, pushing the rings of any polygon it contains.
fn collect_geometry(value: &serde_json::Value, rings: &mut Vec<Vec<(f64, f64)>>) {
    use serde_json::Value as V;
    // A ring is a "coordinates" array of [lon, lat] pairs, decoded once here.
    fn ring(coords: &V) -> Option<Vec<(f64, f64)>> {
        let positions = coords.as_array()?;
        let mut points = Vec::with_capacity(positions.len());
        for p in positions {
            let arr = p.as_array()?;
            let lon = arr.first()?.as_f64()?;
            let lat = arr.get(1)?.as_f64()?;
            points.push((lon, lat));
        }
        if points.len() >= 3 {
            Some(points)
        } else {
            None
        }
    }

    match value {
        // A Feature wraps a geometry under "geometry".
        V::Object(map) if map.get("type").and_then(V::as_str) == Some("Feature") => {
            if let Some(geometry) = map.get("geometry") {
                collect_geometry(geometry, rings);
            }
        }
        V::Object(map) if map.get("type").and_then(V::as_str) == Some("GeometryCollection") => {
            if let Some(arr) = map.get("geometries").and_then(V::as_array) {
                for geometry in arr {
                    collect_geometry(geometry, rings);
                }
            }
        }
        // A FeatureCollection is a list of Features.
        V::Object(map) if map.get("type").and_then(V::as_str) == Some("FeatureCollection") => {
            if let Some(arr) = map.get("features").and_then(V::as_array) {
                for feature in arr {
                    collect_geometry(feature, rings);
                }
            }
        }
        V::Object(map) => match map.get("type").and_then(V::as_str) {
            Some("Polygon") => {
                if let Some(coords) = map.get("coordinates").and_then(V::as_array) {
                    for poly in coords {
                        if let Some(r) = ring(poly) {
                            rings.push(r);
                        }
                    }
                }
            }
            Some("MultiPolygon") => {
                if let Some(coords) = map.get("coordinates").and_then(V::as_array) {
                    for poly in coords {
                        if let Some(coords) = poly.as_array() {
                            for coordinates in coords {
                                if let Some(r) = ring(coordinates) {
                                    rings.push(r);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
}

/// The centroid of a list of degree (lon, lat) points, in degree space.
///
/// The simple plane average, correct for a small region on an equirectangular
/// view; Plot's `geoCentroid` is the spherical mean, which agrees closely for a
/// narrow extent. Returns `None` for empty input.
pub fn centroid(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    let count = points.len() as f64;
    let lon = points.iter().map(|p| p.0).sum::<f64>() / count;
    let lat = points.iter().map(|p| p.1).sum::<f64>() / count;
    Some((lon, lat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equirectangular_projects_the_corners() {
        let p = Projection::equirectangular();
        assert_eq!(p.project(0., 0.), [0.5, 0.5]);
        assert_eq!(p.project(-180., 90.), [0., 0.]);
        assert_eq!(p.project(180., -90.), [1., 1.]);
    }

    #[test]
    fn mercator_widens_the_poles() {
        // At the equator mercator is a flat lon/lat for x.
        let p = Projection::mercator(-180., 180., 85., -85.);
        assert_eq!(p.project(0., 0.), [0.5, 0.5]);
        // A high northern latitude sits in the top half, and — because Mercator
        // stretches mid-latitudes vertically — 45° sits below its flat
        // equirectangular counterpart (y=0.25).
        let merc_y45 = p.project(0., 45.)[1];
        assert!(merc_y45 > 0.25, "Mercator stretches latitude: {merc_y45}");
        let merc_y80 = p.project(0., 80.)[1];
        assert!(merc_y80 < 0.25, "high latitudes crowd the pole: {merc_y80}");
    }

    #[test]
    fn averages_for_centroid() {
        assert_eq!(centroid(&[(10., 20.), (30., 40.)]), Some((20., 30.)));
        assert_eq!(centroid(&[]), None);
    }

    #[test]
    fn parses_geojson_polygons_into_rings() {
        // A tiny square polygon.
        let text = r#"{
            "type": "Feature",
            "geometry": {
                "type": "Polygon",
                "coordinates": [
                    [[-10.0, 40.0], [-5.0, 40.0], [-5.0, 45.0], [-10.0, 45.0], [-10.0, 40.0]]
                ]
            }
        }"#;
        let rings = geojson_rings(text).unwrap();
        assert_eq!(rings.len(), 1);
        let ring = &rings[0];
        assert_eq!(ring[0], (-10.0, 40.0));
        assert_eq!(ring[2], (-5.0, 45.0));
    }

    #[test]
    fn parses_a_multipolygon_and_feature_collection() {
        let text = r#"{
            "type": "FeatureCollection",
            "features": [
                {"type": "Feature", "geometry": {
                    "type": "MultiPolygon",
                    "coordinates": [
                        [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]],
                        [[[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0], [2.0, 2.0]]]
                    ]
                }}
            ]
        }"#;
        let rings = geojson_rings(text).unwrap();
        assert_eq!(rings.len(), 2);
        assert!(!geojson_rings("not json").is_ok());
    }
}
