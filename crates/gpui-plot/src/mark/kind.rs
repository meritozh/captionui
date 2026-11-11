//! Mark kinds and their classification: `MarkKind`, the one `category` table every
//! shape predicate reads, `MarkFamily`, `MarkCategory` and `CrossAxis`.

use crate::mark::Axis;

/// The mark types this library draws, matching Plot's mark set.
///
/// # Adding a mark kind
///
/// A new kind is classified exactly once, in [`MarkKind::category`], and
/// drawn by one arm in its renderer family. That is the whole contract:
///
/// 1. add the variant here,
/// 2. give it a name in [`MarkKind::name`],
/// 3. classify it in [`MarkKind::category`] (the compiler makes this
///    exhaustive, so a forgotten kind is a compile error, not silence),
/// 4. pick a renderer by its [`MarkFamily`] in [`MarkCategory`], and draw it
///    with one arm in `render/basic.rs`, `render/radial.rs` or
///    `render/delaunay.rs`.
///
/// Every predicate (`is_bivariate`, `is_x_only`, …), the band-and-bin
/// decisions and the paint-family dispatch derive from the one [`category`]
/// table, so nothing else needs to change for a new kind.
///
/// [`category`]: MarkKind::category
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    /// A dot per datum, Plot's `dot`.
    Dot,
    /// A dot at each x with no y, Plot's `dotX`: a one-dimensional
    /// distribution, laid along the frame's middle.
    DotX,
    /// A dot at each y with no x, Plot's `dotY`.
    DotY,
    /// A line through the data, grouped by `z`, Plot's `line`.
    Line,
    /// A line at each x with no y, Plot's `lineX`.
    LineX,
    /// A line at each y with no x, Plot's `lineY`.
    LineY,
    /// A filled area under a line, Plot's `areaY`.
    AreaY,
    /// A filled area left of a line, Plot's `areaX`.
    AreaX,
    /// A vertical bar per datum, Plot's `barY`.
    BarY,
    /// A horizontal bar per datum, Plot's `barX`.
    BarX,
    /// A rectangle spanning two x values and two y values, Plot's `rect`.
    Rect,
    /// A rectangle over one x with no y, Plot's `rectX`: a horizontal
    /// histogram bar grown from zero.
    RectX,
    /// A rectangle over one y with no x, Plot's `rectY`.
    RectY,
    /// A rectangle filling a band cell, Plot's `cell`.
    Cell,
    /// A vertical line at each x, Plot's `ruleX`.
    RuleX,
    /// A horizontal line at each y, Plot's `ruleY`.
    RuleY,
    /// A text label per datum, Plot's `text`.
    Text,
    /// A short vertical tick per datum, Plot's `tickX`.
    TickX,
    /// A short horizontal tick per datum, Plot's `tickY`.
    TickY,
    /// An arrow from one point to another, Plot's `vector`.
    Vector,
    /// A box-and-whisker summary along x, Plot's `boxX`.
    BoxX,
    /// A box-and-whisker summary along y, Plot's `boxY`.
    BoxY,
    /// A kernel-density violin along x, Plot's `violinX`.
    ViolinX,
    /// A kernel-density violin along y, Plot's `violinY`.
    ViolinY,
    /// Hexagonal binning of two quantitative channels, Plot's `hexbin`.
    Hexbin,
    /// A kernel density estimate along x, Plot's `densityX`.
    DensityX,
    /// A kernel density estimate along y, Plot's `densityY`.
    DensityY,
    /// Contour lines over a raster, Plot's `contour`.
    Contour,
    /// Two-dimensional density contours of x and y, Plot's `density`.
    Density,
    /// A pie chart of one quantitative channel, Plot's `pie`.
    Pie,
    /// An arc chart, Plot's `arc`.
    Arc,
    /// A unit-square waffle chart, Plot's `waffle`.
    Waffle,
    /// A curved link between two points, Plot's `link`.
    Link,
    /// A pinned annotation at a point, Plot's `tip`: the hover tooltip drawn
    /// permanently at one datum.
    Tip,
    /// An x axis drawn as a mark, Plot's `axisX`, for a plot that wants its
    /// axes inside the frame or in a color of its own.
    AxisX,
    /// A y axis drawn as a mark, Plot's `axisY`.
    AxisY,
    /// Grid lines across x drawn as a mark, Plot's `gridX`.
    GridX,
    /// Grid lines across y drawn as a mark, Plot's `gridY`.
    GridY,
    /// The frame's outline drawn as a mark, Plot's `frame`.
    Frame,
    /// A text label at each x with no y, Plot's `textX`.
    TextX,
    /// A text label at each y with no x, Plot's `textY`.
    TextY,
    /// A cell over one x with no y, Plot's `cellX`.
    CellX,
    /// A cell over one y with no x, Plot's `cellY`.
    CellY,
    /// A vector at each x, Plot's `vectorX`.
    VectorX,
    /// A vector at each y, Plot's `vectorY`.
    VectorY,
    /// A triangular marker at a datum, Plot's `spike`.
    Spike,
    /// A curved arrow between two points, Plot's `arrow`.
    Arrow,
    /// A waffle bar along x, Plot's `waffleX`.
    WaffleX,
    /// A waffle bar along y, Plot's `waffleY`.
    WaffleY,
    /// A static hexagonal grid, Plot's `hexgrid`.
    Hexgrid,
    /// A grid of pixels colored from the data, Plot's `raster` and `image`.
    Raster,
    /// Two areas clipped to the sign of their difference, Plot's
    /// `differenceX`.
    DifferenceX,
    /// The y-axis form of `differenceX`, Plot's `differenceY`.
    DifferenceY,
    /// A least-squares fit of x on y, Plot's `linearRegressionX`.
    LinearRegressionX,
    /// A least-squares fit of y on x, Plot's `linearRegressionY`.
    LinearRegressionY,
    /// A moving mean with deviation bands along x, Plot's `bollingerX`.
    BollingerX,
    /// The y-axis form of `bollingerX`, Plot's `bollingerY`.
    BollingerY,
    /// A Delaunay triangulation drawn as links, Plot's `delaunayLink`.
    DelaunayLink,
    /// A Delaunay triangulation drawn as one mesh, Plot's `delaunayMesh`.
    DelaunayMesh,
    /// The convex hull of the data, Plot's `hull`.
    Hull,
    /// A Voronoi cell per datum, Plot's `voronoi`.
    Voronoi,
    /// The Voronoi mesh of the data, Plot's `voronoiMesh`.
    VoronoiMesh,
    /// A tree diagram of a slash-separated hierarchy, Plot's `tree`.
    Tree,
    /// A raster image painted at a position, Plot's `image`.
    Image,
    /// A geographic mark, Plot's `geo`: longitude/latitude projected
    /// equirectangularly into the plot.
    Geo,
}

