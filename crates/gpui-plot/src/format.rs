//! Number and date formatting the way Plot's `tickFormat` reads it: a
//! d3-format specifier such as `"+f"`, `",.0f"` or `"~s"` for numbers, and a
//! d3-time-format specifier such as `"%Y"` or `"%b %d"` for dates.

use chrono::{DateTime, Utc};

/// A d3-format specifier, `[[fill]align][sign][symbol][0][width][,][.precision][~][type]`.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    fill: char,
    align: Option<char>,
    sign: char,
    symbol: Option<char>,
    zero: bool,
    width: Option<usize>,
    comma: bool,
    precision: Option<usize>,
    trim: bool,
    kind: Option<char>,
}

impl NumberFormat {
    /// Parse a specifier; an invalid one falls back to d3's default, which
    /// formats like JavaScript's `Number.toString` with a minus sign.
    pub fn parse(spec: &str) -> Self {
        let chars: Vec<char> = spec.chars().collect();
        let mut i = 0;
        let mut format = Self {
            fill: ' ',
            align: None,
            sign: '-',
            symbol: None,
            zero: false,
            width: None,
            comma: false,
            precision: None,
            trim: false,
            kind: None,
        };
        let is_align = |c: char| matches!(c, '<' | '>' | '=' | '^');
        if chars.len() >= 2 && is_align(chars[1]) {
            format.fill = chars[0];
            format.align = Some(chars[1]);
            i = 2;
        } else if chars.first().is_some_and(|c| is_align(*c)) {
            format.align = Some(chars[0]);
            i = 1;
        }
        if let Some(c) = chars.get(i)
            && matches!(c, '-' | '+' | ' ' | '(')
        {
            format.sign = *c;
            i += 1;
        }
        if let Some(c) = chars.get(i)
            && matches!(c, '$' | '#')
        {
            format.symbol = Some(*c);
            i += 1;
        }
        if chars.get(i) == Some(&'0') {
            format.zero = true;
            i += 1;
        }
        let start = i;
        while chars.get(i).is_some_and(char::is_ascii_digit) {
            i += 1;
        }
        if i > start {
            format.width = chars[start..i].iter().collect::<String>().parse().ok();
        }
        if chars.get(i) == Some(&',') {
            format.comma = true;
            i += 1;
        }
        if chars.get(i) == Some(&'.') {
            i += 1;
            let start = i;
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            format.precision = chars[start..i].iter().collect::<String>().parse().ok();
        }
        if chars.get(i) == Some(&'~') {
            format.trim = true;
            i += 1;
        }
        format.kind = chars.get(i).copied();
        format
    }

    /// d3's log tick format trims a specifier that names no precision.
    pub fn trim_if_unset(&mut self) {
        if self.precision.is_none() {
            self.trim = true;
        }
    }

    /// Format `value`.
    pub fn format(&self, value: f64) -> String {
        if !value.is_finite() {
            return if value.is_nan() {
                "NaN".into()
            } else if value > 0. {
                "Infinity".into()
            } else {
                "−Infinity".into()
            };
        }
        let negative = value < 0. || (value == 0. && value.is_sign_negative() && false);
        let magnitude = value.abs();
        let (mut body, suffix) = match self.kind {
            Some('f') => (
                to_fixed(magnitude, self.precision.unwrap_or(6)),
                String::new(),
            ),
            Some('%') => (
                to_fixed(magnitude * 100., self.precision.unwrap_or(6)),
                "%".to_string(),
            ),
            Some('p') => (
                significant(magnitude * 100., self.precision.unwrap_or(6)),
                "%".into(),
            ),
            Some('d') => (format!("{}", magnitude.round()), String::new()),
            Some('e') => (
                exponential(magnitude, self.precision.unwrap_or(6)),
                String::new(),
            ),
            Some('g') => (
                significant(magnitude, self.precision.unwrap_or(6)),
                String::new(),
            ),
            Some('r') => (
                significant(magnitude, self.precision.unwrap_or(6)),
                String::new(),
            ),
            Some('s') => si(magnitude, self.precision.unwrap_or(6)),
            Some('x') => (format!("{:x}", magnitude.round() as i64), String::new()),
            Some('X') => (format!("{:X}", magnitude.round() as i64), String::new()),
            Some('b') => (format!("{:b}", magnitude.round() as i64), String::new()),
            Some('o') => (format!("{:o}", magnitude.round() as i64), String::new()),
            Some('c') => (
                char::from_u32(magnitude as u32)
                    .map(String::from)
                    .unwrap_or_default(),
                String::new(),
            ),
            _ => {
                // No type: like `g` but trimmed, d3's default (`~g` with 12
                // significant digits).
                let text = match self.precision {
                    Some(precision) => significant(magnitude, precision),
                    None => shortest(magnitude),
                };
                (text, String::new())
            }
        };
        if self.trim || self.kind.is_none() {
            body = trim_zeros(&body);
        }
        if self.comma {
            body = group_thousands(&body);
        }
        // A value that rounds to zero loses its minus sign, as d3's does.
        let rounds_to_zero = body.chars().all(|c| !c.is_ascii_digit() || c == '0');
        let negative = negative && !rounds_to_zero;
        let sign = match (negative, self.sign) {
            (true, '(') => "(",
            (true, _) => "−",
            (false, '+') => "+",
            (false, ' ') => " ",
            _ => "",
        };
        let close = if negative && self.sign == '(' {
            ")"
        } else {
            ""
        };
        let currency = if self.symbol == Some('$') { "$" } else { "" };
        let mut text = format!("{sign}{currency}{body}{suffix}{close}");
        if let Some(width) = self.width {
            let length = text.chars().count();
            if length < width {
                let pad = width - length;
                if self.zero {
                    let zeros: String = "0".repeat(pad);
                    text = format!("{sign}{currency}{zeros}{body}{suffix}{close}");
                } else {
                    let fill: String = std::iter::repeat_n(self.fill, pad).collect();
                    text = match self.align.unwrap_or('>') {
                        '<' => format!("{text}{fill}"),
                        '^' => {
                            let left: String = std::iter::repeat_n(self.fill, pad / 2).collect();
                            let right: String =
                                std::iter::repeat_n(self.fill, pad - pad / 2).collect();
                            format!("{left}{text}{right}")
                        }
                        _ => format!("{fill}{text}"),
                    };
                }
            }
        }
        text
    }
}

