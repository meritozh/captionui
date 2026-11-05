//! Tick generation and tick labels: Plot's d3-style "nice" ticks for linear
//! scales and calendar ticks for time scales.
//!
//! Plot labels axes at round values rather than at arbitrary divisions of the
//! domain, and time axes at calendar boundaries (days, weeks, months, years).
//! This module reproduces both.

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use gpui::SharedString;

use crate::scale::Transform;
use crate::value::format_number;

/// About `count` round values inside `domain`, for a linear-family scale.
///
/// The step is a power of ten times 1, 2 or 5, so labels read `0, 0.5, 1` or
/// `0, 20, 40` rather than `0, 0.333, 0.667`. A log scale labels its decades
/// and their 2× and 5× multiples, the way Plot does.
pub fn linear_ticks(domain: [f64; 2], count: usize, transform: Transform) -> Vec<f64> {
    let count = count.max(2);
    let [lo, hi] = domain;
    if !lo.is_finite() || !hi.is_finite() {
        return vec![];
    }
    if lo == hi {
        return vec![lo];
    }

    if transform == Transform::Log {
        return log_ticks(lo, hi, count);
    }

    nice_linear_ticks(lo, hi, count)
}

/// Round values spaced by a nice step, the linear-scale tick layout.
///
/// Each tick is `i * step` for a whole `i`, rather than a running sum, so a
/// step below one ulp of the domain cannot stall the loop and rounding error
/// does not accumulate across the axis.
fn nice_linear_ticks(lo: f64, hi: f64, count: usize) -> Vec<f64> {
    /// More ticks than any axis can label; a bound against degenerate input.
    const MAX_TICKS: f64 = 1_000.;
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let span = hi - lo;
    if !span.is_finite() {
        return vec![lo, hi];
    }
    let step = nice_step(span / (count - 1) as f64);
    if !step.is_finite() || step <= 0. {
        return vec![lo];
    }
    let first = (lo / step - 1e-9).ceil();
    let last = (hi / step + 1e-9).floor();
    if !(last - first).is_finite() || last - first > MAX_TICKS || last < first {
        // The step is lost in the domain's magnitude (`1e16..1e16 + 4`), so
        // no round value separates the ends: label the ends themselves.
        return vec![lo, hi];
    }
    (0..=(last - first) as usize)
        .map(|i| round_tick((first + i as f64) * step, step))
        .collect()
}

/// Log ticks: the decades inside `lo..=hi`, subdivided by 2 and 5 when the
/// range spans few decades.
fn log_ticks(lo: f64, hi: f64, count: usize) -> Vec<f64> {
    let lo = lo.max(f64::MIN_POSITIVE);
    if hi <= 0. {
        return vec![];
    }
    let first = lo.log10().floor() as i32;
    let last = hi.log10().ceil() as i32;
    // The decade boundaries inside the domain, with a little slack because
    // `log10(1000)` is `2.9999999999999996`.
    let inner_first = (lo.log10() - 1e-9).ceil() as i32;
    let inner_last = (hi.log10() + 1e-9).floor() as i32;

    // A log axis whose data holds fewer than two decade boundaries has too few
    // to label, so it falls back to the linear nice ticks inside it:
    // 1800..2600 labels 2000, 2200, 2400 rather than a lone 2000. A domain
    // like 10..100 holds both of its decades, so it labels them.
    if inner_last - inner_first < 1 {
        let mut ticks = nice_linear_ticks(lo, hi, count);
        ticks.retain(|tick| *tick >= lo * (1. - 1e-12) && *tick <= hi * (1. + 1e-12));
        return ticks;
    }

    let decades = (last - first) as usize;
    let subdivisions: &[f64] = if decades * 4 <= count {
        &[1., 2., 5.]
    } else if decades * 2 <= count {
        &[1., 2.]
    } else {
        &[1.]
    };

    let mut ticks = Vec::new();
    for exponent in first..=last {
        let base = 10f64.powi(exponent);
        for multiple in subdivisions {
            let tick = base * multiple;
            if tick >= lo * (1. - 1e-12) && tick <= hi * (1. + 1e-12) {
                ticks.push(round_tick(tick, base));
            }
        }
    }
    ticks
}

