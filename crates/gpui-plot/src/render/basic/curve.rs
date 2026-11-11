//! d3's curves: the paths a line, area or link draws through its points.

use gpui::{PathBuilder, Pixels, Point, point, px};

use crate::mark::Curve;
use crate::paint;

/// A path through `points`, connected by `curve`, revealed to `appear`.
///
/// The reveal shortens the path from its end, which is how a line draws in.
pub fn curve_path(
    points: &[Point<Pixels>],
    curve: Curve,
    width: f32,
    appear: f32,
) -> Option<gpui::Path<Pixels>> {
    if points.len() < 2 || !paint::finite(points) {
        return None;
    }
    let visible = ((points.len() as f32 * appear.clamp(0., 1.)) as usize)
        .max(1)
        .min(points.len());
    let points = &points[..visible];
    if points.len() < 2 {
        return None;
    }

    let mut builder = PathBuilder::stroke(px(width.max(0.1)));
    builder.move_to(points[0]);
    match curve {
        Curve::Linear => {
            for point in &points[1..] {
                builder.line_to(*point);
            }
        }
        Curve::StepAfter | Curve::StepBefore => {
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let corner = if curve == Curve::StepAfter {
                    point(b.x, a.y)
                } else {
                    point(a.x, b.y)
                };
                builder.line_to(corner);
                builder.line_to(b);
            }
        }
        Curve::Step => {
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let middle = px((a.x.as_f32() + b.x.as_f32()) / 2.);
                builder.line_to(point(middle, a.y));
                builder.line_to(point(middle, b.y));
            }
        }
        Curve::Natural => {
            let n = points.len();
            for i in 0..n - 1 {
                let p0 = if i == 0 { points[0] } else { points[i - 1] };
                let p1 = points[i];
                let p2 = points[i + 1];
                let p3 = if i + 2 < n {
                    points[i + 2]
                } else {
                    points[n - 1]
                };
                let c1 = point(
                    px(p1.x.as_f32() + (p2.x.as_f32() - p0.x.as_f32()) / 6.),
                    px(p1.y.as_f32() + (p2.y.as_f32() - p0.y.as_f32()) / 6.),
                );
                let c2 = point(
                    px(p2.x.as_f32() - (p3.x.as_f32() - p1.x.as_f32()) / 6.),
                    px(p2.y.as_f32() - (p3.y.as_f32() - p1.y.as_f32()) / 6.),
                );
                builder.cubic_bezier_to(p2, c1, c2);
            }
        }
        Curve::LinearClosed => {
            for point in &points[1..] {
                builder.line_to(*point);
            }
            builder.line_to(points[0]);
        }
        Curve::Basis | Curve::BasisOpen | Curve::BasisClosed => basis(&mut builder, points, curve),
        Curve::Cardinal | Curve::CardinalOpen | Curve::CardinalClosed => {
            cardinal(&mut builder, points, 0., curve)
        }
        Curve::CatmullRom | Curve::CatmullRomOpen | Curve::CatmullRomClosed => {
            catmull_rom(&mut builder, points, curve)
        }
        Curve::MonotoneX => monotone(&mut builder, points, true),
        Curve::MonotoneY => monotone(&mut builder, points, false),
        Curve::BumpX => bump(&mut builder, points, true),
        Curve::BumpY => bump(&mut builder, points, false),
        Curve::Bundle => bundle(&mut builder, points),
    }
    if curve.is_closed() && !matches!(curve, Curve::LinearClosed) {
        builder.line_to(points[0]);
    }
    builder.build().ok()
}

