//! Marks' visual style: Plot's `fill`, `stroke`, `r`, `symbol`, `curve` and
//! `opacity` channels.
//!
//! A channel value is a constant or read per datum, the same three forms a
//! positional channel takes. Style values are resolved before painting, so the
//! marks themselves stay stateless.

use std::rc::Rc;

use gpui::{Hsla, PathBuilder, Pixels, Point, px};

use crate::table::Table;
use crate::value::Value;

/// A color channel: one color for every datum, one per datum, or a field
/// whose values the plot's ordinal color scale colors.
///
/// The field form is Plot's `fill: "sex"`: the field also becomes the mark's
/// `z` series (when it has none), so the legend, the tooltip and grouping
/// transforms see the same series the color shows.
#[derive(Clone)]
pub enum ColorChannel {
    Constant(Hsla),
    Accessor(Rc<dyn Fn(&Table) -> Hsla>),
    /// A field colored through the plot's ordinal color scale. Resolve it with
    /// [`crate::render::color::PlotColors::channel`]; [`ColorChannel::read`]
    /// has no scale and returns transparent.
    Field(gpui::SharedString),
}

impl ColorChannel {
    /// The color for `row`, for the constant and accessor forms. A
    /// [`ColorChannel::Field`] needs the plot's color scale, so it reads as
    /// transparent here.
    pub fn read(&self, row: &Table) -> Hsla {
        match self {
            ColorChannel::Constant(color) => *color,
            ColorChannel::Accessor(accessor) => accessor(row),
            ColorChannel::Field(_) => Hsla::transparent_black(),
        }
    }

    /// The field this channel colors by, when it is the field form.
    pub fn field(&self) -> Option<&gpui::SharedString> {
        match self {
            ColorChannel::Field(name) => Some(name),
            _ => None,
        }
    }
}

impl From<&str> for ColorChannel {
    fn from(field: &str) -> Self {
        ColorChannel::Field(field.to_string().into())
    }
}

impl From<gpui::SharedString> for ColorChannel {
    fn from(field: gpui::SharedString) -> Self {
        ColorChannel::Field(field)
    }
}

impl From<Hsla> for ColorChannel {
    fn from(color: Hsla) -> Self {
        ColorChannel::Constant(color)
    }
}

impl<F> From<F> for ColorChannel
where
    F: Fn(&Table) -> Hsla + 'static,
{
    fn from(accessor: F) -> Self {
        ColorChannel::Accessor(Rc::new(accessor))
    }
}

impl std::fmt::Debug for ColorChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorChannel::Constant(color) => f.debug_tuple("Constant").field(color).finish(),
            ColorChannel::Accessor(_) => f.write_str("Accessor(..)"),
            ColorChannel::Field(name) => f.debug_tuple("Field").field(name).finish(),
        }
    }
}

/// A numeric channel: one value for every datum, or one per datum.
#[derive(Clone)]
pub enum NumberChannel {
    Constant(f32),
    Accessor(Rc<dyn Fn(&Table) -> f32>),
}

impl NumberChannel {
    pub fn read(&self, row: &Table) -> f32 {
        match self {
            NumberChannel::Constant(value) => *value,
            NumberChannel::Accessor(accessor) => accessor(row),
        }
    }
}

impl From<f32> for NumberChannel {
    fn from(value: f32) -> Self {
        NumberChannel::Constant(value)
    }
}

impl From<f64> for NumberChannel {
    fn from(value: f64) -> Self {
        NumberChannel::Constant(value as f32)
    }
}

impl From<Pixels> for NumberChannel {
    fn from(value: Pixels) -> Self {
        NumberChannel::Constant(value.as_f32())
    }
}

impl<F> From<F> for NumberChannel
where
    F: Fn(&Table) -> f32 + 'static,
{
    fn from(accessor: F) -> Self {
        NumberChannel::Accessor(Rc::new(accessor))
    }
}

impl std::fmt::Debug for NumberChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumberChannel::Constant(value) => f.debug_tuple("Constant").field(value).finish(),
            NumberChannel::Accessor(_) => f.write_str("Accessor(..)"),
        }
    }
}

/// The symbol a dot mark draws, Plot's `symbol` channel: the shapes
/// `d3-shape`'s symbol generator provides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Symbol {
    /// A filled circle. (Default.)
    #[default]
    Circle,
    /// A hollow circle.
    CircleOpen,
    /// A filled square.
    Square,
    /// A filled diamond.
    Diamond,
    /// A filled triangle pointing up.
    Triangle,
    /// A cross.
    Cross,
    /// A diagonal cross.
    CrossRotate,
    /// A four-pointed star.
    Star,
    /// A three-pointed star.
    Asterisk,
    /// A plus sign.
    Plus,
    /// An x.
    Times,
    /// An arrow pointing up.
    Arrow,
    /// An arrow pointing down.
    TriangleDown,
}