/// Which renderer family paints a mark.
///
/// The primitive marks draw one shape per row; the radial and statistical
/// marks summarize or arrange their rows first; the Delaunay marks build a
/// lattice over the projected points. The family picks the [`render`] module
/// that draws the mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkFamily {
    /// The primitive row-shape marks: dot, line, area, bar, rect, cell, rule,
    /// text, tick, vector, spike, arrow, link, the waffle bars, hexgrid,
    /// raster and the regression marks.
    Basic,
    /// The radial and statistical marks: pie, arc, waffle, box, violin,
    /// density, hexbin and contour.
    Radial,
    /// The geometric marks over a projected point set: the Delaunay family.
    Delaunay,
}

/// The cross-cutting shape of a mark kind: every fact that is true of all of a
/// kind's marks, decided in one place.
///
/// A [`MarkKind`] sets this table once in [`MarkKind::category`], and every
/// predicate and dispatch that reads a kind's shape derives from it. Historically
/// these were six hand-maintained `matches!` whitelists across the crate, each
/// silently returning `false` for a new kind until remembered — a mark could
/// be misclassified without an error. Here a new kind is classified exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkCategory {
    /// Which renderer family paints the mark.
    pub family: MarkFamily,
    /// Whether the mark is static plot furniture — an axis, a grid or the
    /// frame — which reads no channels and resolves no rows.
    pub furniture: bool,
    /// Whether the mark reads two positional channels (`x` and `y`).
    pub bivariate: bool,
    /// Whether the mark grows from a baseline or angle on its own axis: a
    /// bar, a pie, an arc.
    pub length: bool,
    /// The axis a one-dimensional mark reads, or `None` if it reads both.
    pub single_axis: Option<Axis>,
    /// How a one-dimensional mark sits across the axis it does not read: in
    /// the frame's middle, or spread by the datum's index.
    pub cross: CrossAxis,
    /// Whether the mark fills the width of a band when the x scale is discrete.
    pub band_x: bool,
    /// Whether the mark fills the height of a band when the y scale is discrete.
    pub band_y: bool,
    /// Whether a binned mark counts along x (a vertical bin) rather than y.
    /// `None` for a mark with no fixed bin axis: the mark bins along whatever
    /// channel it names, the way Plot's `bin` infers it from the mark.
    pub bins_on_x: Option<bool>,
    /// Whether a stacked mark's categories run along y — a horizontal mark.
    pub groups_on_y: bool,
}

