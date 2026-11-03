//! The datum a marks read from: Plot's data array plus its field channels.
//!
//! Plot lets a mark read a datum through *channels* — `x`, `y`, `fill`,
//! `r`, … — and each channel is either a field name (`"weight"`), an accessor
//! function, or a constant. A [`Table`] is the record a channel reads; its
//! fields are resolved by name so a chart can be described without a Rust
//! struct for every dataset.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use gpui::SharedString;

use crate::value::Value;

/// A record of named values, like one element of Plot's data array.
///
/// A table is built from pairs, so a chart can read `"weight"` from it:
///
/// ```
/// use gpui_plot::{Table, Value};
///
/// let row = Table::new([("name", Value::text("alpha")), ("weight", Value::Number(1.5))]);
/// assert_eq!(row.number("weight"), Some(1.5));
/// assert_eq!(row.get("name").map(Value::label), Some("alpha".into()));
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    fields: BTreeMap<SharedString, Value>,
}

impl Table {
    /// A table from name/value pairs. Later pairs with the same name replace
    /// earlier ones, matching an object literal's behaviour.
    pub fn new<I, S>(fields: I) -> Self
    where
        I: IntoIterator<Item = (S, Value)>,
        S: Into<SharedString>,
    {
        Self {
            fields: fields
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        }
    }

    /// An empty table.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set a field, replacing any value already under that name.
    pub fn with(mut self, name: impl Into<SharedString>, value: impl Into<Value>) -> Self {
        self.fields.insert(name.into(), value.into());
        self
    }

    /// Read a field by name.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    /// Read a field by name, or [`Value::Null`] when the field is absent.
    ///
    /// Plot treats a missing field as an undefined value, and marks skip it;
    /// returning `Null` here keeps the accessor chain total.
    pub fn value(&self, name: &str) -> Value {
        self.fields.get(name).cloned().unwrap_or(Value::Null)
    }

    /// The field names in the table, in sorted order.
    pub fn field_names(&self) -> impl Iterator<Item = &SharedString> {
        self.fields.keys()
    }

    /// The number of fields.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the table has no fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Read a field as a number.
    pub fn number(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(Value::as_number)
    }

    /// Read a field as text.
    pub fn text(&self, name: &str) -> Option<&SharedString> {
        self.get(name).and_then(Value::as_text)
    }

    /// Read a field as an instant.
    pub fn date(&self, name: &str) -> Option<DateTime<Utc>> {
        self.get(name).and_then(Value::as_date)
    }
}

/// A collection of [`Table`]s: Plot's data array.
///
/// The convenience [`Table::from`] conversions let an iterator of records
/// become a [`Dataset`] directly.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dataset {
    rows: Vec<Table>,
}

impl Dataset {
    pub fn new(rows: impl IntoIterator<Item = Table>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
        }
    }

    pub fn rows(&self) -> &[Table] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The values of one field, in row order.
    pub fn values(&self, name: &str) -> Vec<Value> {
        self.rows.iter().map(|row| row.value(name)).collect()
    }
}

impl From<Vec<Table>> for Dataset {
    fn from(rows: Vec<Table>) -> Self {
        Self { rows }
    }
}

impl FromIterator<Table> for Dataset {
    fn from_iter<I: IntoIterator<Item = Table>>(iter: I) -> Self {
        Self {
            rows: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for Dataset {
    type Item = Table;
    type IntoIter = std::vec::IntoIter<Table>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.into_iter()
    }
}

/// The value a channel read: a field's value, a constant, or an accessor's
/// result.
///
/// Plot's channels accept the same three forms, and the library resolves them
/// once so a mark does not care which it was handed.
#[derive(Clone)]
pub enum Channel {
    /// A constant every datum reads.
    Constant(Value),
    /// A field name resolved against each [`Table`].
    Field(SharedString),
    /// An accessor over the whole [`Table`].
    Accessor(std::rc::Rc<dyn Fn(&Table) -> Value>),
}

impl Channel {
    /// The value this channel reads from `row`.
    pub fn read(&self, row: &Table) -> Value {
        match self {
            Channel::Constant(value) => value.clone(),
            Channel::Field(name) => row.value(name),
            Channel::Accessor(accessor) => accessor(row),
        }
    }
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Constant(value) => f.debug_tuple("Constant").field(value).finish(),
            Channel::Field(name) => f.debug_tuple("Field").field(name).finish(),
            Channel::Accessor(_) => f.write_str("Accessor(..)"),
        }
    }
}

impl From<&str> for Channel {
    fn from(name: &str) -> Self {
        Channel::Field(name.into())
    }
}

impl From<SharedString> for Channel {
    fn from(name: SharedString) -> Self {
        Channel::Field(name)
    }
}

impl From<Value> for Channel {
    fn from(value: Value) -> Self {
        Channel::Constant(value)
    }
}

impl From<f64> for Channel {
    fn from(value: f64) -> Self {
        Channel::Constant(Value::number(value))
    }
}

impl From<i32> for Channel {
    fn from(value: i32) -> Self {
        Channel::Constant(Value::Number(value as f64))
    }
}

impl From<NaiveDate> for Channel {
    fn from(value: NaiveDate) -> Self {
        Channel::Constant(Value::date(value))
    }
}

impl From<DateTime<Utc>> for Channel {
    fn from(value: DateTime<Utc>) -> Self {
        Channel::Constant(Value::Date(value))
    }
}

/// A helper trait so marks take Plot-style accessors directly:
/// `x(|row: &Table| row.value("weight"))` and `x("weight")` both work.
pub trait IntoChannel {
    fn into_channel(self) -> Channel;
}

impl IntoChannel for Channel {
    fn into_channel(self) -> Channel {
        self
    }
}

impl IntoChannel for &str {
    fn into_channel(self) -> Channel {
        Channel::Field(self.into())
    }
}

impl IntoChannel for String {
    fn into_channel(self) -> Channel {
        Channel::Field(self.into())
    }
}

impl IntoChannel for SharedString {
    fn into_channel(self) -> Channel {
        Channel::Field(self)
    }
}

impl IntoChannel for Value {
    fn into_channel(self) -> Channel {
        Channel::Constant(self)
    }
}

impl<F> IntoChannel for F
where
    F: Fn(&Table) -> Value + 'static,
{
    fn into_channel(self) -> Channel {
        Channel::Accessor(std::rc::Rc::new(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_by_name() {
        let row = Table::new([("x", Value::Number(1.)), ("label", Value::text("a"))]);
        assert_eq!(row.number("x"), Some(1.));
        assert_eq!(row.text("label").map(|t| t.to_string()), Some("a".into()));
        assert!(row.value("missing").is_null());
    }

    #[test]
    fn channels_resolve_three_forms() {
        let row = Table::new([("v", Value::Number(4.))]);
        assert_eq!(Channel::from("v").read(&row), Value::Number(4.));
        assert_eq!(
            Channel::from(Value::Number(9.)).read(&row),
            Value::Number(9.)
        );
        let accessor = Channel::Accessor(std::rc::Rc::new(|row: &Table| row.value("v")));
        assert_eq!(accessor.read(&row), Value::Number(4.));
    }

    #[test]
    fn later_fields_replace_earlier() {
        let row = Table::new([("v", Value::Number(1.)), ("v", Value::Number(2.))]);
        assert_eq!(row.number("v"), Some(2.));
    }
}