/// The smallest nice step at least `rough`: `1`, `2` or `5` times a power of ten.
///
/// The cut-offs are d3's `tickIncrement`: the geometric midpoints between the
/// candidates, so `0.25` rounds to `0.2` rather than `0.5` and a domain of
/// `0..1` labels five intervals instead of two.
pub(crate) fn nice_step(rough: f64) -> f64 {
    if !rough.is_finite() || rough <= 0. {
        return 1.;
    }
    let exponent = rough.log10().floor();
    let base = 10f64.powf(exponent);
    let fraction = rough / base;
    let nice = if fraction < 2f64.sqrt() {
        1.
    } else if fraction < 10f64.sqrt() {
        2.
    } else if fraction < 50f64.sqrt() {
        5.
    } else {
        10.
    };
    nice * base
}

/// Remove the float noise a repeated addition leaves in a tick.
fn round_tick(value: f64, step: f64) -> f64 {
    let places = (-step.log10().floor()).max(0.) as i32 + 1;
    let factor = 10f64.powi(places);
    (value * factor).round() / factor
}

/// Calendar ticks inside a millisecond domain, the way Plot labels a time axis.
///
/// The tick interval is chosen from the durations Plot uses — seconds,
/// minutes, hours, days, weeks, months, years — so every label falls on a
/// calendar boundary.
pub fn time_ticks(domain: [f64; 2], count: usize) -> Vec<f64> {
    let count = count.max(2);
    let [lo, hi] = domain;
    if !lo.is_finite() || !hi.is_finite() || lo >= hi {
        return vec![lo];
    }

    /// The most ticks a time axis lays out; a denser request coarsens the
    /// step rather than cutting the axis off partway through its domain.
    const MAX_TICKS: f64 = 64.;
    let span_ms = hi - lo;
    // d3's `tickInterval` target: the span over the count (not count - 1).
    let rough = (span_ms / count as f64).max(span_ms / MAX_TICKS);
    let step = choose_time_step(rough);

    let start = floor_to_step(lo, step);
    let mut ticks = Vec::new();
    let mut current = start;
    while current <= hi {
        if current >= lo {
            ticks.push(current);
        }
        let next = advance(current, step);
        // A date chrono cannot represent stops the walk instead of looping.
        if next <= current || ticks.len() > 4 * MAX_TICKS as usize {
            break;
        }
        current = next;
    }
    ticks
}

/// A calendar interval: fixed durations, or month/year steps that vary with
/// the calendar.
#[derive(Clone, Copy, Debug)]
enum TimeStep {
    Fixed(Duration),
    /// Whole weeks, each starting on a Sunday at midnight.
    Weeks(u32),
    Months(u32),
}

fn choose_time_step(rough_ms: f64) -> TimeStep {
    const SECOND: f64 = 1000.;
    const MINUTE: f64 = 60. * SECOND;
    const HOUR: f64 = 60. * MINUTE;
    const DAY: f64 = 24. * HOUR;
    const WEEK: f64 = 7. * DAY;
    const MONTH: f64 = 30. * DAY;
    const YEAR: f64 = 365. * DAY;

    let candidates: [f64; 12] = [
        SECOND,
        5. * SECOND,
        15. * SECOND,
        30. * SECOND,
        MINUTE,
        5. * MINUTE,
        15. * MINUTE,
        30. * MINUTE,
        HOUR,
        3. * HOUR,
        6. * HOUR,
        12. * HOUR,
    ];
    for candidate in candidates {
        if rough_ms <= candidate {
            return TimeStep::Fixed(Duration::milliseconds(candidate as i64));
        }
    }
    if rough_ms <= DAY {
        return TimeStep::Fixed(Duration::days(1));
    }
    if rough_ms <= 2. * DAY {
        return TimeStep::Fixed(Duration::days(2));
    }
    // d3's interval ladder: weeks start on Sunday, and 2 weeks sits between a
    // week and a month so a ~6-week axis does not collapse to two ticks.
    if rough_ms <= WEEK {
        return TimeStep::Weeks(1);
    }
    if rough_ms <= 2. * WEEK {
        return TimeStep::Weeks(2);
    }
    if rough_ms <= MONTH {
        return TimeStep::Months(1);
    }
    if rough_ms <= 3. * MONTH {
        return TimeStep::Months(3);
    }
    if rough_ms <= 6. * MONTH {
        return TimeStep::Months(6);
    }
    // Years step by 1, 2 or 5 times a power of ten, so the labels read
    // 2000, 2005, 2010 rather than every seventh year.
    let years = nice_step(rough_ms / YEAR).max(1.).round() as u32;
    TimeStep::Months(12 * years.max(1))
}