impl Default for MarkCategory {
    fn default() -> Self {
        Self {
            family: MarkFamily::Basic,
            furniture: false,
            bivariate: true,
            length: false,
            single_axis: None,
            cross: CrossAxis::Index,
            band_x: false,
            band_y: false,
            bins_on_x: None,
            groups_on_y: false,
        }
    }
}

impl MarkKind {
    /// The name Plot uses for this mark.
    pub fn name(self) -> &'static str {
        match self {
            MarkKind::Dot => "dot",
            MarkKind::DotX => "dotX",
            MarkKind::DotY => "dotY",
            MarkKind::Line => "line",
            MarkKind::LineX => "lineX",
            MarkKind::LineY => "lineY",
            MarkKind::AreaY => "areaY",
            MarkKind::AreaX => "areaX",
            MarkKind::BarY => "barY",
            MarkKind::BarX => "barX",
            MarkKind::Rect => "rect",
            MarkKind::RectX => "rectX",
            MarkKind::RectY => "rectY",
            MarkKind::Cell => "cell",
            MarkKind::RuleX => "ruleX",
            MarkKind::RuleY => "ruleY",
            MarkKind::Text => "text",
            MarkKind::TickX => "tickX",
            MarkKind::TickY => "tickY",
            MarkKind::Vector => "vector",
            MarkKind::BoxX => "boxX",
            MarkKind::BoxY => "boxY",
            MarkKind::ViolinX => "violinX",
            MarkKind::ViolinY => "violinY",
            MarkKind::Hexbin => "hexbin",
            MarkKind::DensityX => "densityX",
            MarkKind::DensityY => "densityY",
            MarkKind::Contour => "contour",
            MarkKind::Density => "density",
            MarkKind::Pie => "pie",
            MarkKind::Arc => "arc",
            MarkKind::Waffle => "waffle",
            MarkKind::Link => "link",
            MarkKind::Tip => "tip",
            MarkKind::AxisX => "axisX",
            MarkKind::AxisY => "axisY",
            MarkKind::GridX => "gridX",
            MarkKind::GridY => "gridY",
            MarkKind::Frame => "frame",
            MarkKind::TextX => "textX",
            MarkKind::TextY => "textY",
            MarkKind::CellX => "cellX",
            MarkKind::CellY => "cellY",
            MarkKind::VectorX => "vectorX",
            MarkKind::VectorY => "vectorY",
            MarkKind::Spike => "spike",
            MarkKind::Arrow => "arrow",
            MarkKind::WaffleX => "waffleX",
            MarkKind::WaffleY => "waffleY",
            MarkKind::Hexgrid => "hexgrid",
            MarkKind::Raster => "raster",
            MarkKind::DifferenceX => "differenceX",
            MarkKind::DifferenceY => "differenceY",
            MarkKind::LinearRegressionX => "linearRegressionX",
            MarkKind::LinearRegressionY => "linearRegressionY",
            MarkKind::BollingerX => "bollingerX",
            MarkKind::BollingerY => "bollingerY",
            MarkKind::DelaunayLink => "delaunayLink",
            MarkKind::DelaunayMesh => "delaunayMesh",
            MarkKind::Hull => "hull",
            MarkKind::Voronoi => "voronoi",
            MarkKind::VoronoiMesh => "voronoiMesh",
            MarkKind::Tree => "tree",
            MarkKind::Image => "image",
            MarkKind::Geo => "geo",
        }
    }

    /// The cross-cutting shape of this mark.
    ///
    /// This one table is the single authority for every fact about a kind's
    /// shape; the predicates below and the renderer's dispatch read from it
    /// rather than re-declaring `matches!` whitelists. A new kind is
    /// classified exactly once, here, instead of in half a dozen hand-maintained
    /// lists that each silently default to `false` for a kind they forget.
    pub fn category(self) -> MarkCategory {
        use MarkFamily::*;
        use MarkKind::*;

        // The defaults: a primitive mark reading both axes and drawing one
        // shape per row.
        let basic = MarkCategory::default();
        let radial = MarkCategory {
            family: Radial,
            ..Default::default()
        };
        let delaunay = MarkCategory {
            family: Delaunay,
            ..Default::default()
        };
        // Furniture draws no data, so it reads no channels and holds no rows.
        let furniture = MarkCategory {
            furniture: true,
            bivariate: false,
            ..Default::default()
        };

        // One-dimensional marks place themselves across the axis they do not
        // read. A one-dimensional *dot* or *line* still resolves both channels
        // (its `y`/`x` is the index or the frame's middle), so it stays
        // bivariate; a one-dimensional label, cell or vector is not.
        let axis = |single_axis, cross| MarkCategory {
            single_axis,
            cross,
            // DotX/DotY/LineX/LineY resolve a cross channel of their own, so
            // they stay bivariate; the callers below for labels and cells set
            // it false explicitly.
            ..Default::default()
        };
        let univariate = |single_axis, cross| MarkCategory {
            single_axis,
            cross,
            bivariate: false,
            ..Default::default()
        };
        let anchor = CrossAxis::Anchor;
        let index = CrossAxis::Index;

        match self {
            // Primitive row-shape marks: one shape per datum.
            Dot => basic,
            DotX => axis(Some(Axis::X), anchor),
            DotY => axis(Some(Axis::Y), anchor),
            Line => basic,
            LineX => axis(Some(Axis::X), index),
            LineY => axis(Some(Axis::Y), index),
            // A horizontal mark's categories run along y — the axis it stacks
            // on — so its groups sit on y and its values run along x.
            AreaX => MarkCategory {
                groups_on_y: true,
                ..basic
            },
            AreaY => basic,
            BarY => MarkCategory {
                length: true,
                band_x: true,
                bins_on_x: Some(true),
                ..basic
            },
            // A horizontal bar is a length mark along x and takes a y band.
            BarX => MarkCategory {
                length: true,
                band_y: true,
                groups_on_y: true,
                bins_on_x: Some(false),
                ..basic
            },
            // `rectX` and the horizontal waffle bar take a y band but are not
            // stacked by the mark stack transform, which recognizes only the
            // bar and area families.
            RectX => MarkCategory {
                single_axis: Some(Axis::X),
                band_y: true,
                bins_on_x: Some(false),
                ..basic
            },
            Rect => basic,
            // A one-axis rect grows from the baseline like a bar; `rectX` bins
            // along y (counts on x are a horizontal histogram) and `rectY` the
            // reverse.
            RectY => MarkCategory {
                single_axis: Some(Axis::Y),
                band_x: true,
                bins_on_x: Some(true),
                ..basic
            },
            Cell => MarkCategory {
                band_x: true,
                band_y: true,
                ..basic
            },
            RuleX | RuleY => basic,
            Text => basic,
            // A tick's band sits on the axis it lays against: a `tickX` draws
            // its ticks apart along y, a `tickY` along x.
            TickX => MarkCategory {
                band_y: true,
                ..basic
            },
            TickY => MarkCategory {
                band_x: true,
                ..basic
            },
            // A vector reads an (x, y) point; a spike or arrow reads two ends
            // of its own, so it resolves its own geometry.
            Vector => basic,
            Spike | Arrow => MarkCategory {
                bivariate: false,
                ..basic
            },
            // Radial and statistical marks. A pie, arc or waffle reads a length
            // (an angle or a count), not a second position channel, so it is
            // not bivariate.
            Pie | Arc | Waffle => MarkCategory {
                family: Radial,
                length: true,
                bivariate: false,
                ..Default::default()
            },
            BoxX | ViolinX | DensityX => MarkCategory {
                family: Radial,
                band_y: true,
                ..basic
            },
            BoxY | ViolinY | DensityY => MarkCategory {
                family: Radial,
                band_x: true,
                ..basic
            },
            Hexbin | Contour | Density => radial,
            Link => basic,
            Tip => MarkCategory {
                bivariate: false,
                ..basic
            },
            AxisX | AxisY | GridX | GridY | Frame => furniture,
            // One-dimensional labels, cells and vectors read one channel only,
            // so they are not bivariate and sit in the frame's middle (a label
            // or vector) or spread by index (a cell).
            TextX => univariate(Some(Axis::X), anchor),
            TextY => univariate(Some(Axis::Y), anchor),
            // Plot's cellX has no y: its cells span the frame's height.
            CellX => univariate(Some(Axis::X), anchor),
            CellY => univariate(Some(Axis::Y), anchor),
            VectorX => univariate(Some(Axis::X), anchor),
            VectorY => univariate(Some(Axis::Y), anchor),
            // The oblique waffle bars draw through the basic renderer and take
            // a band, but are not a length mark in the `is_length_mark` sense.
            WaffleX => MarkCategory {
                band_y: true,
                bivariate: false,
                ..basic
            },
            WaffleY => MarkCategory {
                band_x: true,
                bivariate: false,
                ..basic
            },
            // The grid-like and fitted marks draw through the basic renderer
            // but summarize or arrange their rows rather than drawing one
            // shape per datum.
            Hexgrid | Raster | DifferenceX | DifferenceY | LinearRegressionX
            | LinearRegressionY | BollingerX | BollingerY => MarkCategory {
                bivariate: false,
                ..basic
            },
            // The Delaunay marks triangulate the projected points.
            DelaunayLink | DelaunayMesh | Hull | Voronoi | VoronoiMesh => delaunay,
            // The tree reads no positional channels: it lays its own hierarchy
            // out from a `path` field, so it is not bivariate and draws through
            // the basic renderer.
            Tree => MarkCategory {
                bivariate: false,
                ..Default::default()
            },
            // An image is positioned by its own width and height, not by two
            // positional channels, so it is not bivariate.
            Image => MarkCategory {
                bivariate: false,
                ..Default::default()
            },
            // A geo mark reads lon/lat and projects them itself, so it does not
            // go through the frame's linear scales as a bivariate mark.
            Geo => MarkCategory {
                bivariate: false,
                ..Default::default()
            },
        }
    }

    /// Whether this mark draws a part of the plot's furniture — an axis, a
    /// grid or the frame — rather than data, so it reads no channels.
    pub fn is_furniture(self) -> bool {
        self.category().furniture
    }

    /// Whether this mark reads two positional channels (`x` and `y`) rather
    /// than one axis plus a length.
    pub fn is_bivariate(self) -> bool {
        self.category().bivariate
    }

    /// Whether this mark reads a length channel on its own axis: a bar grows
    /// from a baseline, a pie takes an angle.
    pub fn is_length_mark(self) -> bool {
        self.category().length
    }

    /// Whether this mark lays its data out along the x axis only, with no y
    /// channel of its own: Plot's `dotX`, `lineX`, `rectX`.
    pub fn is_x_only(self) -> bool {
        self.category().single_axis == Some(Axis::X)
    }

    /// Whether this mark lays its data out along the y axis only.
    pub fn is_y_only(self) -> bool {
        self.category().single_axis == Some(Axis::Y)
    }

    /// How a mark with one position channel places itself across the axis it
    /// does not read.
    pub fn cross_axis(self) -> CrossAxis {
        self.category().cross
    }
}

