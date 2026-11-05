//! Plot's mark constructors: `Plot.dot(data, options)` is `dot(data)` here,
//! with the options as builder calls, and `mark.plot()` draws it alone.
//!
//! ```
//! use gpui_plot::marks::{dot, rule_y};
//! use gpui_plot::{Plot, Table, Value};
//!
//! let data = vec![Table::empty().with("x", Value::Number(1.)).with("y", Value::Number(2.))];
//! let plot = Plot::plot().mark(dot(data).x("x").y("y")).mark(rule_y([0.]));
//! ```

use crate::mark::*;
use crate::table::Dataset;

/// Plot's `dot(data, …)`.
pub fn dot(data: impl Into<Dataset>) -> DotMark {
    DotMark::new().data(data)
}

/// Plot's `dotX(data, …)`.
pub fn dot_x(data: impl Into<Dataset>) -> DotXMark {
    DotXMark::new().data(data)
}

/// Plot's `dotY(data, …)`.
pub fn dot_y(data: impl Into<Dataset>) -> DotYMark {
    DotYMark::new().data(data)
}

/// Plot's `line(data, …)`.
pub fn line(data: impl Into<Dataset>) -> LineMark {
    LineMark::new().data(data)
}

/// Plot's `lineX(data, …)`.
pub fn line_x(data: impl Into<Dataset>) -> LineXMark {
    LineXMark::new().data(data)
}

/// Plot's `lineY(data, …)`.
pub fn line_y(data: impl Into<Dataset>) -> LineYMark {
    LineYMark::new().data(data)
}

/// Plot's `areaY(data, …)`.
pub fn area_y(data: impl Into<Dataset>) -> AreaYMark {
    AreaYMark::new().data(data)
}

/// Plot's `areaX(data, …)`.
pub fn area_x(data: impl Into<Dataset>) -> AreaXMark {
    AreaXMark::new().data(data)
}

/// Plot's `barY(data, …)`.
pub fn bar_y(data: impl Into<Dataset>) -> BarYMark {
    BarYMark::new().data(data)
}

/// Plot's `barX(data, …)`.
pub fn bar_x(data: impl Into<Dataset>) -> BarXMark {
    BarXMark::new().data(data)
}

/// Plot's `rect(data, …)`.
pub fn rect(data: impl Into<Dataset>) -> RectMark {
    RectMark::new().data(data)
}

/// Plot's `rectX(data, …)`.
pub fn rect_x(data: impl Into<Dataset>) -> RectXMark {
    RectXMark::new().data(data)
}

/// Plot's `rectY(data, …)`.
pub fn rect_y(data: impl Into<Dataset>) -> RectYMark {
    RectYMark::new().data(data)
}

/// Plot's `cell(data, …)`.
pub fn cell(data: impl Into<Dataset>) -> CellMark {
    CellMark::new().data(data)
}

/// Plot's `ruleX(data, …)`.
pub fn rule_x(data: impl Into<Dataset>) -> RuleXMark {
    RuleXMark::new().data(data)
}

/// Plot's `ruleY(data, …)`.
pub fn rule_y(data: impl Into<Dataset>) -> RuleYMark {
    RuleYMark::new().data(data)
}

/// Plot's `text(data, …)`.
pub fn text(data: impl Into<Dataset>) -> TextMark {
    TextMark::new().data(data)
}

/// Plot's `tickX(data, …)`.
pub fn tick_x(data: impl Into<Dataset>) -> TickXMark {
    TickXMark::new().data(data)
}

/// Plot's `tickY(data, …)`.
pub fn tick_y(data: impl Into<Dataset>) -> TickYMark {
    TickYMark::new().data(data)
}

/// Plot's `vector(data, …)`.
pub fn vector(data: impl Into<Dataset>) -> VectorMark {
    VectorMark::new().data(data)
}

/// Plot's `boxX(data, …)`.
pub fn box_x(data: impl Into<Dataset>) -> BoxXMark {
    BoxXMark::new().data(data)
}

