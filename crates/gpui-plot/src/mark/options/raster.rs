//! The raster, contour and density marks' grid options.

use crate::mark::{DensitySpec, Mark, MarkOptions};

/// The raster, contour and density marks' grid options.
pub trait RasterOptions: MarkOptions {
    /// A raster or contour's grid size, Plot's `width` and `height`, for
    /// data that is a dense array of values (row-major, top row first).
    fn grid(mut self, width: usize, height: usize) -> Self {
        self.mark_mut().raster_mut().size = Some((width, height));
        self
    }

    /// A raster or contour's value function, Plot's `fill: (x, y) => …`.
    fn sample(mut self, f: impl Fn(f64, f64) -> f64 + 'static) -> Self {
        self.mark_mut().raster_mut().function = Some(std::rc::Rc::new(move |x, y| {
            crate::value::Value::number(f(x, y))
        }));
        self
    }

    /// Plot's `interpolate`: `"nearest"`, `"barycentric"`, `"random-walk"`.
    fn interpolate(mut self, how: &str) -> Self {
        self.mark_mut().raster_mut().interpolate = crate::raster::Interpolate::parse(how);
        self
    }

    /// Plot's `blur`, in grid cells.
    fn blur(mut self, radius: f64) -> Self {
        self.mark_mut().raster_mut().blur = radius;
        self
    }

    /// A contour's `thresholds`: about this many.
    fn contour_thresholds(mut self, count: usize) -> Self {
        self.mark_mut().raster_mut().thresholds =
            Some(crate::raster::ContourThresholds::Count(count));
        self
    }

    /// A contour's explicit `thresholds`.
    fn contour_threshold_values(mut self, values: impl IntoIterator<Item = f64>) -> Self {
        self.mark_mut().raster_mut().thresholds = Some(crate::raster::ContourThresholds::Values(
            values.into_iter().collect(),
        ));
        self
    }

    /// A contour's `interval`: a threshold at every multiple.
    fn contour_interval(mut self, step: f64) -> Self {
        self.mark_mut().raster_mut().thresholds =
            Some(crate::raster::ContourThresholds::Interval(step));
        self
    }

    /// A density mark's `bandwidth`, in pixels.
    fn bandwidth(mut self, pixels: f32) -> Self {
        self.mark_mut().density_mut().bandwidth = Some(pixels);
        self
    }

    /// A density mark's `thresholds` count.
    fn thresholds(mut self, count: usize) -> Self {
        self.mark_mut().density_mut().count = Some(count);
        self
    }

    /// A density mark's explicit `thresholds`.
    fn threshold_values(mut self, values: impl IntoIterator<Item = f64>) -> Self {
        self.mark_mut().density_mut().values = Some(values.into_iter().collect());
        self
    }

    /// A density mark's `weight`.
    fn weight(mut self, weight: impl Fn(&crate::table::Table) -> f64 + 'static) -> Self {
        self.mark_mut().density_mut().weight = Some(std::rc::Rc::new(weight));
        self
    }
}

impl<T: MarkOptions> RasterOptions for T {}

impl Mark {
    fn raster_mut(&mut self) -> &mut crate::raster::RasterSpec {
        self.raster.get_or_insert_with(Default::default)
    }

    fn density_mut(&mut self) -> &mut DensitySpec {
        self.density.get_or_insert_with(Default::default)
    }
}
