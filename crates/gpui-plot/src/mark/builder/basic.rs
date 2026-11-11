//! The typed builders of the marks the basic renderer draws: dots, lines, areas,
//! bars, rects, cells, rules, text, ticks, vectors, tips and the rest.

use super::typed_marks;

typed_marks! {
    /// A dot mark, Plot's `dot`: one symbol per datum.
    ///
    /// ```
    /// # use gpui_plot::mark::DotMark;
    /// # use gpui_plot::prelude::*;
    /// DotMark::new().x("date").y("value").r(4.);
    /// ```
    DotMark = Dot;
    /// A dot at each x with no y, Plot's `dotX`.
    DotXMark = DotX;
    /// A dot at each y with no x, Plot's `dotY`.
    DotYMark = DotY;
    /// A line mark, Plot's `line`: a polyline through the data, one per series.
    LineMark = Line;
    /// A line at each x with no y, Plot's `lineX`.
    LineXMark = LineX;
    /// A line at each y with no x, Plot's `lineY`.
    LineYMark = LineY;
    /// A vertical area mark, Plot's `areaY`.
    AreaYMark = AreaY;
    /// A horizontal area mark, Plot's `areaX`.
    AreaXMark = AreaX;
    /// A vertical bar mark, Plot's `barY`.
    BarYMark = BarY;
    /// A horizontal bar mark, Plot's `barX`.
    BarXMark = BarX;
    /// A rectangle mark spanning two intervals, Plot's `rect`.
    RectMark = Rect;
    /// A rectangle over one x, Plot's `rectX`: a horizontal histogram bar.
    RectXMark = RectX;
    /// A rectangle over one y, Plot's `rectY`.
    RectYMark = RectY;
    /// A cell mark filling one band of each axis, Plot's `cell`.
    CellMark = Cell;
    /// A vertical rule at each x, Plot's `ruleX`.
    RuleXMark = RuleX;
    /// A horizontal rule at each y, Plot's `ruleY`.
    RuleYMark = RuleY;
    /// A text label per datum, Plot's `text`.
    TextMark = Text;
    /// A vertical tick per datum, Plot's `tickX`.
    TickXMark = TickX;
    /// A horizontal tick per datum, Plot's `tickY`.
    TickYMark = TickY;
    /// An arrow between two points, Plot's `vector`.
    VectorMark = Vector;
    /// A link mark, Plot's `link`.
    LinkMark = Link;
    /// A tree diagram of a slash-separated hierarchy, Plot's `tree`.
    TreeMark = Tree;
    /// A raster image painted at a position, Plot's `image`.
    ImageMark = Image;
    /// A geographic mark, Plot's `geo`: lon/lat projected equirectangularly.
    GeoMark = Geo;
    /// A pinned annotation, Plot's `tip`.
    TipMark = Tip;
    /// A text label at each x, Plot's `textX`.
    TextXMark = TextX;
    /// A text label at each y, Plot's `textY`.
    TextYMark = TextY;
    /// A cell over one x, Plot's `cellX`.
    CellXMark = CellX;
    /// A cell over one y, Plot's `cellY`.
    CellYMark = CellY;
    /// A vector at each x, Plot's `vectorX`.
    VectorXMark = VectorX;
    /// A vector at each y, Plot's `vectorY`.
    VectorYMark = VectorY;
    /// A triangular marker, Plot's `spike`.
    SpikeMark = Spike;
    /// A curved arrow, Plot's `arrow`.
    ArrowMark = Arrow;
    /// A waffle bar along x, Plot's `waffleX`.
    WaffleXMark = WaffleX;
    /// A waffle bar along y, Plot's `waffleY`.
    WaffleYMark = WaffleY;
    /// A static hex grid, Plot's `hexgrid`.
    HexgridMark = Hexgrid;
    /// A pixel grid, Plot's `raster`.
    RasterMark = Raster;
    /// A difference chart along x, Plot's `differenceX`.
    DifferenceXMark = DifferenceX;
    /// A difference chart along y, Plot's `differenceY`.
    DifferenceYMark = DifferenceY;
    /// A regression of x on y, Plot's `linearRegressionX`.
    LinearRegressionXMark = LinearRegressionX;
    /// A regression of y on x, Plot's `linearRegressionY`.
    LinearRegressionYMark = LinearRegressionY;
    /// A moving mean with deviation bands along x, Plot's `bollingerX`.
    BollingerXMark = BollingerX;
    /// A moving mean with deviation bands along y, Plot's `bollingerY`.
    BollingerYMark = BollingerY;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

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
