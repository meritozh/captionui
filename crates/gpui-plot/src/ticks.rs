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
    // d3's `tickIncrement`: the span over the count.
    let step = nice_step(span / count as f64);
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
pub fn nice_step(rough: f64) -> f64 {
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

/// d3's tick step for `count` ticks over `[start, stop]`, as `ticks` lays
/// them out.
pub fn tick_increment_step(start: f64, stop: f64, count: usize) -> f64 {
    let span = (stop - start).abs();
    if span == 0. || !span.is_finite() {
        return 1.;
    }
    nice_step(span / count.max(1) as f64)
}

/// d3's `log.ticks(count)`: every multiple 1–9 of each decade when the
/// domain spans fewer decades than the count, else powers of ten.
pub fn d3_log_ticks(lo: f64, hi: f64, count: usize) -> Vec<f64> {
    let (u, v) = (lo.min(hi), lo.max(hi));
    if u <= 0. || !v.is_finite() {
        return Vec::new();
    }
    let (i, j) = (u.log10(), v.log10());
    let n = count.max(1) as f64;
    let mut z = Vec::new();
    if j - i < n {
        let (i0, j0) = (i.floor() as i32, j.ceil() as i32);
        for e in i0..=j0 {
            for k in 1..10 {
                let t = if e < 0 {
                    k as f64 / 10f64.powi(-e)
                } else {
                    k as f64 * 10f64.powi(e)
                };
                if t < u * (1. - 1e-12) {
                    continue;
                }
                if t > v * (1. + 1e-12) {
                    break;
                }
                z.push(t);
            }
        }
        if (z.len() as f64) * 2. < n {
            z = nice_linear_ticks(u, v, count);
        }
    } else {
        let step = nice_step((j - i) / n.min(j - i).max(1.));
        let first = (i / step).ceil();
        let last = (j / step).floor();
        let mut e = first;
        while e <= last {
            z.push(10f64.powf(e * step));
            e += 1.;
        }
    }
    z
}

/// d3's `log.tickFormat(count)`: SI labels, blank for the multiples beyond
/// what the count leaves room for.
pub fn d3_log_tick_label(
    value: f64,
    count: usize,
    tick_total: usize,
    spec: Option<&str>,
) -> String {
    let k = (10. * count as f64 / tick_total.max(1) as f64).max(1.);
    let mut i = value / 10f64.powf(value.log10().round());
    if i * 10. < 10. - 0.5 {
        i *= 10.;
    }
    if i <= k + 1e-9 {
        let mut format = crate::format::NumberFormat::parse(spec.unwrap_or("s"));
        format.trim_if_unset();
        format.format(value)
    } else {
        String::new()
    }
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
    // d3's `tickInterval` target: the span over the count.
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

/// d3's `utcTickInterval`: the interval of its ladder nearest (by ratio) to
/// the span over the count, or a nice number of years past the ladder.
fn choose_time_step(target: f64) -> TimeStep {
    const SECOND: f64 = 1000.;
    const MINUTE: f64 = 60. * SECOND;
    const HOUR: f64 = 60. * MINUTE;
    const DAY: f64 = 24. * HOUR;
    const WEEK: f64 = 7. * DAY;
    const MONTH: f64 = 30. * DAY;
    const YEAR: f64 = 365. * DAY;
    let fixed = |ms: f64| TimeStep::Fixed(Duration::milliseconds(ms as i64));
    let ladder: [(f64, TimeStep); 18] = [
        (SECOND, fixed(SECOND)),
        (5. * SECOND, fixed(5. * SECOND)),
        (15. * SECOND, fixed(15. * SECOND)),
        (30. * SECOND, fixed(30. * SECOND)),
        (MINUTE, fixed(MINUTE)),
        (5. * MINUTE, fixed(5. * MINUTE)),
        (15. * MINUTE, fixed(15. * MINUTE)),
        (30. * MINUTE, fixed(30. * MINUTE)),
        (HOUR, fixed(HOUR)),
        (3. * HOUR, fixed(3. * HOUR)),
        (6. * HOUR, fixed(6. * HOUR)),
        (12. * HOUR, fixed(12. * HOUR)),
        (DAY, fixed(DAY)),
        (2. * DAY, fixed(2. * DAY)),
        (WEEK, TimeStep::Weeks(1)),
        (MONTH, TimeStep::Months(1)),
        (3. * MONTH, TimeStep::Months(3)),
        (YEAR, TimeStep::Months(12)),
    ];
    let i = ladder
        .iter()
        .position(|(step, _)| *step > target)
        .unwrap_or(ladder.len());
    if i == ladder.len() {
        let years = nice_step(target / YEAR).max(1.).round() as u32;
        return TimeStep::Months(12 * years.max(1));
    }
    if i == 0 {
        return fixed(nice_step(target).max(1.));
    }
    let (lo, hi) = (&ladder[i - 1], &ladder[i]);
    if target / lo.0 < hi.0 / target {
        lo.1
    } else {
        hi.1
    }
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
            Utc.with_ymd_and_hms(
                total.div_euclid(12),
                total.rem_euclid(12) as u32 + 1,
                1,
                0,
                0,
                0,
            )
            .single()
            .map(|d| d.timestamp_millis() as f64)
            .unwrap_or(millis)
        }
    }
}

