//! The data, facets and channels a mark reads: `data`, `fx`, `x`, `fill`, `r`,
//! `text`, `length` …

use gpui::SharedString;

use crate::mark::{Mark, MarkOptions};
use crate::style::{ColorChannel, NumberChannel};
use crate::table::IntoChannel;

/// The data, facets and channels a mark reads, Plot's `data`, `fx`, `x`,
/// `fill`, `r`, `text`, `length` …
pub trait ChannelOptions: MarkOptions {
    /// Give the mark its own data, Plot's `Plot.dot(data, …)`: an array of
    /// records, or of plain values (`[0]` for `Plot.ruleY([0])`), whose
    /// channels default to the value itself.
    fn data(mut self, data: impl Into<crate::table::Dataset>) -> Self {
        self.mark_mut().data = Some(data.into());
        self
    }

    /// Facet the mark into columns by a channel, Plot's mark-level `fx`.
    fn fx(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().fx = Some(channel.into_channel());
        self
    }

    /// Facet the mark into rows by a channel, Plot's mark-level `fy`.
    fn fy(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().fy = Some(channel.into_channel());
        self
    }

    /// Set the x channel.
    fn x(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.x = Some(channel.into_channel());
        self
    }

    /// Set the y channel.
    fn y(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.y = Some(channel.into_channel());
        self
    }

    /// Set the x interval's start.
    fn x1(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.x1 = Some(channel.into_channel());
        self
    }

    /// Set the x interval's end.
    fn x2(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.x2 = Some(channel.into_channel());
        self
    }

    /// Set the y interval's start.
    fn y1(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.y1 = Some(channel.into_channel());
        self
    }

    /// Set the y interval's end.
    fn y2(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.y2 = Some(channel.into_channel());
        self
    }

    /// Set the series channel.
    fn z(mut self, channel: impl IntoChannel) -> Self {
        let mark = self.mark_mut();
        mark.implied_z = false;
        mark.channels.z = Some(channel.into_channel());
        self
    }

    /// Set the fill.
    fn fill(mut self, color: impl Into<ColorChannel>) -> Self {
        let color = color.into();
        let mark = self.mark_mut();
        mark.series_from_color(&color);
        mark.channels.fill = Some(color);
        self
    }

    /// Set the stroke.
    fn stroke(mut self, color: impl Into<ColorChannel>) -> Self {
        let color = color.into();
        let mark = self.mark_mut();
        mark.series_from_color(&color);
        mark.channels.stroke = Some(color);
        self
    }

    /// Set the radius of a dot, in pixels.
    fn r(mut self, radius: impl Into<NumberChannel>) -> Self {
        self.mark_mut().channels.r = Some(radius.into());
        self
    }

    /// Set a node's hierarchy path, for the `tree` mark.
    fn path(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.path = Some(channel.into_channel());
        self
    }

    /// Set the text of a label.
    fn text(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.text = Some(channel.into_channel());
        self
    }

    /// Set the symbol of a dot.
    fn symbol(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.symbol = Some(channel.into_channel());
        self
    }

    /// Set the opacity.
    fn opacity(mut self, opacity: impl Into<NumberChannel>) -> Self {
        self.mark_mut().channels.opacity = Some(opacity.into());
        self
    }

    /// Set a datum's tooltip title.
    fn tip_title(mut self, channel: impl IntoChannel) -> Self {
        self.mark_mut().channels.title = Some(channel.into_channel());
        self
    }

    /// Set the mark's own title, shown in its tooltip and legend.
    fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.mark_mut().title = Some(title.into());
        self
    }

    /// Set a spike's or vector's length, Plot's `length`.
    fn length(mut self, pixels: impl Into<NumberChannel>) -> Self {
        self.mark_mut().length = Some(pixels.into());
        self
    }

    /// Set a vector's rotation, Plot's `rotate`, in degrees clockwise.
    fn rotate(mut self, degrees: impl Into<NumberChannel>) -> Self {
        self.mark_mut().rotate = Some(degrees.into());
        self
    }
}

impl<T: MarkOptions> ChannelOptions for T {}

impl Mark {
    /// A color named by field is also the mark's series, Plot's implicit
    /// `z`: `fill: "sex"` groups, stacks and legends by sex.
    fn series_from_color(&mut self, color: &ColorChannel) {
        // Plot's `maybeZ`: a color field is the series only when its values
        // are ordinal, which the rows decide (see `implied_z`).
        if let Some(field) = color.field()
            && self.channels.z.is_none()
        {
            self.channels.z = Some(crate::table::Channel::Field(field.clone()));
            self.implied_z = true;
        } else if let ColorChannel::Value(accessor) = color
            && self.channels.z.is_none()
        {
            // An accessor is a channel too: `stroke: (d) => d.sensor`.
            self.channels.z = Some(crate::table::Channel::Accessor(accessor.clone()));
            self.implied_z = true;
        }
    }
}
