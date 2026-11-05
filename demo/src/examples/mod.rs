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
        .filter(|state| !matches!(state.value("id").label().as_ref(), "02" | "15" | "72"))
        .filter_map(|state| state.value("geometry").as_geo().cloned())
        .collect();
    gpui_plot::GeoProjection::named("albers")
        .expect("the albers projection")
        .domain(gpui_plot::Geometry::Collection(lower48))
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
