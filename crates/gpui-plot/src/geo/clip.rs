//! d3-geo's spherical pre-clip, ported from `d3-geo/src/clip`: the
//! antimeridian cut (`clipAntimeridian`) and the small-circle clip
//! (`clipCircle`), with the polygon rejoin that closes a clipped ring along
//! the clip edge and the containment test that fills the whole sphere when
//! the clip edge lies inside a polygon. Coordinates are rotated radians.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

const EPSILON: f64 = 1e-6;
const EPSILON2: f64 = 1e-12;

/// A d3 stream's line and point events. `m` is d3's third point argument,
/// the clip's mark on a segment's ends (used by the rejoin).
pub(crate) trait Sink {
    fn point(&mut self, x: f64, y: f64, m: u8);
    fn line_start(&mut self);
    fn line_end(&mut self);
}

/// A projection's pre-clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Clip {
    /// d3's `clipAntimeridian`.
    Antimeridian,
    /// d3's `clipCircle(radius)`, radians.
    Circle(f64),
}

impl Clip {
    /// Whether a point is drawn, d3's `pointVisible`.
    pub fn visible(self, l: f64, f: f64) -> bool {
        match self {
            Clip::Antimeridian => true,
            Clip::Circle(r) => l.cos() * f.cos() > r.cos(),
        }
    }

    /// The point d3 tests for containment to decide whether a polygon with
    /// no visible edge covers the whole clip region.
    fn start(self) -> [f64; 2] {
        match self {
            Clip::Antimeridian => [-PI, -FRAC_PI_2],
            Clip::Circle(r) if r.cos() > 0. => [0., -r],
            Clip::Circle(r) => [-PI, r - PI],
        }
    }

    /// d3's `interpolate`: walk the clip edge from `from` to `to`, or the
    /// whole edge when both are `None`.
    fn interpolate(
        self,
        from: Option<[f64; 2]>,
        to: Option<[f64; 2]>,
        direction: f64,
        sink: &mut dyn Sink,
    ) {
        match self {
            Clip::Antimeridian => match (from, to) {
                (Some(from), Some(to)) => {
                    if (from[0] - to[0]).abs() > EPSILON {
                        let l = if from[0] < to[0] { PI } else { -PI };
                        let f = direction * l / 2.;
                        sink.point(-l, f, 0);
                        sink.point(0., f, 0);
                        sink.point(l, f, 0);
                    } else {
                        sink.point(to[0], to[1], 0);
                    }
                }
                _ => {
                    let f = direction * FRAC_PI_2;
                    for [x, y] in [
                        [-PI, f],
                        [0., f],
                        [PI, f],
                        [PI, 0.],
                        [PI, -f],
                        [0., -f],
                        [-PI, -f],
                        [-PI, 0.],
                        [-PI, f],
                    ] {
                        sink.point(x, y, 0);
                    }
                }
            },
            Clip::Circle(radius) => {
                circle_stream(sink, radius, 2f64.to_radians(), direction, from, to)
            }
        }
    }

    fn line_clipper(self) -> LineClip {
        match self {
            Clip::Antimeridian => LineClip::Antimeridian {
                l0: f64::NAN,
                f0: f64::NAN,
                sign0: f64::NAN,
                clean: 1,
            },
            Clip::Circle(radius) => {
                let cr = radius.cos();
                LineClip::Circle {
                    radius,
                    cr,
                    small: cr > 0.,
                    not_hemisphere: cr.abs() > EPSILON,
                    point0: None,
                    c0: 0,
                    v0: false,
                    v00: false,
                    clean: 1,
                }
            }
        }
    }

    /// A line through the clip, d3's `lineStart`, `point`… `lineEnd`.
    pub fn line(self, points: &[[f64; 2]], sink: &mut dyn Sink) {
        let mut clip = self.line_clipper();
        clip.line_start(sink);
        for p in points {
            clip.point(p[0], p[1], sink);
        }
        clip.line_end(sink);
    }

