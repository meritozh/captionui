//! Plot's stack and normalize transforms: `stack`, `stackY`, `normalizeX` …

use gpui::SharedString;

use crate::mark::{Axis, Mark, StackConfig};
use crate::transform::StackOffset;

impl Mark {
    /// Stack the mark's series.
    pub fn stack(mut self, offset: StackOffset) -> Self {
        let existing = self.stack.unwrap_or_default();
        self.stack = Some(StackConfig { offset, ..existing });
        self
    }

    /// Name the field that separates the mark's series when stacking long
    /// data, Plot's `z`.
    pub fn stack_series(mut self, field: impl Into<SharedString>) -> Self {
        let existing = self.stack.unwrap_or_default();
        self.stack = Some(StackConfig {
            series_field: Some(field.into()),
            ..existing
        });
        self
    }

    /// Name the value fields to stack when the data is wide, one per series:
    /// Plot's `Plot.stackY({y1: "a", y2: "b"})`.
    pub fn stack_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SharedString>,
    {
        let existing = self.stack.unwrap_or_default();
        self.stack = Some(StackConfig {
            fields: fields.into_iter().map(Into::into).collect(),
            ..existing
        });
        self
    }

    /// Plot's `stackY`: stack the rows' y values within each x, outputting
    /// `y1`, `y2` and their middle as `y`.
    pub fn stack_y(mut self) -> Self {
        self.stack_spec = Some(crate::render::stack::StackSpec::new(Axis::Y));
        self
    }

    /// Plot's `stackY1`: as [`Self::stack_y`], with `y` the interval's start.
    pub fn stack_y1(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::Y);
        spec.anchor = crate::render::stack::StackAnchor::Start;
        self.stack_spec = Some(spec);
        self
    }

    /// Plot's `stackY2`: as [`Self::stack_y`], with `y` the interval's end.
    pub fn stack_y2(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::Y);
        spec.anchor = crate::render::stack::StackAnchor::End;
        self.stack_spec = Some(spec);
        self
    }

    /// Plot's `stackX`.
    pub fn stack_x(mut self) -> Self {
        self.stack_spec = Some(crate::render::stack::StackSpec::new(Axis::X));
        self
    }

    /// Plot's `stackX1`.
    pub fn stack_x1(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::X);
        spec.anchor = crate::render::stack::StackAnchor::Start;
        self.stack_spec = Some(spec);
        self
    }

    /// Plot's `stackX2`.
    pub fn stack_x2(mut self) -> Self {
        let mut spec = crate::render::stack::StackSpec::new(Axis::X);
        spec.anchor = crate::render::stack::StackAnchor::End;
        self.stack_spec = Some(spec);
        self
    }

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

    /// Plot's stack `offset`.
    pub fn stack_offset(mut self, offset: crate::render::stack::Offset) -> Self {
        self.stack_spec_mut().offset = offset;
        self
    }

    /// Plot's stack `order`.
    pub fn stack_order(mut self, order: crate::render::stack::StackOrder) -> Self {
        self.stack_spec_mut().order = order;
        self
    }

    /// Plot's stack `reverse`.
    pub fn stack_reverse(mut self, reverse: bool) -> Self {
        self.stack_spec_mut().reverse = reverse;
        self
    }

    /// Plot's `normalizeX`: divide each series' x by its basis.
    pub fn normalize_x(mut self, basis: crate::render::stack::NormalizeBasis) -> Self {
        self.normalize_by = Some((Axis::X, basis));
        self
    }

    /// Plot's `normalizeY`.
    pub fn normalize_y(mut self, basis: crate::render::stack::NormalizeBasis) -> Self {
        self.normalize_by = Some((Axis::Y, basis));
        self
    }
}
