//! The datum a marks read from: Plot's data array plus its field channels.
//!
//! Plot lets a mark read a datum through *channels* — `x`, `y`, `fill`,
//! `r`, … — and each channel is either a field name (`"weight"`), an accessor
//! function, or a constant. A [`Table`] is the record a channel reads; its
//! fields are resolved by name so a chart can be described without a Rust
//! struct for every dataset.

use std::collections::BTreeMap;
use std::rc::Rc;

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
///
/// A table shares its fields behind an `Rc`, so the copies the transforms and
/// prepared rows keep are cheap; [`Table::with`] copies on write.
///
/// The fields are a vector of pairs sorted by name with no duplicates, so a
/// lookup is a binary search over contiguous memory and a copy on write is a
/// single vector clone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    fields: Rc<Vec<(SharedString, Value)>>,
}

/// The bit of a [`Table::get_hinted`] hint that marks a field the last
/// record lacked; the rest is where it would have sat.
const ABSENT_HINT: usize = 1 << (usize::BITS - 1);

/// Up to how many fields [`Table::get`] scans rather than searches.
const LINEAR_GET_FIELDS: usize = 16;

impl Table {
    /// A table from name/value pairs. Later pairs with the same name replace
    /// earlier ones, matching an object literal's behaviour.
    pub fn new<I, S>(fields: I) -> Self
    where
        I: IntoIterator<Item = (S, Value)>,
        S: Into<SharedString>,
    {
        // Collecting through a map keeps the last pair for a repeated name
        // and sorts the fields by name.
        let fields: BTreeMap<SharedString, Value> = fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect();
        Self {
            fields: Rc::new(fields.into_iter().collect()),
        }
    }

    /// The position of `name` among the sorted fields, or where it would go.
    fn position(&self, name: &str) -> Result<usize, usize> {
        self.fields.binary_search_by(|(k, _)| k.as_ref().cmp(name))
    }

    /// An empty table.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set a field, replacing any value already under that name.
    pub fn with(mut self, name: impl Into<SharedString>, value: impl Into<Value>) -> Self {
        self.set(name, value);
        self
    }

    /// Set a field in place, replacing any value already under that name.
    pub fn set(&mut self, name: impl Into<SharedString>, value: impl Into<Value>) {
        let name = name.into();
        let value = value.into();
        match self.position(&name) {
            Ok(i) => Rc::make_mut(&mut self.fields)[i].1 = value,
            Err(i) => match Rc::get_mut(&mut self.fields) {
                Some(fields) => fields.insert(i, (name, value)),
                // A shared table copies its fields once, with room for the
                // new one, rather than copying and then growing.
                None => {
                    let mut fields = Vec::with_capacity(self.fields.len() + 1);
                    fields.extend_from_slice(&self.fields[..i]);
                    fields.push((name, value));
                    fields.extend_from_slice(&self.fields[i..]);
                    self.fields = Rc::new(fields);
                }
            },
        }
    }

    /// Read a field by name.
    pub fn get(&self, name: &str) -> Option<&Value> {
        let fields = &**self.fields;
        // A record of a few fields is faster to scan than to search: most
        // names differ from the one sought in length or first byte, which
        // rules them out without comparing the rest. Names are unique, so
        // the scan finds the field the search would.
        if fields.len() <= LINEAR_GET_FIELDS {
            let name = name.as_bytes();
            return fields
                .iter()
                .find(|(key, _)| {
                    let key = key.as_bytes();
                    key.len() == name.len() && key.first() == name.first() && key == name
                })
                .map(|(_, value)| value);
        }
        self.position(name).ok().map(|i| &fields[i].1)
    }

    /// [`Table::get`], trying `hint` first: the field's position in the last
    /// record read, which records of one shape share. A miss searches, and
    /// leaves the position found in `hint`.
    ///
    /// A field the last record lacked leaves where it would have sat, marked
    /// [`ABSENT_HINT`]: a record of the same shape lacks it too when its
    /// neighbours there bracket the name, which the fields' sorted order
    /// proves in two comparisons rather than a search.
    #[inline]
    pub(crate) fn get_hinted(&self, name: &str, hint: &std::cell::Cell<usize>) -> Option<&Value> {
        let at = hint.get();
        if at & ABSENT_HINT == 0 {
            if let Some((key, value)) = self.fields.get(at)
                && key.as_ref() == name
            {
                return Some(value);
            }
        } else {
            let at = at & !ABSENT_HINT;
            let after = at == 0
                || self
                    .fields
                    .get(at - 1)
                    .is_some_and(|(key, _)| key.as_ref() < name);
            let before = match self.fields.get(at) {
                Some((key, _)) => name < key.as_ref(),
                None => at == self.fields.len(),
            };
            if after && before {
                return None;
            }
        }
        match self.position(name) {
            Ok(i) => {
                hint.set(i);
                Some(&self.fields[i].1)
            }
            Err(i) => {
                hint.set(i | ABSENT_HINT);
                None
            }
        }
    }