    /// The whole sphere through the clip, d3's `sphere`: one ring.
    pub fn sphere(self, sink: &mut dyn Sink) {
        sink.line_start();
        self.interpolate(None, None, 1., sink);
        sink.line_end();
    }

    /// A polygon through the clip, d3's `polygonStart` … `polygonEnd`: each
    /// ring cut into visible segments, rejoined along the clip edge, every
    /// output line a closed ring. Rings are open (no repeated first point).
    pub fn polygon(self, rings: &[Vec<[f64; 2]>], sink: &mut dyn Sink) {
        let mut segments: Vec<Vec<[f64; 3]>> = Vec::new();
        let mut polygon: Vec<&[[f64; 2]]> = Vec::new();
        for ring in rings {
            if ring.is_empty() {
                continue;
            }
            let mut buffer = Buffer::default();
            let mut clip = self.line_clipper();
            clip.line_start(&mut buffer);
            for p in ring.iter().chain(std::iter::once(&ring[0])) {
                clip.point(p[0], p[1], &mut buffer);
            }
            clip.line_end(&mut buffer);
            let clean = clip.clean();
            let mut ring_segments = buffer.lines;
            polygon.push(ring);
            if ring_segments.is_empty() {
                continue;
            }
            // No intersections.
            if clean & 1 != 0 {
                let segment = &ring_segments[0];
                if segment.len() > 1 {
                    sink.line_start();
                    for p in &segment[..segment.len() - 1] {
                        sink.point(p[0], p[1], 0);
                    }
                    sink.line_end();
                }
                continue;
            }
            // Rejoin connected segments.
            if ring_segments.len() > 1 && clean & 2 != 0 {
                let last = ring_segments.pop().unwrap_or_default();
                let first = ring_segments.remove(0);
                ring_segments.push(last.into_iter().chain(first).collect());
            }
            segments.extend(ring_segments.into_iter().filter(|s| s.len() > 1));
        }
        let start_inside = polygon_contains(&polygon, self.start());
        if !segments.is_empty() {
            self.rejoin(segments, start_inside, sink);
        } else if start_inside {
            self.sphere(sink);
        }
    }

