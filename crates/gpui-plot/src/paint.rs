//! Painting helpers: the shapes marks draw, and the text they label with.
//!
//! Every mark reduces to a handful of primitives — a filled path, a stroked
//! path, a quad, a shaped line — so the marks share these helpers and stay
//! short. Painting happens inside the plot's paint phase, which is where GPUI
//! allows `paint_path`, `paint_quad` and text painting.

use gpui::{
    App, Bounds, Corners, Edges, Font, FontWeight, Hsla, Path, PathBuilder, Pixels, Point,
    ShapedLine, SharedString, Size, TextAlign, TextRun, Window, point, px, quad, size,
};

thread_local! {
    /// The convex polygon the current mark is clipped to (Plot's
    /// `clip: "sphere"`), counter-clockwise in window pixels. GPUI's content
    /// masks are rectangles, so the fills and strokes below cut their
    /// geometry against it instead.
    static CLIP: std::cell::RefCell<Option<Vec<Point<Pixels>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Tessellated paths by exactly what they were built from, so a plot that
/// paints the same shapes again (each repaint while the pointer moves) takes
/// them from here rather than tessellating them again.
///
/// A path is a pure function of its builder's input: its kind (a fill, the
/// even-odd fill of several rings, or a stroke and its width) and its
/// contours' points. An entry is used only when all of those are equal bit
/// for bit, so a hit is the path the builder would build. Failures (an
/// outline the tessellator gives up on) are kept too. A path is kept the
/// second time it is built, so a shape painted once (a plot's first frame)
/// pays only for its hash. When the entries hold more points than
/// [`TESSELLATION_BUDGET`], they are dropped and the cache starts again.
#[derive(Default)]
struct Tessellations {
    entries: rustc_hash::FxHashMap<u64, (TessellationKey, Option<Path<Pixels>>)>,
    /// The hashes of the inputs built once and not kept yet.
    seen: rustc_hash::FxHashSet<u64>,
    held: usize,
}

/// What a tessellated path was built from.
#[derive(PartialEq)]
struct TessellationKey {
    /// 0 for a fill, 1 for an even-odd fill of rings, else 2 and a stroke's
    /// width bits.
    kind: (u8, u32),
    /// Each contour's length, then every point's bits.
    lengths: Vec<usize>,
    points: Vec<[u32; 2]>,
}

/// How many points (of the keys and the paths' vertices) the cache holds.
const TESSELLATION_BUDGET: usize = 1 << 21;

thread_local! {
    static TESSELLATIONS: std::cell::RefCell<Tessellations> = Default::default();
}

/// The path `build` makes from `contours`, from the cache when it already
/// built one from the same input.
fn tessellated(
    kind: (u8, u32),
    contours: &[&[Point<Pixels>]],
    build: impl FnOnce() -> Option<Path<Pixels>>,
) -> Option<Path<Pixels>> {
    use std::hash::{Hash, Hasher};
    let bits = |p: &Point<Pixels>| [p.x.as_f32().to_bits(), p.y.as_f32().to_bits()];
    let mut hasher = rustc_hash::FxHasher::default();
    kind.hash(&mut hasher);
    for contour in contours {
        hasher.write_usize(contour.len());
        for point in *contour {
            let [x, y] = bits(point);
            hasher.write_u64(u64::from(x) << 32 | u64::from(y));
        }
    }
    let hash = hasher.finish();
    let matches = |key: &TessellationKey| {
        key.kind == kind
            && key.lengths.len() == contours.len()
            && key.lengths.iter().zip(contours).all(|(n, c)| *n == c.len())
            && key
                .points
                .iter()
                .zip(contours.iter().flat_map(|c| c.iter()))
                .all(|(kept, point)| *kept == bits(point))
    };
    if let Some(path) = TESSELLATIONS.with_borrow(|cache| {
        cache
            .entries
            .get(&hash)
            .filter(|(key, _)| matches(key))
            .map(|(_, path)| path.clone())
    }) {
        return path;
    }
    let path = build();
    let points: usize = contours.iter().map(|c| c.len()).sum();
    let size = points + path.as_ref().map_or(0, |p| p.vertices.len());
    TESSELLATIONS.with_borrow_mut(|cache| {
        if cache.held + size > TESSELLATION_BUDGET || cache.seen.len() > TESSELLATION_BUDGET / 8 {
            cache.entries.clear();
            cache.seen.clear();
            cache.held = 0;
        }
        if size > TESSELLATION_BUDGET / 4 || cache.seen.insert(hash) {
            return;
        }
        let key = TessellationKey {
            kind,
            lengths: contours.iter().map(|c| c.len()).collect(),
            points: contours.iter().flat_map(|c| c.iter()).map(bits).collect(),
        };
        cache.held += size;
        if let Some((old, path)) = cache.entries.insert(hash, (key, path.clone())) {
            cache.held -= old.points.len() + path.map_or(0, |p| p.vertices.len());
        }
    });
    path
}

/// Run `f` with fills and strokes clipped to `ring`, when it is a convex
/// polygon; a concave outline (a conic's sphere) leaves the mark unclipped.
pub fn with_clip_polygon<R>(ring: Option<Vec<Point<Pixels>>>, f: impl FnOnce() -> R) -> R {
    let ring = ring.and_then(convex_ccw);
    let previous = CLIP.with_borrow_mut(|clip| std::mem::replace(clip, ring));
    let result = f();
    CLIP.with_borrow_mut(|clip| *clip = previous);
    result
}

/// `ring` as a counter-clockwise convex polygon, or `None` when it is not
/// convex.
fn convex_ccw(mut ring: Vec<Point<Pixels>>) -> Option<Vec<Point<Pixels>>> {
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < 3 || !finite(&ring) {
        return None;
    }
    let n = ring.len();
    let cross = |i: usize| {
        let (a, b, c) = (ring[i], ring[(i + 1) % n], ring[(i + 2) % n]);
        (b.x - a.x).as_f32() * (c.y - b.y).as_f32() - (b.y - a.y).as_f32() * (c.x - b.x).as_f32()
    };
    let turns: Vec<f32> = (0..n).map(cross).filter(|c| c.abs() > 1e-3).collect();
    let positive = turns.iter().filter(|c| **c > 0.).count();
    if positive != 0 && positive != turns.len() {
        return None;
    }
    // In y-down pixels a positive cross product turns clockwise on screen;
    // the half-plane test below wants the interior on the left.
    if positive == 0 {
        ring.reverse();
    }
    Some(ring)
}

/// Which side of edge `a → b` the point is on: positive inside.
fn side(a: Point<Pixels>, b: Point<Pixels>, p: Point<Pixels>) -> f32 {
    (b.x - a.x).as_f32() * (p.y - a.y).as_f32() - (b.y - a.y).as_f32() * (p.x - a.x).as_f32()
}

fn lerp(a: Point<Pixels>, b: Point<Pixels>, t: f32) -> Point<Pixels> {
    point(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// A polygon cut to the clip polygon (Sutherland–Hodgman), or as it is
/// when nothing clips.
fn clip_polygon(points: &[Point<Pixels>]) -> std::borrow::Cow<'_, [Point<Pixels>]> {
    CLIP.with_borrow(|clip| {
        let Some(clip) = clip else {
            return std::borrow::Cow::Borrowed(points);
        };
        let mut out = points.to_vec();
        for (i, &a) in clip.iter().enumerate() {
            let b = clip[(i + 1) % clip.len()];
            let input = std::mem::take(&mut out);
            for (j, &p) in input.iter().enumerate() {
                let q = input[(j + 1) % input.len()];
                let (sp, sq) = (side(a, b, p), side(a, b, q));
                if sp >= 0. {
                    out.push(p);
                }
                if (sp >= 0.) != (sq >= 0.) {
                    out.push(lerp(p, q, sp / (sp - sq)));
                }
            }
            if out.is_empty() {
                break;
            }
        }
        std::borrow::Cow::Owned(out)
    })
}

/// A polyline cut to the clip polygon (Cyrus–Beck per segment), as the
/// runs that stay inside; the whole line when nothing clips.
fn clip_polyline(points: &[Point<Pixels>]) -> Vec<Vec<Point<Pixels>>> {
    CLIP.with_borrow(|clip| {
        let Some(clip) = clip else {
            return vec![points.to_vec()];
        };
        let mut runs: Vec<Vec<Point<Pixels>>> = Vec::new();
        let mut current: Vec<Point<Pixels>> = Vec::new();
        for pair in points.windows(2) {
            let (p, q) = (pair[0], pair[1]);
            let (mut t0, mut t1) = (0f32, 1f32);
            for (i, &a) in clip.iter().enumerate() {
                let b = clip[(i + 1) % clip.len()];
                let (sp, sq) = (side(a, b, p), side(a, b, q));
                if sp < 0. && sq < 0. {
                    t0 = 1.;
                    t1 = 0.;
                    break;
                }
                let t = sp / (sp - sq);
                if sp < 0. {
                    t0 = t0.max(t);
                } else if sq < 0. {
                    t1 = t1.min(t);
                }
            }
            if t0 > t1 {
                if current.len() > 1 {
                    runs.push(std::mem::take(&mut current));
                }
                current.clear();
                continue;
            }
            let (from, to) = (lerp(p, q, t0), lerp(p, q, t1));
            if current.last() != Some(&from) {
                if current.len() > 1 {
                    runs.push(std::mem::take(&mut current));
                }
                current = vec![from];
            }
            current.push(to);
            if t1 < 1. {
                runs.push(std::mem::take(&mut current));
            }
        }
        if current.len() > 1 {
            runs.push(current);
        }
        runs
    })
}

/// A path built from points, filled.
pub fn fill_path(points: &[Point<Pixels>], color: Hsla) -> Option<Path<Pixels>> {
    let points = clip_polygon(points);
    let points = points.as_ref();
    if points.len() < 3 || !finite(points) {
        return None;
    }
    tessellated((0, 0), &[points], || {
        let mut builder = PathBuilder::fill();
        builder.add_polygon(points, true);
        builder.build().ok()
    })
    .map(|path| with_alpha(path, color))
}

/// Fill a polygon by SVG's `nonzero` rule, scanline by scanline: the
/// fallback for an outline that folds over itself so often the tessellator
/// gives up (an area over unsorted data). Each half-pixel row paints the
/// spans where the polygon's winding number is not zero.
pub fn paint_fill_nonzero(points: &[Point<Pixels>], color: Hsla, window: &mut Window) {
    let points = clip_polygon(points);
    let points = points.as_ref();
    if points.len() < 3 || !finite(points) {
        return;
    }
    let ys = points.iter().map(|p| p.y.as_f32());
    let (lo, hi) = ys.fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), y| {
        (a.min(y), b.max(y))
    });
    const STEP: f32 = 0.5;
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    let mut y = (lo / STEP).floor() * STEP;
    while y < hi {
        let center = y + STEP / 2.;
        crossings.clear();
        for (i, a) in points.iter().enumerate() {
            let b = points[(i + 1) % points.len()];
            let (ay, by) = (a.y.as_f32(), b.y.as_f32());
            if (ay <= center) == (by <= center) {
                continue;
            }
            let t = (center - ay) / (by - ay);
            let x = a.x.as_f32() + t * (b.x.as_f32() - a.x.as_f32());
            crossings.push((x, if by > ay { 1 } else { -1 }));
        }
        crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut winding = 0;
        let mut start = 0.;
        for &(x, direction) in &crossings {
            let was = winding;
            winding += direction;
            if was == 0 && winding != 0 {
                start = x;
            } else if was != 0 && winding == 0 && x > start {
                window.paint_quad(gpui::fill(
                    Bounds::new(point(px(start), px(y)), size(px(x - start), px(STEP))),
                    color,
                ));
            }
        }
        y += STEP;
    }
}