/// d3's `linear.tickFormat(count, specifier)`: the specifier, defaulting to
/// `",f"`, with its precision filled in from the tick step when it names
/// none, so `"+f"` over ticks a fifth apart labels `+0.2`, not `+0.200000`.
pub fn tick_format(start: f64, stop: f64, count: usize, spec: Option<&str>) -> NumberFormat {
    tick_format_fractional(start, stop, count as f64, spec)
}

/// [`tick_format`] for a fractional count, as Plot's `tickSpacing` gives.
pub fn tick_format_fractional(
    start: f64,
    stop: f64,
    count: f64,
    spec: Option<&str>,
) -> NumberFormat {
    let step = crate::ticks::tick_increment_step(start, stop, count);
    let mut format = NumberFormat::parse(spec.unwrap_or(",f"));
    if format.precision.is_none() {
        let exponent = |x: f64| {
            if x == 0. {
                0
            } else {
                x.abs().log10().floor() as i32
            }
        };
        match format.kind {
            Some('s') => {
                let value = start.abs().max(stop.abs());
                let precision = (exponent(value) / 3 * 3 - exponent(step))
                    .clamp(0, 20)
                    .max(0) as usize;
                // d3's `precisionPrefix`.
                let prefix = ((exponent(value) as f64 / 3.).floor().clamp(-8., 8.) * 3.) as i32;
                format.precision = Some((prefix - exponent(step)).max(0) as usize);
                let _ = precision;
            }
            None | Some('e') | Some('g') | Some('p') | Some('r') => {
                // d3's `precisionRound(step, max)`.
                let max = start.abs().max(stop.abs());
                let precision = (exponent(max) - exponent(step)).max(0) + 1;
                format.precision =
                    Some(precision as usize - if format.kind == Some('e') { 1 } else { 0 });
            }
            Some('f') | Some('%') => {
                let precision =
                    (-exponent(step)).max(0) - if format.kind == Some('%') { 2 } else { 0 };
                format.precision = Some(precision.max(0) as usize);
            }
            _ => {}
        }
    }
    format
}

/// Format `value` with a d3-format specifier.
pub fn format_number(spec: &str, value: f64) -> String {
    NumberFormat::parse(spec).format(value)
}

/// JavaScript's `toFixed`: a tie rounds away from zero (`0.25` → `0.3`),
/// where Rust's formatting rounds it to even.
fn to_fixed(value: f64, precision: usize) -> String {
    let scale = 10f64.powi(precision as i32);
    let rounded = (value * scale).round() / scale;
    if rounded.is_finite() && (value * scale).abs() < 1e15 {
        format!("{rounded:.precision$}")
    } else {
        format!("{value:.precision$}")
    }
}

/// `value` to `precision` significant digits, in fixed notation when the
/// exponent allows, as JavaScript's `toPrecision`.
fn significant(value: f64, precision: usize) -> String {
    let precision = precision.max(1);
    if value == 0. {
        return format!("{:.*}", precision - 1, 0.);
    }
    let exponent = value.abs().log10().floor() as i32;
    if exponent < -6 || exponent >= precision as i32 {
        return exponential(value, precision - 1);
    }
    let decimals = (precision as i32 - 1 - exponent).max(0) as usize;
    format!("{value:.decimals$}")
}