/// Plot's `boxY(data, …)`.
pub fn box_y(data: impl Into<Dataset>) -> BoxYMark {
    BoxYMark::new().data(data)
}

/// Plot's `violinX(data, …)`.
pub fn violin_x(data: impl Into<Dataset>) -> ViolinXMark {
    ViolinXMark::new().data(data)
}

/// Plot's `violinY(data, …)`.
pub fn violin_y(data: impl Into<Dataset>) -> ViolinYMark {
    ViolinYMark::new().data(data)
}

/// Plot's `hexbin(data, …)`.
pub fn hexbin(data: impl Into<Dataset>) -> HexbinMark {
    HexbinMark::new().data(data)
}

/// Plot's `densityX(data, …)`.
pub fn density_x(data: impl Into<Dataset>) -> DensityXMark {
    DensityXMark::new().data(data)
}

/// Plot's `densityY(data, …)`.
pub fn density_y(data: impl Into<Dataset>) -> DensityYMark {
    DensityYMark::new().data(data)
}

/// Plot's `contour(data, …)`.
pub fn contour(data: impl Into<Dataset>) -> ContourMark {
    ContourMark::new().data(data)
}

/// Plot's `density(data, …)`.
pub fn density(data: impl Into<Dataset>) -> DensityMark {
    DensityMark::new().data(data)
}

/// Plot's `pie(data, …)`.
pub fn pie(data: impl Into<Dataset>) -> PieMark {
    PieMark::new().data(data)
}

/// Plot's `arc(data, …)`.
pub fn arc(data: impl Into<Dataset>) -> ArcMark {
    ArcMark::new().data(data)
}

/// Plot's `waffle(data, …)`.
pub fn waffle(data: impl Into<Dataset>) -> WaffleMark {
    WaffleMark::new().data(data)
}

/// Plot's `link(data, …)`.
pub fn link(data: impl Into<Dataset>) -> LinkMark {
    LinkMark::new().data(data)
}

/// Plot's `delaunayLink(data, …)`.
pub fn delaunay_link(data: impl Into<Dataset>) -> DelaunayLinkMark {
    DelaunayLinkMark::new().data(data)
}

/// Plot's `delaunayMesh(data, …)`.
pub fn delaunay_mesh(data: impl Into<Dataset>) -> DelaunayMeshMark {
    DelaunayMeshMark::new().data(data)
}

/// Plot's `hull(data, …)`.
pub fn hull(data: impl Into<Dataset>) -> HullMark {
    HullMark::new().data(data)
}

/// Plot's `voronoi(data, …)`.
pub fn voronoi(data: impl Into<Dataset>) -> VoronoiMark {
    VoronoiMark::new().data(data)
}

/// Plot's `voronoiMesh(data, …)`.
pub fn voronoi_mesh(data: impl Into<Dataset>) -> VoronoiMeshMark {
    VoronoiMeshMark::new().data(data)
}

/// Plot's `tree(data, …)`.
pub fn tree(data: impl Into<Dataset>) -> TreeMark {
    TreeMark::new().data(data)
}

/// Plot's `image(data, …)`.
pub fn image(data: impl Into<Dataset>) -> ImageMark {
    ImageMark::new().data(data)
}

/// Plot's `geo(data, …)`.
pub fn geo(data: impl Into<Dataset>) -> GeoMark {
    GeoMark::new().data(data)
}

/// Plot's `tip(data, …)`.
pub fn tip(data: impl Into<Dataset>) -> TipMark {
    TipMark::new().data(data)
}

/// Plot's `axisX`, without data.
pub fn axis_x() -> AxisXMark {
    AxisXMark::new()
}

/// Plot's `axisY`, without data.
pub fn axis_y() -> AxisYMark {
    AxisYMark::new()
}

/// Plot's `gridX`, without data.
pub fn grid_x() -> GridXMark {
    GridXMark::new()
}

/// Plot's `gridY`, without data.
pub fn grid_y() -> GridYMark {
    GridYMark::new()
}

