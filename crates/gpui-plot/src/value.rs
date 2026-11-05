//! The scalar a plot reads from a datum: the analogue of Observable Plot's
//! value types (number, date, string, boolean, null).
//!
//! Plot infers a channel's type from the values it is handed and picks a scale
//! that fits. This library keeps the same shape: a channel hands the library
//! [`Value`]s, the library orders and scales them, and only the final pixel
//! positions are continuous numbers.

use std::cmp::Ordering;
use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use gpui::{Hsla, SharedString};

/// One datum's value in one channel.
///
/// A `Value` is ordered so that the library can compute an extent without the
/// caller deciding which channel is numeric: `Null` sorts below everything,
/// booleans sort as `false < true`, strings sort lexicographically, and numbers
/// and dates sort numerically.
#[derive(Clone, Debug)]
pub enum Value {
    /// A missing value. Plot leaves a mark's position undefined for it; this
    /// library skips the datum in the mark that reads it.
    Null,
    /// A finite or infinite number.
    Number(f64),
    /// An instant, as Plot's `Date` values are.
    Date(DateTime<Utc>),
    /// A categorical value.
    Text(SharedString),
    /// A boolean, which Plot scales as a two-value ordinal domain.
    Bool(bool),
}

impl Value {
    /// A number value. `NaN` becomes [`Value::Null`], because Plot treats a
    /// `NaN` as a missing value rather than a position.
    pub fn number(value: f64) -> Self {
        if value.is_nan() {
            Value::Null
        } else {
            Value::Number(value)
        }
    }

    /// A date value from a UTC timestamp in milliseconds.
    pub fn date_from_millis(millis: i64) -> Self {
        Utc.timestamp_millis_opt(millis)
            .single()
            .map(Value::Date)
            .unwrap_or(Value::Null)
    }

    /// A date value from a calendar date at midnight UTC.
    pub fn date(date: NaiveDate) -> Self {
        Value::Date(
            date.and_hms_opt(0, 0, 0)
                .expect("midnight is valid")
                .and_utc(),
        )
    }

    pub fn text(text: impl Into<SharedString>) -> Self {
        Value::Text(text.into())
    }

    pub fn boolean(value: bool) -> Self {
        Value::Bool(value)
    }

    /// The number this value scales as, or `None` for [`Value::Null`].
    ///
    /// A [`Value::Date`] converts to milliseconds since the epoch, which is the
    /// number its time scale reads.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Null => None,
            Value::Number(n) => Some(*n),
            Value::Date(d) => Some(d.timestamp_millis() as f64),
            Value::Bool(b) => Some(if *b { 1. } else { 0. }),
            Value::Text(_) => None,
        }
    }

    /// The instant this value is, or `None` when it is not a date.
    pub fn as_date(&self) -> Option<DateTime<Utc>> {
        match self {
            Value::Date(d) => Some(*d),
            _ => None,
        }
    }

    /// The text this value displays as.
    pub fn as_text(&self) -> Option<&SharedString> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    /// Whether this value is missing.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// The [`PlotKind`] this value belongs to.
    pub fn kind(&self) -> PlotKind {
        match self {
            Value::Null => PlotKind::Null,
            Value::Number(_) => PlotKind::Number,
            Value::Date(_) => PlotKind::Date,
            Value::Text(_) => PlotKind::Text,
            Value::Bool(_) => PlotKind::Bool,
        }
    }

    /// The value as a label: numbers without a trailing `.0`, dates as
    /// `YYYY-MM-DD`, text and booleans as themselves.
    pub fn label(&self) -> SharedString {
        match self {
            Value::Null => SharedString::from(""),
            Value::Number(n) => SharedString::from(format_number(*n)),
            Value::Date(d) => SharedString::from(d.format("%Y-%m-%d").to_string()),
            Value::Text(t) => t.clone(),
            Value::Bool(b) => SharedString::from(if *b { "true" } else { "false" }),
        }
    }

    /// The key values with the same label share, so a categorical domain can
    /// merge them the way Plot's ordinal domain does.
    pub fn key(&self) -> ValueKey {
        ValueKey(match self {
            Value::Null => KeyRepr::Null,
            Value::Number(n) => KeyRepr::Number(F64Bits::of(*n)),
            Value::Date(d) => KeyRepr::Date(d.timestamp_millis()),
            Value::Text(t) => KeyRepr::Text(t.clone()),
            Value::Bool(b) => KeyRepr::Bool(*b),
        })
    }
}

/// The kind of a channel, inferred from its values.
///
/// This mirrors Plot's channel inference: `quantitative`, `temporal`,
/// `ordinal`, and Plot's implicit boolean and null handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlotKind {
    Null,
    Number,
    Date,
    Text,
    Bool,
}

impl PlotKind {
    /// Whether a scale for this kind is continuous.
    pub fn is_quantitative(self) -> bool {
        matches!(self, PlotKind::Number | PlotKind::Date)
    }

    /// Whether a scale for this kind is categorical (a band or ordinal scale).
    pub fn is_ordinal(self) -> bool {
        matches!(self, PlotKind::Text | PlotKind::Bool)
    }

    /// The name Plot gives this kind, for axis and tooltip defaults.
    pub fn label(self) -> &'static str {
        match self {
            PlotKind::Null => "null",
            PlotKind::Number => "quantitative",
            PlotKind::Date => "temporal",
            PlotKind::Text => "ordinal",
            PlotKind::Bool => "boolean",
        }
    }
}

