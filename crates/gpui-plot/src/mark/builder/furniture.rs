//! The typed builders of the furniture marks: axes, grids and the frame.

use super::typed_marks;

typed_marks! {
    /// An x axis drawn as a mark, Plot's `axisX`.
    AxisXMark = AxisX;
    /// A y axis drawn as a mark, Plot's `axisY`.
    AxisYMark = AxisY;
    /// Grid lines across x, Plot's `gridX`.
    GridXMark = GridX;
    /// Grid lines across y, Plot's `gridY`.
    GridYMark = GridY;
    /// The frame's outline, Plot's `frame`.
    FrameMark = Frame;
}
