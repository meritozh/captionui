//! Plot's `interval` option, `Interval`: a time or numeric bucket.

/// A time or numeric bucket, Plot's `interval` option: turn a singular value
/// into an extent `[floor, floor + step)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interval {
    /// Every `n` milliseconds.
    Millisecond(u32),
    /// Every `n` seconds.
    Second(u32),
    /// Every `n` minutes.
    Minute(u32),
    /// Every `n` hours.
    Hour(u32),
    /// Every `n` days.
    Days(u32),
    /// Every `n` weeks starting on the given weekday (0 = Sunday).
    Week(u32, u32),
    /// Every `n` months.
    Months(u32),
    /// Every `n` quarters.
    Quarter(u32),
    /// Every `n` years.
    Years(u32),
    /// A fixed numeric step.
    Step(f64),
    /// Whole years, `Years(1)`.
    Year,
    /// Whole calendar months, `Months(1)`.
    Month,
    /// Whole days, `Days(1)`.
    Day,
}

impl Interval {
    /// Plot's interval names: `"day"`, `"month"`, `"3 months"`, `"monday"`,
    /// `"week"`, `"quarter"`, `"5 years"` …
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        let (n, unit) = match name.split_once(' ') {
            Some((n, unit)) => (n.parse::<u32>().ok()?, unit.trim().to_string()),
            None => (1, name.clone()),
        };
        let unit = unit.strip_suffix('s').unwrap_or(&unit).to_string();
        let weekday = [
            "sunday",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
        ];
        if let Some(day) = weekday
            .iter()
            .position(|d| d.trim_end_matches('s') == unit || *d == unit)
        {
            return Some(Interval::Week(n, day as u32));
        }
        Some(match unit.as_str() {
            "millisecond" => Interval::Millisecond(n),
            "second" => Interval::Second(n),
            "minute" => Interval::Minute(n),
            "hour" => Interval::Hour(n),
            "day" => Interval::Days(n),
            "week" => Interval::Week(n, 0),
            "month" => Interval::Months(n),
            "quarter" => Interval::Quarter(n),
            "year" => Interval::Years(n),
            _ => return None,
        })
    }

    /// The extent this interval spans for a value, `(start, end)`, in the
    /// value's own units (dates as UTC instants, numbers as numbers).
    pub fn extent(self, value: &crate::value::Value) -> Option<(f64, f64)> {
        use chrono::{Datelike, TimeZone, Utc};
        let instant = value.as_number()?;
        let fixed = |step: f64| {
            let start = (instant / step).floor() * step;
            Some((start, start + step))
        };
        let months = |n: u32| -> Option<(f64, f64)> {
            let date = Utc
                .timestamp_millis_opt(instant as i64)
                .single()?
                .date_naive();
            let total = date.year() * 12 + date.month0() as i32;
            let start = total - total.rem_euclid(n as i32);
            let at = |m: i32| {
                Utc.with_ymd_and_hms(m.div_euclid(12), m.rem_euclid(12) as u32 + 1, 1, 0, 0, 0)
                    .single()
                    .map(|d| d.timestamp_millis() as f64)
            };
            Some((at(start)?, at(start + n as i32)?))
        };
        const DAY: f64 = 86_400_000.;
        match self {
            Interval::Step(width) if width > 0. => fixed(width),
            Interval::Step(_) => None,
            Interval::Millisecond(n) => fixed(n.max(1) as f64),
            Interval::Second(n) => fixed(1_000. * n.max(1) as f64),
            Interval::Minute(n) => fixed(60_000. * n.max(1) as f64),
            Interval::Hour(n) => fixed(3_600_000. * n.max(1) as f64),
            Interval::Day => Interval::Days(1).extent(value),
            Interval::Days(n) => {
                // d3's utcDay.every(n): days counted within each month.
                let date = Utc
                    .timestamp_millis_opt(instant as i64)
                    .single()?
                    .date_naive();
                let n = n.max(1);
                let day0 = date.day0() - date.day0() % n;
                let start = Utc
                    .with_ymd_and_hms(date.year(), date.month(), day0 + 1, 0, 0, 0)
                    .single()?
                    .timestamp_millis() as f64;
                Some((start, start + DAY * n as f64))
            }
            Interval::Week(n, weekday) => {
                let date = Utc
                    .timestamp_millis_opt(instant as i64)
                    .single()?
                    .date_naive();
                let back = (date.weekday().num_days_from_sunday() + 7 - weekday) % 7;
                let start = Utc
                    .from_utc_datetime(
                        &(date - chrono::Duration::days(back as i64)).and_hms_opt(0, 0, 0)?,
                    )
                    .timestamp_millis() as f64;
                Some((start, start + 7. * DAY * n.max(1) as f64))
            }
            Interval::Month => months(1),
            Interval::Months(n) => months(n.max(1)),
            Interval::Quarter(n) => months(3 * n.max(1)),
            Interval::Year => months(12),
            Interval::Years(n) => months(12 * n.max(1)),
        }
    }

    /// Whether the interval steps through time rather than numbers.
    pub fn is_temporal(self) -> bool {
        !matches!(self, Interval::Step(_))
    }
}

impl From<f64> for Interval {
    fn from(step: f64) -> Self {
        Interval::Step(step)
    }
}

impl From<i32> for Interval {
    fn from(step: i32) -> Self {
        Interval::Step(step as f64)
    }
}

impl From<&str> for Interval {
    fn from(name: &str) -> Self {
        Interval::parse(name).unwrap_or(Interval::Step(1.))
    }
}
