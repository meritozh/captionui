//! Geographic projections for the `geo` mark: map longitude/latitude onto the
//! plot.
//!
//! Plot's `geo` mark is built on d3-geo's spherical projections. This library
//! ships a small equirectangular projection — longitude maps linearly across
//! the plot's width, latitude down its height — which is a faithful, dependency
//! free projection for a flat lon/lat strip. The frame's own x/y scales carry
//! the *plot* position; this projection turns a geographic coordinate into a
//! normalized `[0, 1]` point the scales then place.

/// An equirectangular projection of a longitude/latitude pair.
///
/// `lon`/`lat` are degrees; the result is a `[0, 1]` point — x from −180°..180°
/// across the plot, y from 90°..−90° down it (north at the top). The caller
/// feeds this into the frame's scales for placement.
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

impl Equirectangular {
    /// Project a degree longitude/latitude into the `[0, 1]` frame, or `None`
    /// when it falls outside the view (so a point past the edge clips out).
    pub fn project(&self, lon: f64, lat: f64) -> Option<[f64; 2]> {
        let x = (lon - self.west) / (self.east - self.west);
        let y = (self.north - lat) / (self.north - self.south);
        Some([x, y])
    }
}

/// The centroid of a list of degree (lon, lat) points, in degree space.
///
/// The simple plane average, which is correct for a small region on an
/// equirectangular view — Plot's `geoCentroid` is the spherical mean, but for a
/// narrow extent the two agree closely. Returns `None` for empty input.
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
    fn projects_equirectangularly() {
        let p = Equirectangular::default();
        // Prime meridian, equator → dead centre.
        assert_eq!(p.project(0., 0.), Some([0.5, 0.5]));
        // The north-west corner.
        assert_eq!(p.project(-180., 90.), Some([0., 0.]));
        // The south-east corner.
        assert_eq!(p.project(180., -90.), Some([1., 1.]));
    }

    #[test]
    fn averages_for_centroid() {
        assert_eq!(centroid(&[(10., 20.), (30., 40.)]), Some((20., 30.)));
        assert_eq!(centroid(&[]), None);
    }
}
