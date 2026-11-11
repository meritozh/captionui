//! The typed builders of the radial and statistical marks: box, violin,
//! hexbin, density, contour, pie, arc and waffle.

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
    /// A box-and-whisker summary along x, Plot's `boxX`.
    BoxXMark,
    MarkKind::BoxX
);
mark_builder!(
    /// A box-and-whisker summary along y, Plot's `boxY`.
    BoxYMark,
    MarkKind::BoxY
);
mark_builder!(
    /// A density violin along x, Plot's `violinX`.
    ViolinXMark,
    MarkKind::ViolinX
);
mark_builder!(
    /// A density violin along y, Plot's `violinY`.
    ViolinYMark,
    MarkKind::ViolinY
);
mark_builder!(
    /// A hexagonal binning mark, Plot's `hexbin`.
    HexbinMark,
    MarkKind::Hexbin
);
mark_builder!(
    /// A density estimate along x, Plot's `densityX`.
    DensityXMark,
    MarkKind::DensityX
);
mark_builder!(
    /// A density estimate along y, Plot's `densityY`.
    DensityYMark,
    MarkKind::DensityY
);
mark_builder!(
    /// A contour mark over a binned raster, Plot's `contour`.
    ContourMark,
    MarkKind::Contour
);
mark_builder!(
    /// Two-dimensional density contours, Plot's `density`.
    DensityMark,
    MarkKind::Density
);
mark_builder!(
    /// A pie mark, Plot's `pie`.
    PieMark,
    MarkKind::Pie
);
mark_builder!(
    /// An arc mark, Plot's `arc`.
    ArcMark,
    MarkKind::Arc
);
mark_builder!(
    /// A waffle mark, Plot's `waffle`.
    WaffleMark,
    MarkKind::Waffle
);
