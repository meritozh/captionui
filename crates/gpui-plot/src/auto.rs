//! The `auto` mark: pick a mark type from the data's shape, Plot's `Plot.auto`.
//!
//! Plot's `auto` chooses the mark that "best represents the given dimensions
//! of the data": two quantitative dimensions scatter, a monotonic dimension
//! against a numeric one lines, one quantitative dimension histograms, one
//! ordinal dimension bars. These heuristics mirror Plot's `autoSpec`, scoped to
//! the cases this library composes most cleanly — a dot, a line, a bar, a
//! histogram or a rule. The finer reducer-driven heatmap and density branches
//! are left to explicit marks, which a caller reaches just as easily.

use gpui::{Hsla, SharedString};

use crate::mark::{ChannelOptions, StyleOptions, TransformOptions};
use crate::plot::Plot;
use crate::table::{Dataset, Table};
use crate::transform::GroupReducer;
use crate::value::{PlotKind, Value};

/// Build a plot whose mark is chosen from the shapes of `x` and `y`.
///
/// `x`/`y` name fields, and `color` (optional) colors the chosen mark by a
/// field through the plot's quantitative color scale. The choice mirrors Plot's
/// `auto`:
///
/// - two quantitative dimensions scatter (`dot`), or `line` when x is
///   monotonic;
/// - one quantitative dimension histograms (`rectY` binned by that axis);
/// - an ordinal `x` against a quantitative `y` bar-charts (`barY`), and the
///   transposed case gives `barX`;
/// - a bare ordinal axis counts (`group` + bar).
pub fn build(data: &[Table], x: Option<&str>, y: Option<&str>, color: Option<&str>) -> Plot {
    let dataset = Dataset::new(data.iter().cloned());
    let x_kind = x.and_then(|field| first_kind(&dataset, field));
    let y_kind = y.and_then(|field| first_kind(&dataset, field));

    let plot = Plot::new(data.iter().cloned());
    // A `color` field becomes a `z` series on the chosen mark, splitting its
    // fill (or a line's stroke) through the plot's ordinal color scale — the
    // way Plot's `auto` maps `color` onto the mark. A helper per branch avoids
    // repeating the "color if present" dance across every mark type.
    let color = color.map(|s| s.to_string());
    match (x_kind, y_kind) {
        // A quantitative x with no y → a histogram of x.
        (Some(kind), None) if kind.is_quantitative() => hist(plot, x.unwrap(), "x"),
        // An ordinal x with no y → count bars per category.
        (Some(_), None) => plot.bar_y(|mark| {
            let mark = mark.x(x.unwrap()).y("count").group_x();
            if let Some(field) = &color {
                mark.z(field.as_str())
            } else {
                mark
            }
        }),
        // Two quantitative dimensions with a `count` color → a heatmap: bin both
        // axes and fill each cell by how many rows landed in it.
        (Some(xk), Some(yk))
            if xk.is_quantitative()
                && yk.is_quantitative()
                && color.as_deref() == Some("count") =>
        {
            heatmap(plot, x.unwrap(), y.unwrap())
        }
        // Two quantitative dimensions → scatter, or a line when x is monotonic.
        (Some(xk), Some(yk)) if xk.is_quantitative() && yk.is_quantitative() => {
            if monotonic_x(&dataset, x.unwrap()) {
                plot.line(|mark| {
                    let mark = mark.x(x.unwrap()).y(y.unwrap());
                    if let Some(field) = &color {
                        mark.z(field.as_str())
                    } else {
                        mark
                    }
                })
            } else {
                plot.dot(|mark| {
                    let mark = mark.x(x.unwrap()).y(y.unwrap());
                    if let Some(field) = &color {
                        mark.z(field.as_str())
                    } else {
                        mark
                    }
                })
            }
        }
        // Two ordinal dimensions → a dot per category pair. A bar needs a
        // quantitative length, which neither axis has.
        (Some(xk), Some(yk)) if !xk.is_quantitative() && !yk.is_quantitative() => {
            plot.dot(|mark| {
                let mark = mark.x(x.unwrap()).y(y.unwrap());
                match &color {
                    Some(field) if field != "count" => mark.z(field.as_str()),
                    _ => mark,
                }
            })
        }
        // x ordinal, y quantitative → vertical bars.
        (Some(_), Some(yk)) if yk.is_quantitative() => plot.bar_y(|mark| {
            let mark = mark.x(x.unwrap()).y(y.unwrap());
            if let Some(field) = &color {
                mark.z(field.as_str())
            } else {
                mark
            }
        }),
        // x quantitative, y ordinal, with a count color → a `barX` heatmap: bars
        // along x per category, each bin's length its count.
        (Some(_), Some(_)) if color.as_deref() == Some("count") => plot.bar_x(|mark| {
            mark.y(y.unwrap())
                .x(x.unwrap())
                .bin(crate::transform::Thresholds::default())
        }),
        // x quantitative, y ordinal → horizontal bars.
        (Some(_), Some(_)) => plot.bar_x(|mark| {
            let mark = mark.y(y.unwrap()).x(x.unwrap());
            if let Some(field) = &color {
                mark.z(field.as_str())
            } else {
                mark
            }
        }),
        // Only y, quantitative → a histogram of y (horizontal rects).
        (None, Some(kind)) if kind.is_quantitative() => hist(plot, y.unwrap(), "y"),
        // Only y, ordinal → count bars across y.
        (None, Some(_)) => plot.bar_x(|mark| {
            let mark = mark.y(y.unwrap()).x("count").group_y();
            if let Some(field) = &color {
                mark.z(field.as_str())
            } else {
                mark
            }
        }),
        (None, None) => plot,
    }
}

