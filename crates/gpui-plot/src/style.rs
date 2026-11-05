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
    /// Plot's `currentColor`: the theme's foreground.
    Current,
    /// An accessor whose values go through the plot's color scale, Plot's
    /// `stroke: (d) => d.Close - d.Open`.
    Value(Rc<dyn Fn(&Table) -> Value>),
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
            ColorChannel::Current => Hsla::black(),
            ColorChannel::Value(_) => Hsla::transparent_black(),
        }
    }

    /// The field this channel colors by, when it is the field form.
    pub fn field(&self) -> Option<&gpui::SharedString> {
        match self {
            ColorChannel::Field(name) => Some(name),
            _ => None,
        }
    }

    /// The data value this channel reads for the color scale: a field's value
    /// or a value accessor's result. `None` for a constant or color accessor.
    pub fn scaled_value(&self, row: &Table) -> Option<Value> {
        match self {
            ColorChannel::Field(name) => Some(row.value(name)),
            ColorChannel::Value(accessor) => Some(accessor(row)),
            _ => None,
        }
    }

    /// A channel of data values for the color scale, Plot's
    /// `fill: (d) => Math.sign(d.change)`.
    pub fn value(accessor: impl Fn(&Table) -> Value + 'static) -> Self {
        ColorChannel::Value(Rc::new(accessor))
    }

    /// A channel of numbers for the color scale.
    pub fn number(accessor: impl Fn(&Table) -> f64 + 'static) -> Self {
        ColorChannel::Value(Rc::new(move |row| Value::number(accessor(row))))
    }

    /// Plot's `Plot.identity` for a color: the datum (a CSS color string)
    /// is the color.
    pub fn identity() -> Self {
        ColorChannel::Value(Rc::new(|row| row.value(crate::table::VALUE)))
    }
}

/// A string is a color when it parses as one, as Plot reads it: `"red"` and
/// `"#aaa"` are constants, `"currentColor"` the theme's foreground, and any
/// other text a field name.
impl From<&str> for ColorChannel {
    fn from(text: &str) -> Self {
        if text.eq_ignore_ascii_case("currentcolor") {
            return ColorChannel::Current;
        }
        if text.starts_with("var(") {
            // A CSS variable, as the docs use for the page background.
            return ColorChannel::Constant(Hsla::white());
        }
        match crate::css::parse(text) {
            Some(color) => ColorChannel::Constant(color),
            None => ColorChannel::Field(text.to_string().into()),
        }
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
            ColorChannel::Current => f.write_str("Current"),
            ColorChannel::Value(_) => f.write_str("Value(..)"),
        }
    }
}

/// A numeric channel: one value for every datum, or one per datum.
#[derive(Clone)]
pub enum NumberChannel {
    Constant(f32),
    Accessor(Rc<dyn Fn(&Table) -> f32>),
    /// A field whose values go through the plot's scale for the channel —
    /// Plot's `r: "Volume"` through its radius scale.
    Field(gpui::SharedString),
    /// An accessor whose values go through the channel's scale, Plot's
    /// `r: (d) => d.properties.population`.
    Scaled(Rc<dyn Fn(&Table) -> f64>),
}

impl NumberChannel {
    /// The channel's visual value for `row`. A scaled channel has no visual
    /// value without its scale, so it reads `NaN` here; see
    /// [`NumberChannel::data_value`].
    pub fn read(&self, row: &Table) -> f32 {
        match self {
            NumberChannel::Constant(value) => *value,
            NumberChannel::Accessor(accessor) => accessor(row),
            NumberChannel::Field(_) | NumberChannel::Scaled(_) => f32::NAN,
        }
    }

    /// The data value a scaled channel reads, before its scale.
    pub fn data_value(&self, row: &Table) -> Option<f64> {
        match self {
            NumberChannel::Field(field) => row.number(field),
            NumberChannel::Scaled(accessor) => Some(accessor(row)).filter(|v| v.is_finite()),
            _ => None,
        }
    }

    /// Whether the channel's values go through a scale.
    pub fn is_scaled(&self) -> bool {
        matches!(self, NumberChannel::Field(_) | NumberChannel::Scaled(_))
    }

    /// A channel of data values for the channel's scale, from an accessor.
    pub fn scaled(accessor: impl Fn(&Table) -> f64 + 'static) -> Self {
        NumberChannel::Scaled(Rc::new(accessor))
    }
}