fn exponential(value: f64, precision: usize) -> String {
    let text = format!("{value:.precision$e}");
    // Rust writes `1.5e3`; JavaScript writes `1.5e+3`.
    match text.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') => {
            format!("{mantissa}e+{exponent}")
        }
        _ => text,
    }
}

/// JavaScript's shortest round-trip form, for the default specifier.
fn shortest(value: f64) -> String {
    let rounded = format!("{:.12}", value);
    let trimmed = trim_zeros(&rounded);
    if value != 0. && (value.abs() >= 1e21 || value.abs() < 1e-6) {
        return exponential(value, 11).replace(".00000000000", "");
    }
    trimmed
}

/// SI-prefixed: `1.2k`, `3.4M`, d3's `s`.
fn si(value: f64, precision: usize) -> (String, String) {
    const PREFIXES: [&str; 17] = [
        "y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y",
    ];
    if value == 0. {
        return (significant(0., precision), String::new());
    }
    let exponent = value.log10().floor() as i32;
    let index = ((exponent.div_euclid(3)).clamp(-8, 8) + 8) as usize;
    let scaled = value / 10f64.powi((index as i32 - 8) * 3);
    (significant(scaled, precision), PREFIXES[index].to_string())
}

fn trim_zeros(text: &str) -> String {
    match text.split_once('.') {
        Some((whole, fraction)) => {
            let (fraction, rest): (String, String) = {
                let digits: String = fraction.chars().take_while(char::is_ascii_digit).collect();
                let rest = fraction[digits.len()..].to_string();
                (digits.trim_end_matches('0').to_string(), rest)
            };
            if fraction.is_empty() {
                format!("{whole}{rest}")
            } else {
                format!("{whole}.{fraction}{rest}")
            }
        }
        None => text.to_string(),
    }
}

fn group_thousands(text: &str) -> String {
    let (whole, rest) = match text.find(|c: char| !c.is_ascii_digit()) {
        Some(index) => text.split_at(index),
        None => (text, ""),
    };
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("{grouped}{rest}")
}

/// Format a UTC instant with a d3-time-format specifier.
///
/// The directives d3 shares with `strftime` map directly; d3's `%-d`-style
/// padding modifiers and `%e` (space-padded day) are honoured.
pub fn format_time(spec: &str, date: DateTime<Utc>) -> String {
    let mut out = String::new();
    let mut chars = spec.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let pad = match chars.peek() {
            Some('-') | Some('_') | Some('0') => chars.next(),
            _ => None,
        };
        let Some(directive) = chars.next() else {
            break;
        };
        let text = match directive {
            'L' => format!("{:03}", date.timestamp_subsec_millis()),
            'f' => format!("{:06}", date.timestamp_subsec_micros()),
            'Q' => date.timestamp_millis().to_string(),
            's' => date.timestamp().to_string(),
            'q' => (((date.format("%m").to_string().parse::<u32>().unwrap_or(1) - 1) / 3) + 1)
                .to_string(),
            '%' => "%".into(),
            other => date.format(&format!("%{other}")).to_string(),
        };
        match pad {
            Some('-') => out.push_str(text.trim_start_matches('0').trim_start()),
            Some('_') => out.push_str(&text.replacen('0', " ", 1)),
            _ => out.push_str(&text),
        }
        // `%-d` of `01` must still print `1`, and of `00` nothing at all is
        // wrong; restore a lone zero.
        if pad == Some('-') && text.chars().all(|c| c == '0') {
            out.push('0');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format_like_d3() {
        assert_eq!(format_number("+f", 1.5), "+1.500000");
        assert_eq!(format_number("+.1f", -0.25), "−0.3");
        assert_eq!(format_number(",", 1234567.), "1,234,567");
        assert_eq!(format_number("$,.0f", 1234.5), "$1,235");
        assert_eq!(format_number(".0%", 0.256), "26%");
        assert_eq!(format_number("~s", 1500.), "1.5k");
        assert_eq!(format_number(".2s", 42e6), "42M");
        assert_eq!(format_number("d", 3.), "3");
        assert_eq!(format_number("", 0.1 + 0.2), "0.3");
        assert_eq!(format_number("+d", 0.), "+0");
    }

    #[test]
    fn dates_format_like_d3() {
        let date = chrono::NaiveDate::from_ymd_opt(2023, 3, 7)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        assert_eq!(format_time("%Y", date), "2023");
        assert_eq!(format_time("%b %-d", date), "Mar 7");
        assert_eq!(format_time("%B %e", date), "March  7");
    }
}
