//! Vector mark: `marks/vector.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "vector", 1, e01),
        example("marks", "vector", 2, e02),
        example("marks", "vector", 3, e03),
        Example {
            height: 640.,
            ..example("marks", "vector", 4, e04)
        },
    ]
}

/// Plot's default `length` scale: linear from zero to the largest value onto
/// `[0, max]` pixels (12 by default), optionally through a `sqrt`.
fn length_scale(
    data: &Dataset,
    value: impl Fn(&Table) -> f64 + Clone + 'static,
    max: f64,
    sqrt: bool,
) -> impl Fn(&Table) -> f32 + 'static {
    let warp = move |v: f64| if sqrt { v.abs().sqrt() } else { v.abs() };
    let top = data
        .rows()
        .iter()
        .map(|d| warp(value(d)))
        .filter(|v| v.is_finite())
        .fold(0., f64::max);
    move |d: &Table| {
        let v = warp(value(d));
        if top > 0. && v.is_finite() {
            (v / top * max) as f32
        } else {
            0.
        }
    }
}

fn e01() -> Plot {
    let wind = data::wind();
    let speed = |d: &Table| d.num("u").hypot(d.num("v"));
    Plot::plot()
        .inset(10.)
        .aspect_ratio(1.)
        .color(|c| c.label("Speed (m/s)").zero(true).legend(true))
        .mark(
            vector(wind.clone())
                .x("longitude")
                .y("latitude")
                .rotate(|d: &Table| (d.num("u").atan2(d.num("v")).to_degrees()) as f32)
                .length(length_scale(&wind, speed, 12., false))
                .stroke(ColorChannel::number(speed)),
        )
}

/// Gap: the plot's `length` scale options are not wired to the vector's
/// length channel, so the `{type: "sqrt", transform: Math.abs}` scale is
/// applied here.
fn e02() -> Plot {
    let counties = data::counties_election();
    let length = |d: &Table| d.num("margin2020") * d.num("votes");
    Plot::plot()
        .projection("albers-usa")
        .mark(geo(data::statemesh()).stroke_width(0.5))
        .mark(geo(data::nation()))
        .mark(
            vector(counties.clone())
                .centroid()
                .anchor(VectorAnchor::Start)
                .length(length_scale(&counties, length, 12., true))
                .stroke(|d: &Table| {
                    if d.num("margin2020") > 0. {
                        gpui_plot::css::parse("red").unwrap()
                    } else {
                        gpui_plot::css::parse("blue").unwrap()
                    }
                })
                .rotate(|d: &Table| if d.num("margin2020") > 0. { 60. } else { -60. }),
        )
}

/// Gap: as above, `length: {range: [0, 200]}` is applied here.
fn e03() -> Plot {
    let counties = data::counties();
    Plot::plot()
        .width(688.)
        .projection("albers-usa")
        .mark(geo(data::statemesh()).stroke_opacity(0.5))
        .mark(geo(data::nation()))
        .mark(
            spike(counties.clone())
                .geo_centroid()
                .length(length_scale(
                    &counties,
                    |d| d.num("population"),
                    200.,
                    false,
                ))
                .stroke("green"),
        )
}

/// The docs' Perlin noise field over a Poisson-disk sample.
fn e04() -> Plot {
    let noise = |x: f64, y: f64| perlin::octave2(x, y, 2);
    let points = Dataset::tuples(poisson([0., 0., 2., 2.], 4000.));
    Plot::plot()
        .inset(6.)
        .width(1024.)
        .aspect_ratio(1.)
        .no_axes()
        .mark(
            vector(points.clone())
                .x("0")
                .y("1")
                .length(length_scale(
                    &points,
                    move |d| (noise(d.num("0") + 2., d.num("1")) + 0.5) * 24.,
                    12.,
                    false,
                ))
                .rotate(move |d: &Table| (noise(d.num("0"), d.num("1")) * 360.) as f32),
        )
}

