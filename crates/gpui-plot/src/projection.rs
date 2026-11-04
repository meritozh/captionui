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
    pub fn mercator(west: f64, east: f64, north: f64, south: f64) -> Self {
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
                let merc = |lat_deg: f64| {
                    (std::f64::consts::FRAC_PI_4 + lat_deg.to_radians() / 2.)
                        .tan()
                        .ln()
                };
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
}
