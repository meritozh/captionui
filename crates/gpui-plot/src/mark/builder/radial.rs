//! The typed builders of the radial and statistical marks: box, violin,
//! hexbin, density, contour, pie, arc and waffle.

use super::typed_marks;

typed_marks! {
    /// A box-and-whisker summary along x, Plot's `boxX`.
    BoxXMark = BoxX;
    /// A box-and-whisker summary along y, Plot's `boxY`.
    BoxYMark = BoxY;
    /// A density violin along x, Plot's `violinX`.
    ViolinXMark = ViolinX;
    /// A density violin along y, Plot's `violinY`.
    ViolinYMark = ViolinY;
    /// A hexagonal binning mark, Plot's `hexbin`.
    HexbinMark = Hexbin;
    /// A density estimate along x, Plot's `densityX`.
    DensityXMark = DensityX;
    /// A density estimate along y, Plot's `densityY`.
    DensityYMark = DensityY;
    /// A contour mark over a binned raster, Plot's `contour`.
    ContourMark = Contour;
    /// Two-dimensional density contours, Plot's `density`.
    DensityMark = Density;
    /// A pie mark, Plot's `pie`.
    PieMark = Pie;
    /// An arc mark, Plot's `arc`.
    ArcMark = Arc;
    /// A waffle mark, Plot's `waffle`.
    WaffleMark = Waffle;
}
