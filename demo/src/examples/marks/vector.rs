//! Vector mark: `marks/vector.md`.
//!
//! Plot draws the two maps through its `albers-usa` projection; the library
//! has no such projection or TopoJSON reader yet, so [`us`] decodes the
//! atlas and projects it here, and the maps draw on linear x/y scales.

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

/// Plot draws the margin's length through `length: {type: "sqrt",
/// transform: Math.abs}`; the library has no length scale, so the sqrt scale
/// is applied here.
fn e02() -> Plot {
    let counties = us::counties();
    let length = |d: &Table| d.num("margin2020") * d.num("votes");
    us::map(Plot::plot())
        .mark(us::statemesh().stroke_width(0.5))
        .mark(us::nation())
        .mark(
            vector(counties.clone())
                .x("cx")
                .y("cy")
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

fn e03() -> Plot {
    let counties = us::counties();
    us::map(Plot::plot())
        .mark(us::statemesh().stroke_opacity(0.5))
        .mark(us::nation())
        .mark(
            spike(counties.clone())
                .x("cx")
                .y("cy")
                .length(length_scale(&counties, |d| d.num("population"), 200., false))
                .stroke("green"),
        )
}

/// The docs' Perlin noise field over a Poisson-disk sample. Plot draws it 1024
/// pixels wide; the port draws at the default width.
fn e04() -> Plot {
    let noise = |x: f64, y: f64| perlin::octave2(x, y, 2);
    let points = Dataset::tuples(poisson([0., 0., 2., 2.], 4000.));
    Plot::plot()
        .inset(6.)
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

    let mut sample = |x: f64, y: f64, grid: &mut Vec<Option<[f64; 2]>>, queue: &mut Vec<[f64; 2]>| {
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
    sample(width / 2. + a * radius, height / 2. + b * radius, &mut grid, &mut queue);
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
        164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126,
        255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
        119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253,
        19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242,
        193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192,
        214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138,
        236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
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
            lerp(u, grad2(p(a + 1), x, y - 1.), grad2(p(b + 1), x - 1., y - 1.)),
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

/// The US atlas, `us-counties-10m.json`, decoded from TopoJSON and projected
/// through d3's `geoAlbersUsa` here, since the library has neither.
///
/// Projected points come out in d3's pixel space (y down); the maps draw them
/// on linear x/y scales with y reversed and a 1:1 aspect ratio, which is what
/// Plot's projection fitting amounts to.
pub(crate) mod us {
    use crate::prelude::*;
    use std::collections::HashMap;

    struct Atlas {
        /// Every arc, as decoded lon/lat points.
        arcs: Vec<Vec<(f64, f64)>>,
        json: serde_json::Value,
    }

    fn atlas() -> &'static Atlas {
        static ATLAS: std::sync::OnceLock<Atlas> = std::sync::OnceLock::new();
        ATLAS.get_or_init(|| {
            let json = data::json("us-counties-10m.json");
            let scale = &json["transform"]["scale"];
            let translate = &json["transform"]["translate"];
            let (kx, ky) = (scale[0].as_f64().unwrap(), scale[1].as_f64().unwrap());
            let (dx, dy) = (
                translate[0].as_f64().unwrap(),
                translate[1].as_f64().unwrap(),
            );
            let arcs = json["arcs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arc| {
                    let (mut x, mut y) = (0., 0.);
                    arc.as_array()
                        .unwrap()
                        .iter()
                        .map(|p| {
                            x += p[0].as_f64().unwrap();
                            y += p[1].as_f64().unwrap();
                            (x * kx + dx, y * ky + dy)
                        })
                        .collect()
                })
                .collect();
            Atlas { arcs, json }
        })
    }

    /// d3's `geoConicEqualArea` with the given rotation, center, parallels
    /// and scale, translated to `t`.
    fn conic(
        lon: f64,
        lat: f64,
        rotate: f64,
        center: (f64, f64),
        parallels: (f64, f64),
        k: f64,
        t: (f64, f64),
    ) -> (f64, f64) {
        let (y0, y1) = (parallels.0.to_radians(), parallels.1.to_radians());
        let sy0 = y0.sin();
        let n = (sy0 + y1.sin()) / 2.;
        let c = 1. + sy0 * (2. * n - sy0);
        let r0 = c.sqrt() / n;
        let raw = |x: f64, y: f64| {
            let r = (c - 2. * n * y.sin()).max(0.).sqrt() / n;
            (r * (x * n).sin(), r0 - r * (x * n).cos())
        };
        let mut l = lon + rotate;
        while l > 180. {
            l -= 360.;
        }
        while l < -180. {
            l += 360.;
        }
        let (px, py) = raw(l.to_radians(), lat.to_radians());
        let (cx, cy) = raw(center.0.to_radians(), center.1.to_radians());
        (t.0 + k * (px - cx), t.1 - k * (py - cy))
    }

    /// d3's `geoAlbersUsa` at its default scale and translate; `None` outside
    /// the lower 48, Alaska and Hawaii.
    pub fn project(lon: f64, lat: f64) -> Option<(f64, f64)> {
        let (k, x, y) = (1070., 480., 250.);
        if lat > 50. && (lon < -129. || lon > 170.) {
            Some(conic(
                lon,
                lat,
                154.,
                (-2., 58.5),
                (55., 65.),
                0.35 * k,
                (x - 0.307 * k, y + 0.201 * k),
            ))
        } else if lat > 18. && lat < 23. && lon < -150. && lon > -179. {
            Some(conic(
                lon,
                lat,
                157.,
                (-3., 19.9),
                (8., 18.),
                k,
                (x - 0.205 * k, y + 0.212 * k),
            ))
        } else if lat > 23. && lon < -60. && lon > -130. {
            Some(conic(lon, lat, 96., (-0.6, 38.7), (29.5, 45.5), k, (x, y)))
        } else {
            None
        }
    }

    fn arc(index: i64) -> Vec<(f64, f64)> {
        let arcs = &atlas().arcs;
        if index >= 0 {
            arcs[index as usize].clone()
        } else {
            let mut arc = arcs[!index as usize].clone();
            arc.reverse();
            arc
        }
    }

    /// The polygons of a geometry, each a list of rings of lon/lat points.
    fn polygons(geometry: &serde_json::Value) -> Vec<Vec<Vec<(f64, f64)>>> {
        let ring = |r: &serde_json::Value| -> Vec<(f64, f64)> {
            r.as_array()
                .unwrap()
                .iter()
                .flat_map(|i| arc(i.as_i64().unwrap()))
                .collect()
        };
        let polygon = |p: &serde_json::Value| p.as_array().unwrap().iter().map(ring).collect();
        match geometry["type"].as_str() {
            Some("Polygon") => vec![polygon(&geometry["arcs"])],
            Some("MultiPolygon") => geometry["arcs"]
                .as_array()
                .unwrap()
                .iter()
                .map(polygon)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The arc indices a geometry cites, unsigned.
    fn cited_arcs(geometry: &serde_json::Value, out: &mut Vec<usize>) {
        match geometry {
            serde_json::Value::Number(n) => {
                let i = n.as_i64().unwrap();
                out.push(if i < 0 { !i as usize } else { i as usize });
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    cited_arcs(item, out);
                }
            }
            _ => {}
        }
    }

    /// `topojson.mesh(us, us.objects[object], filter)` as rows of a line
    /// series per arc: projected `x`, `y` and the arc's id in `arc`.
    fn mesh(object: &str, interior_only: bool) -> Dataset {
        let geometries = atlas().json["objects"][object]["geometries"]
            .as_array()
            .unwrap();
        let mut owners: HashMap<usize, Vec<usize>> = HashMap::new();
        for (g, geometry) in geometries.iter().enumerate() {
            let mut cited = Vec::new();
            cited_arcs(&geometry["arcs"], &mut cited);
            for a in cited {
                owners.entry(a).or_default().push(g);
            }
        }
        let mut arcs: Vec<usize> = owners
            .iter()
            .filter(|(_, owners)| !interior_only || owners.iter().any(|&g| g != owners[0]))
            .map(|(&a, _)| a)
            .collect();
        arcs.sort();
        let mut rows = Vec::new();
        for a in arcs {
            for &(lon, lat) in &atlas().arcs[a] {
                let point = project(lon, lat);
                let (x, y) = point.unwrap_or((f64::NAN, f64::NAN));
                rows.push(
                    Table::empty()
                        .with("arc", Value::Number(a as f64))
                        .with("x", Value::number(x))
                        .with("y", Value::number(y)),
                );
            }
        }
        Dataset::new(rows)
    }

    fn mesh_line(data: Dataset) -> LineMark {
        line(data)
            .x("x")
            .y("y")
            .z("arc")
            .curve(Curve::Linear)
            .stroke("currentColor")
            .stroke_width(1.)
    }

    /// `Plot.geo(statemesh)`: the borders between states.
    pub fn statemesh() -> LineMark {
        mesh_line(mesh("states", true))
    }

    /// `Plot.geo(nation)`: the nation's outline.
    pub fn nation() -> LineMark {
        mesh_line(mesh("nation", false))
    }

    /// The planar centroid of projected polygons, Plot's `centroid`.
    fn centroid(polygons: &[Vec<Vec<(f64, f64)>>]) -> Option<(f64, f64)> {
        let (mut area, mut cx, mut cy) = (0., 0., 0.);
        for polygon in polygons {
            for ring in polygon {
                let points: Vec<(f64, f64)> = ring
                    .iter()
                    .filter_map(|&(lon, lat)| project(lon, lat))
                    .collect();
                for i in 0..points.len() {
                    let (x0, y0) = points[i];
                    let (x1, y1) = points[(i + 1) % points.len()];
                    let cross = x0 * y1 - x1 * y0;
                    area += cross;
                    cx += (x0 + x1) * cross;
                    cy += (y0 + y1) * cross;
                }
            }
        }
        (area.abs() > 1e-12).then(|| (cx / (3. * area), cy / (3. * area)))
    }

    /// One row per feature of `object`, with its properties, `id`, and
    /// projected centroid in `cx`, `cy`.
    fn features(object: &str) -> Vec<Table> {
        atlas().json["objects"][object]["geometries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|geometry| {
                let (cx, cy) = centroid(&polygons(geometry))?;
                let mut row = Table::empty()
                    .with("id", Value::text(geometry["id"].as_str().unwrap_or("")))
                    .with("cx", Value::number(cx))
                    .with("cy", Value::number(cy));
                if let Some(name) = geometry["properties"]["name"].as_str() {
                    row = row.with("name", Value::text(name.to_string()));
                }
                Some(row)
            })
            .collect()
    }

    /// `topojson.feature(us, us.objects.states).features`.
    pub fn states() -> Dataset {
        Dataset::new(features("states"))
    }

    /// The docs' counties, joined with each county's 2020 margin and votes
    /// and its population.
    pub fn counties() -> Dataset {
        let population: HashMap<String, f64> =
            gpui_plot::io::rows(&data::text("us-county-population.csv"), b',')
                .into_iter()
                .skip(1)
                .filter(|r| r.len() >= 3)
                .map(|r| (format!("{}{}", r[0], r[1]), r[2].parse().unwrap_or(f64::NAN)))
                .collect();
        let election_rows =
            gpui_plot::io::rows(&data::text("us-presidential-election-2020.csv"), b',');
        let header = &election_rows[0];
        let column = |name: &str| header.iter().position(|h| h == name).unwrap();
        let (fips, margin, votes) = (column("fips"), column("margin2020"), column("votes"));
        let election: HashMap<String, (f64, f64)> = election_rows[1..]
            .iter()
            .filter(|r| r.len() > margin.max(votes))
            .map(|r| {
                (
                    r[fips].clone(),
                    (
                        r[margin].parse().unwrap_or(f64::NAN),
                        r[votes].parse().unwrap_or(f64::NAN),
                    ),
                )
            })
            .collect();
        Dataset::new(features("counties").into_iter().map(|row| {
            let id = row.text("id").map(|s| s.to_string()).unwrap_or_default();
            let mut row = row.with(
                "population",
                Value::number(population.get(&id).copied().unwrap_or(f64::NAN)),
            );
            if let Some(&(margin, votes)) = election.get(&id) {
                row = row
                    .with("margin2020", Value::number(margin))
                    .with("votes", Value::number(votes));
            }
            row
        }))
    }

    /// Plot's `projection: "albers-usa"` frame: no axes, y down, one pixel
    /// per projected unit both ways.
    pub fn map(plot: Plot) -> Plot {
        plot.no_axes()
            .aspect_ratio(1.)
            .margin(0.)
            .y(|y| y.reverse(true))
    }
}
