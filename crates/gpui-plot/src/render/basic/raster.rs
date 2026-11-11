//! Plot's `raster` and `contour`: a dense grid as an image or as isobands,
//! with the caches that keep both across repaints.

use gpui::{Hsla, Pixels, Point, point, px};

use crate::paint;
use crate::render::PreparedMark;
use crate::value::Value;

use super::DrawContext;

/// Plot's `raster`: a dense pixel grid, one rectangle per datum, colored from
/// its fill. The grid comes from each row's position and the nearest gap
/// between positions, so a row's rectangle covers the cell around it.
pub(super) fn draw_raster(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let Some(grid) = crate::render::resolve_raster(prepared, ctx.frame) else {
        return;
    };
    let opacity = prepared.mark.style.opacity * prepared.mark.style.fill_opacity * ctx.appear;
    let row = crate::table::Table::empty();
    let place = |x: f64, y: f64, ctx: &DrawContext<'_>| -> Option<Point<Pixels>> {
        if let Some(projection) = &ctx.frame.projection {
            let [px_, py_] = projection.project(x, y)?;
            return Some(point(px(px_ as f32), px(py_ as f32)));
        }
        Some(point(
            px(ctx.frame.x.scale.position(&Value::Number(x))?),
            px(ctx.frame.y.scale.position(&Value::Number(y))?),
        ))
    };
    let _ = row;
    let cell_color = |value: &Value, ctx: &DrawContext<'_>| -> Option<Hsla> {
        if value.is_null() {
            return None;
        }
        value
            .as_text()
            .and_then(|t| crate::css::parse(t))
            .or_else(|| ctx.colors.scale_color(value))
    };
    // Unprojected, the grid is an axis-aligned image, as Plot draws it: one
    // pixel per cell, built once and stretched over the grid's extent.
    if ctx.frame.projection.is_none() {
        let (Some(a), Some(b)) = (place(grid.x1, grid.y1, ctx), place(grid.x2, grid.y2, ctx))
        else {
            return;
        };
        // Grid row 0 is the low end of y, drawn at the bottom unless reversed.
        let (flip_x, flip_y) = (a.x > b.x, a.y > b.y);
        let bounds = paint::rect_between(ctx.absolute(a), ctx.absolute(b));
        // Upscaled to device pixels, nearest-neighbour, so GPUI's linear
        // sampler keeps each cell crisp, as the quads it replaces were.
        let scale = ctx.window.scale_factor();
        let device = (
            (bounds.size.width.as_f32() * scale).round().clamp(1., 4096.) as usize,
            (bounds.size.height.as_f32() * scale).round().clamp(1., 4096.) as usize,
        );
        let image = raster_image(&grid, opacity, flip_x, flip_y, device, |v| cell_color(v, ctx));
        ctx.clipped(|ctx| {
            let _ = ctx
                .window
                .paint_image(bounds, bounds, Default::default(), image, 0, false);
        });
        return;
    }
    // Projected, Plot projects each sample to the frame and fills a grid
    // laid over the whole frame from them (by the mark's `interpolate`,
    // placing each sample in its pixel by default), drawn as one image;
    // `clip: "sphere"` masks it to the projected globe.
    let Some(projection) = ctx.frame.projection.as_ref() else {
        return;
    };
    let (pw, ph) = (
        ctx.frame.plot.size.width.as_f32() as f64,
        ctx.frame.plot.size.height.as_f32() as f64,
    );
    let spec = prepared.mark.raster.clone().unwrap_or_default();
    let size = spec
        .size
        .unwrap_or(((pw.round() as usize).max(1), (ph.round() as usize).max(1)));
    let how = spec.interpolate.unwrap_or(crate::raster::Interpolate::None);
    let sphere = if prepared.mark.style.clip_sphere {
        projection.sphere()
    } else {
        None
    };
    let scale = ctx.window.scale_factor();
    let device = (
        (pw * scale as f64).round().clamp(1., 4096.) as usize,
        (ph * scale as f64).round().clamp(1., 4096.) as usize,
    );
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (std::rc::Rc::as_ptr(&grid) as usize, size, how as u8, device).hash(&mut hasher);
        (opacity.to_bits(), sphere.is_some()).hash(&mut hasher);
        // The projection's fit, by where it puts a few points.
        for (x, y) in [(0., 0.), (90., 45.), (-120., -30.)] {
            projection
                .project(x, y)
                .map(|p| p.map(f64::to_bits))
                .hash(&mut hasher);
        }
        let n = grid.values.len();
        for k in (0..n).step_by((n / 31).max(1)) {
            cell_color(&grid.values[k], ctx)
                .map(|c| [c.h, c.s, c.l, c.a].map(f32::to_bits))
                .hash(&mut hasher);
        }
        hasher.finish()
    };
    // Each entry holds its grid, so the grid's address in the key can't be
    // reused by another grid while the entry lives.
    type Entry = (
        u64,
        std::rc::Rc<crate::raster::RasterGrid>,
        std::sync::Arc<gpui::RenderImage>,
    );
    thread_local! {
        static PROJECTED: std::cell::RefCell<Vec<Entry>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    let cached = PROJECTED.with_borrow(|images| {
        images
            .iter()
            .find(|(k, g, _)| *k == key && std::rc::Rc::ptr_eq(g, &grid))
            .map(|(_, _, i)| i.clone())
    });
    let image = match cached {
        Some(image) => image,
        None => {
            let pixels = projected_raster(&grid, projection, size, [pw, ph], how);
            let (dw, dh) = device;
            let mut rgba = vec![0u8; dw * dh * 4];
            // Several device pixels fall in one resampled cell: each cell's
            // bytes are worked out once per row of cells, then copied.
            let mut row_sy = usize::MAX;
            let mut row_bytes: Vec<Option<Option<[u8; 4]>>> = vec![None; size.0];
            for y in 0..dh {
                let py_ = (y as f64 + 0.5) / scale as f64;
                // The sphere's spans on this row, even-odd.
                let spans = sphere.as_ref().map(|ring| {
                    let mut xs: Vec<f64> = (0..ring.len())
                        .filter_map(|k| {
                            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                            ((a[1] <= py_) != (b[1] <= py_))
                                .then(|| a[0] + (py_ - a[1]) / (b[1] - a[1]) * (b[0] - a[0]))
                        })
                        .collect();
                    xs.sort_by(f64::total_cmp);
                    xs
                });
                let sy = ((py_ / ph * size.1 as f64) as usize).min(size.1 - 1);
                if sy != row_sy {
                    row_sy = sy;
                    row_bytes.fill(None);
                }
                for x in 0..dw {
                    let px_ = (x as f64 + 0.5) / scale as f64;
                    if let Some(xs) = &spans
                        && xs.iter().filter(|&&e| e <= px_).count() % 2 == 0
                    {
                        continue;
                    }
                    let sx = ((px_ / pw * size.0 as f64) as usize).min(size.0 - 1);
                    let bytes = *row_bytes[sx].get_or_insert_with(|| {
                        let c =
                            gpui::Rgba::from(cell_color(&pixels.values[sy * size.0 + sx], ctx)?);
                        // GPUI's images are BGRA.
                        Some([
                            (c.b * 255.).round() as u8,
                            (c.g * 255.).round() as u8,
                            (c.r * 255.).round() as u8,
                            ((c.a * opacity).clamp(0., 1.) * 255.).round() as u8,
                        ])
                    });
                    let Some(bytes) = bytes else {
                        continue;
                    };
                    let k = (y * dw + x) * 4;
                    rgba[k..k + 4].copy_from_slice(&bytes);
                }
            }
            let image = crate::image_from_rgba(dw as u32, dh as u32, rgba);
            PROJECTED.with_borrow_mut(|images| {
                images.insert(0, (key, grid.clone(), image.clone()));
                images.truncate(8);
            });
            image
        }
    };
    let bounds = ctx.frame.plot;
    ctx.clipped(|ctx| {
        let _ = ctx
            .window
            .paint_image(bounds, bounds, Default::default(), image, 0, false);
    });
}