fn floor_to_step(millis: f64, step: TimeStep) -> f64 {
    let Some(datetime) = Utc.timestamp_millis_opt(millis as i64).single() else {
        return millis;
    };
    let floored = match step {
        TimeStep::Fixed(duration) => {
            let ms = duration.num_milliseconds();
            if ms < 60_000 {
                let secs = duration.num_seconds().max(1);
                let second = (datetime.second() / secs as u32) * secs as u32;
                datetime
                    .with_second(second)
                    .and_then(|d| d.with_nanosecond(0))
                    .unwrap_or(datetime)
            } else if ms < 3_600_000 {
                let minutes = duration.num_minutes().max(1);
                let minute = (datetime.minute() / minutes as u32) * minutes as u32;
                datetime
                    .with_minute(minute)
                    .and_then(|d| d.with_second(0))
                    .and_then(|d| d.with_nanosecond(0))
                    .unwrap_or(datetime)
            } else {
                let hours = duration.num_hours().max(1);
                let hour = (datetime.hour() / hours as u32) * hours as u32;
                datetime
                    .with_hour(hour)
                    .and_then(|d| d.with_minute(0))
                    .and_then(|d| d.with_second(0))
                    .and_then(|d| d.with_nanosecond(0))
                    .unwrap_or(datetime)
            }
        }
        TimeStep::Weeks(_) => {
            let back = datetime.weekday().num_days_from_sunday() as i64;
            (datetime - Duration::days(back))
                .with_hour(0)
                .and_then(|d| d.with_minute(0))
                .and_then(|d| d.with_second(0))
                .and_then(|d| d.with_nanosecond(0))
                .unwrap_or(datetime)
        }
        TimeStep::Months(months) => {
            let total_months = datetime.year() * 12 + datetime.month0() as i32;
            // `div_euclid` floors, so a year before 0 still lands on a boundary.
            let floored_months = total_months.div_euclid(months as i32) * months as i32;
            Utc.with_ymd_and_hms(
                floored_months.div_euclid(12),
                floored_months.rem_euclid(12) as u32 + 1,
                1,
                0,
                0,
                0,
            )
            .single()
            .unwrap_or(datetime)
        }
    };
    floored.timestamp_millis() as f64
}

fn advance(millis: f64, step: TimeStep) -> f64 {
    match step {
        TimeStep::Fixed(duration) => millis + duration.num_milliseconds() as f64,
        TimeStep::Weeks(weeks) => millis + Duration::weeks(weeks as i64).num_milliseconds() as f64,
        TimeStep::Months(months) => {
            let Some(datetime) = Utc.timestamp_millis_opt(millis as i64).single() else {
                return millis;
            };
            let total = datetime.year() * 12 + datetime.month0() as i32 + months as i32;
            Utc.with_ymd_and_hms(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1, 0, 0, 0)
                .single()
                .map(|d| d.timestamp_millis() as f64)
                .unwrap_or(millis)
        }
    }
}

/// The label for a time-axis tick, chosen from the domain's span the way Plot
/// chooses: a span of days labels dates, a span of hours labels times, and a
/// long span labels months or years.
pub fn format_time_tick(millis: f64, domain: [f64; 2]) -> SharedString {
    let Some(datetime) = Utc.timestamp_millis_opt(millis as i64).single() else {
        return SharedString::from("");
    };
    let span = (domain[1] - domain[0]).abs();
    const DAY: f64 = 86_400_000.;
    const MONTH: f64 = 30. * DAY;
    const YEAR: f64 = 365. * DAY;

    let text = if span >= 3. * YEAR {
        datetime.format("%Y").to_string()
    } else if span >= 2. * MONTH {
        datetime.format("%b %Y").to_string()
    } else if span >= 3. * DAY {
        datetime.format("%b %-d").to_string()
    } else if span >= DAY {
        datetime.format("%b %-d, %H:%M").to_string()
    } else {
        datetime.format("%H:%M").to_string()
    };
    SharedString::from(text)
}

