//! The docs' Marks section.

#[path = "area.rs"]
pub mod area;
#[path = "arrow.rs"]
pub mod arrow;
#[path = "auto.rs"]
pub mod auto;
#[path = "axis.rs"]
pub mod axis;
#[path = "bar.rs"]
pub mod bar;
#[path = "bollinger.rs"]
pub mod bollinger;
#[path = "box.rs"]
pub mod box_;
#[path = "cell.rs"]
pub mod cell;
#[path = "contour.rs"]
pub mod contour;
#[path = "delaunay.rs"]
pub mod delaunay;
#[path = "density.rs"]
pub mod density;
#[path = "difference.rs"]
pub mod difference;
#[path = "dot.rs"]
pub mod dot;
#[path = "frame.rs"]
pub mod frame;
#[path = "geo.rs"]
pub mod geo;
#[path = "grid.rs"]
pub mod grid;
#[path = "hexgrid.rs"]
pub mod hexgrid;
#[path = "image.rs"]
pub mod image;
#[path = "line.rs"]
pub mod line;
#[path = "linear-regression.rs"]
pub mod linear_regression;
#[path = "link.rs"]
pub mod link;
#[path = "raster.rs"]
pub mod raster;
#[path = "rect.rs"]
pub mod rect;
#[path = "rule.rs"]
pub mod rule;
#[path = "text.rs"]
pub mod text;
#[path = "tick.rs"]
pub mod tick;
#[path = "tip.rs"]
pub mod tip;
#[path = "tree.rs"]
pub mod tree;
#[path = "vector.rs"]
pub mod vector;
#[path = "waffle.rs"]
pub mod waffle;

/// This section's ported examples, page by page.
pub fn examples() -> Vec<super::Example> {
    let mut all = Vec::new();
    all.extend(area::examples());
    all.extend(arrow::examples());
    all.extend(auto::examples());
    all.extend(axis::examples());
    all.extend(bar::examples());
    all.extend(bollinger::examples());
    all.extend(box_::examples());
    all.extend(cell::examples());
    all.extend(contour::examples());
    all.extend(delaunay::examples());
    all.extend(density::examples());
    all.extend(difference::examples());
    all.extend(dot::examples());
    all.extend(frame::examples());
    all.extend(geo::examples());
    all.extend(grid::examples());
    all.extend(hexgrid::examples());
    all.extend(image::examples());
    all.extend(line::examples());
    all.extend(linear_regression::examples());
    all.extend(link::examples());
    all.extend(raster::examples());
    all.extend(rect::examples());
    all.extend(rule::examples());
    all.extend(text::examples());
    all.extend(tick::examples());
    all.extend(tip::examples());
    all.extend(tree::examples());
    all.extend(vector::examples());
    all.extend(waffle::examples());
    all
}
