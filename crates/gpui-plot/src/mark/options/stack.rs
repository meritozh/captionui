//! Plot's stack and normalize transforms: `stack`, `stackY`, `normalizeX` …

use gpui::SharedString;

use crate::mark::{Axis, Mark, MarkOptions, StackConfig};
use crate::transform::StackOffset;

/// Plot's stack and normalize transforms: `stack`, `stackY`, `normalizeX` …
pub trait StackOptions: MarkOptions {
    /// Stack the mark's series.
    fn stack(mut self, offset: StackOffset) -> Self {
        let mark = self.mark_mut();
        let existing = mark.stack.take().unwrap_or_default();
        mark.stack = Some(StackConfig { offset, ..existing });
        self
    }

    /// Name the field that separates the mark's series when stacking long
    /// data, Plot's `z`.
    fn stack_series(mut self, field: impl Into<SharedString>) -> Self {
        let mark = self.mark_mut();
        let existing = mark.stack.take().unwrap_or_default();
        mark.stack = Some(StackConfig {
            series_field: Some(field.into()),
            ..existing
        });
        self
    }

    /// Name the value fields to stack when the data is wide, one per series:
    /// Plot's `Plot.stackY({y1: "a", y2: "b"})`.
    fn stack_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SharedString>,
    {
        let mark = self.mark_mut();
        let existing = mark.stack.take().unwrap_or_default();
        mark.stack = Some(StackConfig {
            fields: fields.into_iter().map(Into::into).collect(),
            ..existing
        });
        self
    }

    /// Plot's `stackY`: stack the rows' y values within each x, outputting
    /// `y1`, `y2` and their middle as `y`.
    fn stack_y(mut self) -> Self {
        self.mark_mut().stack_spec = Some(crate::render::stack::StackSpec::new(Axis::Y));
        self
    }

    /// Plot's `stackY1`: as [`Self::stack_y`], with `y` the interval's start.
    fn stack_y1(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::Y);
        spec.anchor = crate::render::stack::StackAnchor::Start;
        self.mark_mut().stack_spec = Some(spec);
        self
    }

    /// Plot's `stackY2`: as [`Self::stack_y`], with `y` the interval's end.
    fn stack_y2(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::Y);
        spec.anchor = crate::render::stack::StackAnchor::End;
        self.mark_mut().stack_spec = Some(spec);
        self
    }

    /// Plot's `stackX`.
    fn stack_x(mut self) -> Self {
        self.mark_mut().stack_spec = Some(crate::render::stack::StackSpec::new(Axis::X));
        self
    }

    /// Plot's `stackX1`.
    fn stack_x1(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::X);
        spec.anchor = crate::render::stack::StackAnchor::Start;
        self.mark_mut().stack_spec = Some(spec);
        self
    }

    /// Plot's `stackX2`.
    fn stack_x2(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::X);
        spec.anchor = crate::render::stack::StackAnchor::End;
        self.mark_mut().stack_spec = Some(spec);
        self
    }

    /// Plot's stack `offset`.
    fn stack_offset(mut self, offset: crate::render::stack::Offset) -> Self {
        self.mark_mut().stack_spec_mut().offset = offset;
        self
    }

    /// Plot's stack `order`.
    fn stack_order(mut self, order: crate::render::stack::StackOrder) -> Self {
        self.mark_mut().stack_spec_mut().order = order;
        self
    }

    /// Plot's stack `reverse`.
    fn stack_reverse(mut self, reverse: bool) -> Self {
        self.mark_mut().stack_spec_mut().reverse = reverse;
        self
    }

    /// Plot's `normalizeX`: divide each series' x by its basis.
    fn normalize_x(mut self, basis: crate::render::stack::NormalizeBasis) -> Self {
        self.mark_mut().normalize_by = Some((Axis::X, basis));
        self
    }

    /// Plot's `normalizeY`.
    fn normalize_y(mut self, basis: crate::render::stack::NormalizeBasis) -> Self {
        self.mark_mut().normalize_by = Some((Axis::Y, basis));
        self
    }
}

impl<T: MarkOptions> StackOptions for T {}

impl Mark {
    fn stack_spec_mut(&mut self) -> &mut crate::render::stack::StackSpec {
        if self.stack_spec.is_none() {
            let axis = match self.kind.category().groups_on_y {
                true => Axis::X,
                false => Axis::Y,
            };
            self.stack_spec = Some(crate::render::stack::StackSpec::new(axis));
        }
        self.stack_spec.as_mut().expect("just set")
    }
}
