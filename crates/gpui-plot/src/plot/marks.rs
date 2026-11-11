//! The mark shortcuts: [`Plot::mark`] and one builder per mark type,
//! [`Plot::dot`], [`Plot::line`] and the rest.

use crate::mark::{Mark, MarkKind};
use crate::plot::Plot;

impl Plot {
    /// Add a mark from its builder.
    pub fn mark(mut self, mark: impl Into<Mark>) -> Self {
        let mark = mark.into();
        // Plot's box marks are composites: a whisker rule, an
        // interquartile bar, a median tick and outlier dots.
        match mark.kind {
            MarkKind::BoxX | MarkKind::BoxY => self.marks.extend(crate::mark::expand_box(mark)),
            // Plot's tree mark composes a link, a dot and labels, over the
            // tree transforms.
            MarkKind::Tree if mark.tree.is_some() => {
                let spec = mark.tree.clone().map(|(s, _)| s).unwrap_or_default();
                self.marks.extend(crate::mark::expand_tree(mark, spec))
            }
            _ => self.marks.push(mark),
        }
        self
    }

    /// Add a dot mark.
    pub fn dot(
        mut self,
        configure: impl FnOnce(crate::mark::DotMark) -> crate::mark::DotMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DotMark::new()).into());
        self
    }

    /// Add a dot mark along x only, Plot's `dotX`.
    pub fn dot_x(
        mut self,
        configure: impl FnOnce(crate::mark::DotXMark) -> crate::mark::DotXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DotXMark::new()).into());
        self
    }

    /// Add a dot mark along y only, Plot's `dotY`.
    pub fn dot_y(
        mut self,
        configure: impl FnOnce(crate::mark::DotYMark) -> crate::mark::DotYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DotYMark::new()).into());
        self
    }

    /// Add a line mark.
    pub fn line(
        mut self,
        configure: impl FnOnce(crate::mark::LineMark) -> crate::mark::LineMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LineMark::new()).into());
        self
    }

    /// Add a line mark along x only, Plot's `lineX`.
    pub fn line_x(
        mut self,
        configure: impl FnOnce(crate::mark::LineXMark) -> crate::mark::LineXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LineXMark::new()).into());
        self
    }

    /// Add a line mark along y only, Plot's `lineY`.
    pub fn line_y(
        mut self,
        configure: impl FnOnce(crate::mark::LineYMark) -> crate::mark::LineYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LineYMark::new()).into());
        self
    }

    /// Add a vertical area mark.
    pub fn area_y(
        mut self,
        configure: impl FnOnce(crate::mark::AreaYMark) -> crate::mark::AreaYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::AreaYMark::new()).into());
        self
    }

    /// Add a horizontal area mark.
    pub fn area_x(
        mut self,
        configure: impl FnOnce(crate::mark::AreaXMark) -> crate::mark::AreaXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::AreaXMark::new()).into());
        self
    }

    /// Add a vertical bar mark.
    pub fn bar_y(
        mut self,
        configure: impl FnOnce(crate::mark::BarYMark) -> crate::mark::BarYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::BarYMark::new()).into());
        self
    }

    /// Add a horizontal bar mark.
    pub fn bar_x(
        mut self,
        configure: impl FnOnce(crate::mark::BarXMark) -> crate::mark::BarXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::BarXMark::new()).into());
        self
    }

    /// Add a rectangle mark.
    pub fn rect(
        mut self,
        configure: impl FnOnce(crate::mark::RectMark) -> crate::mark::RectMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RectMark::new()).into());
        self
    }

    /// Add a rectangle over one x, Plot's `rectX`.
    pub fn rect_x(
        mut self,
        configure: impl FnOnce(crate::mark::RectXMark) -> crate::mark::RectXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RectXMark::new()).into());
        self
    }

    /// Add a rectangle over one y, Plot's `rectY`.
    pub fn rect_y(
        mut self,
        configure: impl FnOnce(crate::mark::RectYMark) -> crate::mark::RectYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RectYMark::new()).into());
        self
    }

    /// Add a cell mark.
    pub fn cell(
        mut self,
        configure: impl FnOnce(crate::mark::CellMark) -> crate::mark::CellMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::CellMark::new()).into());
        self
    }

    /// Add a vertical rule mark.
    pub fn rule_x(
        mut self,
        configure: impl FnOnce(crate::mark::RuleXMark) -> crate::mark::RuleXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RuleXMark::new()).into());
        self
    }

    /// Add a horizontal rule mark.
    /// Add a `tip` mark, Plot's `tip`: the hover tooltip, drawn permanently at
    /// the datum the mark selects.
    pub fn tip(
        mut self,
        configure: impl FnOnce(crate::mark::TipMark) -> crate::mark::TipMark,
    ) -> Self {
        let builder = configure(crate::mark::TipMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add an `axisX` mark, Plot's `axisX`: an x axis drawn as a mark rather
    /// than as a plot-level option, so a chart can put one inside the frame.
    pub fn axis_x(
        mut self,
        configure: impl FnOnce(crate::mark::AxisXMark) -> crate::mark::AxisXMark,
    ) -> Self {
        let builder = configure(crate::mark::AxisXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add an `axisY` mark, Plot's `axisY`.
    pub fn axis_y(
        mut self,
        configure: impl FnOnce(crate::mark::AxisYMark) -> crate::mark::AxisYMark,
    ) -> Self {
        let builder = configure(crate::mark::AxisYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `gridX` mark, Plot's `gridX`: grid lines at each x tick.
    pub fn grid_x(
        mut self,
        configure: impl FnOnce(crate::mark::GridXMark) -> crate::mark::GridXMark,
    ) -> Self {
        let builder = configure(crate::mark::GridXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `gridY` mark, Plot's `gridY`.
    pub fn grid_y(
        mut self,
        configure: impl FnOnce(crate::mark::GridYMark) -> crate::mark::GridYMark,
    ) -> Self {
        let builder = configure(crate::mark::GridYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `frame` mark, Plot's `frame`: the plot's outline, which Plot
    /// draws only when the plot asks for it.
    pub fn frame_mark(
        mut self,
        configure: impl FnOnce(crate::mark::FrameMark) -> crate::mark::FrameMark,
    ) -> Self {
        let builder = configure(crate::mark::FrameMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `textX` mark, Plot's `textX`: A text label at each x with no y.
    pub fn text_x(
        mut self,
        configure: impl FnOnce(crate::mark::TextXMark) -> crate::mark::TextXMark,
    ) -> Self {
        let builder = configure(crate::mark::TextXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `textY` mark, Plot's `textY`: A text label at each y with no x.
    pub fn text_y(
        mut self,
        configure: impl FnOnce(crate::mark::TextYMark) -> crate::mark::TextYMark,
    ) -> Self {
        let builder = configure(crate::mark::TextYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `cellX` mark, Plot's `cellX`: A cell over one x with no y.
    pub fn cell_x(
        mut self,
        configure: impl FnOnce(crate::mark::CellXMark) -> crate::mark::CellXMark,
    ) -> Self {
        let builder = configure(crate::mark::CellXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `cellY` mark, Plot's `cellY`: A cell over one y with no x.
    pub fn cell_y(
        mut self,
        configure: impl FnOnce(crate::mark::CellYMark) -> crate::mark::CellYMark,
    ) -> Self {
        let builder = configure(crate::mark::CellYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `vectorX` mark, Plot's `vectorX`: A vector at each x.
    pub fn vector_x(
        mut self,
        configure: impl FnOnce(crate::mark::VectorXMark) -> crate::mark::VectorXMark,
    ) -> Self {
        let builder = configure(crate::mark::VectorXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `vectorY` mark, Plot's `vectorY`: A vector at each y.
    pub fn vector_y(
        mut self,
        configure: impl FnOnce(crate::mark::VectorYMark) -> crate::mark::VectorYMark,
    ) -> Self {
        let builder = configure(crate::mark::VectorYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `spike` mark, Plot's `spike`: A triangular marker at each datum.
    pub fn spike(
        mut self,
        configure: impl FnOnce(crate::mark::SpikeMark) -> crate::mark::SpikeMark,
    ) -> Self {
        let builder = configure(crate::mark::SpikeMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `arrow` mark, Plot's `arrow`: A curved arrow between two points.
    pub fn arrow(
        mut self,
        configure: impl FnOnce(crate::mark::ArrowMark) -> crate::mark::ArrowMark,
    ) -> Self {
        let builder = configure(crate::mark::ArrowMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `waffleX` mark, Plot's `waffleX`: A waffle bar along x.
    pub fn waffle_x(
        mut self,
        configure: impl FnOnce(crate::mark::WaffleXMark) -> crate::mark::WaffleXMark,
    ) -> Self {
        let builder = configure(crate::mark::WaffleXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `waffleY` mark, Plot's `waffleY`: A waffle bar along y.
    pub fn waffle_y(
        mut self,
        configure: impl FnOnce(crate::mark::WaffleYMark) -> crate::mark::WaffleYMark,
    ) -> Self {
        let builder = configure(crate::mark::WaffleYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `hexgrid` mark, Plot's `hexgrid`: A static hexagonal grid.
    pub fn hexgrid(
        mut self,
        configure: impl FnOnce(crate::mark::HexgridMark) -> crate::mark::HexgridMark,
    ) -> Self {
        let builder = configure(crate::mark::HexgridMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `raster` mark, Plot's `raster`: A pixel grid colored from the data.
    pub fn raster(
        mut self,
        configure: impl FnOnce(crate::mark::RasterMark) -> crate::mark::RasterMark,
    ) -> Self {
        let builder = configure(crate::mark::RasterMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `differenceX` mark, Plot's `differenceX`: A difference chart along x.
    pub fn difference_x(
        mut self,
        configure: impl FnOnce(crate::mark::DifferenceXMark) -> crate::mark::DifferenceXMark,
    ) -> Self {
        let builder = configure(crate::mark::DifferenceXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `differenceY` mark, Plot's `differenceY`: A difference chart along y.
    pub fn difference_y(
        mut self,
        configure: impl FnOnce(crate::mark::DifferenceYMark) -> crate::mark::DifferenceYMark,
    ) -> Self {
        let builder = configure(crate::mark::DifferenceYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `linearRegressionX` mark, Plot's `linearRegressionX`: A least-squares fit of x on y.
    pub fn linear_regression_x(
        mut self,
        configure: impl FnOnce(crate::mark::LinearRegressionXMark) -> crate::mark::LinearRegressionXMark,
    ) -> Self {
        let builder = configure(crate::mark::LinearRegressionXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `linearRegressionY` mark, Plot's `linearRegressionY`: A least-squares fit of y on x.
    pub fn linear_regression_y(
        mut self,
        configure: impl FnOnce(crate::mark::LinearRegressionYMark) -> crate::mark::LinearRegressionYMark,
    ) -> Self {
        let builder = configure(crate::mark::LinearRegressionYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `bollingerX` mark, Plot's `bollingerX`: A moving mean with deviation bands along x.
    pub fn bollinger_x(
        mut self,
        configure: impl FnOnce(crate::mark::BollingerXMark) -> crate::mark::BollingerXMark,
    ) -> Self {
        let builder = configure(crate::mark::BollingerXMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    /// Add a `bollingerY` mark, Plot's `bollingerY`: A moving mean with deviation bands along y.
    pub fn bollinger_y(
        mut self,
        configure: impl FnOnce(crate::mark::BollingerYMark) -> crate::mark::BollingerYMark,
    ) -> Self {
        let builder = configure(crate::mark::BollingerYMark::new());
        self.marks.push(builder.into_mark());
        self
    }

    pub fn rule_y(
        mut self,
        configure: impl FnOnce(crate::mark::RuleYMark) -> crate::mark::RuleYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::RuleYMark::new()).into());
        self
    }

    /// Add a text mark.
    pub fn text(
        mut self,
        configure: impl FnOnce(crate::mark::TextMark) -> crate::mark::TextMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TextMark::new()).into());
        self
    }

    /// Add a vertical tick mark.
    pub fn tick_x(
        mut self,
        configure: impl FnOnce(crate::mark::TickXMark) -> crate::mark::TickXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TickXMark::new()).into());
        self
    }

    /// Add a horizontal tick mark.
    pub fn tick_y(
        mut self,
        configure: impl FnOnce(crate::mark::TickYMark) -> crate::mark::TickYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TickYMark::new()).into());
        self
    }

    /// Add an arrow mark.
    pub fn vector(
        mut self,
        configure: impl FnOnce(crate::mark::VectorMark) -> crate::mark::VectorMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::VectorMark::new()).into());
        self
    }

    /// Add a box plot along x.
    pub fn box_x(
        mut self,
        configure: impl FnOnce(crate::mark::BoxXMark) -> crate::mark::BoxXMark,
    ) -> Self {
        let mark: Mark = configure(crate::mark::BoxXMark::new()).into();
        self.marks.extend(crate::mark::expand_box(mark));
        self
    }

    /// Add a box plot along y.
    pub fn box_y(
        mut self,
        configure: impl FnOnce(crate::mark::BoxYMark) -> crate::mark::BoxYMark,
    ) -> Self {
        let mark: Mark = configure(crate::mark::BoxYMark::new()).into();
        self.marks.extend(crate::mark::expand_box(mark));
        self
    }

    /// Add a violin plot along x.
    pub fn violin_x(
        mut self,
        configure: impl FnOnce(crate::mark::ViolinXMark) -> crate::mark::ViolinXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ViolinXMark::new()).into());
        self
    }

    /// Add a violin plot along y.
    pub fn violin_y(
        mut self,
        configure: impl FnOnce(crate::mark::ViolinYMark) -> crate::mark::ViolinYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ViolinYMark::new()).into());
        self
    }

    /// Add a hexagonal binning mark.
    pub fn hexbin(
        mut self,
        configure: impl FnOnce(crate::mark::HexbinMark) -> crate::mark::HexbinMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::HexbinMark::new()).into());
        self
    }

    /// Add a density estimate along x.
    pub fn density_x(
        mut self,
        configure: impl FnOnce(crate::mark::DensityXMark) -> crate::mark::DensityXMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DensityXMark::new()).into());
        self
    }

    /// Add a density estimate along y.
    pub fn density_y(
        mut self,
        configure: impl FnOnce(crate::mark::DensityYMark) -> crate::mark::DensityYMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DensityYMark::new()).into());
        self
    }

    /// Add a contour mark.
    pub fn contour(
        mut self,
        configure: impl FnOnce(crate::mark::ContourMark) -> crate::mark::ContourMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ContourMark::new()).into());
        self
    }

    /// Add a density mark, Plot's `density`: contours of the two-dimensional
    /// kernel density of x and y, one set per `z` series (or per `stroke`
    /// field), over thresholds shared by every series.
    pub fn density(
        mut self,
        configure: impl FnOnce(crate::mark::DensityMark) -> crate::mark::DensityMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DensityMark::new()).into());
        self
    }

    /// Add a pie mark.
    pub fn pie(
        mut self,
        configure: impl FnOnce(crate::mark::PieMark) -> crate::mark::PieMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::PieMark::new()).into());
        self
    }

    /// Add an arc mark.
    pub fn arc(
        mut self,
        configure: impl FnOnce(crate::mark::ArcMark) -> crate::mark::ArcMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ArcMark::new()).into());
        self
    }

    /// Add a waffle mark.
    pub fn waffle(
        mut self,
        configure: impl FnOnce(crate::mark::WaffleMark) -> crate::mark::WaffleMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::WaffleMark::new()).into());
        self
    }

    /// Add a link mark.
    pub fn link(
        mut self,
        configure: impl FnOnce(crate::mark::LinkMark) -> crate::mark::LinkMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::LinkMark::new()).into());
        self
    }

    /// Add a tree diagram of a slash-separated hierarchy, Plot's `tree`.
    pub fn tree(
        mut self,
        configure: impl FnOnce(crate::mark::TreeMark) -> crate::mark::TreeMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::TreeMark::new()).into());
        self
    }

    /// Add a raster image painted at a position, Plot's `image`.
    pub fn image(
        mut self,
        configure: impl FnOnce(crate::mark::ImageMark) -> crate::mark::ImageMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::ImageMark::new()).into());
        self
    }

    /// Add a geographic mark, Plot's `geo`: lon/lat projected equirectangularly.
    pub fn geo(
        mut self,
        configure: impl FnOnce(crate::mark::GeoMark) -> crate::mark::GeoMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::GeoMark::new()).into());
        self
    }

    /// Add a Delaunay triangulation drawn as links, Plot's `delaunayLink`.
    pub fn delaunay_link(
        mut self,
        configure: impl FnOnce(crate::mark::DelaunayLinkMark) -> crate::mark::DelaunayLinkMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DelaunayLinkMark::new()).into());
        self
    }

    /// Add a Delaunay triangulation drawn as one mesh, Plot's `delaunayMesh`.
    pub fn delaunay_mesh(
        mut self,
        configure: impl FnOnce(crate::mark::DelaunayMeshMark) -> crate::mark::DelaunayMeshMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::DelaunayMeshMark::new()).into());
        self
    }

    /// Add the convex hull of the data, Plot's `hull`.
    pub fn hull(
        mut self,
        configure: impl FnOnce(crate::mark::HullMark) -> crate::mark::HullMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::HullMark::new()).into());
        self
    }

    /// Add a Voronoi cell per datum, Plot's `voronoi`.
    pub fn voronoi(
        mut self,
        configure: impl FnOnce(crate::mark::VoronoiMark) -> crate::mark::VoronoiMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::VoronoiMark::new()).into());
        self
    }

    /// Add the Voronoi mesh of the data, Plot's `voronoiMesh`.
    pub fn voronoi_mesh(
        mut self,
        configure: impl FnOnce(crate::mark::VoronoiMeshMark) -> crate::mark::VoronoiMeshMark,
    ) -> Self {
        self.marks
            .push(configure(crate::mark::VoronoiMeshMark::new()).into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use crate::render;
    use crate::table::Table;
    use crate::value::Value;

    #[test]
    fn tip_and_furniture_marks_prepare() {
        let data = vec![
            Table::empty()
                .with("x", Value::Number(1.))
                .with("y", Value::Number(2.)),
            Table::empty()
                .with("x", Value::Number(2.))
                .with("y", Value::Number(5.)),
        ];
        let plot = Plot::new(data)
            .tip(|mark| mark.x("x").y("y").text("hi"))
            .grid_x(|mark| mark)
            .frame_mark(|mark| mark);
        let prepared = render::prepare(plot.data(), plot.marks());
        // The tip reads the data; a furniture mark reads one placeholder
        // record, which its constant channels read.
        assert_eq!(prepared[0].rows.len(), 2);
        assert_eq!(prepared[1].rows.len(), 1);
        assert_eq!(prepared[2].rows.len(), 1);
        assert!(prepared[1].mark.kind.is_furniture());
    }

    fn sample() -> Vec<Table> {
        [("a", 4.), ("b", 7.), ("c", 2.)]
            .into_iter()
            .map(|(x, y)| {
                Table::empty()
                    .with("x", Value::text(x))
                    .with("y", Value::Number(y))
            })
            .collect()
    }

    #[test]
    fn plot_builds_marks() {
        let plot = Plot::new(sample())
            .dot(|mark| mark.x("x").y("y"))
            .line(|mark| mark.x("x").y("y").stroke_width(2.));
        assert_eq!(plot.marks().len(), 2);
        assert_eq!(plot.marks()[0].kind, MarkKind::Dot);
        assert_eq!(plot.marks()[1].style.stroke_width, 2.);
    }
}
