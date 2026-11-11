//! The grid a raster or contour mark draws: from dense values, from
//! scattered samples, or from a function sampled once the frame is known.

use crate::mark::{Mark, MarkKind};
use crate::render::PreparedMark;
use crate::table::{Dataset, Table};
use crate::value::Value;

/// The extent a raster or contour mark covers, `[x1, y1, x2, y2]`, from its
/// constant `x1`/`y1`/`x2`/`y2` channels, else its grid (`[0, width] ×
/// [0, height]`, Plot's default for dense values).
pub fn raster_extent(mark: &Mark) -> Option<[f64; 4]> {
    let constant = |c: &Option<crate::table::Channel>| match c {
        Some(crate::table::Channel::Constant(v)) => v.as_number(),
        _ => None,
    };
    let c = &mark.channels;
    match (
        constant(&c.x1),
        constant(&c.y1),
        constant(&c.x2),
        constant(&c.y2),
    ) {
        (Some(x1), Some(y1), Some(x2), Some(y2)) => Some([x1, y1, x2, y2]),
        _ => mark
            .raster
            .as_ref()?
            .size
            .map(|(w, h)| [0., 0., w as f64, h as f64]),
    }
}

/// A raster from dense values: the mark's records in order, each its fill
/// (or its value, for plain-value data), laid out `width` across.
pub(super) fn dense_raster(
    mark: &Mark,
    source: &Dataset,
) -> Option<std::rc::Rc<crate::raster::RasterGrid>> {
    if !matches!(mark.kind, MarkKind::Raster | MarkKind::Contour) {
        return None;
    }
    let spec = mark.raster.as_ref()?;
    let (w, h) = spec.size?;
    let extent = raster_extent(mark)?;
    let read = |row: &Table| -> Value {
        if let Some(fill) = &mark.channels.fill
            && let Some(v) = fill.scaled_value(row)
        {
            return v;
        }
        row.value(crate::table::VALUE)
    };
    let values: Vec<Value> = source.rows().iter().map(read).collect();
    let mut grid = crate::raster::RasterGrid::from_values(values, w, h, extent);
    grid.blur(spec.blur);
    Some(std::rc::Rc::new(grid))
}