/// The docs' `poisson([x0, y0, x1, y1], n)`: Bridson's Poisson-disk sampling,
/// seeded with `d3.randomLcg(42)`.
fn poisson([x0, y0, x1, y1]: [f64; 4], n: f64) -> Vec<[f64; 2]> {
    const K: usize = 30;
    let mut random = lcg(42);
    let width = x1 - x0;
    let height = y1 - y0;
    let radius2 = width * height / (n * 1.5);
    let radius = radius2.sqrt();
    let radius2_3 = 3. * radius2;
    let cell_size = radius * std::f64::consts::FRAC_1_SQRT_2;
    let grid_width = (width / cell_size).ceil() as usize;
    let grid_height = (height / cell_size).ceil() as usize;
    let mut grid: Vec<Option<[f64; 2]>> = vec![None; grid_width * grid_height];
    let mut queue: Vec<[f64; 2]> = Vec::new();
    let mut out = Vec::new();

    let mut sample =
        |x: f64, y: f64, grid: &mut Vec<Option<[f64; 2]>>, queue: &mut Vec<[f64; 2]>| {
            let s = [x, y];
            grid[grid_width * ((y / cell_size) as usize) + (x / cell_size) as usize] = Some(s);
            queue.push(s);
            out.push([x + x0, y + y0]);
        };
    let far = |x: f64, y: f64, grid: &Vec<Option<[f64; 2]>>| {
        let i = (x / cell_size) as usize;
        let j = (y / cell_size) as usize;
        let i0 = i.saturating_sub(2);
        let j0 = j.saturating_sub(2);
        let i1 = (i + 3).min(grid_width);
        let j1 = (j + 3).min(grid_height);
        for j in j0..j1 {
            for i in i0..i1 {
                if let Some(s) = grid[j * grid_width + i] {
                    let (dx, dy) = (s[0] - x, s[1] - y);
                    if dx * dx + dy * dy < radius2 {
                        return false;
                    }
                }
            }
        }
        true
    };

    let (a, b) = (random(), random());
    sample(
        width / 2. + a * radius,
        height / 2. + b * radius,
        &mut grid,
        &mut queue,
    );
    'pick: while !queue.is_empty() {
        let i = (random() * queue.len() as f64) as usize;
        let parent = queue[i];
        for _ in 0..K {
            let a = 2. * std::f64::consts::PI * random();
            let r = (random() * radius2_3 + radius2).sqrt();
            let x = parent[0] + r * a.cos();
            let y = parent[1] + r * a.sin();
            if 0. <= x && x < width && 0. <= y && y < height && far(x, y, &grid) {
                sample(x, y, &mut grid, &mut queue);
                continue 'pick;
            }
        }
        let last = queue.pop().unwrap();
        if i < queue.len() {
            queue[i] = last;
        }
    }
    out
}

/// `d3.randomLcg(seed)`.
fn lcg(seed: u32) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_mul(0x19660D).wrapping_add(0x3C6EF35F);
        state as f64 / 4294967296.
    }
}

/// The docs' `components/perlin.js`: Ken Perlin's improved noise in two
/// dimensions, and `octave` summing octaves of it.
mod perlin {
    const P: [u8; 256] = [
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30,
        69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94,
        252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171,
        168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60,
        211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1,
        216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86,
        164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118,
        126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170,
        213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39,
        253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34,
        242, 193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49,
        192, 214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254,
        138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    ];

    fn p(i: usize) -> usize {
        P[i & 255] as usize
    }

    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6. - 15.) + 10.)
    }

    fn lerp(t: f64, a: f64, b: f64) -> f64 {
        a + t * (b - a)
    }

    fn grad2(i: usize, x: f64, y: f64) -> f64 {
        let v = if i & 1 != 0 { y } else { x };
        if i & 2 != 0 { -v } else { v }
    }

    /// `perlin2(x, y)`.
    pub fn perlin2(x: f64, y: f64) -> f64 {
        let (xf, yf) = (x.floor(), y.floor());
        let xi = (xf as i64 & 255) as usize;
        let yi = (yf as i64 & 255) as usize;
        let (x, y) = (x - xf, y - yf);
        let (u, v) = (fade(x), fade(y));
        let a = p(xi) + yi;
        let b = p(xi + 1) + yi;
        lerp(
            v,
            lerp(u, grad2(p(a), x, y), grad2(p(b), x - 1., y)),
            lerp(
                u,
                grad2(p(a + 1), x, y - 1.),
                grad2(p(b + 1), x - 1., y - 1.),
            ),
        )
    }

    /// `octave(perlin2, octaves)(x, y)`.
    pub fn octave2(x: f64, y: f64, octaves: usize) -> f64 {
        let (mut total, mut frequency, mut amplitude, mut value) = (0., 1., 1., 0.);
        for _ in 0..octaves {
            value += perlin2(x * frequency, y * frequency) * amplitude;
            total += amplitude;
            amplitude *= 0.5;
            frequency *= 2.;
        }
        value / total
    }
}