impl From<&str> for NumberChannel {
    fn from(field: &str) -> Self {
        NumberChannel::Field(field.to_string().into())
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
            NumberChannel::Field(name) => f.debug_tuple("Field").field(name).finish(),
            NumberChannel::Scaled(_) => f.write_str("Scaled(..)"),
        }
    }
}

/// The symbol a dot mark draws, Plot's `symbol` channel: d3-shape's symbols.
///
/// The first seven are d3's `symbolsFill`, designed to be filled; the next six
/// its `symbolsStroke`, designed to be stroked. Plot's symbol scale draws from
/// one set or the other depending on whether the dot is filled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Symbol {
    #[default]
    Circle,
    Cross,
    Diamond,
    Square,
    Star,
    Triangle,
    Wye,
    Plus,
    Times,
    Triangle2,
    Asterisk,
    Square2,
    Diamond2,
    /// Plot's `hexagon`.
    Hexagon,
}

/// d3's `symbolsFill`, the range of Plot's symbol scale for a filled dot.
pub const SYMBOLS_FILL: [Symbol; 7] = [
    Symbol::Circle,
    Symbol::Cross,
    Symbol::Diamond,
    Symbol::Square,
    Symbol::Star,
    Symbol::Triangle,
    Symbol::Wye,
];

/// d3's `symbolsStroke`, the range of Plot's symbol scale for a stroked dot.
pub const SYMBOLS_STROKE: [Symbol; 7] = [
    Symbol::Circle,
    Symbol::Plus,
    Symbol::Times,
    Symbol::Triangle2,
    Symbol::Asterisk,
    Symbol::Square2,
    Symbol::Diamond2,
];