    /// d3's `clipRejoin`: join a polygon's visible segments into rings by
    /// interpolating along the clip edge between their ends.
    fn rejoin(self, mut segments: Vec<Vec<[f64; 3]>>, mut start_inside: bool, sink: &mut dyn Sink) {
        struct Intersection {
            x: [f64; 3],
            z: Option<usize>,
            o: usize,
            e: bool,
            v: bool,
            n: usize,
            p: usize,
        }
        let mut nodes: Vec<Intersection> = Vec::new();
        let (mut subject, mut clip): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        let node = |nodes: &mut Vec<Intersection>, x, z, e| {
            nodes.push(Intersection {
                x,
                z,
                o: 0,
                e,
                v: false,
                n: 0,
                p: 0,
            });
            nodes.len() - 1
        };
        for (s, segment) in segments.iter_mut().enumerate() {
            let n = segment.len() - 1;
            if n == 0 {
                continue;
            }
            let (p0, p1) = (segment[0], segment[n]);
            if point_equal(p0, p1) {
                if p0[2] == 0. && p1[2] == 0. {
                    sink.line_start();
                    for p in &segment[..n] {
                        sink.point(p[0], p[1], 0);
                    }
                    sink.line_end();
                    continue;
                }
                // Handle degenerate cases by moving the point.
                segment[n][0] += 2. * EPSILON;
            }
            let p1 = segment[n];
            let a = node(&mut nodes, p0, Some(s), true);
            let b = node(&mut nodes, p0, None, false);
            nodes[a].o = b;
            nodes[b].o = a;
            subject.push(a);
            clip.push(b);
            let a = node(&mut nodes, p1, Some(s), false);
            let b = node(&mut nodes, p1, None, true);
            nodes[a].o = b;
            nodes[b].o = a;
            subject.push(a);
            clip.push(b);
        }
        if subject.is_empty() {
            return;
        }
        let key = |x: [f64; 3]| {
            if x[0] < 0. {
                x[1] - FRAC_PI_2 - EPSILON
            } else {
                FRAC_PI_2 - x[1]
            }
        };
        clip.sort_by(|&a, &b| {
            (key(nodes[a].x) - key(nodes[b].x))
                .partial_cmp(&0.)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for list in [&subject, &clip] {
            for (i, &a) in list.iter().enumerate() {
                let b = list[(i + 1) % list.len()];
                nodes[a].n = b;
                nodes[b].p = a;
            }
        }
        for &c in &clip {
            start_inside = !start_inside;
            nodes[c].e = start_inside;
        }
        let start = subject[0];
        let xy = |x: [f64; 3]| Some([x[0], x[1]]);
        loop {
            // Find the first unvisited intersection.
            let mut current = start;
            let mut is_subject = true;
            while nodes[current].v {
                current = nodes[current].n;
                if current == start {
                    return;
                }
            }
            let mut points = nodes[current].z;
            sink.line_start();
            loop {
                let o = nodes[current].o;
                nodes[current].v = true;
                nodes[o].v = true;
                if nodes[current].e {
                    if is_subject {
                        if let Some(s) = points {
                            for p in &segments[s] {
                                sink.point(p[0], p[1], 0);
                            }
                        }
                    } else {
                        let next = nodes[current].n;
                        self.interpolate(xy(nodes[current].x), xy(nodes[next].x), 1., sink);
                    }
                    current = nodes[current].n;
                } else {
                    let prev = nodes[current].p;
                    if is_subject {
                        if let Some(s) = nodes[prev].z {
                            for p in segments[s].iter().rev() {
                                sink.point(p[0], p[1], 0);
                            }
                        }
                    } else {
                        self.interpolate(xy(nodes[current].x), xy(nodes[prev].x), -1., sink);
                    }
                    current = prev;
                }
                current = nodes[current].o;
                points = nodes[current].z;
                is_subject = !is_subject;
                if nodes[current].v {
                    break;
                }
            }
            sink.line_end();
        }
    }
}

/// d3's `clipBuffer`: the segments a ring's line clip emits.
#[derive(Default)]
struct Buffer {
    lines: Vec<Vec<[f64; 3]>>,
}

impl Sink for Buffer {
    fn point(&mut self, x: f64, y: f64, m: u8) {
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        if let Some(line) = self.lines.last_mut() {
            line.push([x, y, f64::from(m)]);
        }
    }
    fn line_start(&mut self) {
        self.lines.push(Vec::new());
    }
    fn line_end(&mut self) {}
}

/// d3's `clipAntimeridianLine` and `clipCircle`'s `clipLine`: cut a line
/// into its visible segments.
enum LineClip {
    Antimeridian {
        l0: f64,
        f0: f64,
        sign0: f64,
        clean: u8,
    },
    Circle {
        radius: f64,
        cr: f64,
        small: bool,
        not_hemisphere: bool,
        point0: Option<[f64; 2]>,
        c0: u8,
        v0: bool,
        v00: bool,
        clean: u8,
    },
}

impl LineClip {
    fn line_start(&mut self, sink: &mut dyn Sink) {
        match self {
            LineClip::Antimeridian { clean, .. } => {
                sink.line_start();
                *clean = 1;
            }
            LineClip::Circle { v0, v00, clean, .. } => {
                *v00 = false;
                *v0 = false;
                *clean = 1;
            }
        }
    }

    fn line_end(&mut self, sink: &mut dyn Sink) {
        match self {
            LineClip::Antimeridian { l0, f0, .. } => {
                sink.line_end();
                *l0 = f64::NAN;
                *f0 = f64::NAN;
            }
            LineClip::Circle { v0, point0, .. } => {
                if *v0 {
                    sink.line_end();
                }
                *point0 = None;
            }
        }
    }

    /// 0: there were intersections or the line was empty; 1: none; 2: there
    /// were, and the first and last segments should be rejoined.
    fn clean(&self) -> u8 {
        match self {
            LineClip::Antimeridian { clean, .. } => 2 - clean,
            LineClip::Circle { clean, v0, v00, .. } => *clean | (u8::from(*v00 && *v0) << 1),
        }
    }

    fn point(&mut self, l1: f64, f1: f64, sink: &mut dyn Sink) {
        match self {
            LineClip::Antimeridian {
                l0,
                f0,
                sign0,
                clean,
            } => {
                let mut l1 = l1;
                let sign1 = if l1 > 0. { PI } else { -PI };
                let delta = (l1 - *l0).abs();
                if (delta - PI).abs() < EPSILON {
                    // The line crosses a pole.
                    *f0 = if (*f0 + f1) / 2. > 0. {
                        FRAC_PI_2
                    } else {
                        -FRAC_PI_2
                    };
                    sink.point(*l0, *f0, 0);
                    sink.point(*sign0, *f0, 0);
                    sink.line_end();
                    sink.line_start();
                    sink.point(sign1, *f0, 0);
                    sink.point(l1, *f0, 0);
                    *clean = 0;
                } else if *sign0 != sign1 && delta >= PI {
                    // The line crosses the antimeridian.
                    if (*l0 - *sign0).abs() < EPSILON {
                        *l0 -= *sign0 * EPSILON;
                    }
                    if (l1 - sign1).abs() < EPSILON {
                        l1 -= sign1 * EPSILON;
                    }
                    *f0 = antimeridian_intersect(*l0, *f0, l1, f1);
                    sink.point(*sign0, *f0, 0);
                    sink.line_end();
                    sink.line_start();
                    sink.point(sign1, *f0, 0);
                    *clean = 0;
                }
                *l0 = l1;
                *f0 = f1;
                sink.point(l1, f1, 0);
                *sign0 = sign1;
            }
            LineClip::Circle {
                radius,
                cr,
                small,
                not_hemisphere,
                point0,
                c0,
                v0,
                v00,
                clean,
            } => {
                let (radius, cr, small) = (*radius, *cr, *small);
                let point1 = [l1, f1];
                let v = l1.cos() * f1.cos() > cr;
                let code = |l: f64, f: f64| {
                    let r = if small { radius } else { PI - radius };
                    let mut code = 0;
                    if l < -r {
                        code |= 1;
                    } else if l > r {
                        code |= 2;
                    }
                    if f < -r {
                        code |= 4;
                    } else if f > r {
                        code |= 8;
                    }
                    code
                };
                let c = if small {
                    if v { 0 } else { code(l1, f1) }
                } else if v {
                    code(l1 + if l1 < 0. { PI } else { -PI }, f1)
                } else {
                    0
                };
                if point0.is_none() {
                    *v00 = v;
                    *v0 = v;
                    if v {
                        sink.line_start();
                    }
                }
                let mut previous = *point0;
                if v != *v0 {
                    *clean = 0;
                    let p0 = previous.unwrap_or(point1);
                    if v {
                        // Outside going in.
                        sink.line_start();
                        let p2 = circle_intersect(cr, point1, p0, false).map_or(point1, |(q, _)| q);
                        sink.point(p2[0], p2[1], 0);
                        previous = Some(p2);
                    } else {
                        // Inside going out.
                        let p2 = circle_intersect(cr, p0, point1, false).map_or(p0, |(q, _)| q);
                        sink.point(p2[0], p2[1], 2);
                        sink.line_end();
                        previous = Some(p2);
                    }
                } else if *not_hemisphere
                    && let Some(p0) = previous
                    && (small ^ v)
                {
                    // Both ends on one side, but the segment may still cross
                    // the small circle twice.
                    if c & *c0 == 0
                        && let Some((t0, Some(t1))) = circle_intersect(cr, point1, p0, true)
                    {
                        *clean = 0;
                        if small {
                            sink.line_start();
                            sink.point(t0[0], t0[1], 0);
                            sink.point(t1[0], t1[1], 0);
                            sink.line_end();
                        } else {
                            sink.point(t1[0], t1[1], 0);
                            sink.line_end();
                            sink.line_start();
                            sink.point(t0[0], t0[1], 3);
                        }
                    }
                }
                if v && previous.is_none_or(|p0| !point_equal2(p0, point1)) {
                    sink.point(point1[0], point1[1], 0);
                }
                *point0 = Some(point1);
                *v0 = v;
                *c0 = c;
            }
        }
    }
}

fn point_equal(a: [f64; 3], b: [f64; 3]) -> bool {
    (a[0] - b[0]).abs() < EPSILON && (a[1] - b[1]).abs() < EPSILON
}

fn point_equal2(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < EPSILON && (a[1] - b[1]).abs() < EPSILON
}

fn antimeridian_intersect(l0: f64, f0: f64, l1: f64, f1: f64) -> f64 {
    let s = (l0 - l1).sin();
    if s.abs() > EPSILON {
        let (c0, c1) = (f0.cos(), f1.cos());
        ((f0.sin() * c1 * l1.sin() - f1.sin() * c0 * l0.sin()) / (c0 * c1 * s)).atan()
    } else {
        (f0 + f1) / 2.
    }
}

pub(crate) fn cartesian([l, f]: [f64; 2]) -> [f64; 3] {
    let c = f.cos();
    [c * l.cos(), c * l.sin(), f.sin()]
}

pub(crate) fn spherical(p: [f64; 3]) -> [f64; 2] {
    [p[1].atan2(p[0]), p[2].clamp(-1., 1.).asin()]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(d: [f64; 3]) -> [f64; 3] {
    let l = dot(d, d).sqrt();
    [d[0] / l, d[1] / l, d[2] / l]
}

/// `clipCircle`'s `intersect`: where the great circle from `a` to `b` meets
/// the clip circle; with `two`, both crossings when they lie between `a`
/// and `b`.
fn circle_intersect(
    cr: f64,
    a: [f64; 2],
    b: [f64; 2],
    two: bool,
) -> Option<([f64; 2], Option<[f64; 2]>)> {
    let (pa, pb) = (cartesian(a), cartesian(b));
    // Two planes, n1.p = d1 and n2.p = d2; their intersection line is
    // p(t) = c1 n1 + c2 n2 + t (n1 ⨯ n2).
    let n1 = [1., 0., 0.];
    let n2 = cross(pa, pb);
    let n2n2 = dot(n2, n2);
    let n1n2 = n2[0];
    let determinant = n2n2 - n1n2 * n1n2;
    // Two polar points.
    if determinant == 0. {
        return if two { None } else { Some((a, None)) };
    }
    let c1 = cr * n2n2 / determinant;
    let c2 = -cr * n1n2 / determinant;
    let u = cross(n1, n2);
    let a3 = [n1[0] * c1 + n2[0] * c2, n2[1] * c2, n2[2] * c2];
    // Solve |p(t)|^2 = 1.
    let w = dot(a3, u);
    let uu = dot(u, u);
    let t2 = w * w - uu * (dot(a3, a3) - 1.);
    if t2 < 0. {
        return None;
    }
    let t = t2.sqrt();
    let at = |k: f64| spherical([a3[0] + u[0] * k, a3[1] + u[1] * k, a3[2] + u[2] * k]);
    let q = at((-w - t) / uu);
    if !two {
        return Some((q, None));
    }
    // Two intersection points.
    let (mut l0, mut l1, mut f0, mut f1) = (a[0], b[0], a[1], b[1]);
    if l1 < l0 {
        std::mem::swap(&mut l0, &mut l1);
    }
    let delta = l1 - l0;
    let polar = (delta - PI).abs() < EPSILON;
    let meridian = polar || delta < EPSILON;
    if !polar && f1 < f0 {
        std::mem::swap(&mut f0, &mut f1);
    }
    let between = if meridian {
        if polar {
            (f0 + f1 > 0.) ^ (q[1] < if (q[0] - l0).abs() < EPSILON { f0 } else { f1 })
        } else {
            f0 <= q[1] && q[1] <= f1
        }
    } else {
        (delta > PI) ^ (l0 <= q[0] && q[0] <= l1)
    };
    between.then(|| (q, Some(at((-w + t) / uu))))
}

/// d3's `circleStream`: the circle `radius` from `[0, 0]`, every `delta`,
/// from `from` to `to` (or all of it).
fn circle_stream(
    sink: &mut dyn Sink,
    radius: f64,
    delta: f64,
    direction: f64,
    from: Option<[f64; 2]>,
    to: Option<[f64; 2]>,
) {
    let (cr, sr) = (radius.cos(), radius.sin());
    let step = direction * delta;
    let (mut t0, t1) = match (from, to) {
        (Some(from), Some(to)) => {
            let (t0, t1) = (circle_radius(cr, from), circle_radius(cr, to));
            (t0, t1)
        }
        _ => (radius + direction * TAU, radius - step / 2.),
    };
    if from.is_some() && to.is_some() && (if direction > 0. { t0 < t1 } else { t0 > t1 }) {
        t0 += direction * TAU;
    }
    let mut t = t0;
    while if direction > 0. { t > t1 } else { t < t1 } {
        let p = spherical([cr, -sr * t.cos(), -sr * t.sin()]);
        sink.point(p[0], p[1], 0);
        t -= step;
    }
}

/// d3's `circleRadius`: the signed angle of a point about `[cr, 0, 0]`.
fn circle_radius(cr: f64, point: [f64; 2]) -> f64 {
    let mut p = cartesian(point);
    p[0] -= cr;
    let p = normalize(p);
    let radius = (-p[1]).clamp(-1., 1.).acos();
    ((if -p[2] < 0. { -radius } else { radius }) + TAU - EPSILON) % TAU
}

fn longitude(point: [f64; 2]) -> f64 {
    if point[0].abs() <= PI {
        point[0]
    } else {
        point[0].signum() * ((point[0].abs() + PI) % TAU - PI)
    }
}

/// d3's `polygonContains`: whether a spherical polygon contains a point.
pub(crate) fn polygon_contains(polygon: &[&[[f64; 2]]], point: [f64; 2]) -> bool {
    let lambda = longitude(point);
    let mut phi = point[1];
    let sin_phi = phi.sin();
    let normal = [lambda.sin(), -lambda.cos(), 0.];
    let mut angle = 0.;
    let mut winding: i64 = 0;
    // A compensated sum, as d3's Adder.
    let (mut sum, mut compensation) = (0f64, 0f64);
    if sin_phi == 1. {
        phi = FRAC_PI_2 + EPSILON;
    } else if sin_phi == -1. {
        phi = -FRAC_PI_2 - EPSILON;
    }
    for ring in polygon {
        let Some(&last) = ring.last() else {
            continue;
        };
        let mut point0 = last;
        let mut lambda0 = longitude(point0);
        let phi0 = point0[1] / 2. + FRAC_PI_4;
        let (mut sin_phi0, mut cos_phi0) = (phi0.sin(), phi0.cos());
        for &point1 in ring.iter() {
            let lambda1 = longitude(point1);
            let phi1 = point1[1] / 2. + FRAC_PI_4;
            let (sin_phi1, cos_phi1) = (phi1.sin(), phi1.cos());
            let delta = lambda1 - lambda0;
            let sign = if delta >= 0. { 1. } else { -1. };
            let abs_delta = sign * delta;
            let antimeridian = abs_delta > PI;
            let k = sin_phi0 * sin_phi1;
            let term =
                (k * sign * abs_delta.sin()).atan2(cos_phi0 * cos_phi1 + k * abs_delta.cos());
            let y = term - compensation;
            let t = sum + y;
            compensation = (t - sum) - y;
            sum = t;
            angle += if antimeridian {
                delta + sign * TAU
            } else {
                delta
            };
            // Are the longitudes either side of the point's meridian, and
            // are the latitudes smaller than the parallel?
            if antimeridian ^ (lambda0 >= lambda) ^ (lambda1 >= lambda) {
                let arc = normalize(cross(cartesian(point0), cartesian(point1)));
                let intersection = normalize(cross(normal, arc));
                let phi_arc = if antimeridian ^ (delta >= 0.) {
                    -1.
                } else {
                    1.
                } * intersection[2].clamp(-1., 1.).asin();
                if phi > phi_arc || (phi == phi_arc && (arc[0] != 0. || arc[1] != 0.)) {
                    winding += if antimeridian ^ (delta >= 0.) { 1 } else { -1 };
                }
            }
            lambda0 = lambda1;
            sin_phi0 = sin_phi1;
            cos_phi0 = cos_phi1;
            point0 = point1;
        }
    }
    // The south pole is inside when the polygon winds clockwise around it,
    // or doesn't wind but has a negative area; each crossing of the
    // meridian below the point flips that.
    ((angle < -EPSILON) || (angle < EPSILON && sum < -EPSILON2)) ^ (winding & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Lines(Vec<Vec<[f64; 2]>>);

    impl Sink for Lines {
        fn point(&mut self, x: f64, y: f64, _: u8) {
            if let Some(line) = self.0.last_mut() {
                line.push([x, y]);
            }
        }
        fn line_start(&mut self) {
            self.0.push(Vec::new());
        }
        fn line_end(&mut self) {}
    }

    fn deg(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        points
            .iter()
            .map(|p| [p[0].to_radians(), p[1].to_radians()])
            .collect()
    }

    #[test]
    fn a_line_across_the_antimeridian_splits_there() {
        let mut out = Lines::default();
        Clip::Antimeridian.line(&deg(&[[170., 0.], [-170., 0.]]), &mut out);
        assert_eq!(out.0.len(), 2);
        assert!((out.0[0][1][0] - PI).abs() < 1e-9);
        assert!((out.0[1][0][0] + PI).abs() < 1e-9);
    }

    #[test]
    fn a_polygon_over_the_horizon_closes_along_it() {
        // A band from the front of the globe to the back: clipped at the
        // hemisphere, the visible part is closed along the horizon.
        let ring = deg(&[[0., -10.], [150., -10.], [150., 10.], [0., 10.]]);
        let mut out = Lines::default();
        let clip = Clip::Circle(FRAC_PI_2);
        clip.polygon(&[ring], &mut out);
        assert_eq!(out.0.len(), 1);
        assert!(out.0[0].iter().all(|p| p[0].cos() * p[1].cos() > -1e-6));
    }

    #[test]
    fn a_polygon_around_the_clip_fills_the_sphere() {
        // A small ring on the far side, wound so its inside is everything
        // else, covers the whole visible disc.
        let ring = deg(&[[180., 0.], [175., 5.], [170., 0.], [175., -5.]]);
        let ring: Vec<[f64; 2]> = ring.into_iter().rev().collect();
        let mut contains = Lines::default();
        Clip::Circle(FRAC_PI_2).polygon(std::slice::from_ref(&ring), &mut contains);
        let mut small = Lines::default();
        Clip::Circle(FRAC_PI_2).polygon(&[ring.into_iter().rev().collect()], &mut small);
        assert_eq!(contains.0.len() + small.0.len(), 1);
    }
}