/// One of `auto`'s dimensions, Plot's `{value, reduce, zero}`.
#[derive(Clone, Debug, Default)]
pub struct AutoChannel {
    /// The field the dimension reads.
    pub value: Option<SharedString>,
    /// Plot's `reduce`: `None` leaves Plot's default, `Some(None)` is an
    /// explicit `reduce: null`.
    pub reduce: Option<Option<GroupReducer>>,
    /// Plot's `zero`.
    pub zero: Option<bool>,
}

impl AutoChannel {
    /// A dimension reading `field`.
    pub fn field(field: impl Into<SharedString>) -> Self {
        Self {
            value: Some(field.into()),
            ..Self::default()
        }
    }

    /// Plot's `reduce` for the dimension; `None` is `reduce: null`.
    pub fn reduce(mut self, reducer: Option<GroupReducer>) -> Self {
        self.reduce = Some(reducer);
        self
    }

    /// Plot's `zero`.
    pub fn zero(mut self, zero: bool) -> Self {
        self.zero = Some(zero);
        self
    }
}

/// The mark `auto` draws, Plot's `mark` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoMark {
    Dot,
    Line,
    Area,
    Rule,
    Bar,
}

/// Plot's `auto` options: `x`, `y`, `color`, `size` and `mark`.
#[derive(Clone, Debug, Default)]
pub struct AutoOptions {
    pub x: AutoChannel,
    pub y: AutoChannel,
    /// The color dimension; a `value` of `"count"` (or another reducer
    /// name) is Plot's `color: "count"` reducer.
    pub color: AutoChannel,
    pub size: AutoChannel,
    pub mark: Option<AutoMark>,
}

/// Plot's `isZeroReducer`.
fn is_zero_reducer(reducer: &Option<GroupReducer>) -> bool {
    matches!(
        reducer,
        Some(GroupReducer::Count | GroupReducer::Sum | GroupReducer::Proportion)
    )
}