/// The identity of a [`Value`] for categorical domains and maps.
///
/// Ordered the way [`Value`] is, so a `BTreeMap` keyed by a `ValueKey` reads in
/// domain order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueKey(KeyRepr);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum KeyRepr {
    Null,
    Bool(bool),
    Number(F64Bits),
    Date(i64),
    Text(SharedString),
}

/// A number ordered numerically rather than by its bits.
#[derive(Clone, Copy, Debug)]
struct F64Bits(u64);

impl F64Bits {
    fn of(value: f64) -> Self {
        // Total order over floats: -0 sorts below +0, NaN sorts last.
        Self(if value.is_nan() {
            // A positive NaN, which `total_cmp` sorts above +inf.
            f64::NAN.to_bits()
        } else if value == 0. {
            0
        } else {
            value.to_bits()
        })
    }
}

impl PartialEq for F64Bits {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for F64Bits {}

impl PartialOrd for F64Bits {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for F64Bits {
    fn cmp(&self, other: &Self) -> Ordering {
        f64::from_bits(self.0).total_cmp(&f64::from_bits(other.0))
    }
}

impl std::hash::Hash for F64Bits {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl ValueKey {
    /// The value this key came from, for a domain built from keys.
    pub fn to_value(&self) -> Value {
        match &self.0 {
            KeyRepr::Null => Value::Null,
            KeyRepr::Number(bits) => Value::Number(f64::from_bits(bits.0)),
            KeyRepr::Date(millis) => Value::date_from_millis(*millis),
            KeyRepr::Text(t) => Value::Text(t.clone()),
            KeyRepr::Bool(b) => Value::Bool(*b),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            // Through the key's bits, so `NaN == NaN` (as `Eq` promises)
            // and `-0 == +0`, matching `cmp` and `key`.
            (Value::Number(a), Value::Number(b)) => F64Bits::of(*a) == F64Bits::of(*b),
            (Value::Date(a), Value::Date(b)) => a == b,
            (Value::Text(a), Value::Text(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Value {}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        let rank = |value: &Value| match value {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 2,
            Value::Date(_) => 3,
            Value::Text(_) => 4,
        };

        match rank(self).cmp(&rank(other)) {
            Ordering::Equal => match (self, other) {
                (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
                // A total order consistent with `Eq`, even when a caller
                // builds a `Number(NaN)` directly: NaN sorts last.
                (Value::Number(a), Value::Number(b)) => F64Bits::of(*a).cmp(&F64Bits::of(*b)),
                (Value::Date(a), Value::Date(b)) => a.cmp(b),
                (Value::Text(a), Value::Text(b)) => a.cmp(b),
                _ => Ordering::Equal,
            },
            ordering => ordering,
        }
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::number(value)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Value::number(value as f64)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Number(value as f64)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Number(value as f64)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::Number(value as f64)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value::Number(value as f64)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Text(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Text(value.into())
    }
}

impl From<SharedString> for Value {
    fn from(value: SharedString) -> Self {
        Value::Text(value)
    }
}

impl From<NaiveDate> for Value {
    fn from(value: NaiveDate) -> Self {
        Value::date(value)
    }
}

impl From<DateTime<Utc>> for Value {
    fn from(value: DateTime<Utc>) -> Self {
        Value::Date(value)
    }
}

impl From<Hsla> for Value {
    fn from(value: Hsla) -> Self {
        let rgba: gpui::Rgba = value.into();
        Value::Text(SharedString::from(format!(
            "#{:02x}{:02x}{:02x}",
            (rgba.r * 255.) as u8,
            (rgba.g * 255.) as u8,
            (rgba.b * 255.) as u8
        )))
    }
}

/// A number as a label, with Plot's default formatting: integers print without
/// a decimal point, and non-integers keep up to three significant decimals
/// rather than a long float tail.
pub fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let magnitude = value.abs();
    let decimals = if magnitude >= 100. {
        0
    } else if magnitude >= 10. {
        1
    } else if magnitude >= 1. {
        2
    } else {
        3
    };
    let formatted = format!("{value:.decimals$}");
    // Only a fractional part has trailing zeros to drop: trimming a whole
    // number would turn `1000` into `1`.
    let trimmed = if formatted.contains('.') {
        formatted.trim_end_matches('0').trim_end_matches('.')
    } else {
        formatted.as_str()
    };
    // A tiny negative value rounds to `-0`; show it as `0`.
    if trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn nan_is_null() {
        assert!(Value::number(f64::NAN).is_null());
        assert_eq!(Value::number(2.).as_number(), Some(2.));
    }

    #[test]
    fn dates_order_and_scale_as_millis() {
        let early = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let late = Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap();
        let (a, b) = (Value::Date(early), Value::Date(late));
        assert!(a < b);
        assert_eq!(a.as_number(), Some(early.timestamp_millis() as f64));
    }

    #[test]
    fn categories_are_distinct() {
        assert_ne!(Value::text("1").key(), Value::Number(1.).key());
        assert_eq!(Value::text("a").key(), Value::text("a").key());
    }

    #[test]
    fn numbers_format_like_plot() {
        assert_eq!(format_number(3.), "3");
        assert_eq!(format_number(3.5), "3.5");
        assert_eq!(format_number(0.125), "0.125");
        assert_eq!(format_number(1234.567), "1235");
    }

    #[test]
    fn null_sorts_first() {
        let mut values = [Value::text("z"), Value::Null, Value::Number(1.)];
        values.sort();
        assert!(values[0].is_null());
        assert_eq!(values[1], Value::Number(1.));
    }
}