/// Plot's time tick labels for a list of ticks: the finest calendar field
/// every tick sits on (year, month, day, hour …) labels each tick, and the
/// next field up is added on a second line where it changes — `Jan\n2021`,
/// then `Apr`, `Jul` … — Plot's `formatConditional`. `top` puts the second
/// line above, as Plot does for a top axis.
pub fn plot_time_labels(ticks: &[f64], top: bool) -> Vec<String> {
    let dates: Vec<DateTime<Utc>> = ticks
        .iter()
        .filter_map(|m| Utc.timestamp_millis_opt(*m as i64).single())
        .collect();
    if dates.len() != ticks.len() {
        return ticks
            .iter()
            .map(|m| format_time_tick(*m, [ticks[0], *ticks.last().unwrap_or(m)]).to_string())
            .collect();
    }
    let max_step = ticks
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0., f64::max);
    const DAY: f64 = 86_400_000.;
    const MONTH: f64 = 30. * DAY;
    let on = |f: &dyn Fn(&DateTime<Utc>) -> bool| dates.iter().all(f);
    let midnight = |d: &DateTime<Utc>| {
        d.hour() == 0 && d.minute() == 0 && d.second() == 0 && d.timestamp_subsec_millis() == 0
    };
    // (first line, second line) formats, d3-time-format specifiers.
    let (f1, f2): (&str, Option<&str>) = if on(&|d| midnight(d) && d.day() == 1 && d.month() == 1) {
        ("%Y", None)
    } else if on(&|d| midnight(d) && d.day() == 1) {
        ("%b", Some("%Y"))
    } else if max_step <= 6. * MONTH && on(&midnight) {
        ("%-d", Some("%b"))
    } else if max_step <= 3. * DAY && on(&|d| d.minute() == 0 && d.second() == 0) {
        ("%-I %p", Some("%b %-d"))
    } else if max_step <= 6. * 3_600_000. && on(&|d| d.second() == 0) {
        ("%-I:%M", Some("%p"))
    } else {
        (":%S", Some("%-I:%M"))
    };
    let mut previous: Option<String> = None;
    dates
        .iter()
        .map(|d| {
            let first = crate::format::format_time(f1, *d);
            let Some(f2) = f2 else {
                return first;
            };
            let second = crate::format::format_time(f2, *d);
            let changed = previous.as_ref() != Some(&second);
            previous = Some(second.clone());
            match (changed, top) {
                (false, _) => first,
                (true, false) => format!("{first}\n{second}"),
                (true, true) => format!("{second}\n{first}"),
            }
        })
        .collect()
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