/// d3's `basis`: a cubic basis spline that passes near, but not through, the
/// data's points.
fn basis(builder: &mut PathBuilder, points: &[Point<Pixels>], curve: Curve) {
    let n = points.len();
    let open = curve.is_open();
    let (x0, y0) = (points[0].x.as_f32(), points[0].y.as_f32());
    let (x1, y1) = (points[1].x.as_f32(), points[1].y.as_f32());
    // An open spline starts on the first point; a closed one repeats it.
    if open {
        builder.line_to(points[0]);
    } else {
        builder.line_to(point(px((5. * x0 + x1) / 6.), px((5. * y0 + y1) / 6.)));
    }
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let (ax, ay) = (p0.x.as_f32(), p0.y.as_f32());
        let (bx, by) = (p1.x.as_f32(), p1.y.as_f32());
        let (cx, cy) = (p2.x.as_f32(), p2.y.as_f32());
        let c1 = point(px((2. * ax + bx) / 3.), px((2. * ay + by) / 3.));
        let c2 = point(px((ax + 2. * bx) / 3.), px((ay + 2. * by) / 3.));
        let end = point(px((ax + 4. * bx + cx) / 6.), px((ay + 4. * by + cy) / 6.));
        builder.cubic_bezier_to(end, c1, c2);
    }
    let last = points[n - 1];
    let prev = points[n - 2];
    if open {
        builder.line_to(last);
    } else {
        builder.line_to(point(
            px((5. * last.x.as_f32() + prev.x.as_f32()) / 6.),
            px((5. * last.y.as_f32() + prev.y.as_f32()) / 6.),
        ));
    }
    builder.line_to(last);
}

/// d3's `cardinal`: a cardinal spline whose tension shapes the corners.
fn cardinal(builder: &mut PathBuilder, points: &[Point<Pixels>], tension: f32, curve: Curve) {
    let n = points.len();
    let k = (1. - tension) / 6.;
    let open = curve.is_open();
    if open {
        builder.line_to(points[0]);
    }
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1]
        };
        let (ax, ay) = (p0.x.as_f32(), p0.y.as_f32());
        let (bx, by) = (p1.x.as_f32(), p1.y.as_f32());
        let (cx, cy) = (p2.x.as_f32(), p2.y.as_f32());
        let (dx, dy) = (p3.x.as_f32(), p3.y.as_f32());
        let c1 = point(px(bx + k * (cx - ax)), px(by + k * (cy - ay)));
        let c2 = point(px(cx + k * (bx - dx)), px(cy + k * (by - dy)));
        builder.cubic_bezier_to(p2, c1, c2);
    }
    if open {
        builder.line_to(points[n - 1]);
    }
}

/// d3's `catmull-rom`: a cardinal spline whose tangents come from the
/// neighbouring points' distances.
fn catmull_rom(builder: &mut PathBuilder, points: &[Point<Pixels>], curve: Curve) {
    let n = points.len();
    let open = curve.is_open();
    if open {
        builder.line_to(points[0]);
    }
    // Centripetal parameterization, d3's `alpha = 0.5`.
    const ALPHA: f32 = 0.5;
    let dist = |a: Point<Pixels>, b: Point<Pixels>| {
        let dx = b.x.as_f32() - a.x.as_f32();
        let dy = b.y.as_f32() - a.y.as_f32();
        (dx * dx + dy * dy).sqrt().powf(ALPHA)
    };
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1]
        };
        let (ax, ay) = (p0.x.as_f32(), p0.y.as_f32());
        let (bx, by) = (p1.x.as_f32(), p1.y.as_f32());
        let (cx, cy) = (p2.x.as_f32(), p2.y.as_f32());
        let (dx, dy) = (p3.x.as_f32(), p3.y.as_f32());
        let l01 = dist(p0, p1);
        let l12 = dist(p1, p2);
        let l23 = dist(p2, p3);
        let (mut c1x, mut c1y) = (bx, by);
        let (mut c2x, mut c2y) = (cx, cy);
        if l01 > f32::EPSILON {
            let a = 2. * l01 * l01 + 3. * l01 * l12 + l12 * l12;
            let den = 3. * l01 * (l01 + l12);
            c1x = (bx * a - ax * l12 * l12 + cx * l01 * l01) / den;
            c1y = (by * a - ay * l12 * l12 + cy * l01 * l01) / den;
        }
        if l23 > f32::EPSILON {
            let b = 2. * l23 * l23 + 3. * l23 * l12 + l12 * l12;
            let den = 3. * l23 * (l23 + l12);
            c2x = (cx * b + bx * l23 * l23 - dx * l12 * l12) / den;
            c2y = (cy * b + by * l23 * l23 - dy * l12 * l12) / den;
        }
        builder.cubic_bezier_to(p2, point(px(c1x), px(c1y)), point(px(c2x), px(c2y)));
    }
    if open {
        builder.line_to(points[n - 1]);
    }
}