/// A raster's grid resampled into frame pixels through a projection, Plot's
/// way: each cell's centre projected onto a `size` grid over the frame,
/// then interpolated there. Plot triangulates the projected samples for
/// `barycentric`; a dense grid's samples are a regular lattice, so its own
/// cells, split in two, stand in for that triangulation.
fn projected_raster(
    grid: &crate::raster::RasterGrid,
    projection: &crate::geo::Fitted,
    size: (usize, usize),
    [pw, ph]: [f64; 2],
    how: crate::raster::Interpolate,
) -> crate::raster::RasterGrid {
    let (w, h) = size;
    let at = |i: usize, j: usize| -> Option<([f64; 2], f64)> {
        let value = grid.values[j * grid.width + i].as_number()?;
        if !value.is_finite() {
            return None;
        }
        let (x, y) = grid.center(i, j);
        let [px_, py_] = projection.project(x, y)?;
        Some(([px_ / pw * w as f64, py_ / ph * h as f64], value))
    };
    if how != crate::raster::Interpolate::Barycentric {
        let samples: Vec<(f64, f64, Value)> = (0..grid.height)
            .flat_map(|j| (0..grid.width).map(move |i| (i, j)))
            .filter_map(|(i, j)| {
                let value = &grid.values[j * grid.width + i];
                if value.is_null() || value.as_number().is_some_and(f64::is_nan) {
                    return None;
                }
                let (x, y) = grid.center(i, j);
                let [px_, py_] = projection.project(x, y)?;
                Some((px_, py_, value.clone()))
            })
            .collect();
        return crate::raster::RasterGrid::from_samples(&samples, w, h, [0., 0., pw, ph], how);
    }
    let points: Vec<Option<([f64; 2], f64)>> = (0..grid.height)
        .flat_map(|j| (0..grid.width).map(move |i| (i, j)))
        .map(|(i, j)| at(i, j))
        .collect();
    let mut values = vec![f64::NAN; w * h];
    let mut fill = |[a, b, c]: [([f64; 2], f64); 3]| {
        let det = (b.0[1] - c.0[1]) * (a.0[0] - c.0[0]) + (c.0[0] - b.0[0]) * (a.0[1] - c.0[1]);
        if det.abs() < 1e-12 {
            return;
        }
        let xs = [a.0[0], b.0[0], c.0[0]];
        let ys = [a.0[1], b.0[1], c.0[1]];
        let (x0, x1) = (xs.iter().copied().fold(f64::MAX, f64::min), xs.iter().copied().fold(f64::MIN, f64::max));
        let (y0, y1) = (ys.iter().copied().fold(f64::MAX, f64::min), ys.iter().copied().fold(f64::MIN, f64::max));
        // A cell torn across the antimeridian spans the frame: not a
        // triangle of the projected lattice.
        if x1 - x0 > w as f64 / 4. || y1 - y0 > h as f64 / 4. {
            return;
        }
        let (i0, i1) = ((x0 - 0.5).ceil().max(0.) as usize, (x1 - 0.5).floor().min(w as f64 - 1.));
        let (j0, j1) = ((y0 - 0.5).ceil().max(0.) as usize, (y1 - 0.5).floor().min(h as f64 - 1.));
        if i1 < 0. || j1 < 0. {
            return;
        }
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let (px_, py_) = (i as f64 + 0.5, j as f64 + 0.5);
                let l1 = ((b.0[1] - c.0[1]) * (px_ - c.0[0]) + (c.0[0] - b.0[0]) * (py_ - c.0[1])) / det;
                let l2 = ((c.0[1] - a.0[1]) * (px_ - c.0[0]) + (a.0[0] - c.0[0]) * (py_ - c.0[1])) / det;
                let l3 = 1. - l1 - l2;
                if l1 >= -1e-9 && l2 >= -1e-9 && l3 >= -1e-9 {
                    values[j * w + i] = l1 * a.1 + l2 * b.1 + l3 * c.1;
                }
            }
        }
    };
    for j in 0..grid.height.saturating_sub(1) {
        for i in 0..grid.width.saturating_sub(1) {
            let k = j * grid.width + i;
            let (p00, p10, p01, p11) = (points[k], points[k + 1], points[k + grid.width], points[k + grid.width + 1]);
            if let (Some(a), Some(b), Some(c)) = (p00, p10, p11) {
                fill([a, b, c]);
            }
            if let (Some(a), Some(b), Some(c)) = (p00, p11, p01) {
                fill([a, b, c]);
            }
        }
    }
    let mut out = crate::raster::RasterGrid::from_values(
        values.iter().map(|&v| if v.is_nan() { Value::Null } else { Value::number(v) }).collect(),
        w,
        h,
        [0., 0., pw, ph],
    );
    // Plot extrapolates beyond the samples' hull from its edge; the nearest
    // sample stands in for that.
    let samples: Vec<(f64, f64, Value)> = points
        .iter()
        .flatten()
        .map(|(p, v)| (p[0] / w as f64 * pw, p[1] / h as f64 * ph, Value::number(*v)))
        .collect();
    let nearest = crate::raster::RasterGrid::from_samples(
        &samples,
        w,
        h,
        [0., 0., pw, ph],
        crate::raster::Interpolate::Nearest,
    );
    for (k, v) in out.values.iter_mut().enumerate() {
        if v.is_null() {
            *v = nearest.values[k].clone();
        }
    }
    out
}

