//! The docs' Features section.

#[path = "curves.rs"]
pub mod curves;
#[path = "facets.rs"]
pub mod facets;
#[path = "formats.rs"]
pub mod formats;
#[path = "interactions.rs"]
pub mod interactions;
#[path = "legends.rs"]
pub mod legends;
#[path = "markers.rs"]
pub mod markers;
#[path = "marks.rs"]
pub mod marks;
#[path = "plots.rs"]
pub mod plots;
#[path = "projections.rs"]
pub mod projections;
#[path = "scales.rs"]
pub mod scales;
#[path = "shorthand.rs"]
pub mod shorthand;
#[path = "transforms.rs"]
pub mod transforms;

/// This section's ported examples, page by page.
pub fn examples() -> Vec<super::Example> {
    let mut all = Vec::new();
    all.extend(curves::examples());
    all.extend(facets::examples());
    all.extend(formats::examples());
    all.extend(interactions::examples());
    all.extend(legends::examples());
    all.extend(markers::examples());
    all.extend(marks::examples());
    all.extend(plots::examples());
    all.extend(projections::examples());
    all.extend(scales::examples());
    all.extend(shorthand::examples());
    all.extend(transforms::examples());
    all
}
