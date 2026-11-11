//! Plot's markers: the dots, arrows and ticks at a path's vertices.

use gpui::{Hsla, Pixels, Point, Window, point, px};

use crate::paint;

/// Paint Plot's markers at a path's vertices: `markerStart` at the first,
/// `markerMid` between, `markerEnd` at the last. A marker scales with the
/// stroke width, as SVG's `markerUnits="strokeWidth"` does.
pub fn paint_markers(
    points: &[Point<Pixels>],
    markers: &[Option<crate::mark::Marker>; 3],
    color: Hsla,
    stroke_width: f32,
    background: Hsla,
    window: &mut Window,
) {
    use crate::mark::Marker;
    if points.is_empty() || markers.iter().all(Option::is_none) {
        return;
    }
    let k = 0.667 * stroke_width;
    let direction = |i: usize| -> f32 {
        let (a, b) = if i + 1 < points.len() {
            (points[i], points[i + 1])
        } else if i > 0 {
            (points[i - 1], points[i])
        } else {
            return 0.;
        };
        (b.y.as_f32() - a.y.as_f32()).atan2(b.x.as_f32() - a.x.as_f32())
    };
    let last = points.len() - 1;
    for (i, at) in points.iter().enumerate() {
        let which = if i == 0 {
            0
        } else if i == last {
            2
        } else {
            1
        };
        let marker = if points.len() == 1 {
            markers[0].or(markers[2])
        } else {
            markers[which]
        };
        let Some(marker) = marker else {
            continue;
        };
        let (cx, cy) = (at.x.as_f32(), at.y.as_f32());
        let rotate = |x: f32, y: f32, angle: f32| {
            let (s, c) = angle.sin_cos();
            point(px(cx + (x * c - y * s) * k), px(cy + (x * s + y * c) * k))
        };
        match marker {
            Marker::Dot => {
                paint::paint_fill(&paint::circle_points(*at, 2.5 * k, 20), color, window)
            }
            Marker::CircleFill => {
                paint::paint_fill(&paint::circle_points(*at, 3. * k, 20), color, window);
                let mut ring = paint::circle_points(*at, 3. * k, 20);
                ring.push(ring[0]);
                paint::paint_stroke(&ring, 1.5 * k, background, window);
            }
            Marker::CircleStroke => {
                paint::paint_fill(&paint::circle_points(*at, 3. * k, 20), background, window);
                let mut ring = paint::circle_points(*at, 3. * k, 20);
                ring.push(ring[0]);
                paint::paint_stroke(&ring, 1.5 * k, color, window);
            }
            Marker::Arrow | Marker::ArrowReverse => {
                let mut angle = direction(i);
                if marker == Marker::ArrowReverse && which == 0 {
                    angle += std::f32::consts::PI;
                }
                let chevron = [
                    rotate(-1.5, -3., angle),
                    rotate(1.5, 0., angle),
                    rotate(-1.5, 3., angle),
                ];
                paint::paint_stroke(&chevron, 1.5 * k, color, window);
            }
            Marker::Tick | Marker::TickX | Marker::TickY => {
                let angle = match marker {
                    Marker::TickX => std::f32::consts::FRAC_PI_2,
                    Marker::TickY => 0.,
                    _ => direction(i),
                };
                let scale = stroke_width;
                let rot = |x: f32, y: f32| {
                    let (s, c) = angle.sin_cos();
                    point(
                        px(cx + (x * c - y * s) * scale),
                        px(cy + (x * s + y * c) * scale),
                    )
                };
                paint::paint_stroke(
                    &[rot(0., -3.), rot(0., 3.)],
                    stroke_width.min(1.),
                    color,
                    window,
                );
            }
        }
    }
}