impl Symbol {
    /// The outline of the symbol with the given radius, centered at `center`.
    ///
    /// The points are in the plot's coordinate space, so the caller translates
    /// them by the plot's origin.
    pub fn path(&self, center: Point<Pixels>, radius: f32) -> Vec<Point<Pixels>> {
        let r = radius;
        let (x, y) = (center.x.as_f32(), center.y.as_f32());
        let p = |dx: f32, dy: f32| Point::new(px(x + dx), px(y + dy));
        let k = std::f32::consts::FRAC_1_SQRT_2 * r;

        match self {
            Symbol::Circle | Symbol::CircleOpen => circle_path(center, radius, 32),
            Symbol::Square => vec![p(-r, -r), p(r, -r), p(r, r), p(-r, r)],
            Symbol::Diamond => vec![p(0., -r), p(r, 0.), p(0., r), p(-r, 0.)],
            Symbol::Triangle => vec![p(0., -r), p(r, r * 0.75), p(-r, r * 0.75)],
            Symbol::TriangleDown => vec![p(0., r), p(r, -r * 0.75), p(-r, -r * 0.75)],
            Symbol::Cross => vec![p(-r, 0.), p(r, 0.), p(0., -r), p(0., r)],
            Symbol::CrossRotate => vec![p(-k, -k), p(k, k), p(k, -k), p(-k, k)],
            Symbol::Star => {
                let mut points = Vec::new();
                for i in 0..8 {
                    let angle = std::f32::consts::PI / 2. * i as f32 / 2.;
                    let radius = if i % 2 == 0 { r } else { r / 2. };
                    points.push(p(radius * angle.cos(), radius * angle.sin()));
                }
                points
            }
            Symbol::Asterisk => {
                let mut points = Vec::new();
                for i in 0..6 {
                    let angle = std::f32::consts::PI / 3. * i as f32;
                    points.push(p(r * angle.cos(), r * angle.sin()));
                    points.push(Point::new(px(x), px(y)));
                }
                points
            }
            Symbol::Plus => vec![p(-r, 0.), p(r, 0.), p(0., -r), p(0., r)],
            Symbol::Times => vec![p(-k, -k), p(k, k), p(k, -k), p(-k, k)],
            Symbol::Arrow => vec![p(0., -r), p(r, r), p(0., r * 0.5), p(-r, r)],
        }
    }

    /// Whether the symbol paints a stroke rather than a filled outline.
    pub fn is_line(&self) -> bool {
        matches!(
            self,
            Symbol::Cross
                | Symbol::CrossRotate
                | Symbol::Star
                | Symbol::Asterisk
                | Symbol::Plus
                | Symbol::Times
                | Symbol::Arrow
                | Symbol::CircleOpen
        )
    }
}

fn circle_path(center: Point<Pixels>, radius: f32, segments: usize) -> Vec<Point<Pixels>> {
    (0..segments)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / segments as f32;
            Point::new(
                px(center.x.as_f32() + radius * angle.cos()),
                px(center.y.as_f32() + radius * angle.sin()),
            )
        })
        .collect()
}

/// Build a filled path through `points`, closed.
pub fn filled_path(points: &[Point<Pixels>]) -> Option<gpui::Path<Pixels>> {
    if points.len() < 3 {
        return None;
    }
    let mut builder = PathBuilder::fill();
    builder.add_polygon(points, true);
    builder.build().ok()
}

/// Build a stroked path through `points`, open.
pub fn stroked_path(points: &[Point<Pixels>], width: f32) -> Option<gpui::Path<Pixels>> {
    if points.is_empty() {
        return None;
    }
    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(points[0]);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.build().ok()
}

/// A channel that reads a [`Value`], used by marks whose style depends on the
/// datum's own value.
pub trait ValueStyle {
    fn read(&self, row: &Table) -> Value;
}

impl ValueStyle for &str {
    fn read(&self, row: &Table) -> Value {
        row.value(self)
    }
}

impl<F> ValueStyle for F
where
    F: Fn(&Table) -> Value,
{
    fn read(&self, row: &Table) -> Value {
        self(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px};

    #[test]
    fn symbols_have_outlines() {
        let center = point(px(10.), px(10.));
        assert!(Symbol::Circle.path(center, 4.).len() >= 16);
        assert_eq!(Symbol::Square.path(center, 4.).len(), 4);
        assert_eq!(Symbol::Diamond.path(center, 4.).len(), 4);
        assert!(Symbol::Cross.is_line());
        assert!(!Symbol::Circle.is_line());
    }

    #[test]
    fn paths_build() {
        let center = point(px(0.), px(0.));
        let points = Symbol::Square.path(center, 2.);
        assert!(filled_path(&points).is_some());
        assert!(stroked_path(&points, 1.).is_some());
        assert!(filled_path(&points[..2]).is_none());
    }

    #[test]
    fn channels_read_per_row() {
        let row = Table::empty().with("v", Value::Number(2.));
        let number =
            NumberChannel::Accessor(Rc::new(|row: &Table| row.number("v").unwrap_or(0.) as f32));
        assert_eq!(number.read(&row), 2.);
        assert_eq!(NumberChannel::Constant(1.).read(&row), 1.);
    }
}