    /// Read a field by name, or [`Value::Null`] when the field is absent.
    ///
    /// Plot treats a missing field as an undefined value, and marks skip it;
    /// returning `Null` here keeps the accessor chain total.
    pub fn value(&self, name: &str) -> Value {
        self.get(name).cloned().unwrap_or(Value::Null)
    }

    /// The field names in the table, in sorted order.
    pub fn field_names(&self) -> impl Iterator<Item = &SharedString> {
        self.fields.iter().map(|(name, _)| name)
    }

    /// The number of fields.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the table has no fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Read a field as a number, `NaN` when it is missing: JavaScript's
    /// `+d[name]`, for accessors that compute with it.
    pub fn num(&self, name: &str) -> f64 {
        self.number(name).unwrap_or(f64::NAN)
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
/// become a [`Dataset`] directly. The rows are shared behind an `Rc`, so a
/// dataset handed to several marks, or kept across frames, is not copied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dataset {
    rows: Rc<Vec<Table>>,
}

impl Dataset {
    pub fn new(rows: impl IntoIterator<Item = Table>) -> Self {
        Self {
            rows: Rc::new(rows.into_iter().collect()),
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

    /// Whether `other` shares this dataset's rows (the same `Rc`), a cheap
    /// identity check for caching work done per source.
    pub fn same_rows(&self, other: &Dataset) -> bool {
        Rc::ptr_eq(&self.rows, &other.rows)
    }

    /// The values of one field, in row order.
    pub fn values(&self, name: &str) -> Vec<Value> {
        self.rows.iter().map(|row| row.value(name)).collect()
    }
}

/// The field a plain value's record holds it in, so `Plot.ruleY([0])` reads
/// as a dataset whose `y` defaults to [`Channel::identity`].
pub const VALUE: &str = "$value";

/// The field holding a plain value's (or tuple's) index in its array, which
/// Plot's `indexOf` reads.
pub const INDEX: &str = "$index";

impl Dataset {
    /// A dataset of plain values, Plot's arrays of numbers, strings or dates:
    /// each value becomes a record holding it under [`VALUE`].
    pub fn values_of<V: Into<Value>>(values: impl IntoIterator<Item = V>) -> Self {
        Self::new(values.into_iter().enumerate().map(|(i, value)| {
            Table::new([
                (SharedString::new_static(VALUE), value.into()),
                (SharedString::new_static(INDEX), Value::Number(i as f64)),
            ])
        }))
    }

    /// A dataset of tuples, Plot's arrays of `[x, y]` pairs: each element is a
    /// record with fields `"0"`, `"1"`, …, which the marks' `x` and `y` read
    /// by default, as Plot's do.
    pub fn tuples<V: Into<Value>, const N: usize>(rows: impl IntoIterator<Item = [V; N]>) -> Self {
        Self::new(rows.into_iter().enumerate().map(|(i, row)| {
            Table::new(
                row.into_iter()
                    .enumerate()
                    .map(|(index, value)| (SharedString::from(index.to_string()), value.into()))
                    .chain([(SharedString::new_static(INDEX), Value::Number(i as f64))]),
            )
        }))
    }

    /// `n` records that hold only their index, Plot's `{length: n}` data for
    /// marks whose channels are all functions.
    pub fn length(n: usize) -> Self {
        Self::new(
            (0..n)
                .map(|i| Table::new([(SharedString::new_static(INDEX), Value::Number(i as f64))])),
        )
    }

    /// Whether this is a dataset of plain values ([`Dataset::values_of`]).
    pub fn is_values(&self) -> bool {
        self.rows
            .first()
            .is_some_and(|row| row.get(VALUE).is_some())
    }

    /// Whether this is a dataset of tuples ([`Dataset::tuples`]).
    pub fn is_tuples(&self) -> bool {
        self.rows
            .first()
            .is_some_and(|row| row.get("0").is_some() && row.get(VALUE).is_none())
    }
}

impl<const N: usize> From<[f64; N]> for Dataset {
    fn from(values: [f64; N]) -> Self {
        Dataset::values_of(values)
    }
}

impl<const N: usize> From<[i32; N]> for Dataset {
    fn from(values: [i32; N]) -> Self {
        Dataset::values_of(values.map(|v| v as f64))
    }
}

impl From<Vec<f64>> for Dataset {
    fn from(values: Vec<f64>) -> Self {
        Dataset::values_of(values)
    }
}

impl From<Vec<Value>> for Dataset {
    fn from(values: Vec<Value>) -> Self {
        Dataset::values_of(values)
    }
}

impl<const N: usize> From<[&str; N]> for Dataset {
    fn from(values: [&str; N]) -> Self {
        Dataset::values_of(values.map(|v| Value::text(v.to_string())))
    }
}

impl From<&Dataset> for Dataset {
    fn from(data: &Dataset) -> Self {
        data.clone()
    }
}

impl From<Vec<Table>> for Dataset {
    fn from(rows: Vec<Table>) -> Self {
        Self {
            rows: Rc::new(rows),
        }
    }
}

impl FromIterator<Table> for Dataset {
    fn from_iter<I: IntoIterator<Item = Table>>(iter: I) -> Self {
        Self::new(iter)
    }
}

impl IntoIterator for Dataset {
    type Item = Table;
    type IntoIter = std::vec::IntoIter<Table>;

    fn into_iter(self) -> Self::IntoIter {
        Rc::try_unwrap(self.rows)
            .unwrap_or_else(|rows| (*rows).clone())
            .into_iter()
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
    /// Plot's `Plot.identity`: the datum itself, for a dataset of plain
    /// values.
    pub fn identity() -> Self {
        Channel::Field(VALUE.into())
    }

    /// Plot's `indexOf`: the datum's index in its array.
    pub fn index() -> Self {
        Channel::Field(INDEX.into())
    }

    /// A channel computed from each datum as a number, Plot's
    /// `(d) => 100 / d.mpg`; a non-finite result is missing.
    pub fn number(accessor: impl Fn(&Table) -> f64 + 'static) -> Self {
        Channel::Accessor(std::rc::Rc::new(move |row| Value::number(accessor(row))))
    }

    /// Whether `other` is known to read the same values as this channel: the
    /// same field, the same accessor (by pointer) or an equal constant. Two
    /// distinct accessors are never the same, so this only keys caches.
    pub fn same(&self, other: &Channel) -> bool {
        match (self, other) {
            (Channel::Field(a), Channel::Field(b)) => a == b,
            (Channel::Accessor(a), Channel::Accessor(b)) => std::rc::Rc::ptr_eq(a, b),
            (Channel::Constant(a), Channel::Constant(b)) => a.key() == b.key(),
            _ => false,
        }
    }

    /// The value this channel reads from `row`.
    pub fn read(&self, row: &Table) -> Value {
        match self {
            Channel::Constant(value) => value.clone(),
            // A NaN stored in a record is a missing value, as Plot reads it.
            Channel::Field(name) => match row.value(name) {
                Value::Number(n) if n.is_nan() => Value::Null,
                value => value,
            },
            Channel::Accessor(accessor) => accessor(row),
        }
    }

    /// [`Channel::read`], finding a field through `hint` (see
    /// [`Table::get_hinted`]), for a loop that reads one channel over many
    /// records.
    #[inline]
    pub(crate) fn read_hinted(&self, row: &Table, hint: &std::cell::Cell<usize>) -> Value {
        match self {
            Channel::Field(name) => match row.get_hinted(name, hint) {
                Some(Value::Number(n)) if n.is_nan() => Value::Null,
                Some(value) => value.clone(),
                None => Value::Null,
            },
            _ => self.read(row),
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
    fn get_scans_and_searches_alike() {
        // Names that share lengths, first bytes and prefixes, the empty name
        // and non-ASCII ones, in tables on both sides of the scan's limit.
        let pool = [
            "", "a", "b", "ab", "ba", "aa", "abc", "abd", "x", "x1", "x2", "y", "y1", "y2", "z",
            "fill", "fx", "fy", "geometry", "date", "Date", "é", "éa", "ア", "$in:x", "$box:q",
            "carat", "cut", "color", "clarity", "price",
        ];
        let mut seed = 17_u64;
        for size in 0..=pool.len() {
            for _ in 0..8 {
                let row = Table::new((0..size).map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let name = pool[(seed >> 33) as usize % pool.len()];
                    (name, Value::text(name))
                }));
                for name in pool.iter().copied().chain(["q", "abcd", "fil", "fills"]) {
                    let searched = row.position(name).ok().map(|i| &row.fields[i].1);
                    assert_eq!(row.get(name), searched, "{name:?} in {row:?}");
                }
            }
        }
    }

    #[test]
    fn later_fields_replace_earlier() {
        let row = Table::new([("v", Value::Number(1.)), ("v", Value::Number(2.))]);
        assert_eq!(row.number("v"), Some(2.));
    }

    #[test]
    fn hinted_reads_match_plain_reads_across_shapes() {
        let rows = [
            Table::new([("a", Value::Number(1.)), ("v", Value::Number(2.))]),
            Table::new([("v", Value::Number(3.))]),
            Table::new([("a", Value::Number(4.)), ("b", Value::Number(5.))]),
            Table::new([("v", Value::Number(f64::NAN)), ("z", Value::Number(6.))]),
            Table::new([("a", Value::Number(7.)), ("v", Value::Number(8.))]),
        ];
        let channel = Channel::from("v");
        let hint = std::cell::Cell::new(0);
        for row in &rows {
            assert_eq!(channel.read_hinted(row, &hint), channel.read(row));
        }
    }

    /// A hint left by an absent field (or a present one) never hides a
    /// field nor finds one that is not there, whatever shape the next
    /// record has: absent names before, between and after the fields, in
    /// empty and one-field tables, from stale hints of either kind.
    #[test]
    fn hinted_reads_of_absent_fields_match_plain_reads() {
        let shapes: Vec<Vec<&str>> = vec![
            vec!["b", "d", "f"],
            vec!["b", "d", "f"],
            vec![],
            vec!["d"],
            vec!["a", "b", "c", "d", "e", "f", "g"],
            vec!["c", "e"],
            vec!["b", "d", "f"],
            vec!["g"],
            vec!["a"],
            vec!["b", "dd", "f"],
        ];
        let rows: Vec<Table> = shapes
            .iter()
            .enumerate()
            .map(|(i, names)| {
                Table::new(
                    names
                        .iter()
                        .enumerate()
                        .map(|(j, name)| (*name, Value::Number((i * 10 + j) as f64))),
                )
            })
            .collect();
        let orders: [Vec<usize>; 3] = [
            (0..rows.len()).collect(),
            (0..rows.len()).rev().collect(),
            (0..rows.len()).map(|i| (i * 7) % rows.len()).collect(),
        ];
        let stale = [
            0,
            1,
            3,
            99,
            ABSENT_HINT,
            ABSENT_HINT | 1,
            ABSENT_HINT | 3,
            ABSENT_HINT | 99,
        ];
        for name in [
            "", "a", "b", "c", "d", "dd", "de", "e", "f", "g", "h", "0", "~",
        ] {
            for order in &orders {
                for start in stale {
                    let hint = std::cell::Cell::new(start);
                    for &i in order {
                        let row = &rows[i];
                        assert_eq!(row.get_hinted(name, &hint), row.get(name), "{name} in {i}");
                        // Twice, so the hint the read left is tried on the
                        // record it came from.
                        assert_eq!(row.get_hinted(name, &hint), row.get(name), "{name} in {i}");
                    }
                }
            }
        }
    }

    #[test]
    fn set_copies_a_shared_table_once() {
        let shared = Table::new([("a", Value::Number(1.)), ("c", Value::Number(3.))]);
        let mut copy = shared.clone();
        copy.set("b", Value::Number(2.));
        copy.set("a", Value::Number(0.));
        assert_eq!(shared.number("a"), Some(1.));
        assert!(shared.get("b").is_none());
        assert_eq!(
            copy,
            Table::new([
                ("a", Value::Number(0.)),
                ("b", Value::Number(2.)),
                ("c", Value::Number(3.)),
            ])
        );
    }

    #[test]
    fn fields_sort_by_name_whatever_the_build_order() {
        // Static, owned and multi-byte names, digits that sort as text, and a
        // repeated name, built through `new`, `with` and `set`.
        let owned = String::from("$index");
        let mut row = Table::new([
            ("b", Value::Number(1.)),
            ("10", Value::Number(10.)),
            ("é", Value::text("accent")),
            ("b", Value::Number(3.)),
        ])
        .with(owned.as_str(), Value::Number(9.))
        .with(SharedString::new_static("a"), Value::Bool(true));
        row.set("2", Value::Number(2.));
        row.set("$value", Value::Null);
        row.set("é", Value::text("again"));
        let names: Vec<&str> = row.field_names().map(|name| name.as_ref()).collect();
        assert_eq!(names, ["$index", "$value", "10", "2", "a", "b", "é"]);
        assert_eq!(row.number("b"), Some(3.));
        assert_eq!(row.number("$index"), Some(9.));
        assert_eq!(row.text("é").map(|t| t.as_ref()), Some("again"));
        for absent in ["", "1", "c", "zz", "$in:fill", "ü"] {
            assert!(row.get(absent).is_none(), "{absent}");
        }
        // The same fields in another order make an equal table.
        let other = Table::new([
            ("é", Value::text("again")),
            ("a", Value::Bool(true)),
            ("2", Value::Number(2.)),
            ("$value", Value::Null),
            ("b", Value::Number(3.)),
            ("10", Value::Number(10.)),
            ("$index", Value::Number(9.)),
        ]);
        assert_eq!(row, other);
        assert_ne!(row, other.with("b", Value::Number(4.)));
    }

    #[test]
    fn tuples_and_values_keep_their_index_and_wide_rows() {
        let data = Dataset::tuples([std::array::from_fn::<f64, 12, _>(|j| j as f64); 2]);
        assert!(data.is_tuples() && !data.is_values());
        for (i, row) in data.rows().iter().enumerate() {
            for j in 0..12 {
                assert_eq!(row.number(&j.to_string()), Some(j as f64));
            }
            assert_eq!(row.number(INDEX), Some(i as f64));
            assert_eq!(row.len(), 13);
        }
        let values = Dataset::values_of([Value::text("a"), Value::Null]);
        assert!(values.is_values() && !values.is_tuples());
        assert_eq!(values.rows()[1].get(VALUE), Some(&Value::Null));
        assert_eq!(values.rows()[1].number(INDEX), Some(1.));
        assert_eq!(Dataset::length(3).rows()[2].number(INDEX), Some(2.));
    }

    #[test]
    fn hinted_reads_survive_stale_hints_and_pass_other_channels_through() {
        let row = Table::new([("a", Value::Number(1.)), ("v", Value::Number(2.))]);
        for stale in [0, 1, 2, 99] {
            let hint = std::cell::Cell::new(stale);
            assert_eq!(
                Channel::from("v").read_hinted(&row, &hint),
                Value::Number(2.)
            );
            assert_eq!(Channel::from("w").read_hinted(&row, &hint), Value::Null);
        }
        let hint = std::cell::Cell::new(1);
        let accessor = Channel::Accessor(std::rc::Rc::new(|row: &Table| row.value("a")));
        assert_eq!(accessor.read_hinted(&row, &hint), Value::Number(1.));
        let constant = Channel::Constant(Value::Number(f64::NAN));
        assert_eq!(
            constant.read_hinted(&row, &hint).key(),
            constant.read(&row).key()
        );
    }

    #[test]
    fn channels_are_the_same_only_when_known_to_read_alike() {
        let read = |row: &Table| row.value("v");
        let a = Channel::Accessor(std::rc::Rc::new(read));
        // The same code in another closure is another channel: identity is
        // all a cache can trust.
        let b = Channel::Accessor(std::rc::Rc::new(read));
        assert!(a.same(&a.clone()));
        assert!(!a.same(&b));
        assert!(Channel::from("v").same(&Channel::Field("v".into())));
        assert!(!Channel::from("v").same(&Channel::from("w")));
        assert!(!Channel::from("v").same(&a));
        assert!(Channel::Constant(Value::Number(0.)).same(&Channel::Constant(Value::Number(-0.))));
        assert!(!Channel::Constant(Value::Number(1.)).same(&Channel::Constant(Value::text("1"))));
        assert!(!Channel::Constant(Value::text("v")).same(&Channel::from("v")));
        let rows = Dataset::new([Table::empty()]);
        assert!(rows.same_rows(&rows.clone()));
        assert!(!rows.same_rows(&Dataset::new([Table::empty()])));
    }
}