impl Symbol {
    /// The symbol Plot names `name` (case-insensitive), if any.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "circle" => Self::Circle,
            "cross" => Self::Cross,
            "diamond" => Self::Diamond,
            "square" => Self::Square,
            "star" => Self::Star,
            "triangle" => Self::Triangle,
            "wye" => Self::Wye,
            "plus" => Self::Plus,
            "times" | "x" => Self::Times,
            "triangle2" => Self::Triangle2,
            "asterisk" => Self::Asterisk,
            "square2" => Self::Square2,
            "diamond2" => Self::Diamond2,
            "hexagon" => Self::Hexagon,
            _ => return None,
        })
    }

    /// The symbol's subpaths for a dot of radius `radius` (d3's symbol of
    /// area `π r²`, as Plot sizes it) at `center`, and whether each closes.
    pub fn subpaths(&self, center: Point<Pixels>, radius: f32) -> Vec<(Vec<Point<Pixels>>, bool)> {
        use std::f32::consts::{PI, TAU};
        let size = PI * radius * radius;
        let (cx, cy) = (center.x.as_f32(), center.y.as_f32());
        let p = |x: f32, y: f32| Point::new(px(cx + x), px(cy + y));
        let sqrt3 = 3f32.sqrt();
        match self {
            Symbol::Circle => vec![(circle_path(center, (size / PI).sqrt(), 32), true)],
            Symbol::Cross => {
                let r = (size / 5.).sqrt() / 2.;
                vec![(
                    vec![
                        p(-3. * r, -r),
                        p(-r, -r),
                        p(-r, -3. * r),
                        p(r, -3. * r),
                        p(r, -r),
                        p(3. * r, -r),
                        p(3. * r, r),
                        p(r, r),
                        p(r, 3. * r),
                        p(-r, 3. * r),
                        p(-r, r),
                        p(-3. * r, r),
                    ],
                    true,
                )]
            }
            Symbol::Diamond => {
                let tan30 = (1f32 / 3.).sqrt();
                let y = (size / (tan30 * 2.)).sqrt();
                let x = y * tan30;
                vec![(vec![p(0., -y), p(x, 0.), p(0., y), p(-x, 0.)], true)]
            }
            Symbol::Square => {
                let w = size.sqrt() / 2.;
                vec![(vec![p(-w, -w), p(w, -w), p(w, w), p(-w, w)], true)]
            }
            Symbol::Star => {
                let ka = 0.890_813_1_f32;
                let kr = (PI / 10.).sin() / (7. * PI / 10.).sin();
                let kx = (TAU / 10.).sin() * kr;
                let ky = -(TAU / 10.).cos() * kr;
                let r = (size * ka).sqrt();
                let (x, y) = (kx * r, ky * r);
                let mut points = vec![p(0., -r), p(x, y)];
                for i in 1..5 {
                    let a = TAU * i as f32 / 5.;
                    let (c, s) = (a.cos(), a.sin());
                    points.push(p(s * r, -c * r));
                    points.push(p(c * x - s * y, s * x + c * y));
                }
                vec![(points, true)]
            }
            Symbol::Triangle => {
                let y = -(size / (sqrt3 * 3.)).sqrt();
                vec![(
                    vec![p(0., y * 2.), p(-sqrt3 * y, -y), p(sqrt3 * y, -y)],
                    true,
                )]
            }
            Symbol::Wye => {
                let (c, s) = (-0.5_f32, sqrt3 / 2.);
                let k = 1. / 12f32.sqrt();
                let a = (k / 2. + 1.) * 3.;
                let r = (size / a).sqrt();
                let (x0, y0) = (r / 2., r * k);
                let (x1, y1) = (x0, r * k + r);
                let (x2, y2) = (-x1, y1);
                vec![(
                    vec![
                        p(x0, y0),
                        p(x1, y1),
                        p(x2, y2),
                        p(c * x0 - s * y0, s * x0 + c * y0),
                        p(c * x1 - s * y1, s * x1 + c * y1),
                        p(c * x2 - s * y2, s * x2 + c * y2),
                        p(c * x0 + s * y0, c * y0 - s * x0),
                        p(c * x1 + s * y1, c * y1 - s * x1),
                        p(c * x2 + s * y2, c * y2 - s * x2),
                    ],
                    true,
                )]
            }
            Symbol::Plus => {
                let r = (size - (size / 7.).min(2.)).sqrt() * 0.87559;
                vec![
                    (vec![p(-r, 0.), p(r, 0.)], false),
                    (vec![p(0., r), p(0., -r)], false),
                ]
            }
            Symbol::Times => {
                let r = (size - (size / 6.).min(1.7)).sqrt() * 0.6189;
                vec![
                    (vec![p(-r, -r), p(r, r)], false),
                    (vec![p(-r, r), p(r, -r)], false),
                ]
            }
            Symbol::Triangle2 => {
                let s = size.sqrt() * 0.6824;
                let (t, u) = (s / 2., s * sqrt3 / 2.);
                vec![(vec![p(0., -s), p(u, t), p(-u, t)], true)]
            }
            Symbol::Asterisk => {
                let r = (size + (size / 28.).min(0.75)).sqrt() * 0.59436;
                let t = r / 2.;
                let u = t * sqrt3;
                vec![
                    (vec![p(0., r), p(0., -r)], false),
                    (vec![p(-u, -t), p(u, t)], false),
                    (vec![p(-u, t), p(u, -t)], false),
                ]
            }
            Symbol::Square2 => {
                let r = size.sqrt() * 0.4431;
                vec![(vec![p(r, r), p(r, -r), p(-r, -r), p(-r, r)], true)]
            }
            Symbol::Diamond2 => {
                let r = size.sqrt() * 0.62625;
                vec![(vec![p(0., -r), p(r, 0.), p(0., r), p(-r, 0.)], true)]
            }
            Symbol::Hexagon => {
                // Plot's hexagon: pointy-top, inscribed in the dot's radius.
                let r = (size / (1.5 * sqrt3)).sqrt();
                vec![(
                    (0..6)
                        .map(|i| {
                            let a = PI / 3. * i as f32;
                            p(r * a.sin(), -r * a.cos())
                        })
                        .collect(),
                    true,
                )]
            }
        }
    }

    /// The outline of a closed symbol, for hit tests and legends.
    pub fn path(&self, center: Point<Pixels>, radius: f32) -> Vec<Point<Pixels>> {
        self.subpaths(center, radius)
            .into_iter()
            .next()
            .map(|(points, _)| points)
            .unwrap_or_default()
    }

    /// Whether the symbol is one of d3's stroke symbols, drawn as lines.
    pub fn is_line(&self) -> bool {
        matches!(
            self,
            Symbol::Plus
                | Symbol::Times
                | Symbol::Triangle2
                | Symbol::Asterisk
                | Symbol::Square2
                | Symbol::Diamond2
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
        assert_eq!(Symbol::Cross.path(center, 4.).len(), 12);
        assert!(Symbol::Plus.is_line() && !Symbol::Cross.is_line());
        assert_eq!(Symbol::parse("x"), Some(Symbol::Times));
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
