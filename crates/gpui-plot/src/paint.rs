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

/// A path built from points, filled.
pub fn fill_path(points: &[Point<Pixels>], color: Hsla) -> Option<Path<Pixels>> {
    if points.len() < 3 || !finite(points) {
        return None;
    }
    let mut builder = PathBuilder::fill();
    builder.add_polygon(points, true);
    builder.build().ok().map(|path| with_alpha(path, color))
}

/// A path built from points, stroked.
pub fn stroke_path(points: &[Point<Pixels>], width: f32, color: Hsla) -> Option<Path<Pixels>> {
    // A non-finite point (a NaN that slipped through a channel) would panic
    // the tessellator; skip the shape instead.
    if points.is_empty() || !finite(points) {
        return None;
    }
    let mut builder = PathBuilder::stroke(px(width.max(0.1)));
    builder.move_to(points[0]);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.build().ok().map(|path| with_alpha(path, color))
}

/// Whether every point is finite, which the tessellator requires.
pub fn finite(points: &[Point<Pixels>]) -> bool {
    points.iter().all(|p| p.x.as_f32().is_finite() && p.y.as_f32().is_finite())
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
    let mut builder = PathBuilder::fill();
    let mut any = false;
    for ring in rings.iter().filter(|r| r.len() >= 3 && finite(r)) {
        builder.add_polygon(ring, true);
        any = true;
    }
    if !any {
        return;
    }
    if let Ok(path) = builder.build() {
        window.paint_path(with_alpha(path, color), color);
    }
}