/// A path built from points, stroked.
pub fn stroke_path(points: &[Point<Pixels>], width: f32, color: Hsla) -> Option<Path<Pixels>> {
    // A non-finite point (a NaN that slipped through a channel) would panic
    // the tessellator; skip the shape instead.
    if points.is_empty() || !finite(points) {
        return None;
    }
    let width = width.max(0.1);
    let clipped = CLIP.with_borrow(Option::is_some);
    let runs = if clipped {
        let runs = clip_polyline(points);
        if runs.is_empty() {
            return None;
        }
        Some(runs)
    } else {
        None
    };
    // Each run is a contour: moved to its first point, lined to the rest.
    let contours: Vec<&[Point<Pixels>]> = match &runs {
        Some(runs) => runs.iter().map(Vec::as_slice).collect(),
        None => vec![points],
    };
    tessellated((2, width.to_bits()), &contours, || {
        let mut builder = PathBuilder::stroke(px(width));
        for run in &contours {
            builder.move_to(run[0]);
            for point in &run[1..] {
                builder.line_to(*point);
            }
        }
        builder.build().ok()
    })
    .map(|path| with_alpha(path, color))
}

/// Whether every point is finite, which the tessellator requires.
pub fn finite(points: &[Point<Pixels>]) -> bool {
    points
        .iter()
        .all(|p| p.x.as_f32().is_finite() && p.y.as_f32().is_finite())
}