/// Plot's `frame`, without data.
pub fn frame() -> FrameMark {
    FrameMark::new()
}

/// Plot's `textX(data, …)`.
pub fn text_x(data: impl Into<Dataset>) -> TextXMark {
    TextXMark::new().data(data)
}

/// Plot's `textY(data, …)`.
pub fn text_y(data: impl Into<Dataset>) -> TextYMark {
    TextYMark::new().data(data)
}

/// Plot's `cellX(data, …)`.
pub fn cell_x(data: impl Into<Dataset>) -> CellXMark {
    CellXMark::new().data(data)
}

/// Plot's `cellY(data, …)`.
pub fn cell_y(data: impl Into<Dataset>) -> CellYMark {
    CellYMark::new().data(data)
}

/// Plot's `vectorX(data, …)`.
pub fn vector_x(data: impl Into<Dataset>) -> VectorXMark {
    VectorXMark::new().data(data)
}

/// Plot's `vectorY(data, …)`.
pub fn vector_y(data: impl Into<Dataset>) -> VectorYMark {
    VectorYMark::new().data(data)
}

/// Plot's `spike(data, …)`.
pub fn spike(data: impl Into<Dataset>) -> SpikeMark {
    SpikeMark::new().data(data)
}

/// Plot's `arrow(data, …)`.
pub fn arrow(data: impl Into<Dataset>) -> ArrowMark {
    ArrowMark::new().data(data)
}

/// Plot's `waffleX(data, …)`.
pub fn waffle_x(data: impl Into<Dataset>) -> WaffleXMark {
    WaffleXMark::new().data(data)
}

/// Plot's `waffleY(data, …)`.
pub fn waffle_y(data: impl Into<Dataset>) -> WaffleYMark {
    WaffleYMark::new().data(data)
}

/// Plot's `hexgrid`, without data.
pub fn hexgrid() -> HexgridMark {
    HexgridMark::new()
}

/// Plot's `raster(data, …)`.
pub fn raster(data: impl Into<Dataset>) -> RasterMark {
    RasterMark::new().data(data)
}

/// Plot's `differenceX(data, …)`.
pub fn difference_x(data: impl Into<Dataset>) -> DifferenceXMark {
    DifferenceXMark::new().data(data)
}

/// Plot's `differenceY(data, …)`.
pub fn difference_y(data: impl Into<Dataset>) -> DifferenceYMark {
    DifferenceYMark::new().data(data)
}

/// Plot's `linearRegressionX(data, …)`.
pub fn linear_regression_x(data: impl Into<Dataset>) -> LinearRegressionXMark {
    LinearRegressionXMark::new().data(data)
}

/// Plot's `linearRegressionY(data, …)`.
pub fn linear_regression_y(data: impl Into<Dataset>) -> LinearRegressionYMark {
    LinearRegressionYMark::new().data(data)
}

/// Plot's `bollingerX(data, …)`.
pub fn bollinger_x(data: impl Into<Dataset>) -> BollingerXMark {
    BollingerXMark::new().data(data)
}

/// Plot's `bollingerY(data, …)`.
pub fn bollinger_y(data: impl Into<Dataset>) -> BollingerYMark {
    BollingerYMark::new().data(data)
}

/// Plot's `tree(data, options)`: the composite of a link, dot and text mark
/// over the tree transforms. Pass it to [`crate::Plot::mark`].
pub fn tree_of(data: impl Into<Dataset>, spec: crate::hierarchy::TreeSpec) -> TreeMark {
    let mut mark = TreeMark::new().data(data).into_mark();
    mark.tree = Some((spec, false));
    TreeMark::from_mark(mark)
}

/// Plot's `cluster(data, options)`: [`tree_of`] with the cluster layout.
pub fn cluster(data: impl Into<Dataset>, spec: crate::hierarchy::TreeSpec) -> TreeMark {
    tree_of(
        data,
        crate::hierarchy::TreeSpec {
            layout: crate::hierarchy::TreeLayout::Cluster,
            ..spec
        },
    )
}
