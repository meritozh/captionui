//! The channels a mark reads, `Channels`.

use crate::style::{ColorChannel, NumberChannel};
use crate::table::Channel;

/// The channels a mark reads, Plot's mark options.
#[derive(Clone, Default)]
pub struct Channels {
    /// The x position.
    pub x: Option<Channel>,
    /// The y position.
    pub y: Option<Channel>,
    /// The x interval's start, Plot's `x1`.
    pub x1: Option<Channel>,
    /// The x interval's end, Plot's `x2`.
    pub x2: Option<Channel>,
    /// The y interval's start, Plot's `y1`.
    pub y1: Option<Channel>,
    /// The y interval's end, Plot's `y2`.
    pub y2: Option<Channel>,
    /// The series channel, Plot's `z`.
    pub z: Option<Channel>,
    /// The fill color.
    pub fill: Option<ColorChannel>,
    /// The stroke color.
    pub stroke: Option<ColorChannel>,
    /// The radius of a dot.
    pub r: Option<NumberChannel>,
    /// A node's slash-separated path in a hierarchy, for the `tree` mark.
    pub path: Option<Channel>,
    /// The text of a label, Plot's `text`.
    pub text: Option<Channel>,
    /// The symbol of a dot, Plot's `symbol`.
    pub symbol: Option<Channel>,
    /// The opacity, Plot's `opacity`.
    pub opacity: Option<NumberChannel>,
    /// A datum's tooltip title.
    pub title: Option<Channel>,
    /// A datum's tooltip body.
    pub href: Option<Channel>,
}

impl std::fmt::Debug for Channels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channels")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish_non_exhaustive()
    }
}

impl Channels {
    /// The x channel a mark positions with: `x`, or `x1` when only an interval
    /// is given.
    pub fn x_channel(&self) -> Option<&Channel> {
        self.x.as_ref().or(self.x1.as_ref()).or(self.x2.as_ref())
    }

    /// The y channel a mark positions with: `y`, or `y1`/`y2`.
    pub fn y_channel(&self) -> Option<&Channel> {
        self.y.as_ref().or(self.y1.as_ref()).or(self.y2.as_ref())
    }
}