/// A raster grid as an RGBA image, one pixel per cell (top row first), kept
/// across repaints for the same grid, opacity and colors.
fn raster_image(
    grid: &std::rc::Rc<crate::raster::RasterGrid>,
    opacity: f32,
    flip_x: bool,
    flip_y: bool,
    device: (usize, usize),
    color: impl Fn(&Value) -> Option<Hsla>,
) -> std::sync::Arc<gpui::RenderImage> {
    let (w, h) = (grid.width, grid.height);
    // The grid is itself remembered across repaints, so its address names
    // it; a few cells' colors stand in for the color scale.
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (std::rc::Rc::as_ptr(grid) as usize, opacity.to_bits(), flip_x, flip_y, device)
            .hash(&mut hasher);
        let n = grid.values.len();
        for k in (0..n).step_by((n / 31).max(1)) {
            color(&grid.values[k])
                .map(|c| [c.h, c.s, c.l, c.a].map(f32::to_bits))
                .hash(&mut hasher);
        }
        hasher.finish()
    };
    type Images = Vec<(
        u64,
        std::rc::Rc<crate::raster::RasterGrid>,
        std::sync::Arc<gpui::RenderImage>,
    )>;
    thread_local! {
        static IMAGES: std::cell::RefCell<Images> = const { std::cell::RefCell::new(Vec::new()) };
    }
    if let Some(image) = IMAGES
        .with_borrow(|images| images.iter().find(|(k, _, _)| *k == key).map(|(_, _, i)| i.clone()))
    {
        return image;
    }
    let mut rgba = vec![0u8; w * h * 4];
    for j in 0..h {
        for i in 0..w {
            let Some(c) = color(&grid.values[j * w + i]) else {
                continue;
            };
            let c = gpui::Rgba::from(c);
            let (pi, pj) = (if flip_x { w - 1 - i } else { i }, if flip_y { h - 1 - j } else { j });
            let k = (pj * w + pi) * 4;
            // GPUI's images are BGRA, premultiplied by the sprite shader.
            let a = (c.a * opacity).clamp(0., 1.);
            rgba[k] = (c.b * 255.).round() as u8;
            rgba[k + 1] = (c.g * 255.).round() as u8;
            rgba[k + 2] = (c.r * 255.).round() as u8;
            rgba[k + 3] = (a * 255.).round() as u8;
        }
    }
    // Nearest-neighbour upscale, at least one device pixel per cell.
    let (dw, dh) = (device.0.max(w), device.1.max(h));
    let mut scaled = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let sy = y * h / dh;
        for x in 0..dw {
            let sx = x * w / dw;
            let (from, to) = ((sy * w + sx) * 4, (y * dw + x) * 4);
            scaled[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    let image = crate::image_from_rgba(dw as u32, dh as u32, scaled);
    IMAGES.with_borrow_mut(|images| {
        images.insert(0, (key, grid.clone(), image.clone()));
        images.truncate(8);
    });
    image
}

/// Plot's `contour`: the grid's isobands, one closed region per threshold
/// (the area at or above it), filled through the color scale at the
/// threshold's value and stroked in `currentColor` unless filled.
pub fn draw_contour_grid(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let Some(grid) = crate::render::resolve_raster(prepared, ctx.frame) else {
        return;
    };
    let spec = prepared.mark.raster.clone().unwrap_or_default();
    let thresholds = grid.thresholds(spec.thresholds.as_ref());
    let style = &prepared.mark.style;
    let filled = prepared.mark.channels.fill.is_some();
    let stroke = match &prepared.mark.channels.stroke {
        Some(channel) => Some(ctx.colors.channel(channel, &crate::table::Table::empty())),
        None if !filled => Some(ctx.colors.default_stroke(prepared, ctx.mark_index)),
        None => None,
    };
    let place = |p: [f64; 2], ctx: &DrawContext<'_>| -> Option<Point<Pixels>> {
        if let Some(projection) = &ctx.frame.projection {
            let [x, y] = projection.project(p[0], p[1])?;
            return Some(ctx.absolute(point(px(x as f32), px(y as f32))));
        }
        Some(ctx.absolute(point(
            px(ctx.frame.x.scale.position(&Value::Number(p[0]))?),
            px(ctx.frame.y.scale.position(&Value::Number(p[1]))?),
        )))
    };
    // A grid's rings at a threshold are the same on every repaint: the last
    // few grids' rings are kept. Each entry holds its grid weakly, which
    // keeps the address it is found by from being reused without keeping
    // the grid's cells alive; an entry whose grid is gone is dropped.
    type Rings = std::rc::Rc<Vec<Vec<[f64; 2]>>>;
    type Entry = (std::rc::Weak<crate::raster::RasterGrid>, u64, Rings);
    thread_local! {
        static CONTOURS: std::cell::RefCell<Vec<Entry>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    let contour = |threshold: f64| -> Rings {
        let bits = threshold.to_bits();
        if let Some(rings) = CONTOURS.with_borrow(|kept| {
            kept.iter()
                .find(|(g, t, _)| {
                    *t == bits && std::ptr::eq(g.as_ptr(), std::rc::Rc::as_ptr(&grid))
                })
                .map(|(_, _, rings)| rings.clone())
        }) {
            return rings;
        }
        let rings = std::rc::Rc::new(grid.contour(threshold));
        CONTOURS.with_borrow_mut(|kept| {
            kept.retain(|(g, _, _)| g.strong_count() > 0);
            kept.insert(0, (std::rc::Rc::downgrade(&grid), bits, rings.clone()));
            kept.truncate(64);
        });
        rings
    };
    ctx.clipped(|ctx| {
        for threshold in thresholds {
            let rings: Vec<Vec<Point<Pixels>>> = contour(threshold)
                .iter()
                .filter_map(|ring| {
                    ring.iter()
                        .map(|p| place(*p, ctx))
                        .collect::<Option<Vec<_>>>()
                })
                .collect();
            if filled {
                // A constant fill paints as itself; a channel fill through
                // the color scale at the threshold.
                let color = match &prepared.mark.channels.fill {
                    Some(crate::style::ColorChannel::Constant(c)) => Some(*c),
                    _ => ctx.colors.scale_color(&Value::Number(threshold)),
                };
                if let Some(color) = color {
                    paint::paint_fill_rings(
                        &rings,
                        paint::alpha(color, style.opacity * style.fill_opacity),
                        ctx.window,
                    );
                }
            }
            if let Some(stroke) = stroke {
                for ring in &rings {
                    paint::paint_stroke(
                        ring,
                        style.stroke_width,
                        paint::alpha(stroke, style.opacity * style.stroke_opacity),
                        ctx.window,
                    );
                }
            }
        }
    });
}