/// Paint a filled path.
pub fn paint_fill(points: &[Point<Pixels>], color: Hsla, window: &mut Window) {
    if let Some(path) = fill_path(points, color) {
        window.paint_path(path, color);
    }
}

/// Paint a stroked path.
pub fn paint_stroke(points: &[Point<Pixels>], width: f32, color: Hsla, window: &mut Window) {
    if let Some(path) = stroke_path(points, width, color) {
        window.paint_path(path, color);
    }
}

/// Paint an unfilled path built by the caller.
pub fn paint_path(path: Option<Path<Pixels>>, color: Hsla, window: &mut Window) {
    if let Some(path) = path {
        window.paint_path(path, color);
    }
}

/// Paint a rectangle with rounded corners.
pub fn paint_rect(bounds: Bounds<Pixels>, radius: f32, color: Hsla, window: &mut Window) {
    window.paint_quad(quad(
        bounds,
        Corners::all(px(radius)),
        color,
        Edges::all(px(0.)),
        Hsla::transparent_black(),
        gpui::BorderStyle::default(),
    ));
}

/// Paint a stroked rectangle outline.
pub fn paint_rect_outline(
    bounds: Bounds<Pixels>,
    radius: f32,
    width: f32,
    color: Hsla,
    window: &mut Window,
) {
    window.paint_quad(quad(
        bounds,
        Corners::all(px(radius)),
        Hsla::transparent_black(),
        Edges::all(px(width)),
        color,
        gpui::BorderStyle::default(),
    ));
}

