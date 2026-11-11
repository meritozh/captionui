//! The interval ends a mark derives from its value channels: its
//! `interval` buckets and the baselines its kind grows from.

use crate::mark::{Mark, MarkKind};
use crate::value::Value;

/// Grow a mark's interval channels from its value channel, the way Plot's
/// `barY` grows from zero to `y` and its `rectY`/`cell` marks fill their
/// missing interval ends.
///
/// Plot derives these from the mark's own options, so the same derivation has
/// to run again on a grouped row: the grouping writes the reduced value into
/// `y` after the raw rows already had their intervals derived, and a bar with
/// a `y` but no `y1`/`y2` draws nothing.
pub(super) fn derive_intervals(
    mark: &Mark,
    x: &Value,
    y: &Value,
    x1: &mut Option<Value>,
    x2: &mut Option<Value>,
    y1: &mut Option<Value>,
    y2: &mut Option<Value>,
) {
    let unset = |start: &Option<Value>, end: &Option<Value>| start.is_none() && end.is_none();
    // An interval buckets a singular date or number into a continuous extent:
    // `x` (or `y`) becomes the `[start, end)` that spans its bucket, so a
    // `rectY` over dates draws one bar per bucket. This runs first, before the
    // mark's own baseline derivation, and only when the mark names no interval
    // channels of its own.
    if let Some(interval) = mark.interval {
        // The interval buckets the mark's *primary* positional channel — x when
        // the mark reads an x, else y — so a `rectY` over dates buckets its
        // date axis and leaves the value axis to the baseline derivation below.
        // Bucketing both would turn the count into an extent too.
        //
        // Plot names the axes per mark: a `rect` buckets both
        // (`maybeTrivialIntervalX(maybeTrivialIntervalY)`), a `barY` its
        // value, y (`maybeIntervalY`), and a `barX` its value, x.
        let (on_x, on_y) = match mark.kind {
            MarkKind::Rect => (true, true),
            MarkKind::BarY => (false, true),
            MarkKind::BarX => (true, false),
            _ if mark.channels.x.is_some() => (true, false),
            _ => (false, true),
        };
        if on_x
            && unset(x1, x2)
            && !x.is_null()
            && let Some((start, end)) = interval.extent(x)
        {
            *x1 = Some(interval_value(x, start));
            *x2 = Some(interval_value(x, end));
        }
        if on_y
            && unset(y1, y2)
            && !y.is_null()
            && let Some((start, end)) = interval.extent(y)
        {
            *y1 = Some(interval_value(y, start));
            *y2 = Some(interval_value(y, end));
        }
    }
    // Plot's `maybeOptionalZero` for rules: a `ruleX` given a `y` spans from
    // zero to it, and one given only `y1` or `y2` spans from zero to that.
    let optional_zero = |v: &Value, start: &mut Option<Value>, end: &mut Option<Value>| {
        if !v.is_null() && start.is_none() && end.is_none() {
            *start = Some(Value::Number(0.));
            *end = Some(v.clone());
        } else if v.is_null() {
            match (start.is_some(), end.is_some()) {
                (true, false) => {
                    *end = start.take();
                    *start = Some(Value::Number(0.));
                }
                (false, true) => *start = Some(Value::Number(0.)),
                _ => {}
            }
        }
    };
    match mark.kind {
        MarkKind::RuleX => optional_zero(y, y1, y2),
        MarkKind::RuleY => optional_zero(x, x1, x2),
        // Plot's difference compares y with zero when given only y and one
        // x; with two x's (Plot's `shiftX`) it compares the two placements.
        MarkKind::DifferenceY if unset(y1, y2) && unset(x1, x2) && !y.is_null() => {
            *y1 = Some(Value::Number(0.));
            *y2 = Some(y.clone());
        }
        MarkKind::DifferenceX if unset(x1, x2) && unset(y1, y2) && !x.is_null() => {
            *x1 = Some(Value::Number(0.));
            *x2 = Some(x.clone());
        }
        _ => {}
    }
    match mark.kind {
        MarkKind::BarY | MarkKind::AreaY | MarkKind::WaffleY if unset(y1, y2) && !y.is_null() => {
            *y1 = Some(Value::Number(0.));
            *y2 = Some(y.clone());
        }
        MarkKind::BarX | MarkKind::AreaX | MarkKind::WaffleX if unset(x1, x2) && !x.is_null() => {
            *x1 = Some(Value::Number(0.));
            *x2 = Some(x.clone());
        }
        // `rectX`/`rectY` with one value channel is a bar: its interval runs
        // from the baseline to the value, which is how Plot's implicit stack
        // reads it.
        MarkKind::RectY if unset(y1, y2) && !y.is_null() => {
            *y1 = Some(Value::Number(0.));
            *y2 = Some(y.clone());
        }
        // `rectX` grows along x from the baseline, so its *value* is the x
        // interval's end and its cross axis is the index.
        MarkKind::RectX if unset(x1, x2) && !x.is_null() => {
            *x1 = Some(Value::Number(0.));
            *x2 = Some(x.clone());
        }
        // Plot's bars and rects given only an end (`{y2: "count"}`) span
        // from zero to it, unstacked, so the series overlap.
        MarkKind::BarY | MarkKind::RectY | MarkKind::AreaY if y1.is_none() && y2.is_some() => {
            *y1 = Some(Value::Number(0.));
        }
        MarkKind::BarX | MarkKind::RectX | MarkKind::AreaX if x1.is_none() && x2.is_some() => {
            *x1 = Some(Value::Number(0.));
        }
        MarkKind::Cell => {
            if x1.is_none() && !x.is_null() {
                *x1 = Some(x.clone());
            }
            if y1.is_none() && !y.is_null() {
                *y1 = Some(y.clone());
            }
        }
        _ => {}
    }
}