/// d3's `monotone-x`/`monotone-y`: a Hermite spline whose tangents keep the
/// data's direction, so a curve never overshoots between points.
fn monotone(builder: &mut PathBuilder, points: &[Point<Pixels>], along_x: bool) {
    let n = points.len();
    let sign = |value: f32| if value < 0. { -1. } else { 1. };
    // `h` is the distance along the monotone axis, `v` the value across it.
    let deltas = |a: Point<Pixels>, b: Point<Pixels>| {
        if along_x {
            (b.x.as_f32() - a.x.as_f32(), b.y.as_f32() - a.y.as_f32())
        } else {
            (b.y.as_f32() - a.y.as_f32(), b.x.as_f32() - a.x.as_f32())
        }
    };
    let mut tangent = f32::NAN;
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1]
        };
        let (h_a, v_a) = deltas(p0, p1);
        let (h_b, v_b) = deltas(p1, p2);
        let (h_c, v_c) = deltas(p2, p3);
        let _ = (h_c, v_c);
        let s0 = if h_a != 0. { v_a / h_a } else { 0. };
        let s1 = if h_b != 0. { v_b / h_b } else { 0. };
        let p: f32 = if h_a + h_b != 0. {
            (s0 * h_b + s1 * h_a) / (h_a + h_b)
        } else {
            0.
        };
        let t1 = if s0 == 0. && s1 == 0. {
            0.
        } else {
            (sign(s0) + sign(s1)) * s0.abs().min(s1.abs()).min(0.5 * p.abs())
        };
        let t0 = if i == 0 {
            // The first segment gets a one-sided tangent.
            if h_a != 0. {
                (3. * v_a / h_a - t1) / 2.
            } else {
                t1
            }
        } else {
            tangent
        };
        let (dx, dy) = (p2.x.as_f32() - p1.x.as_f32(), p2.y.as_f32() - p1.y.as_f32());
        let (c1, c2) = if along_x {
            (
                point(
                    px(p1.x.as_f32() + dx / 3.),
                    px(p1.y.as_f32() + dx / 3. * t0),
                ),
                point(
                    px(p2.x.as_f32() - dx / 3.),
                    px(p2.y.as_f32() - dx / 3. * t1),
                ),
            )
        } else {
            (
                point(
                    px(p1.x.as_f32() + dy / 3. * t0),
                    px(p1.y.as_f32() + dy / 3.),
                ),
                point(
                    px(p2.x.as_f32() - dy / 3. * t1),
                    px(p2.y.as_f32() - dy / 3.),
                ),
            )
        };
        builder.cubic_bezier_to(p2, c1, c2);
        tangent = t1;
    }
}

/// d3's `bump-x`/`bump-y`: a Bézier whose control points share a coordinate,
/// so the curve leaves and enters each point horizontally or vertically.
fn bump(builder: &mut PathBuilder, points: &[Point<Pixels>], along_x: bool) {
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (c1, c2) = if along_x {
            let mid = (a.x.as_f32() + b.x.as_f32()) / 2.;
            (point(px(mid), a.y), point(px(mid), b.y))
        } else {
            let mid = (a.y.as_f32() + b.y.as_f32()) / 2.;
            (point(a.x, px(mid)), point(b.x, px(mid)))
        };
        builder.cubic_bezier_to(b, c1, c2);
    }
}

/// d3's `bundle`: a basis spline over points pulled toward the straight line
/// between the first and last, so the curve bends less than `basis`.
fn bundle(builder: &mut PathBuilder, points: &[Point<Pixels>]) {
    const BETA: f32 = 0.85;
    let n = points.len();
    if n < 2 {
        return;
    }
    let (x0, y0) = (points[0].x.as_f32(), points[0].y.as_f32());
    let (x1, y1) = (points[n - 1].x.as_f32(), points[n - 1].y.as_f32());
    // Each point moves `BETA` of the way from its own position toward the
    // line's, then the basis spline runs through the moved points.
    let pulled: Vec<Point<Pixels>> = points
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let t = index as f32 / (n - 1) as f32;
            point(
                px(BETA * p.x.as_f32() + (1. - BETA) * (x0 + t * (x1 - x0))),
                px(BETA * p.y.as_f32() + (1. - BETA) * (y0 + t * (y1 - y0))),
            )
        })
        .collect();
    basis(builder, &pulled, Curve::Basis);
}
