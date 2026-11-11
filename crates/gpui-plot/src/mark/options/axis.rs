//! The axis, grid and frame marks' options: ticks, tick format, anchor, label.

use gpui::SharedString;

use crate::mark::{Interval, Mark};

impl Mark {
    /// An axis or grid mark's tick count, Plot's `ticks: n`.
    pub fn ticks(mut self, count: usize) -> Self {
        self.axis.ticks = Some(crate::options::Ticks::Count(count));
        self
    }

    /// An axis or grid mark's explicit ticks, Plot's `ticks: [values]`.
    pub fn tick_values<V: Into<crate::value::Value>>(
        mut self,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        self.axis.ticks = Some(crate::options::Ticks::Values(
            values.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// An axis or grid mark's tick interval, Plot's `interval`.
    pub fn tick_interval(mut self, interval: Interval) -> Self {
        self.axis.ticks = Some(crate::options::Ticks::Interval(interval));
        self
    }

    /// An axis mark's `tickFormat`.
    pub fn tick_format(mut self, format: impl Into<crate::options::TickFormat>) -> Self {
        self.axis.tick_format = Some(format.into());
        self
    }

    /// An axis mark's `tickSize`.
    pub fn tick_size(mut self, size: f32) -> Self {
        self.axis.tick_size = Some(size);
        self
    }

    /// An axis mark's `tickPadding`.
    pub fn tick_padding(mut self, padding: f32) -> Self {
        self.axis.tick_padding = Some(padding);
        self
    }

    /// An axis mark's `tickRotate`, in degrees.
    pub fn tick_rotate(mut self, degrees: f32) -> Self {
        self.axis.tick_rotate = Some(degrees);
        self
    }

    /// An axis or frame mark's `anchor`: which side it draws on.
    pub fn axis_anchor(mut self, anchor: crate::options::AxisAnchor) -> Self {
        self.axis.axis = Some(Some(anchor));
        self
    }

    /// An axis mark's `label`.
    pub fn axis_label(mut self, label: impl Into<SharedString>) -> Self {
        self.axis.label = Some(Some(label.into()));
        self
    }

    /// An axis mark's `label: null`.
    pub fn no_axis_label(mut self) -> Self {
        self.axis.label = Some(None);
        self
    }

    /// An axis mark's `labelAnchor`.
    pub fn axis_label_anchor(mut self, anchor: crate::options::LabelAnchor) -> Self {
        self.axis.label_anchor = Some(anchor);
        self
    }

    /// An axis mark's `labelOffset`.
    pub fn axis_label_offset(mut self, offset: f32) -> Self {
        self.axis.label_offset = Some(offset);
        self
    }

    /// An axis mark's `labelArrow`.
    pub fn axis_label_arrow(mut self, arrow: bool) -> Self {
        self.axis.label_arrow = Some(arrow);
        self
    }

    /// An axis mark's `color`: its ticks and labels.
    pub fn color(self, color: &str) -> Self {
        self.stroke(color).fill(color)
    }

    /// Plot's `textStroke` for an axis's labels, a halo.
    pub fn axis_line(mut self, line: bool) -> Self {
        self.axis.line = Some(line);
        self
    }
}
