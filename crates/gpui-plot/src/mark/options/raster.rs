//! The raster, contour and density marks' grid options.

use crate::mark::{DensitySpec, Mark};

impl Mark {
    fn raster_mut(&mut self) -> &mut crate::raster::RasterSpec {
        self.raster.get_or_insert_with(Default::default)
    }

    /// A raster or contour's grid size, Plot's `width` and `height`, for
    /// data that is a dense array of values (row-major, top row first).
    pub fn grid(mut self, width: usize, height: usize) -> Self {
        self.raster_mut().size = Some((width, height));
        self
    }

    /// A raster or contour's value function, Plot's `fill: (x, y) => …`.
    pub fn sample(mut self, f: impl Fn(f64, f64) -> f64 + 'static) -> Self {
        self.raster_mut().function = Some(std::rc::Rc::new(move |x, y| {
            crate::value::Value::number(f(x, y))
        }));
        self
    }

    /// Plot's `interpolate`: `"nearest"`, `"barycentric"`, `"random-walk"`.
    pub fn interpolate(mut self, how: &str) -> Self {
        self.raster_mut().interpolate = crate::raster::Interpolate::parse(how);
        self
    }

    /// Plot's `blur`, in grid cells.
    pub fn blur(mut self, radius: f64) -> Self {
        self.raster_mut().blur = radius;
        self
    }

    /// A contour's `thresholds`: about this many.
    pub fn contour_thresholds(mut self, count: usize) -> Self {
        self.raster_mut().thresholds = Some(crate::raster::ContourThresholds::Count(count));
        self
    }

    /// A contour's explicit `thresholds`.
    pub fn contour_threshold_values(mut self, values: impl IntoIterator<Item = f64>) -> Self {
        self.raster_mut().thresholds = Some(crate::raster::ContourThresholds::Values(
            values.into_iter().collect(),
        ));
        self
    }

    /// A contour's `interval`: a threshold at every multiple.
    pub fn contour_interval(mut self, step: f64) -> Self {
        self.raster_mut().thresholds = Some(crate::raster::ContourThresholds::Interval(step));
        self
    }

    fn density_mut(&mut self) -> &mut DensitySpec {
        self.density.get_or_insert_with(Default::default)
    }

    /// A density mark's `bandwidth`, in pixels.
    pub fn bandwidth(mut self, pixels: f32) -> Self {
        self.density_mut().bandwidth = Some(pixels);
        self
    }

    /// A density mark's `thresholds` count.
    pub fn thresholds(mut self, count: usize) -> Self {
        self.density_mut().count = Some(count);
        self
    }

    /// A density mark's explicit `thresholds`.
    pub fn threshold_values(mut self, values: impl IntoIterator<Item = f64>) -> Self {
        self.density_mut().values = Some(values.into_iter().collect());
        self
    }

    /// A density mark's `weight`.
    pub fn weight(mut self, weight: impl Fn(&crate::table::Table) -> f64 + 'static) -> Self {
        self.density_mut().weight = Some(std::rc::Rc::new(weight));
        self
    }
}