/// A dimension's reducer named by its value, Plot's `color: "count"`.
fn reducer_name(value: &str) -> Option<GroupReducer> {
    match value {
        "count" => Some(GroupReducer::Count),
        "sum" => Some(GroupReducer::Sum),
        "mean" => Some(GroupReducer::Mean),
        "proportion" => Some(GroupReducer::Proportion),
        _ => None,
    }
}

/// Plot's `auto` with its options: the default reducers, mark type and
/// transform follow Plot's `autoSpec`. The cases it composes from the
/// library's marks are a rule (`reduce: null`), a binned or grouped line or
/// area (`y: {reduce}`), and binned bars colored by a count (`color:
/// "count"` over a quantitative and an ordinal dimension); the rest fall
/// back to [`build`].
pub fn build_with(data: &[Table], options: &AutoOptions) -> Plot {
    let dataset = Dataset::new(data.iter().cloned());
    let kind = |c: &AutoChannel| c.value.as_deref().and_then(|f| first_kind(&dataset, f));
    let ordinal = |c: &AutoChannel| kind(c).is_some_and(|k| !k.is_quantitative());
    let (x_value, y_value) = (options.x.value.clone(), options.y.value.clone());
    // A color that names a reducer reduces; otherwise it is a field.
    let color_reduce = match &options.color.reduce {
        Some(reducer) => reducer.clone(),
        None => options.color.value.as_deref().and_then(reducer_name),
    };
    let color_field = options
        .color
        .value
        .clone()
        .filter(|_| color_reduce.is_none());
    let size_value = options.size.value.clone();
    // Plot's default reducers.
    let x_reduce = match &options.x.reduce {
        Some(reducer) => reducer.clone(),
        None => {
            let y_reduce_unset = matches!(options.y.reduce, None | Some(None));
            (y_reduce_unset && x_value.is_none() && size_value.is_none() && y_value.is_some())
                .then_some(GroupReducer::Count)
        }
    };
    let y_reduce = match &options.y.reduce {
        Some(reducer) => reducer.clone(),
        None => (x_reduce.is_none()
            && y_value.is_none()
            && size_value.is_none()
            && x_value.is_some())
        .then_some(GroupReducer::Count),
    };
    let size_reduce = (size_value.is_none()
        && color_reduce.is_none()
        && x_reduce.is_none()
        && y_reduce.is_none()
        && (x_value.is_none() || ordinal(&options.x))
        && (y_value.is_none() || ordinal(&options.y)))
    .then_some(GroupReducer::Count);
    let monotonic =
        |c: &AutoChannel| c.value.as_deref().is_some_and(|f| monotonic_x(&dataset, f));
    let mark = options.mark.unwrap_or(
        if size_value.is_some() || size_reduce.is_some() {
            AutoMark::Dot
        } else if is_zero_reducer(&x_reduce)
            || is_zero_reducer(&y_reduce)
            || color_reduce.is_some()
        {
            AutoMark::Bar
        } else if x_value.is_some() && y_value.is_some() {
            if ordinal(&options.x)
                || ordinal(&options.y)
                || (x_reduce.is_none()
                    && y_reduce.is_none()
                    && !monotonic(&options.x)
                    && !monotonic(&options.y))
            {
                AutoMark::Dot
            } else {
                AutoMark::Line
            }
        } else {
            AutoMark::Rule
        },
    );
    let plot = Plot::new(data.iter().cloned());
    let fallback = || {
        build(
            data,
            x_value.as_deref(),
            y_value.as_deref(),
            options.color.value.as_deref(),
        )
    };
    match mark {
        // `reduce: null` on the only other dimension: a rule per value.
        AutoMark::Rule if x_reduce.is_none() && y_reduce.is_none() => {
            let color = color_field.clone();
            match (&x_value, &y_value) {
                (Some(x), None) => plot.rule_x(|m| {
                    let m = m.x(x.as_ref());
                    match &color {
                        Some(c) => m.stroke(c.as_ref()),
                        None => m,
                    }
                }),
                (None, Some(y)) => plot.rule_y(|m| {
                    let m = m.y(y.as_ref());
                    match &color {
                        Some(c) => m.stroke(c.as_ref()),
                        None => m,
                    }
                }),
                _ => fallback(),
            }
        }
        // A line or area reducing y over x: Plot's `binX` (or `groupX` for
        // an ordinal x).
        AutoMark::Line | AutoMark::Area
            if y_reduce.is_some()
                && x_reduce.is_none()
                && x_value.is_some()
                && y_value.is_some() =>
        {
            let x = x_value.clone().unwrap_or_default();
            let read = y_value.clone().unwrap_or_default();
            let reducer = y_reduce.clone().unwrap_or(GroupReducer::Count);
            let by_x = ordinal(&options.x);
            let color = color_field.clone();
            let kind_of = if mark == AutoMark::Line {
                crate::mark::MarkKind::LineY
            } else {
                crate::mark::MarkKind::AreaY
            };
            // Dates bin on calendar intervals.
            if kind(&options.x) == Some(PlotKind::Date)
                && let Some(rows) = bin_dates(data, &x, &read, color.as_deref(), &reducer)
            {
                let mut m = crate::mark::Mark::new(kind_of).x("$x").y("$y");
                m = match &color {
                    Some(c) if mark == AutoMark::Line => m.stroke(c.as_ref()),
                    Some(c) => m.fill(c.as_ref()),
                    None => m,
                };
                return Plot::new(rows)
                    .mark(m)
                    .x(|o| o.label(x.as_ref()))
                    .y(|o| o.label(read.as_ref()));
            }
            let configure = |m: crate::mark::Mark| -> crate::mark::Mark {
                // The value reads through an accessor so the bin lays its
                // edges over x alone, Plot's one-dimensional `binX`.
                let field = read.clone();
                let value = crate::table::Channel::Accessor(std::rc::Rc::new(
                    move |row: &Table| row.value(&field),
                ));
                let m = m.x(x.as_ref()).y(value);
                let m = if by_x {
                    m.group_x().group_output("y", reducer.clone())
                } else {
                    m.bin(crate::transform::Thresholds::default())
                        .bin_output("y", reducer.clone())
                };
                match &color {
                    Some(c) if mark == AutoMark::Line => m.stroke(c.as_ref()),
                    Some(c) => m.fill(c.as_ref()),
                    None => m,
                }
            };
            let kind = if mark == AutoMark::Line {
                crate::mark::MarkKind::LineY
            } else {
                crate::mark::MarkKind::AreaY
            };
            plot.mark(configure(crate::mark::Mark::new(kind)))
        }
        // Bars colored by a count over a quantitative and an ordinal
        // dimension: Plot's `barX(binX({fill: "count"}, {x, y}))`, and its
        // transpose.
        AutoMark::Bar
            if color_reduce.is_some()
                && x_reduce.is_none()
                && y_reduce.is_none()
                && x_value.is_some()
                && y_value.is_some()
                && (ordinal(&options.x) != ordinal(&options.y)) =>
        {
            let horizontal = ordinal(&options.y);
            let (value, category) = if horizontal {
                (x_value.clone(), y_value.clone())
            } else {
                (y_value.clone(), x_value.clone())
            };
            let (Some(value), Some(category)) = (value, category) else {
                return fallback();
            };
            let numbers: Vec<f64> = data
                .iter()
                .filter_map(|row| row.value(&value).as_number())
                .collect();
            let Some(edges) = crate::transform::BinEdges::of(
                &numbers,
                crate::transform::Thresholds::default(),
            ) else {
                return fallback();
            };
            let reducer = color_reduce.clone().unwrap_or(GroupReducer::Count);
            // One row per category and bin.
            let mut cells: Vec<(Value, usize, Vec<Table>)> = Vec::new();
            for row in data {
                let Some(v) = row.value(&value).as_number().filter(|v| v.is_finite()) else {
                    continue;
                };
                let key = row.value(&category);
                let slot = edges.slot(v);
                match cells.iter_mut().find(|(k, s, _)| *k == key && *s == slot) {
                    Some((_, _, members)) => members.push(row.clone()),
                    None => cells.push((key, slot, vec![row.clone()])),
                }
            }
            let rows: Vec<Table> = cells
                .into_iter()
                .map(|(key, slot, members)| {
                    let (lo, hi) = edges.bounds(slot);
                    let values: Vec<Value> = members.iter().map(|m| m.value(&value)).collect();
                    let refs: Vec<&Table> = members.iter().collect();
                    Table::empty()
                        .with(category.as_ref(), key)
                        .with("$lo", Value::Number(lo))
                        .with("$hi", Value::Number(hi))
                        .with("count", reducer.reduce(&values, &refs))
                })
                .collect();
            let plot = Plot::new(rows);
            if horizontal {
                plot.bar_x(|m| m.x1("$lo").x2("$hi").y(category.as_ref()).fill("count").inset(0.5))
                    .x(|x| x.label(value.as_ref()))
            } else {
                plot.bar_y(|m| m.y1("$lo").y2("$hi").x(category.as_ref()).fill("count").inset(0.5))
                    .y(|y| y.label(value.as_ref()))
            }
        }
        _ => fallback(),
    }
}

