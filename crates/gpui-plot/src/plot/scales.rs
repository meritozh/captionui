//! The per-scale options, Plot's `x: {…}`, `color: {…}` and so on, and
//! the builders that set them.

use crate::plot::Plot;

/// Every scale's options, Plot's `x`, `y`, `fx`, `fy`, `color`, `r`,
/// `opacity`, `symbol` and `length` objects.
#[derive(Clone, Debug, Default)]
pub struct Scales {
    pub x: crate::options::ScaleOptions,
    pub y: crate::options::ScaleOptions,
    pub fx: crate::options::ScaleOptions,
    pub fy: crate::options::ScaleOptions,
    pub color: crate::options::ScaleOptions,
    pub r: crate::options::ScaleOptions,
    pub opacity: crate::options::ScaleOptions,
    pub symbol: crate::options::ScaleOptions,
    pub length: crate::options::ScaleOptions,
}

impl Plot {
    /// Plot's `x: {…}` scale options.
    pub fn x(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.x = f(std::mem::take(&mut self.scales.x));
        self
    }

    /// Plot's `y: {…}` scale options.
    pub fn y(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.y = f(std::mem::take(&mut self.scales.y));
        self
    }

    /// Plot's `fx: {…}` scale options.
    pub fn fx_scale(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.fx = f(std::mem::take(&mut self.scales.fx));
        self
    }

    /// Plot's `fy: {…}` scale options.
    pub fn fy_scale(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.fy = f(std::mem::take(&mut self.scales.fy));
        self
    }

    /// Plot's `color: {…}` scale options.
    pub fn color(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.color = f(std::mem::take(&mut self.scales.color));
        self
    }

    /// Plot's `r: {…}` scale options.
    pub fn r(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.r = f(std::mem::take(&mut self.scales.r));
        self
    }

    /// Plot's `opacity: {…}` scale options.
    pub fn opacity(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.opacity = f(std::mem::take(&mut self.scales.opacity));
        self
    }

    /// Plot's `symbol: {…}` scale options.
    pub fn symbol(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.symbol = f(std::mem::take(&mut self.scales.symbol));
        self
    }

    /// Plot's `length: {…}` scale options.
    pub fn length(
        mut self,
        f: impl FnOnce(crate::options::ScaleOptions) -> crate::options::ScaleOptions,
    ) -> Self {
        self.scales.length = f(std::mem::take(&mut self.scales.length));
        self
    }

    /// The scale options set so far.
    pub fn scales(&self) -> &Scales {
        &self.scales
    }
}
