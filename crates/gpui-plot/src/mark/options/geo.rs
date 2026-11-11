//! The geo mark's options: geometry, centroids, projection, rings and GeoJSON.

use crate::mark::MarkOptions;
use crate::table::IntoChannel;

/// The geo mark's options: geometry, centroids, projection, rings and
/// GeoJSON.
pub trait GeoOptions: MarkOptions {
    /// A `geo` mark's geometry channel, Plot's `geometry` option; the
    /// records' `"geometry"` field by default.
    fn geometry(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.path = Some(channel.into_channel());
        self
    }

    /// Plot's `geoCentroid` transform: `x` and `y` are the longitude and
    /// latitude of each record's geometry's spherical centroid.
    fn geo_centroid(mut self) -> Self {
        // The x and y accessors read the same centroid: the last geometry's
        // is kept, with its `Rc` so its address cannot be reused while it is
        // remembered, and the y read right after the x one reuses it.
        type Last = Option<(std::rc::Rc<crate::geo::Geometry>, Option<[f64; 2]>)>;
        let last: std::rc::Rc<std::cell::RefCell<Last>> = Default::default();
        let centroid = |axis: usize| {
            let last = last.clone();
            move |row: &crate::table::Table| {
                let Some(crate::value::Value::Geo(geometry)) = row.get("geometry") else {
                    return crate::value::Value::Null;
                };
                let mut last = last.borrow_mut();
                let c = match &*last {
                    Some((kept, c)) if std::rc::Rc::ptr_eq(kept, geometry) => *c,
                    _ => {
                        let c = crate::geo::centroid(geometry);
                        *last = Some((geometry.clone(), c));
                        c
                    }
                };
                c.map(|c| crate::value::Value::Number(c[axis]))
                    .unwrap_or(crate::value::Value::Null)
            }
        };
        let mark = self.mark_mut();
        mark.channels.x = Some(crate::table::Channel::Accessor(std::rc::Rc::new(centroid(
            0,
        ))));
        mark.channels.y = Some(crate::table::Channel::Accessor(std::rc::Rc::new(centroid(
            1,
        ))));
        self
    }

    /// Plot's `centroid` transform. Plot takes the projected centroid; this
    /// takes the spherical one, which lands within a pixel or two of it for
    /// shapes the size of a county or a country.
    fn centroid(self) -> Self {
        self.geo_centroid()
    }

    /// Set the equirectangular projection a `geo` mark maps lon/lat through.
    fn projection(mut self, projection: crate::projection::Projection) -> Self {
        self.mark_mut().projection = Some(projection);
        self
    }

    /// Add a GeoJSON-like ring to a `geo` mark: a closed loop of `(lon, lat)`
    /// degree points the mark fills as a projected shape.
    fn ring(mut self, ring: Vec<(f64, f64)>) -> Self {
        let geometry = self.mark_mut().geo_geometry.get_or_insert_with(Vec::new);
        geometry.push(ring);
        self
    }

    /// Add every polygon in a GeoJSON document to a `geo` mark, Plot's way of
    /// loading a geography file's shapes. `text` is the GeoJSON source — a
    /// file or URL's contents a caller has already read.
    fn geojson(mut self, text: &str) -> Self {
        if let Ok(rings) = crate::projection::geojson_rings(text) {
            let geometry = self.mark_mut().geo_geometry.get_or_insert_with(Vec::new);
            geometry.extend(rings);
        }
        self
    }
}

impl<T: MarkOptions> GeoOptions for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::{Mark, MarkKind};

    #[test]
    fn geo_centroid_reads_each_records_own_centroid() {
        use crate::geo::Geometry;
        use crate::table::Table;
        let shape = |dx: f64| {
            std::rc::Rc::new(Geometry::Polygon(vec![vec![
                [dx, 0.],
                [dx + 10., 0.],
                [dx + 10., 10.],
                [dx, 10.],
                [dx, 0.],
            ]]))
        };
        let (a, b) = (shape(0.), shape(40.));
        let row = |g: &std::rc::Rc<Geometry>| {
            Table::empty().with("geometry", crate::value::Value::Geo(g.clone()))
        };
        // Two records sharing one geometry, a different one, a record with
        // none and one whose geometry has no centroid.
        let none = std::rc::Rc::new(Geometry::Sphere);
        let rows = [
            row(&a),
            row(&a),
            row(&b),
            Table::empty(),
            row(&none),
            row(&a),
        ];
        let mark = Mark::new(MarkKind::Geo).geo_centroid();
        let (x, y) = (
            mark.channels.x.clone().unwrap(),
            mark.channels.y.clone().unwrap(),
        );
        let expect = |r: &Table, axis: usize| {
            r.get("geometry")
                .and_then(crate::value::Value::as_geo)
                .and_then(crate::geo::centroid)
                .map_or(crate::value::Value::Null, |c| {
                    crate::value::Value::Number(c[axis])
                })
        };
        // x then y per record, y alone, and x twice: each read is its own
        // record's.
        for order in [[0, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0], [1; 12], [0; 12]] {
            for (i, r) in rows.iter().cycle().take(12).enumerate() {
                let axis = order[i];
                let read = if axis == 0 { x.read(r) } else { y.read(r) };
                assert_eq!(read, expect(r, axis), "record {i} axis {axis}");
            }
        }
    }
}
