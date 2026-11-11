//! The typed builders of the furniture marks: axes, grids and the frame.

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
    /// An x axis drawn as a mark, Plot's `axisX`.
    AxisXMark,
    MarkKind::AxisX
);
mark_builder!(
    /// A y axis drawn as a mark, Plot's `axisY`.
    AxisYMark,
    MarkKind::AxisY
);
mark_builder!(
    /// Grid lines across x, Plot's `gridX`.
    GridXMark,
    MarkKind::GridX
);
mark_builder!(
    /// Grid lines across y, Plot's `gridY`.
    GridYMark,
    MarkKind::GridY
);
mark_builder!(
    /// The frame's outline, Plot's `frame`.
    FrameMark,
    MarkKind::Frame
);
