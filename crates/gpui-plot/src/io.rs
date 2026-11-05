//! Reading data the way Plot's examples do: `d3.csvParse`, `d3.tsvParse` and
//! `d3.autoType`.
//!
//! Plot's documentation loads its datasets with `d3.csv(url, d3.autoType)`:
//! the delimited text is split into records, and each field is typed by its
//! text — numbers, ISO 8601 dates, booleans and empty fields become numbers,
//! dates, booleans and nulls. [`csv`] and [`tsv`] do the same here, so a
//! chart reads `"Date"` as a date and `"Close"` as a number without a schema.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, Utc};

use crate::table::{Dataset, Table};
use crate::value::Value;

/// Parse comma-separated text with a header row, typing each field like
/// `d3.autoType`.
pub fn csv(text: &str) -> Dataset {
    delimited(text, b',')
}

/// Parse tab-separated text with a header row, typing each field like
/// `d3.autoType`.
pub fn tsv(text: &str) -> Dataset {
    delimited(text, b'\t')
}

/// The column names of delimited text: its header row, in order.
///
/// A [`Table`] keeps its fields sorted, so a caller that needs the file's
/// column order (Plot's `data.columns`) reads it here.
pub fn columns(text: &str, delimiter: u8) -> Vec<String> {
    rows(text, delimiter).into_iter().next().unwrap_or_default()
}

/// Parse delimited text with a header row into records typed by
/// [`auto_type`].
pub fn delimited(text: &str, delimiter: u8) -> Dataset {
    let mut rows = rows(text, delimiter).into_iter();
    let Some(header) = rows.next() else {
        return Dataset::default();
    };
    Dataset::new(
        rows.filter(|row| !(row.len() == 1 && row[0].is_empty()))
            .map(|row| {
                Table::new(header.iter().enumerate().map(|(index, name)| {
                    (
                        name.clone(),
                        auto_type(row.get(index).map_or("", String::as_str)),
                    )
                }))
            }),
    )
}

/// Split delimited text into rows of fields, RFC 4180 style: a field may be
/// quoted, a quoted field may hold the delimiter, newlines and doubled
/// quotes, and both `\n` and `\r\n` end a row.
pub fn rows(text: &str, delimiter: u8) -> Vec<Vec<String>> {
    let bytes = text.as_bytes();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = Vec::new();
    let mut i = 0;
    let mut quoted = false;
    // Whether anything has been read since the last row ended, so a trailing
    // newline does not add an empty row.
    let mut pending = false;
    while i < bytes.len() {
        let byte = bytes[i];
        if quoted {
            if byte == b'"' {
                if bytes.get(i + 1) == Some(&b'"') {
                    field.push(b'"');
                    i += 1;
                } else {
                    quoted = false;
                }
            } else {
                field.push(byte);
            }
        } else if byte == b'"' && field.is_empty() {
            quoted = true;
            pending = true;
        } else if byte == delimiter {
            row.push(String::from_utf8_lossy(&field).into_owned());
            field.clear();
            pending = true;
        } else if byte == b'\n' || byte == b'\r' {
            if byte == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
            row.push(String::from_utf8_lossy(&field).into_owned());
            field.clear();
            rows.push(std::mem::take(&mut row));
            pending = false;
        } else {
            field.push(byte);
            pending = true;
        }
        i += 1;
    }
    if pending || !field.is_empty() {
        row.push(String::from_utf8_lossy(&field).into_owned());
        rows.push(row);
    }
    rows
}

/// Type one field's text the way `d3.autoType` does.
///
/// Empty text is null; `true` and `false` are booleans; `NaN` is a missing
/// number; text that parses as a number is a number; an ISO 8601 date or
/// date-time is a date (a bare date at UTC midnight, as d3 reads it); any
/// other text stays text.
pub fn auto_type(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    match trimmed {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "NaN" => return Value::Null,
        _ => {}
    }
    if let Some(number) = parse_number(trimmed) {
        return Value::number(number);
    }
    if let Some(date) = parse_iso_date(trimmed) {
        return Value::Date(date);
    }
    Value::text(text.to_string())
}

/// JavaScript's unary `+` on a string, for the forms data files use.
fn parse_number(text: &str) -> Option<f64> {
    // `+"1,000"` is NaN in JavaScript, and Rust agrees; Rust also accepts
    // "inf"/"nan" spellings JavaScript does not, so those are refused.
    let first = text.as_bytes()[0];
    if !(first.is_ascii_digit() || matches!(first, b'-' | b'+' | b'.')) {
        return None;
    }
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).ok().map(|n| n as f64);
    }
    if text.contains(['i', 'n', 'I', 'N']) {
        return None;
    }
    text.parse::<f64>().ok()
}

/// An ISO 8601 date (`2016-01-01`, `2016-01`, `2016`) or date-time
/// (`2016-01-01T12:30`, with optional seconds, fraction and offset).
fn parse_iso_date(text: &str) -> Option<DateTime<Utc>> {
    let bytes = text.as_bytes();
    if bytes.len() < 7 || !bytes[..4].iter().all(u8::is_ascii_digit) || bytes[4] != b'-' {
        return None;
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Some(date.and_hms_opt(0, 0, 0)?.and_utc());
    }
    if bytes.len() == 7
        && let Ok(date) = NaiveDate::parse_from_str(&format!("{text}-01"), "%Y-%m-%d")
    {
        return Some(date.and_hms_opt(0, 0, 0)?.and_utc());
    }
    if let Ok(datetime) = DateTime::<FixedOffset>::parse_from_rfc3339(text) {
        return Some(datetime.with_timezone(&Utc));
    }
    let text = text.strip_suffix('Z').unwrap_or(text);
    for format in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(text, format) {
            return Some(datetime.and_utc());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_type_reads_like_d3() {
        assert_eq!(auto_type(""), Value::Null);
        assert_eq!(auto_type("NaN"), Value::Null);
        assert_eq!(auto_type("true"), Value::Bool(true));
        assert_eq!(auto_type("12.5"), Value::Number(12.5));
        assert_eq!(auto_type("-3e2"), Value::Number(-300.));
        assert_eq!(auto_type("Adelie"), Value::text("Adelie"));
        assert_eq!(auto_type("Infinity"), Value::text("Infinity"));
        let date = auto_type("2013-05-13");
        assert_eq!(
            date.as_date().unwrap().to_rfc3339(),
            "2013-05-13T00:00:00+00:00"
        );
        let datetime = auto_type("2013-05-13T09:30Z");
        assert_eq!(
            datetime.as_date().unwrap().to_rfc3339(),
            "2013-05-13T09:30:00+00:00"
        );
    }

    #[test]
    fn csv_handles_quotes_and_crlf() {
        let data = csv("name,note,n\r\n\"a, b\",\"say \"\"hi\"\"\",1\r\nc,,2\r\n");
        assert_eq!(data.len(), 2);
        assert_eq!(data.rows()[0].value("name"), Value::text("a, b"));
        assert_eq!(data.rows()[0].value("note"), Value::text("say \"hi\""));
        assert_eq!(data.rows()[1].value("note"), Value::Null);
        assert_eq!(data.rows()[1].value("n"), Value::Number(2.));
        assert_eq!(columns("x,y\n1,2", b','), vec!["x", "y"]);
    }
}