/// A rectangle between two corners, normalised so `min` is its origin.
pub fn rect_between(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    let min = point(a.x.min(b.x), a.y.min(b.y));
    let max = point(a.x.max(b.x), a.y.max(b.y));
    Bounds::new(min, size(max.x - min.x, max.y - min.y))
}

/// A circle as a polygon.
pub fn circle_points(center: Point<Pixels>, radius: f32, segments: usize) -> Vec<Point<Pixels>> {
    (0..segments.max(8))
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / segments.max(8) as f32;
            point(
                px(center.x.as_f32() + radius * angle.cos()),
                px(center.y.as_f32() + radius * angle.sin()),
            )
        })
        .collect()
}

/// An arc's outline: the outer edge from `start` to `end` (radians, clockwise
/// from twelve o'clock), then the inner edge back.
pub fn arc_points(
    center: Point<Pixels>,
    inner_radius: f32,
    outer_radius: f32,
    start_angle: f32,
    end_angle: f32,
) -> Vec<Point<Pixels>> {
    let span = (end_angle - start_angle).abs();
    let segments = ((span / (std::f32::consts::TAU)) * 96.).ceil().max(2.) as usize;
    let mut points = Vec::with_capacity(segments * 2 + 2);
    // Angles run clockwise from twelve o'clock: sine and cosine with y negated.
    for i in 0..=segments {
        let angle = start_angle + (end_angle - start_angle) * i as f32 / segments as f32;
        points.push(point(
            px(center.x.as_f32() + outer_radius * angle.sin()),
            px(center.y.as_f32() - outer_radius * angle.cos()),
        ));
    }
    for i in (0..=segments).rev() {
        let angle = start_angle + (end_angle - start_angle) * i as f32 / segments as f32;
        points.push(point(
            px(center.x.as_f32() + inner_radius * angle.sin()),
            px(center.y.as_f32() - inner_radius * angle.cos()),
        ));
    }
    points
}