/// The grid a raster or contour mark draws once the frame is known: its
/// dense grid, a function sampled at Plot's `pixelSize` (one cell per two
/// pixels), or its scattered rows interpolated.
pub fn resolve_raster(
    prepared: &PreparedMark,
    frame: &crate::frame::Frame,
) -> Option<std::rc::Rc<crate::raster::RasterGrid>> {
    if let Some(grid) = &prepared.raster {
        return Some(grid.clone());
    }
    let spec = prepared.mark.raster.clone().unwrap_or_default();
    let (pw, ph) = (
        frame.plot.size.width.as_f32() as f64,
        frame.plot.size.height.as_f32() as f64,
    );
    let size = spec.size.unwrap_or((
        (pw / 2.).round().max(1.) as usize,
        (ph / 2.).round().max(1.) as usize,
    ));
    let invert = |scale: &crate::scale::PositionScale, p: f64| match scale {
        crate::scale::PositionScale::Continuous(s) => s.invert(p as f32),
        _ => p,
    };
    // The data extent: the mark's own, else the frame's.
    let extent = raster_extent(&prepared.mark).unwrap_or_else(|| {
        [
            invert(&frame.x.scale, 0.),
            invert(&frame.y.scale, ph),
            invert(&frame.x.scale, pw),
            invert(&frame.y.scale, 0.),
        ]
    });
    let samples: Vec<(f64, f64, Value)> = if spec.function.is_some() {
        Vec::new()
    } else {
        prepared
            .rows
            .iter()
            .filter_map(|row| {
                let value = match &prepared.mark.channels.fill {
                    Some(fill) => fill.scaled_value(&row.row)?,
                    None => row.row.value(crate::table::VALUE),
                };
                Some((
                    row.x_value().as_number()?,
                    row.y_value().as_number()?,
                    value,
                ))
            })
            .collect()
    };
    let how = spec
        .interpolate
        .unwrap_or(if prepared.mark.kind == MarkKind::Contour {
            crate::raster::Interpolate::Nearest
        } else {
            crate::raster::Interpolate::None
        });
    // Interpolating thousands of samples (or sampling a function per cell) is
    // the costly part, and a plot repaints on every pointer move with the same
    // inputs; remember the last few grids by exactly what they were built
    // from. A function is known by its `Rc`, which the entry holds so the
    // address cannot be reused: a retained plot keeps the same function from
    // frame to frame, and a rebuilt one simply resolves again.
    let key = RasterKey {
        size,
        how,
        blur: spec.blur.to_bits(),
        extent: extent.map(f64::to_bits),
        function: spec.function.clone(),
        samples: samples
            .iter()
            .map(|(x, y, v)| (x.to_bits(), y.to_bits(), v.key()))
            .collect(),
    };
    thread_local! {
        static GRIDS: std::cell::RefCell<Vec<(RasterKey, std::rc::Rc<crate::raster::RasterGrid>)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    if let Some(grid) = GRIDS.with_borrow(|grids| {
        grids.iter().find(|(k, _)| k.same(&key)).map(|(_, g)| g.clone())
    }) {
        return Some(grid);
    }
    let mut grid = if let Some(f) = &spec.function {
        crate::raster::RasterGrid::from_function(&|x, y| f(x, y), size.0, size.1, extent)
    } else {
        crate::raster::RasterGrid::from_samples(&samples, size.0, size.1, extent, how)
    };
    grid.blur(spec.blur);
    let grid = std::rc::Rc::new(grid);
    // Keep as many grids as a page of raster charts needs (a plot rebuilt
    // every frame comes back for its grid each frame), within a budget of
    // cells, so a page of rasters does not thrash and memory stays bounded.
    const CELL_BUDGET: usize = 1 << 21;
    GRIDS.with_borrow_mut(|grids| {
        grids.insert(0, (key, grid.clone()));
        let mut cells = 0;
        let keep = grids
            .iter()
            .position(|(_, g)| {
                cells += g.values.len();
                cells > CELL_BUDGET
            })
            .unwrap_or(grids.len())
            .max(1);
        grids.truncate(keep);
    });
    Some(grid)
}

/// Everything a resolved raster grid was built from, compared exactly.
struct RasterKey {
    size: (usize, usize),
    how: crate::raster::Interpolate,
    blur: u64,
    extent: [u64; 4],
    function: Option<std::rc::Rc<dyn Fn(f64, f64) -> Value>>,
    samples: Vec<(u64, u64, crate::value::ValueKey)>,
}

impl RasterKey {
    fn same(&self, other: &Self) -> bool {
        self.size == other.size
            && self.how == other.how
            && self.blur == other.blur
            && self.extent == other.extent
            && match (&self.function, &other.function) {
                (Some(a), Some(b)) => std::rc::Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && self.samples == other.samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(function: Option<std::rc::Rc<dyn Fn(f64, f64) -> Value>>) -> RasterKey {
        RasterKey {
            size: (4, 3),
            how: crate::raster::Interpolate::None,
            blur: 0f64.to_bits(),
            extent: [0., 0., 1., 1.].map(f64::to_bits),
            function,
            samples: Vec::new(),
        }
    }

    #[test]
    fn functions_are_told_apart_by_identity_not_by_sampling() {
        // Two functions that agree almost everywhere must not share a grid.
        let f: std::rc::Rc<dyn Fn(f64, f64) -> Value> = std::rc::Rc::new(|x, _| Value::number(x));
        let g: std::rc::Rc<dyn Fn(f64, f64) -> Value> =
            std::rc::Rc::new(|x, y| Value::number(if y > 0.99 { -x } else { x }));
        assert!(key(Some(f.clone())).same(&key(Some(f.clone()))));
        assert!(!key(Some(f.clone())).same(&key(Some(g))));
        assert!(!key(Some(f)).same(&key(None)));
    }

    #[test]
    fn samples_and_parameters_compare_exactly() {
        let mut a = key(None);
        a.samples = vec![(1f64.to_bits(), 2f64.to_bits(), Value::number(3.).key())];
        let mut b = key(None);
        b.samples = a.samples.clone();
        assert!(a.same(&b));
        b.samples[0].2 = Value::number(3.000_000_1).key();
        assert!(!a.same(&b));
        let mut c = key(None);
        c.samples = a.samples.clone();
        c.blur = 2f64.to_bits();
        assert!(!a.same(&c));
    }
}
