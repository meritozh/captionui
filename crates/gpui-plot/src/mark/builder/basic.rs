//! The typed builders of the marks the basic renderer draws: dots, lines, areas,
//! bars, rects, cells, rules, text, ticks, vectors, tips and the rest.

use gpui::SharedString;

use super::mark_builder;
use crate::mark::{
    BlendMode, Curve, DomainSort, FrameAnchor, Interval, LineAnchor, MapReducer, Mark, MarkKind,
    TextAnchor, VectorAnchor,
};
use crate::style::{ColorChannel, NumberChannel};
use crate::table::IntoChannel;
use crate::transform::{
    Anchor, Dodge, GroupReducer, Mapper, Reducer, Selector, StackOffset, Thresholds, WindowReducer,
};

mark_builder!(
    /// A dot mark, Plot's `dot`: one symbol per datum.
    ///
    /// ```
    /// # use gpui_plot::mark::DotMark;
    /// DotMark::new().x("date").y("value").r(4.);
    /// ```
    DotMark,
    MarkKind::Dot
);
mark_builder!(
    /// A dot at each x with no y, Plot's `dotX`.
    DotXMark,
    MarkKind::DotX
);
mark_builder!(
    /// A dot at each y with no x, Plot's `dotY`.
    DotYMark,
    MarkKind::DotY
);
mark_builder!(
    /// A line mark, Plot's `line`: a polyline through the data, one per series.
    LineMark,
    MarkKind::Line
);
mark_builder!(
    /// A line at each x with no y, Plot's `lineX`.
    LineXMark,
    MarkKind::LineX
);
mark_builder!(
    /// A line at each y with no x, Plot's `lineY`.
    LineYMark,
    MarkKind::LineY
);
mark_builder!(
    /// A vertical area mark, Plot's `areaY`.
    AreaYMark,
    MarkKind::AreaY
);
mark_builder!(
    /// A horizontal area mark, Plot's `areaX`.
    AreaXMark,
    MarkKind::AreaX
);
mark_builder!(
    /// A vertical bar mark, Plot's `barY`.
    BarYMark,
    MarkKind::BarY
);
mark_builder!(
    /// A horizontal bar mark, Plot's `barX`.
    BarXMark,
    MarkKind::BarX
);
mark_builder!(
    /// A rectangle mark spanning two intervals, Plot's `rect`.
    RectMark,
    MarkKind::Rect
);
mark_builder!(
    /// A rectangle over one x, Plot's `rectX`: a horizontal histogram bar.
    RectXMark,
    MarkKind::RectX
);
mark_builder!(
    /// A rectangle over one y, Plot's `rectY`.
    RectYMark,
    MarkKind::RectY
);
mark_builder!(
    /// A cell mark filling one band of each axis, Plot's `cell`.
    CellMark,
    MarkKind::Cell
);
mark_builder!(
    /// A vertical rule at each x, Plot's `ruleX`.
    RuleXMark,
    MarkKind::RuleX
);
mark_builder!(
    /// A horizontal rule at each y, Plot's `ruleY`.
    RuleYMark,
    MarkKind::RuleY
);
mark_builder!(
    /// A text label per datum, Plot's `text`.
    TextMark,
    MarkKind::Text
);
mark_builder!(
    /// A vertical tick per datum, Plot's `tickX`.
    TickXMark,
    MarkKind::TickX
);
mark_builder!(
    /// A horizontal tick per datum, Plot's `tickY`.
    TickYMark,
    MarkKind::TickY
);
mark_builder!(
    /// An arrow between two points, Plot's `vector`.
    VectorMark,
    MarkKind::Vector
);
mark_builder!(
    /// A link mark, Plot's `link`.
    LinkMark,
    MarkKind::Link
);
mark_builder!(
    /// A tree diagram of a slash-separated hierarchy, Plot's `tree`.
    TreeMark,
    MarkKind::Tree
);
mark_builder!(
    /// A raster image painted at a position, Plot's `image`.
    ImageMark,
    MarkKind::Image
);
mark_builder!(
    /// A geographic mark, Plot's `geo`: lon/lat projected equirectangularly.
    GeoMark,
    MarkKind::Geo
);
mark_builder!(
    /// A pinned annotation, Plot's `tip`.
    TipMark,
    MarkKind::Tip
);
mark_builder!(
    /// A text label at each x, Plot's `textX`.
    TextXMark,
    MarkKind::TextX
);
mark_builder!(
    /// A text label at each y, Plot's `textY`.
    TextYMark,
    MarkKind::TextY
);
mark_builder!(
    /// A cell over one x, Plot's `cellX`.
    CellXMark,
    MarkKind::CellX
);
mark_builder!(
    /// A cell over one y, Plot's `cellY`.
    CellYMark,
    MarkKind::CellY
);
mark_builder!(
    /// A vector at each x, Plot's `vectorX`.
    VectorXMark,
    MarkKind::VectorX
);
mark_builder!(
    /// A vector at each y, Plot's `vectorY`.
    VectorYMark,
    MarkKind::VectorY
);
mark_builder!(
    /// A triangular marker, Plot's `spike`.
    SpikeMark,
    MarkKind::Spike
);
mark_builder!(
    /// A curved arrow, Plot's `arrow`.
    ArrowMark,
    MarkKind::Arrow
);
mark_builder!(
    /// A waffle bar along x, Plot's `waffleX`.
    WaffleXMark,
    MarkKind::WaffleX
);
mark_builder!(
    /// A waffle bar along y, Plot's `waffleY`.
    WaffleYMark,
    MarkKind::WaffleY
);
mark_builder!(
    /// A static hex grid, Plot's `hexgrid`.
    HexgridMark,
    MarkKind::Hexgrid
);
mark_builder!(
    /// A pixel grid, Plot's `raster`.
    RasterMark,
    MarkKind::Raster
);
mark_builder!(
    /// A difference chart along x, Plot's `differenceX`.
    DifferenceXMark,
    MarkKind::DifferenceX
);
mark_builder!(
    /// A difference chart along y, Plot's `differenceY`.
    DifferenceYMark,
    MarkKind::DifferenceY
);
mark_builder!(
    /// A regression of x on y, Plot's `linearRegressionX`.
    LinearRegressionXMark,
    MarkKind::LinearRegressionX
);
mark_builder!(
    /// A regression of y on x, Plot's `linearRegressionY`.
    LinearRegressionYMark,
    MarkKind::LinearRegressionY
);
mark_builder!(
    /// A moving mean with deviation bands along x, Plot's `bollingerX`.
    BollingerXMark,
    MarkKind::BollingerX
);
mark_builder!(
    /// A moving mean with deviation bands along y, Plot's `bollingerY`.
    BollingerYMark,
    MarkKind::BollingerY
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_share_channels() {
        let dot = DotMark::new().x("a").y("b").r(5.);
        assert_eq!(dot.kind, MarkKind::Dot);
        assert!(dot.channels.x.is_some());
        assert_eq!(dot.style.radius, 3.);
        assert!(dot.into_mark().channels.y.is_some());
    }

    #[test]
    fn channels_accept_constants_and_accessors() {
        let line = LineMark::new().x(crate::value::Value::Number(1.));
        assert!(line.channels.x.is_some());
    }

    #[test]
    fn stack_and_sort_compose() {
        let area = AreaYMark::new()
            .stack(StackOffset::Normalize)
            .stack_series("group")
            .sort("date", true);
        let stack = area.stack.clone().unwrap();
        assert_eq!(stack.offset, StackOffset::Normalize);
        assert_eq!(stack.series_field.as_deref(), Some("group"));
        assert!(area.sort.as_ref().unwrap().descending);
    }

    #[test]
    fn reverse_flips_sort() {
        let mark = BarYMark::new().reverse();
        assert!(mark.sort.as_ref().unwrap().reverse);
        let mark = mark.reverse();
        assert!(!mark.sort.as_ref().unwrap().reverse);
    }
}