/// An interval boundary as a [`Value`], matching the singuar value it buckets:
/// a date stays a date (so the axis scales temporally and labels as calendar
/// dates), a number stays a number.
fn interval_value(source: &Value, millis_or_number: f64) -> Value {
    if source.as_date().is_some() {
        Value::date_from_millis(millis_or_number as i64)
    } else {
        Value::Number(millis_or_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::{Interval, Mark, MarkKind};
    use crate::prelude::*;
    use crate::render::prepare;
    use crate::table::{Dataset, Table};
    use crate::value::PlotKind;

    #[test]
    fn a_numeric_step_buckets_x_into_an_interval() {
        let data = vec![
            Table::empty()
                .with("t", Value::Number(5.))
                .with("v", Value::Number(2.)),
        ];
        let mark = Mark::new(MarkKind::RectY)
            .x("t")
            .y("v")
            .interval(Interval::Step(10.));
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        // 5 floors to [0, 10); the value grows the y interval to [0, 2].
        assert_eq!(rows[0].x1, Some(Value::Number(0.)));
        assert_eq!(rows[0].x2, Some(Value::Number(10.)));
        assert_eq!(rows[0].y1, Some(Value::Number(0.)));
        assert_eq!(rows[0].y2, Some(Value::Number(2.)));
    }

    #[test]
    fn a_day_interval_buckets_dates_by_calendar_day() {
        let date = Value::date(chrono::NaiveDate::from_ymd_opt(2024, 3, 15).unwrap());
        let data = vec![Table::empty().with("t", date).with("v", Value::Number(4.))];
        let mark = Mark::new(MarkKind::RectY)
            .x("t")
            .y("v")
            .interval(Interval::Day);
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        let start = rows[0].x1.as_ref().unwrap().as_number().unwrap();
        let end = rows[0].x2.as_ref().unwrap().as_number().unwrap();
        // Midnight on the 15th to midnight on the 16th.
        let midnight = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2024, 3, 15, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis() as f64;
        assert_eq!(start, midnight);
        assert_eq!(end, midnight + 86_400_000.);
    }

    #[test]
    fn month_interval_domain_spans_the_data() {
        let dates = (0..3)
            .map(|m| {
                Table::empty()
                    .with(
                        "d",
                        Value::date(chrono::NaiveDate::from_ymd_opt(2024, m + 1, 15).unwrap()),
                    )
                    .with("v", Value::Number(1.))
            })
            .collect::<Vec<_>>();
        let mark = Mark::new(MarkKind::RectY)
            .x("d")
            .y("v")
            .interval(Interval::Month);
        let prepared = prepare(&Dataset::new(dates), &[mark]);
        let xs = crate::render::basic::x_values(&prepared[0]);
        let first = xs[0].clone();
        let kinds: Vec<_> = xs.iter().map(|v| v.kind()).collect();
        assert!(
            kinds.iter().all(|k| *k == PlotKind::Date),
            "interval boundaries must stay dates: {kinds:?}"
        );
        assert_eq!(first.kind(), PlotKind::Date);
        // First boundary is Jan 1 2024, not 1980.
        let millis = first.as_date().unwrap().timestamp_millis();
        assert_eq!(
            millis,
            chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2024, 1, 1, 0, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );
    }
}