/// The point on an arc at `angle` and `radius`, in the same clockwise
/// convention [`arc_points`] uses.
pub fn arc_point(center: Point<Pixels>, radius: f32, angle: f32) -> Point<Pixels> {
    point(
        px(center.x.as_f32() + radius * angle.sin()),
        px(center.y.as_f32() - radius * angle.cos()),
    )
}

fn with_alpha(path: Path<Pixels>, color: Hsla) -> Path<Pixels> {
    // The path's own color is set by `paint_path`; carrying the alpha here
    // keeps a translucent mark translucent.
    let mut path = path;
    path.color = color.into();
    path
}

/// A shaped line ready to paint, with the width the caller can align against.
pub struct Label {
    line: ShapedLine,
    pub width: Pixels,
    pub font_size: Pixels,
    pub height: Pixels,
}

impl Label {
    /// Shape `text` at `font_size` in the window's font, in `color`.
    pub fn new(
        text: SharedString,
        font_size: Pixels,
        color: Hsla,
        weight: FontWeight,
        font: Font,
        window: &mut Window,
    ) -> Self {
        let font = Font { weight, ..font };
        let run = TextRun {
            len: text.len(),
            font,
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(text, font_size, &[run], None);
        let width = line.width();
        Self {
            line,
            width,
            font_size,
            height: font_size,
        }
    }

    /// How far below a label's top [`Label::paint_at`] puts its baseline:
    /// GPUI centres the font's ascent and descent in the line box.
    pub fn baseline(&self) -> Pixels {
        (self.height + self.line.ascent - self.line.descent) / 2.
    }

    /// The top that puts this label's baseline at `baseline`, for placing
    /// text the way SVG does, by its baseline (Plot's `dy: "0.71em"`).
    pub fn top_for_baseline(&self, baseline: Pixels) -> Pixels {
        baseline - self.baseline()
    }

    /// Paint the line with its top-left at `origin`, aligned within `width`.
    pub fn paint_at(
        &self,
        origin: Point<Pixels>,
        align: TextAlign,
        align_width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = match align {
            TextAlign::Left => origin,
            TextAlign::Center => origin - point(self.width / 2., px(0.)),
            TextAlign::Right => origin - point(self.width, px(0.)),
        };
        let _ = self
            .line
            .paint(origin, self.height, align, align_width, window, cx);
    }
}

/// Measure `text` at `font_size` without shaping it twice when the caller then
/// paints it.
pub fn measure(text: &SharedString, font_size: Pixels, window: &mut Window) -> Label {
    let style = window.text_style();
    let color = style.color;
    let font = style.font();
    Label::new(
        text.clone(),
        font_size,
        color,
        FontWeight::NORMAL,
        font,
        window,
    )
}

/// A translucent version of `color`.
pub fn alpha(color: Hsla, alpha: f32) -> Hsla {
    Hsla {
        a: color.a * alpha.clamp(0., 1.),
        ..color
    }
}

/// A size from a width and height.
pub fn size_f32(width: f32, height: f32) -> Size<Pixels> {
    size(px(width), px(height))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use gpui::{point, px};

    #[test]
    fn rects_normalize() {
        let rect = rect_between(point(px(10.), px(20.)), point(px(4.), px(6.)));
        assert_eq!(rect.origin, point(px(4.), px(6.)));
        assert_eq!(rect.size, size(px(6.), px(14.)));
    }

    #[test]
    fn arcs_trace_out_and_back() {
        let points = arc_points(point(px(0.), px(0.)), 4., 8., 0., std::f32::consts::PI);
        assert!(points.len() > 20);
        // Twelve o'clock sits above the center.
        assert_eq!(points[0], point(px(0.), px(-8.)));
    }

    #[test]
    fn paths_need_enough_points() {
        assert!(fill_path(&[], Hsla::default()).is_none());
        assert!(fill_path(&[point(px(0.), px(0.)); 2], Hsla::default()).is_none());
        assert!(stroke_path(&[point(px(0.), px(0.))], 1., Hsla::default()).is_some());
    }

    fn square() -> Vec<Point<Pixels>> {
        // Clockwise on screen (y down), to check the winding is normalised.
        [(0., 0.), (10., 0.), (10., 10.), (0., 10.)]
            .map(|(x, y)| point(px(x), px(y)))
            .to_vec()
    }

    #[test]
    fn a_fill_is_cut_to_a_convex_clip() {
        let big: Vec<_> = [(-5., -5.), (5., -5.), (5., 5.), (-5., 5.)]
            .map(|(x, y)| point(px(x), px(y)))
            .to_vec();
        let cut = with_clip_polygon(Some(square()), || clip_polygon(&big).into_owned());
        for p in &cut {
            assert!((0. ..=5.).contains(&p.x.as_f32()) && (0. ..=5.).contains(&p.y.as_f32()));
        }
        assert_eq!(cut.len(), 4);
        // Without a clip the polygon is untouched.
        assert_eq!(clip_polygon(&big).len(), 4);
    }

    #[test]
    fn a_polyline_keeps_only_its_inside_runs() {
        let line: Vec<_> = [(-5., 5.), (5., 5.), (15., 5.), (15., 8.), (5., 8.)]
            .map(|(x, y)| point(px(x), px(y)))
            .to_vec();
        let runs = with_clip_polygon(Some(square()), || clip_polyline(&line));
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].first().map(|p| p.x.as_f32()), Some(0.));
        assert_eq!(runs[0].last().map(|p| p.x.as_f32()), Some(10.));
        assert_eq!(runs[1].first().map(|p| p.x.as_f32()), Some(10.));
        assert_eq!(runs[1].last().map(|p| p.x.as_f32()), Some(5.));
    }

    #[test]
    fn a_concave_clip_is_ignored() {
        let notch: Vec<_> = [(0., 0.), (10., 0.), (5., 5.), (10., 10.), (0., 10.)]
            .map(|(x, y)| point(px(x), px(y)))
            .to_vec();
        assert!(convex_ccw(notch).is_none());
    }
}