/// The label for a linear tick at `value`, with the precision the tick step
/// implies: a step of `0.25` needs two decimals, a step of `0.5` one.
pub fn format_linear_tick(value: f64, step: f64) -> SharedString {
    if step >= 1. {
        // A whole step prints whole numbers even when the domain itself is
        // fractional, so a tick of 50 in 0..100 reads "50".
        // Below 1e15 an `i64` holds the value exactly; above it the cast
        // would saturate, so the general formatter takes over.
        if value == value.trunc() && value.abs() < 1e15 {
            return SharedString::from(format!("{}", value as i64));
        }
        return SharedString::from(format_number(value));
    }
    SharedString::from(format!(
        "{value:.decimals$}",
        decimals = step_decimals(step)
    ))
}

/// The decimals a step needs to be written exactly: the smallest `d` for which
/// `step * 10^d` is a whole number.
fn step_decimals(step: f64) -> usize {
    let step = step.abs();
    for decimals in 0..=10 {
        let scaled = step * 10f64.powi(decimals as i32);
        if (scaled - scaled.round()).abs() < 1e-9 {
            return decimals;
        }
    }
    10
}

/// A datetime as Plot's default date label: the ISO date, with a time when one
/// is not midnight.
pub fn format_datetime(datetime: DateTime<Utc>) -> SharedString {
    if datetime.time() == chrono::NaiveTime::MIN {
        SharedString::from(datetime.format("%Y-%m-%d").to_string())
    } else {
        SharedString::from(datetime.format("%Y-%m-%d %H:%M").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_ticks_are_round() {
        // A step of 2 rather than 2.5: d3's nice ticks use 1, 2 and 5.
        assert_eq!(
            linear_ticks([0., 10.], 6, Transform::Linear),
            vec![0., 2., 4., 6., 8., 10.]
        );
        assert_eq!(
            linear_ticks([0., 100.], 6, Transform::Linear),
            vec![0., 20., 40., 60., 80., 100.]
        );
        assert_eq!(
            linear_ticks([0., 1.], 5, Transform::Linear),
            vec![0., 0.2, 0.4, 0.6, 0.8, 1.]
        );
    }

    #[test]
    fn log_ticks_are_powers() {
        // Three decades leave room for 1, 2, 5 within each.
        let ticks = linear_ticks([1., 1000.], 12, Transform::Log);
        assert_eq!(
            ticks,
            vec![1., 2., 5., 10., 20., 50., 100., 200., 500., 1000.]
        );

        // Few ticks per decade: decades only.
        let ticks = linear_ticks([1., 1000.], 4, Transform::Log);
        assert_eq!(ticks, vec![1., 10., 100., 1000.]);
    }

    #[test]
    fn log_ticks_subdivide_a_single_decade() {
        // A domain that stays inside one decade still needs several labels.
        let ticks = linear_ticks([1800., 2600.], 6, Transform::Log);
        assert!(ticks.len() >= 3, "only {ticks:?}");
        assert!(
            ticks.iter().all(|t| (1800. ..=2600.).contains(t)),
            "{ticks:?}"
        );
        assert!(ticks.contains(&2000.));

        // Two decades still pick their subdivisions from the tick count.
        let ticks = linear_ticks([1., 100.], 8, Transform::Log);
        assert_eq!(ticks, vec![1., 2., 5., 10., 20., 50., 100.]);
    }

    #[test]
    fn time_ticks_land_on_days() {
        let start = Utc
            .with_ymd_and_hms(2020, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp_millis() as f64;
        let end = Utc
            .with_ymd_and_hms(2020, 1, 31, 0, 0, 0)
            .unwrap()
            .timestamp_millis() as f64;
        let ticks = time_ticks([start, end], 6);
        assert!(ticks.len() >= 4);
        for tick in ticks {
            let dt = Utc.timestamp_millis_opt(tick as i64).single().unwrap();
            assert_eq!((dt.hour(), dt.minute(), dt.second()), (0, 0, 0));
        }
    }

    #[test]
    fn time_labels_scale_with_span() {
        let day = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let domain = [
            day.timestamp_millis() as f64,
            day.timestamp_millis() as f64 + 86_400_000. * 400.,
        ];
        let label = format_time_tick(day.timestamp_millis() as f64, domain);
        assert!(label.contains("2020"));
    }

    #[test]
    fn decimals_follow_step() {
        assert_eq!(format_linear_tick(0.5, 0.25), "0.50");
        assert_eq!(format_linear_tick(20., 20.), "20");
    }
}
