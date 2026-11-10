//! Plot's documentation examples, organized as the docs are: a section
//! (Features, Marks, Transforms, Interactions), a page per topic, and the
//! page's examples in order.
//!
//! Each example is the docs' JavaScript, kept verbatim in `plot-docs.json`,
//! and its port: the Rust that builds the same chart.

use std::sync::OnceLock;

use gpui_plot::Plot;

pub mod features;
pub mod interactions;
pub mod marks;
pub mod transforms;

/// One documentation example and its port.
#[derive(Clone, Copy)]
pub struct Example {
    /// The docs section: `features`, `marks`, `transforms` or `interactions`.
    pub section: &'static str,
    /// The page's file name in the docs, `dot` for `marks/dot.md`.
    pub page: &'static str,
    /// The example's place on its page, from 1.
    pub index: usize,
    /// The height the example draws at, in pixels.
    pub height: f32,
    /// Builds the chart.
    pub build: fn() -> Plot,
}

/// What the docs say about one example.
#[derive(Clone, Debug)]
pub struct Doc {
    pub section: String,
    pub page: String,
    pub page_title: String,
    pub index: usize,
    pub heading: Option<String>,
    pub code: String,
    pub prose: String,
}

impl Example {
    /// The example's id, `marks-dot-03`, which its snapshot is named by.
    pub fn id(&self) -> String {
        format!("{}-{}-{:02}", self.section, self.page, self.index)
    }

    /// The docs' entry for this example.
    pub fn doc(&self) -> Option<&'static Doc> {
        docs().iter().find(|doc| {
            doc.section == self.section && doc.page == self.page && doc.index == self.index
        })
    }
}

/// An example at the default height, Plot's 400 pixels plus room for its
/// legend and labels.
pub const fn example(
    section: &'static str,
    page: &'static str,
    index: usize,
    build: fn() -> Plot,
) -> Example {
    Example {
        section,
        page,
        index,
        height: 420.,
        build,
    }
}

/// Every documentation example, with the code the docs show.
pub fn docs() -> &'static [Doc] {
    static DOCS: OnceLock<Vec<Doc>> = OnceLock::new();
    DOCS.get_or_init(|| {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("plot-docs.json"))
            .expect("the docs manifest is JSON");
        manifest
            .as_array()
            .expect("the docs manifest is an array")
            .iter()
            .map(|entry| {
                let text = |key: &str| entry[key].as_str().unwrap_or_default().to_string();
                Doc {
                    section: text("section"),
                    page: text("page"),
                    page_title: text("page_title"),
                    index: entry["index"].as_u64().unwrap_or(0) as usize,
                    heading: entry["heading"].as_str().map(str::to_string),
                    code: text("code"),
                    prose: text("prose"),
                }
            })
            .collect()
    })
}

/// Plot's `projection: "albers"`. Gap: the library's `"albers"` lacks
/// d3.geoAlbers' `center([-0.6, 38.7])`, so without a domain the lower 48
/// project above the frame and nothing shows. Fitting it to the lower 48
/// states frames the map where d3's default scale and translate put it.
pub fn albers() -> gpui_plot::GeoProjection {
    let lower48 = crate::data::states()
        .rows()
        .iter()
        // The states' FIPS codes stop at 56; Alaska (02), Hawaii (15) and
        // the territories (60 and up) lie outside d3's default view.
        .filter(|state| {
            let id = state.value("id").label();
            id.parse::<u32>()
                .is_ok_and(|n| n <= 56 && n != 2 && n != 15)
        })
        .filter_map(|state| state.value("geometry").as_geo().cloned())
        .collect();
    gpui_plot::GeoProjection::named("albers")
        .expect("the albers projection")
        .domain(gpui_plot::Geometry::Collection(lower48))
}