/// Where a one-dimensional mark puts itself on the axis it does not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossAxis {
    /// The frame's middle, Plot's `dotX`.
    Anchor,
    /// The datum's index, Plot's `lineX` and `rectX`.
    Index,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_classifies_every_kind() {
        use MarkFamily::*;
        use MarkKind::*;
        // Every mark is classified exactly once, in `MarkKind::category`. This
        // test pins the classification so a new kind or a regression can't
        // silently reclassify a mark. We assert the *derived predicates* hold,
        // because those are the contract the renderer relies on.
        let bivariate = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_bivariate());
        let x_only = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_x_only());
        let y_only = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_y_only());
        let furniture = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_furniture());
        let length = |kinds: &[MarkKind]| kinds.iter().all(|k| k.is_length_mark());
        let family = |kinds: &[MarkKind], f| kinds.iter().all(|k| k.category().family == f);

        assert!(bivariate(&[
            Dot,
            DotX,
            DotY,
            Line,
            LineX,
            LineY,
            AreaY,
            AreaX,
            BarY,
            BarX,
            Rect,
            RectY,
            RectX,
            Cell,
            RuleX,
            RuleY,
            Text,
            TickX,
            TickY,
            Vector,
            BoxX,
            BoxY,
            ViolinX,
            ViolinY,
            Hexbin,
            DensityX,
            DensityY,
            Contour,
            Density,
            Link,
            DelaunayLink,
            DelaunayMesh,
            Hull,
            Voronoi,
            VoronoiMesh
        ]));
        // A one-dimensional label, cell or vector reads one channel only; a
        // bar's baseline and the radial marks read a length, not a second
        // position.
        assert!(!bivariate(&[
            TextX,
            TextY,
            CellX,
            CellY,
            VectorX,
            VectorY,
            Tip,
            Spike,
            Arrow,
            Hexgrid,
            Raster,
            Pie,
            Arc,
            Waffle,
            WaffleX,
            WaffleY,
            AxisX,
            AxisY,
            GridX,
            GridY,
            Frame,
            DifferenceX,
            DifferenceY,
            LinearRegressionX,
            LinearRegressionY,
            BollingerX,
            BollingerY
        ]));

        assert!(x_only(&[DotX, LineX, RectX, TextX, CellX, VectorX]));
        assert!(y_only(&[DotY, LineY, RectY, TextY, CellY, VectorY]));

        assert!(furniture(&[AxisX, AxisY, GridX, GridY, Frame]));
        assert!(!furniture(&[Dot, Line, Tip]));

        assert!(length(&[BarY, BarX, Pie, Arc, Waffle]));
        assert!(!length(&[RectX, Cell, WaffleX, WaffleY, Line]));

        // The renderer family drives which module draws the mark.
        assert!(family(
            &[
                Pie, Arc, Waffle, BoxX, BoxY, ViolinX, ViolinY, DensityX, DensityY, Hexbin,
                Contour, Density
            ],
            Radial
        ));
        assert!(family(
            &[DelaunayLink, DelaunayMesh, Hull, Voronoi, VoronoiMesh],
            Delaunay
        ));
        assert!(family(
            &[Dot, Line, BarY, Cell, Raster, Hexgrid, Rect, Link],
            Basic
        ));

        // Cross-axis anchoring: one-dimensional dots, labels and vectors sit
        // in the frame's middle; lines, cells and rects spread by index.
        assert_eq!(DotX.cross_axis(), CrossAxis::Anchor);
        assert_eq!(TextY.cross_axis(), CrossAxis::Anchor);
        assert_eq!(VectorX.cross_axis(), CrossAxis::Anchor);
        assert_eq!(LineX.cross_axis(), CrossAxis::Index);
        assert_eq!(CellY.cross_axis(), CrossAxis::Anchor);
        assert_eq!(RectX.cross_axis(), CrossAxis::Index);
    }
}
