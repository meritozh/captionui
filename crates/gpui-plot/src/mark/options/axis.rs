//! The axis, grid and frame marks' options: ticks, tick format, anchor, label.

use gpui::SharedString;

use crate::mark::{ChannelOptions, Interval, MarkOptions};

/// The axis, grid and frame marks' options: ticks, tick format, anchor,
/// label.
pub trait AxisOptions: MarkOptions {
    /// An axis or grid mark's tick count, Plot's `ticks: n`.
    fn ticks(mut self, count: usize) -> Self {
        self.mark_mut().axis.ticks = Some(crate::options::Ticks::Count(count));
        self
    }

    /// An axis or grid mark's explicit ticks, Plot's `ticks: [values]`.
    fn tick_values<V: Into<crate::value::Value>>(
        mut self,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        self.mark_mut().axis.ticks = Some(crate::options::Ticks::Values(
            values.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// An axis or grid mark's tick interval, Plot's `interval`.
    fn tick_interval(mut self, interval: Interval) -> Self {
        self.mark_mut().axis.ticks = Some(crate::options::Ticks::Interval(interval));
        self
    }

    /// An axis mark's `tickFormat`.
    fn tick_format(mut self, format: impl Into<crate::options::TickFormat>) -> Self {
        self.mark_mut().axis.tick_format = Some(format.into());
        self
    }

    /// An axis mark's `tickSize`.
    fn tick_size(mut self, size: f32) -> Self {
        self.mark_mut().axis.tick_size = Some(size);
        self
    }

    /// An axis mark's `tickPadding`.
    fn tick_padding(mut self, padding: f32) -> Self {
        self.mark_mut().axis.tick_padding = Some(padding);
        self
    }

    /// An axis mark's `tickRotate`, in degrees.
    fn tick_rotate(mut self, degrees: f32) -> Self {
        self.mark_mut().axis.tick_rotate = Some(degrees);
        self
    }

    /// An axis or frame mark's `anchor`: which side it draws on.
    fn axis_anchor(mut self, anchor: crate::options::AxisAnchor) -> Self {
        self.mark_mut().axis.axis = Some(Some(anchor));
        self
    }

    /// An axis mark's `label`.
    fn axis_label(mut self, label: impl Into<SharedString>) -> Self {
        self.mark_mut().axis.label = Some(Some(label.into()));
        self
    }

    /// An axis mark's `label: null`.
    fn no_axis_label(mut self) -> Self {
        self.mark_mut().axis.label = Some(None);
        self
    }

    /// An axis mark's `labelAnchor`.
    fn axis_label_anchor(mut self, anchor: crate::options::LabelAnchor) -> Self {
        self.mark_mut().axis.label_anchor = Some(anchor);
        self
    }

    /// An axis mark's `labelOffset`.
    fn axis_label_offset(mut self, offset: f32) -> Self {
        self.mark_mut().axis.label_offset = Some(offset);
        self
    }

    /// An axis mark's `labelArrow`.
    fn axis_label_arrow(mut self, arrow: bool) -> Self {
        self.mark_mut().axis.label_arrow = Some(arrow);
        self
    }

    /// An axis mark's `color`: its ticks and labels.
    fn color(self, color: &str) -> Self {
        self.stroke(color).fill(color)
    }

    /// Plot's `textStroke` for an axis's labels, a halo.
    fn axis_line(mut self, line: bool) -> Self {
        self.mark_mut().axis.line = Some(line);
        self
    }
}

impl<T: MarkOptions> AxisOptions for T {}