/// Plot's `binX` over dates for a line or area: d3's `utcTickInterval` for
/// the Scott bin count picks a calendar interval (days, weeks, months or
/// years), each bin reduces its y values, and its x is the bin's middle.
/// Returns the binned rows, with `$x` and `$y` fields and the color field.
fn bin_dates(
    data: &[Table],
    x: &str,
    y: &str,
    z: Option<&str>,
    reducer: &GroupReducer,
) -> Option<Vec<Table>> {
    use chrono::{Datelike, TimeZone, Utc};
    let millis: Vec<f64> = data
        .iter()
        .filter_map(|row| row.value(x).as_date().map(|d| d.timestamp_millis() as f64))
        .collect();
    let edges = crate::transform::BinEdges::of(&millis, crate::transform::Thresholds::default())?;
    let (lo, hi) = millis
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    // The Scott count, as the numeric edges lay it, then d3's interval for it.
    let count = edges.count.max(1) as f64;
    let target = (hi - lo) / count;
    const DAY: f64 = 86_400_000.;
    const YEAR: f64 = DAY * 365.;
    // d3's `tickIntervals` from a day up, as (duration, unit, step).
    let intervals: [(f64, char, i64); 6] = [
        (DAY, 'd', 1),
        (2. * DAY, 'd', 2),
        (7. * DAY, 'w', 1),
        (30. * DAY, 'm', 1),
        (90. * DAY, 'm', 3),
        (YEAR, 'y', 1),
    ];
    let (unit, step) = if target > YEAR {
        let step = crate::ticks::tick_increment_step(lo / YEAR, hi / YEAR, count).max(1.);
        ('y', step as i64)
    } else {
        // d3 bisects on the target and takes the nearer neighbour.
        let i = intervals.iter().position(|(d, _, _)| *d >= target).unwrap_or(5);
        let pick = if i > 0 && target / intervals[i - 1].0 < intervals[i].0 / target {
            i - 1
        } else {
            i
        };
        (intervals[pick].1, intervals[pick].2)
    };
    let floor = |t: chrono::DateTime<Utc>| -> chrono::DateTime<Utc> {
        let date = t.date_naive();
        let floored = match unit {
            'y' => {
                let year = date.year() - date.year().rem_euclid(step as i32);
                chrono::NaiveDate::from_ymd_opt(year, 1, 1)
            }
            'm' => {
                let month0 = date.month0() as i64 - (date.month0() as i64).rem_euclid(step);
                chrono::NaiveDate::from_ymd_opt(date.year(), month0 as u32 + 1, 1)
            }
            'w' => Some(date - chrono::Duration::days(date.weekday().num_days_from_sunday() as i64)),
            _ => {
                let days = date.num_days_from_ce() as i64;
                Some(date - chrono::Duration::days(days.rem_euclid(step)))
            }
        }
        .unwrap_or(date);
        Utc.from_utc_datetime(&floored.and_hms_opt(0, 0, 0).unwrap_or_default())
    };
    let offset = |t: chrono::DateTime<Utc>| -> chrono::DateTime<Utc> {
        let date = t.date_naive();
        let next = match unit {
            'y' => chrono::NaiveDate::from_ymd_opt(date.year() + step as i32, 1, 1),
            'm' => date.checked_add_months(chrono::Months::new(step as u32)),
            'w' => Some(date + chrono::Duration::days(7 * step)),
            _ => Some(date + chrono::Duration::days(step)),
        }
        .unwrap_or(date);
        Utc.from_utc_datetime(&next.and_hms_opt(0, 0, 0).unwrap_or_default())
    };
    // Group by series, then by bin start, in the series' first-seen order.
    let mut groups: Vec<(Value, i64, Vec<Table>)> = Vec::new();
    for row in data {
        let Some(date) = row.value(x).as_date() else {
            continue;
        };
        let start = floor(date).timestamp_millis();
        let key = z.map(|z| row.value(z)).unwrap_or(Value::Null);
        match groups.iter_mut().find(|(k, s, _)| *k == key && *s == start) {
            Some((_, _, members)) => members.push(row.clone()),
            None => groups.push((key, start, vec![row.clone()])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Some(
        groups
            .into_iter()
            .map(|(key, start, members)| {
                let begin = Utc.timestamp_millis_opt(start).single().unwrap_or_default();
                let end = offset(begin).timestamp_millis();
                let values: Vec<Value> = members.iter().map(|m| m.value(y)).collect();
                let refs: Vec<&Table> = members.iter().collect();
                let mut row = Table::empty()
                    .with("$x", Value::date_from_millis((start + end) / 2))
                    .with("$y", reducer.reduce(&values, &refs));
                if let Some(z) = z {
                    row = row.with(z, key);
                }
                row
            })
            .collect(),
    )
}

/// A heatmap of two quantitative fields: bin each axis into a grid of cells
/// and count the rows per cell, filling each cell by its count.
///
/// The data is re-binned here — Plot's `bin({fill: "count"}, {x, y})` — into
/// rows carrying the cell's bounds and count, drawn as a `rect` and colored by
/// the count through a sequential ramp.
fn heatmap(plot: Plot, x: &str, y: &str) -> Plot {
    let data: Vec<Table> = plot.data().rows().to_vec();
    let cells = bin2(&data, x, y, 10);
    // Each cell becomes a rect row carrying its bounds and its count.
    let rows: Vec<Table> = cells
        .into_iter()
        .map(|[x0, x1, y0, y1, count]| {
            Table::empty()
                .with("x0", Value::Number(x0))
                .with("x1", Value::Number(x1))
                .with("y0", Value::Number(y0))
                .with("y1", Value::Number(y1))
                .with("count", Value::Number(count))
        })
        .collect();
    if rows.is_empty() {
        return plot;
    }
    // A rect spanning each cell, filled by its count through a sequential ramp.
    Plot::new(rows)
        .rect(|mark| mark.x1("x0").x2("x1").y1("y0").y2("y1"))
        .color_by(
            "count",
            crate::scale::ColorRange::Continuous(vec![
                Hsla {
                    h: 0.6,
                    s: 0.4,
                    l: 0.9,
                    a: 1.,
                },
                Hsla {
                    h: 0.6,
                    s: 0.8,
                    l: 0.4,
                    a: 1.,
                },
            ]),
        )
}

/// Bin two quantitative fields into a `count` per cell, Plot's 2D `bin`.
///
/// Returns `(cells, counts)` where `cells` is the flattened grid —
/// `(x0, x1, y0, y1, count)` — for every cell that holds at least one row, in
/// sweep order.
fn bin2(data: &[Table], x: &str, y: &str, per_axis: usize) -> Vec<[f64; 5]> {
    // Collect the (x, y) pairs that are both numbers.
    let points: Vec<(f64, f64)> = data
        .iter()
        .filter_map(|row| {
            let (Some(xv), Some(yv)) = (row.value(x).as_number(), row.value(y).as_number()) else {
                return None;
            };
            Some((xv, yv))
        })
        .collect();
    let (x0, x1) = match crate::scale::extent(points.iter().map(|p| Value::Number(p.0))) {
        Some([a, b]) if a != b => (a, b),
        Some([a, b]) => (a - 0.5, b + 0.5),
        None => return Vec::new(),
    };
    let (y0, y1) = match crate::scale::extent(points.iter().map(|p| Value::Number(p.1))) {
        Some([a, b]) if a != b => (a, b),
        Some([a, b]) => (a - 0.5, b + 0.5),
        None => return Vec::new(),
    };
    let (xw, yw) = ((x1 - x0) / per_axis as f64, (y1 - y0) / per_axis as f64);
    // `per_axis` cells per axis; the maximum lands in the last cell rather
    // than in an extra one past the extent.
    let slots = per_axis.max(1);
    let mut grid = vec![0usize; slots * slots];
    for (px, py) in &points {
        let xi = (((px - x0) / xw).floor().max(0.) as usize).min(slots - 1);
        let yi = (((py - y0) / yw).floor().max(0.) as usize).min(slots - 1);
        grid[xi + yi * slots] += 1;
    }
    let mut cells = Vec::new();
    for yi in 0..slots {
        for xi in 0..slots {
            let count = grid[xi + yi * slots];
            if count > 0 {
                cells.push([
                    x0 + xi as f64 * xw,
                    x0 + (xi + 1) as f64 * xw,
                    y0 + yi as f64 * yw,
                    y0 + (yi + 1) as f64 * yw,
                    count as f64,
                ]);
            }
        }
    }
    cells
}

/// A histogram of `field` along one axis: `rectY` with a bin on x, or `rectX`
/// with a bin on y, counting the datum's presence per bin.
fn hist(plot: Plot, field: &str, axis: &str) -> Plot {
    if axis == "x" {
        plot.rect_y(|mark| {
            mark.x(field)
                .y("count")
                .bin(crate::transform::Thresholds::default())
        })
    } else {
        plot.rect_x(|mark| {
            mark.y(field)
                .x("count")
                .bin(crate::transform::Thresholds::default())
        })
    }
}

/// The kind of a field's first non-null value, if any.
fn first_kind(dataset: &Dataset, field: &str) -> Option<PlotKind> {
    dataset
        .rows()
        .iter()
        .map(|row| row.value(field).kind())
        .find(|kind| *kind != PlotKind::Null)
}

/// Whether a field's values are monotonic — every value strictly advances along
/// the rows, which is Plot's `isMonotonic` test that picks a line over a dot.
fn monotonic_x(dataset: &Dataset, field: &str) -> bool {
    let values: Vec<_> = dataset
        .rows()
        .iter()
        .map(|row| row.value(field))
        .filter(|v| !v.is_null())
        .collect();
    // Plot's `isMonotonic`: never decreasing or never increasing, so sorted
    // data with ties, or data sorted descending, still reads as a line.
    values.windows(2).all(|pair| pair[0] <= pair[1])
        || values.windows(2).all(|pair| pair[0] >= pair[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn rows(xs: &[f64], ys: Option<&[f64]>, g: Option<&[&str]>) -> Vec<Table> {
        xs.iter()
            .enumerate()
            .map(|(i, x)| {
                let mut row = Table::empty().with("x", Value::Number(*x));
                if let Some(ys) = ys {
                    row = row.with("y", Value::Number(ys[i]));
                }
                if let Some(g) = g {
                    row = row.with("g", Value::text(g[i]));
                }
                row
            })
            .collect()
    }

    #[test]
    fn two_quantitative_non_monotonic_scatter() {
        let data = rows(&[1., 2., 5., 3., 8., 7.], None, None);
        let plot = build(&data, Some("x"), Some("x"), None);
        // x is its own y (both quantitative, shuffled) → dot.
        assert!(plot.marks().iter().all(|mark| matches!(
            mark.kind,
            crate::mark::MarkKind::Dot | crate::mark::MarkKind::DotX
        )));
    }

    #[test]
    fn monotonic_x_lines() {
        let data = rows(&[1., 2., 3., 4., 5.], Some(&[1., 3., 2., 4., 5.]), None);
        let plot = build(&data, Some("x"), Some("y"), None);
        // Strictly increasing x → a line.
        assert!(plot.marks().iter().all(|mark| matches!(
            mark.kind,
            crate::mark::MarkKind::Line | crate::mark::MarkKind::LineX
        )));
    }

    #[test]
    fn ordinal_x_bar_charts() {
        let data = [
            Table::empty()
                .with("x", Value::text("a"))
                .with("y", Value::Number(4.)),
            Table::empty()
                .with("x", Value::text("b"))
                .with("y", Value::Number(7.)),
        ];
        let plot = build(&data, Some("x"), Some("y"), None);
        // x ordinal, y quantitative → vertical bars.
        assert_eq!(plot.marks().len(), 1);
        assert_eq!(plot.marks()[0].kind, crate::mark::MarkKind::BarY);
    }

    #[test]
    fn quantitative_x_histograms() {
        let data = rows(&[1., 2., 3., 4., 5.], None, None);
        let plot = build(&data, Some("x"), None, None);
        // A quantitative x with no y → a binned histogram.
        assert_eq!(plot.marks()[0].kind, crate::mark::MarkKind::RectY);
        assert!(plot.marks()[0].bin.is_some());
    }

    #[test]
    fn color_becomes_a_series() {
        let data = rows(&[1., 2.], Some(&[4., 5.]), Some(&["a", "b"]));
        let plot = build(&data, Some("x"), Some("y"), Some("g"));
        // A color field becomes a z series, so the mark colors by it.
        assert!(plot.marks()[0].channels.z.is_some());
    }

    #[test]
    fn count_color_heatmaps_two_quantitative_fields() {
        // A dense two-cluster cloud, so binning both axes yields many cells.
        let data = rows(
            &[1., 1., 5., 5., 5., 5.],
            Some(&[1., 2., 3., 3., 4., 4.]),
            None,
        );
        let plot = build(&data, Some("x"), Some("y"), Some("count"));
        // Auto re-bins into a rect mark, not the raw dot.
        assert_eq!(plot.marks().len(), 1);
        assert_eq!(plot.marks()[0].kind, crate::mark::MarkKind::Rect);
        // The plot's data is the re-binned cells (with x1/x2/y1/y2/count),
        // not the original dots.
        let first = &plot.data().rows()[0];
        assert!(first.value("count").as_number().is_some());
        assert!(first.value("x0").as_number().is_some());
    }

    #[test]
    fn count_color_on_ordinal_y_gives_a_binned_barx() {
        // x quantitative, y ordinal, color count → a barX heatmap that bins x.
        let data = [
            Table::empty()
                .with("x", Value::Number(1.))
                .with("y", Value::text("a")),
            Table::empty()
                .with("x", Value::Number(2.))
                .with("y", Value::text("a")),
            Table::empty()
                .with("x", Value::Number(9.))
                .with("y", Value::text("b")),
        ];
        let plot = build(&data, Some("x"), Some("y"), Some("count"));
        assert_eq!(plot.marks()[0].kind, crate::mark::MarkKind::BarX);
        assert!(plot.marks()[0].bin.is_some(), "barX heatmap bins x");
    }
}