/// Gap: the voronoi and hexbin marks ignore the plot's `projection`
/// (they tessellate and bin in x/y scale space, so their output lands far
/// from the map). Until they project, these ports place each record by the
/// library's own fitted projection, as `x`/`y` fields, and pin linear x/y
/// scales to the projection's frame so a unit is a pixel both ways: what
/// Plot's projection does before those marks see the points.
pub fn pre_projected(
    data: gpui_plot::Dataset,
    projection: &gpui_plot::GeoProjection,
    lonlat: impl Fn(&gpui_plot::Table) -> Option<[f64; 2]>,
) -> (gpui_plot::Dataset, gpui_plot::Plot) {
    let (rows, frame) = pre_projected_rows(data, projection, lonlat);
    (rows, pre_projected_plot(frame))
}

/// [`pre_projected`]'s records, with the projection's frame the plot pins
/// its scales to.
pub fn pre_projected_rows(
    data: gpui_plot::Dataset,
    projection: &gpui_plot::GeoProjection,
    lonlat: impl Fn(&gpui_plot::Table) -> Option<[f64; 2]>,
) -> (gpui_plot::Dataset, [[f64; 2]; 2]) {
    let fitted = projection.fit(1000., 1000.);
    let [[x0, y0], [x1, y1]] = projection
        .domain
        .as_ref()
        .and_then(|domain| fitted.bounds(domain))
        .unwrap_or([[0., 0.], [1000., 1000.]]);
    let rows = data.rows().iter().filter_map(|row| {
        let [lon, lat] = lonlat(row)?;
        let [x, y] = fitted.project(lon, lat)?;
        Some(
            row.clone()
                .with("x", gpui_plot::Value::Number(x))
                .with("y", gpui_plot::Value::Number(y)),
        )
    });
    (gpui_plot::Dataset::new(rows), [[x0, y0], [x1, y1]])
}

/// [`pre_projected`]'s plot: linear x and y pinned to the projection's
/// frame, a pixel a unit both ways.
pub fn pre_projected_plot([[x0, y0], [x1, y1]]: [[f64; 2]; 2]) -> gpui_plot::Plot {
    gpui_plot::Plot::plot()
        .no_axes()
        .margin(0.)
        .aspect_ratio(1.)
        .x(move |x| x.domain([x0, x1]).nice(false).zero(false))
        // Projected y grows downward.
        .y(move |y| y.domain([y0, y1]).reverse(true).nice(false).zero(false))
}

/// A geometry's lines through the same fitted projection as
/// [`pre_projected`], as a `line` mark (one series per path), so a map can
/// sit under pre-projected points.
pub fn pre_projected_lines(
    geometry: &gpui_plot::Geometry,
    projection: &gpui_plot::GeoProjection,
) -> gpui_plot::mark::LineMark {
    let fitted = projection.fit(1000., 1000.);
    let rows = fitted
        .paths(geometry)
        .into_iter()
        .enumerate()
        .flat_map(|(i, (path, _))| {
            path.into_iter().map(move |[x, y]| {
                gpui_plot::Table::empty()
                    .with("x", gpui_plot::Value::Number(x))
                    .with("y", gpui_plot::Value::Number(y))
                    .with("path", gpui_plot::Value::Number(i as f64))
            })
        });
    gpui_plot::marks::line(gpui_plot::Dataset::new(rows))
        .x("x")
        .y("y")
        .z("path")
        .clip(true)
}

/// A record's geometry's spherical centroid, Plot's `geoCentroid`.
pub fn geo_centroid(row: &gpui_plot::Table) -> Option<[f64; 2]> {
    gpui_plot::geo::centroid(row.value("geometry").as_geo()?)
}

/// Every ported example, in the docs' order.
pub fn examples() -> Vec<Example> {
    let mut all = Vec::new();
    all.extend(features::examples());
    all.extend(marks::examples());
    all.extend(transforms::examples());
    all.extend(interactions::examples());
    all
}

/// The docs' examples that have no port yet.
pub fn missing() -> Vec<&'static Doc> {
    let ported = examples();
    docs()
        .iter()
        .filter(|doc| {
            !ported
                .iter()
                .any(|e| e.section == doc.section && e.page == doc.page && e.index == doc.index)
        })
        .collect()
}