/// Paint a dashed line from `start` to `end`, Plot's `strokeDasharray`.
pub fn paint_dashed(
    start: Point<Pixels>,
    end: Point<Pixels>,
    dash: &[f32],
    width: f32,
    color: Hsla,
    window: &mut Window,
) {
    let dx = end.x.as_f32() - start.x.as_f32();
    let dy = end.y.as_f32() - start.y.as_f32();
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0. || dash.is_empty() {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let mut offset = 0.0_f32;
    let mut on = true;
    let mut index = 0;
    while offset < length {
        let segment = dash[index % dash.len()].max(0.5);
        let end_offset = (offset + segment).min(length);
        if on {
            let from = point(
                px(start.x.as_f32() + ux * offset),
                px(start.y.as_f32() + uy * offset),
            );
            let to = point(
                px(start.x.as_f32() + ux * end_offset),
                px(start.y.as_f32() + uy * end_offset),
            );
            paint_stroke(&[from, to], width, color, window);
        }
        offset = end_offset;
        on = !on;
        index += 1;
    }
}

/// Paint a dashed polyline, the dash running on across its vertices.
pub fn paint_dashed_path(
    points: &[Point<Pixels>],
    dash: &[f32],
    width: f32,
    color: Hsla,
    window: &mut Window,
) {
    for pair in points.windows(2) {
        paint_dashed(pair[0], pair[1], dash, width, color, window);
    }
}

/// Fill several rings as one shape with the even-odd rule, so a polygon's
/// holes stay open.
pub fn paint_fill_rings(rings: &[Vec<Point<Pixels>>], color: Hsla, window: &mut Window) {
    let clipped: Vec<std::borrow::Cow<'_, [Point<Pixels>]>> = rings
        .iter()
        .filter(|r| r.len() >= 3 && finite(r))
        .map(|ring| clip_polygon(ring))
        .filter(|ring| ring.len() >= 3)
        .collect();
    if clipped.is_empty() {
        return;
    }
    let contours: Vec<&[Point<Pixels>]> = clipped.iter().map(|ring| ring.as_ref()).collect();
    let path = tessellated((1, 0), &contours, || {
        let mut builder = PathBuilder::fill();
        for ring in &contours {
            builder.add_polygon(ring, true);
        }
        builder.build().ok()
    });
    if let Some(path) = path {
        window.paint_path(with_alpha(path, color), color);
    }
}

/// Paint `text` rotated by `degrees` clockwise about `anchor`, Plot's
/// `rotate` and `tickRotate`.
///
/// GPUI rotates sprites but not shaped text, so the label is drawn as a tiny
/// SVG `<text>` (rendered with GPUI's SVG renderer and its system fonts) and
/// the sprite is rotated about the anchor. `align` places the text's start,
/// middle or end at the anchor; it is vertically centred on it.
#[allow(clippy::too_many_arguments)]
pub fn paint_rotated_text(
    text: &str,
    font_size: f32,
    color: Hsla,
    family: &str,
    anchor: Point<Pixels>,
    degrees: f32,
    align: TextAlign,
    window: &mut Window,
    cx: &mut App,
) {
    if text.is_empty() {
        return;
    }
    // A label may run to several lines (`16\nMar`): each is a line of the
    // block, which centres on the anchor as Plot's middle line anchor does.
    let lines: Vec<&str> = text.split('\n').collect();
    let widest = lines
        .iter()
        .map(|line| {
            Label::new(
                SharedString::from(line.to_string()),
                px(font_size),
                color,
                FontWeight::NORMAL,
                gpui::font(SharedString::from(family.to_string())),
                window,
            )
            .width
            .as_f32()
        })
        .fold(0., f32::max);
    let width = widest.ceil() + 4.;
    let line_height = font_size * 1.2;
    let height = (font_size * 1.4 + line_height * (lines.len() - 1) as f32).ceil();
    let escape = |line: &str| {
        line.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    // Quoted, so a family CSS can't parse bare (`.SystemUIFont`) doesn't
    // void the whole list and drop the text to the serif default.
    let quoted = family.replace(['\'', '"', '<', '&'], "");
    let (x, text_anchor) = match align {
        TextAlign::Left => (2., "start"),
        TextAlign::Center => (width / 2., "middle"),
        TextAlign::Right => (width - 2., "end"),
    };
    let first = height * 0.5 + font_size * 0.35 - line_height * (lines.len() - 1) as f32 / 2.;
    let spans: String = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            format!(
                r#"<tspan x="{x}" y="{}">{}</tspan>"#,
                first + line_height * i as f32,
                escape(line)
            )
        })
        .collect();
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><text font-family="'{quoted}', 'Helvetica Neue', Helvetica, Arial, sans-serif" font-size="{font_size}" text-anchor="{text_anchor}" fill="black">{spans}</text></svg>"#,
    );
    // The SVG box's own point that sits on the anchor.
    let offset_x = x;
    let origin = point(anchor.x - px(offset_x), anchor.y - px(height / 2.));
    let bounds = Bounds::new(origin, size(px(width), px(height)));
    // Rotate about the anchor, in the scaled pixels the scene uses.
    let scale = window.scale_factor();
    let center = gpui::point(
        gpui::ScaledPixels(anchor.x.as_f32() * scale),
        gpui::ScaledPixels(anchor.y.as_f32() * scale),
    );
    let matrix = gpui::TransformationMatrix::unit()
        .translate(center)
        .rotate(gpui::Radians(degrees.to_radians()))
        .translate(gpui::point(
            gpui::ScaledPixels(-center.x.0),
            gpui::ScaledPixels(-center.y.0),
        ));
    let key = SharedString::from(format!(
        "gpui-plot-text:{family}:{font_size}:{text_anchor}:{text}"
    ));
    let _ = window.paint_svg(bounds, key, Some(svg.as_bytes()), matrix, color, cx);
}
