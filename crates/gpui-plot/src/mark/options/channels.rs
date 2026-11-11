//! The data, facets and channels a mark reads: `data`, `fx`, `x`, `fill`, `r`,
//! `text`, `length` …

use gpui::SharedString;

use crate::mark::Mark;
use crate::style::{ColorChannel, NumberChannel};
use crate::table::IntoChannel;

impl Mark {
    /// Give the mark its own data, Plot's `Plot.dot(data, …)`: an array of
    /// records, or of plain values (`[0]` for `Plot.ruleY([0])`), whose
    /// channels default to the value itself.
    pub fn data(mut self, data: impl Into<crate::table::Dataset>) -> Self {
        self.data = Some(data.into());
        self
    }

    /// Facet the mark into columns by a channel, Plot's mark-level `fx`.
    pub fn fx(mut self, channel: impl IntoChannel) -> Self {
        self.fx = Some(channel.into_channel());
        self
    }

    /// Facet the mark into rows by a channel, Plot's mark-level `fy`.
    pub fn fy(mut self, channel: impl IntoChannel) -> Self {
        self.fy = Some(channel.into_channel());
        self
    }

    /// Set the x channel.
    pub fn x(mut self, channel: impl IntoChannel) -> Self {
        self.channels.x = Some(channel.into_channel());
        self
    }

    /// Set the y channel.
    pub fn y(mut self, channel: impl IntoChannel) -> Self {
        self.channels.y = Some(channel.into_channel());
        self
    }

    /// Set the x interval's start.
    pub fn x1(mut self, channel: impl IntoChannel) -> Self {
        self.channels.x1 = Some(channel.into_channel());
        self
    }

    /// Set the x interval's end.
    pub fn x2(mut self, channel: impl IntoChannel) -> Self {
        self.channels.x2 = Some(channel.into_channel());
        self
    }

    /// Set the y interval's start.
    pub fn y1(mut self, channel: impl IntoChannel) -> Self {
        self.channels.y1 = Some(channel.into_channel());
        self
    }

    /// Set the y interval's end.
    pub fn y2(mut self, channel: impl IntoChannel) -> Self {
        self.channels.y2 = Some(channel.into_channel());
        self
    }

    /// Set the series channel.
    pub fn z(mut self, channel: impl IntoChannel) -> Self {
        self.implied_z = false;
        self.channels.z = Some(channel.into_channel());
        self
    }

    /// Set the fill.
    pub fn fill(mut self, color: impl Into<ColorChannel>) -> Self {
        let color = color.into();
        self.series_from_color(&color);
        self.channels.fill = Some(color);
        self
    }

    /// Set the stroke.
    pub fn stroke(mut self, color: impl Into<ColorChannel>) -> Self {
        let color = color.into();
        self.series_from_color(&color);
        self.channels.stroke = Some(color);
        self
    }

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

    /// Set the radius of a dot, in pixels.
    pub fn r(mut self, radius: impl Into<NumberChannel>) -> Self {
        self.channels.r = Some(radius.into());
        self
    }

    /// Set a node's hierarchy path, for the `tree` mark.
    pub fn path(mut self, channel: impl IntoChannel) -> Self {
        self.channels.path = Some(channel.into_channel());
        self
    }

    /// Set the text of a label.
    pub fn text(mut self, channel: impl IntoChannel) -> Self {
        self.channels.text = Some(channel.into_channel());
        self
    }

    /// Set the symbol of a dot.
    pub fn symbol(mut self, channel: impl IntoChannel) -> Self {
        self.channels.symbol = Some(channel.into_channel());
        self
    }

    /// Set the opacity.
    pub fn opacity(mut self, opacity: impl Into<NumberChannel>) -> Self {
        self.channels.opacity = Some(opacity.into());
        self
    }

    /// Set a datum's tooltip title.
    pub fn tip_title(mut self, channel: impl IntoChannel) -> Self {
        self.channels.title = Some(channel.into_channel());
        self
    }

    /// Set the mark's own title, shown in its tooltip and legend.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set a spike's or vector's length, Plot's `length`.
    pub fn length(mut self, pixels: impl Into<NumberChannel>) -> Self {
        self.length = Some(pixels.into());
        self
    }

    /// Set a vector's rotation, Plot's `rotate`, in degrees clockwise.
    pub fn rotate(mut self, degrees: impl Into<NumberChannel>) -> Self {
        self.rotate = Some(degrees.into());
        self
    }
}
