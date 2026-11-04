//! Mark rendering: resolve a mark's channels against the data, then draw the
//! shapes Plot's mark types describe.
//!
//! Resolution and drawing are separate: [`prepare`] turns a [`Mark`] and a
//! [`Dataset`] into [`PreparedMark`]s, which hold plain values, and the frame
//! unions their values to build the scales. Drawing then only reads pixels.

pub(crate) mod basic;
pub(crate) mod delaunay;
pub(crate) mod radial;
pub(crate) mod stat;

use gpui::{Hsla, SharedString};

use crate::facet::Grid;
use crate::frame::Frame;
use crate::mark::{Axis, Channels, CrossAxis, MapReducer, Mark, MarkKind};
use crate::table::{Dataset, Table};
use crate::transform::{self, Anchor, Reducer, Selector, StackOffset, StackedSeries, Thresholds};
use crate::value::Value;

/// A mark whose channels have been resolved against the data.
#[derive(Clone, Debug)]
pub struct PreparedMark {
    pub mark: Mark,
    pub rows: Vec<PreparedRow>,
    /// Whether a dodge packed this mark's rows along x, so its offsets are
    /// vertical; meaningless when the mark does not dodge.
    pub dodge_along_x: bool,
}

/// One datum's resolved channels.
#[derive(Clone, Debug)]
pub struct PreparedRow {
    /// The original record, which the tooltip reads.
    pub row: Table,
    /// The index of the datum in the source data.
    pub index: usize,
    pub x: Value,
    pub y: Value,
    /// The x interval, when the mark draws one.
    pub x1: Option<Value>,
    pub x2: Option<Value>,
    /// The y interval, when the mark draws one.
    pub y1: Option<Value>,
    pub y2: Option<Value>,
    /// The series a mark groups by.
    pub z: Option<Value>,
    /// The resolved radius of a dot.
    pub r: Option<f32>,
    /// The resolved symbol of a dot.
    pub symbol: Option<crate::style::Symbol>,
    /// The row's opacity from its `opacity` channel, when it has one.
    pub opacity: Option<f32>,
    /// Where the row sits across the axis a dodge packs along, in pixels from
    /// that axis's start. Set by [`apply_dodge`] once the frame is known.
    pub dodge_offset: Option<f32>,
}

impl PreparedRow {
    /// The value the x channel reads, preferring the interval's start.
    pub fn x_value(&self) -> &Value {
        if !self.x.is_null() {
            &self.x
        } else {
            self.x1.as_ref().unwrap_or(&self.x)
        }
    }

    /// The value the y channel reads, preferring the interval's start.
    pub fn y_value(&self) -> &Value {
        if !self.y.is_null() {
            &self.y
        } else {
            self.y1.as_ref().unwrap_or(&self.y)
        }
    }

    /// The y interval's ends, `(start, end)`, when the mark draws one.
    pub fn y_interval(&self) -> Option<(Value, Value)> {
        let start = self
            .y1
            .clone()
            .or_else(|| (!self.y.is_null()).then(|| self.y.clone()))?;
        let end = self
            .y2
            .clone()
            .or_else(|| (!self.y.is_null()).then(|| self.y.clone()))?;
        Some((start, end))
    }

    /// The x interval's ends, `(start, end)`, when the mark draws one.
    pub fn x_interval(&self) -> Option<(Value, Value)> {
        let start = self
            .x1
            .clone()
            .or_else(|| (!self.x.is_null()).then(|| self.x.clone()))?;
        let end = self
            .x2
            .clone()
            .or_else(|| (!self.x.is_null()).then(|| self.x.clone()))?;
        Some((start, end))
    }
}

/// Resolve every mark against `data`, for an unfaceted plot.
pub fn prepare(data: &Dataset, marks: &[Mark]) -> Vec<PreparedMark> {
    marks.iter().map(|mark| prepare_mark(data, mark)).collect()
}

/// Resolve every mark against the part of `data` that lands in each facet.
///
/// A mark's rows are split by the facet fields, and each facet's share then
/// resolves on its own, so a stack, bin or reducer summarizes within its
/// facet. The facets come back in the grid's order, one list of marks per
/// facet, with the facets that hold no rows; a figure that is not faceted has
/// one facet holding every row and no empty facets.
pub fn prepare_facets(
    data: &Dataset,
    marks: &[Mark],
    grid: &Grid,
    fx: Option<&str>,
    fy: Option<&str>,
) -> (Vec<Vec<PreparedMark>>, Vec<bool>) {
    if !grid.is_faceted() {
        return (vec![prepare(data, marks)], Vec::new());
    }
    let (groups, populated) = grid.partition(data, fx, fy);
    let prepared = groups
        .into_iter()
        .map(|rows| prepare(&Dataset::new(rows), marks))
        .collect();
    (prepared, populated)
}

fn prepare_mark(data: &Dataset, mark: &Mark) -> PreparedMark {
    // A furniture mark draws the plot's axes, grid or frame rather than data,
    // so it reads no channels and resolves no rows.
    if mark.kind.is_furniture() {
        return PreparedMark {
            mark: mark.clone(),
            rows: Vec::new(),
            dodge_along_x: false,
        };
    }
    let channels = &mark.channels;

    // The data transforms rewrite the records themselves, so they run before
    // any row is resolved: `select` drops rows, and `map`/`shift` add fields
    // the channels may read. Plot applies them to the mark's data in the same
    // order, and a mark's `shuffle` is only a reordering of what is left.
    let mut source = data.clone();
    if let Some(config) = &mark.select {
        let group = config.group.as_deref().map(|field| field.to_string());
        let selector = match (&config.selector, config.axis) {
            // A select over an axis reads that axis's channel, which may be a
            // field name or an accessor; only the field form can be expressed
            // here, and an accessor's extreme falls back to the first row.
            (Selector::Min(_), Some(Axis::X)) => Selector::Min("x".into()),
            (Selector::Max(_), Some(Axis::X)) => Selector::Max("x".into()),
            (Selector::Min(_), Some(Axis::Y)) => Selector::Min("y".into()),
            (Selector::Max(_), Some(Axis::Y)) => Selector::Max("y".into()),
            (selector, _) => selector.clone(),
        };
        source = transform::select(
            &source,
            &transform::Select {
                selector,
                group: group.map(Into::into),
            },
        );
    }
    if let Some(config) = &mark.map {
        let rows = source.rows();
        let mut running: Vec<(String, f64, f64, usize)> = Vec::new();
        let mut output: Vec<Value> = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let key = config
                .group
                .as_ref()
                .map(|field| row.value(field).label().to_string())
                .unwrap_or_default();
            let value = row.value(&config.input).as_number().unwrap_or(f64::NAN);
            let slot = match running.iter().position(|(existing, ..)| *existing == key) {
                Some(index) => index,
                None => {
                    running.push((key, 0., f64::NAN, 0));
                    running.len() - 1
                }
            };
            let (_, total, extreme, count) = &mut running[slot];
            *count += 1;
            let mapped = match config.reducer {
                MapReducer::Rank => Value::Number((*count - 1) as f64),
                MapReducer::Sum => {
                    *total += value;
                    Value::Number(*total)
                }
                MapReducer::Min => {
                    *extreme = if extreme.is_nan() {
                        value
                    } else {
                        extreme.min(value)
                    };
                    Value::Number(*extreme)
                }
                MapReducer::Max => {
                    *extreme = if extreme.is_nan() {
                        value
                    } else {
                        extreme.max(value)
                    };
                    Value::Number(*extreme)
                }
                MapReducer::Mean => {
                    *total += value;
                    Value::Number(*total / *count as f64)
                }
            };
            output.push(mapped);
        }
        source = Dataset::new(
            rows.iter()
                .cloned()
                .zip(output)
                .map(|(row, value)| row.with(&config.output, value)),
        );
    }
    if let Some(config) = &mark.shift {
        source = transform::shift(&source, &config.input, &config.output, config.offset);
    }
    if let Some(seed) = mark.shuffle {
        source = transform::shuffle(&source, seed);
    }

    let mut rows: Vec<PreparedRow> = source
        .rows()
        .iter()
        .enumerate()
        .map(|(index, row)| resolve_row(row, index, channels, mark))
        .collect();

    // A mark's own ordering decides where its shapes land, and a line's order
    // is its path, so sorting happens before anything positional.
    apply_sort(&mut rows, mark);

    // Windows and normalization rewrite the value the mark draws, so they run
    // before the shapes read it.
    apply_window(data, mark, &mut rows);
    if mark.normalize {
        normalize_rows(&mut rows);
    }

    // A grouping collapses the rows sharing a category into one row each,
    // Plot's `groupX`/`groupY`/`groupZ`.
    if let Some(config) = &mark.group {
        rows = collapse_groups(rows, mark, config);
    }

    // A reducer collapses the rows sharing a category, Plot's `groupX`.
    if let Some(reducer) = mark.reduce {
        rows = reduce_rows(rows, mark, reducer);
    }

    // Binning replaces the rows with one row per bin.
    if mark.bin.is_some() {
        rows = bin_rows(data, mark, &rows);
    }

    // Stacking rewrites the y interval of every row.
    if mark.stack.is_some() {
        rows = stack_rows(rows, mark);
    }

    // Statistical marks summarize their category themselves, so they keep the
    // raw rows and group at draw time.

    PreparedMark {
        mark: mark.clone(),
        rows,
        dodge_along_x: false,
    }
}

/// Pack a dodging mark's rows into a beeswarm, Plot's `dodgeX`/`dodgeY`.
///
/// The transform works in pixels, so it runs after the frame has resolved the
/// scales: each row's position on the axis the data varies on becomes an
/// offset across the other axis, which the mark then reads instead of its
/// own cross-axis value.
pub fn apply_dodge(prepared: &mut [PreparedMark], frame: &Frame) {
    for mark in prepared.iter_mut() {
        let Some(config) = mark.mark.dodge else {
            continue;
        };
        // The dodge packs along the axis the mark's data varies on, and writes
        // the offset across it: a `dotX` varies on x and spreads vertically,
        // a `dotY` varies on y and spreads horizontally.
        let along_x = mark.mark.kind.is_x_only();
        // The positions come from the axis the data varies on; the packed
        // offsets land on the *other* axis, so the extent is that one's.
        let (scale, extent) = if along_x {
            (&frame.x.scale, frame.plot.size.height.as_f32())
        } else {
            (&frame.y.scale, frame.plot.size.width.as_f32())
        };
        let positions: Vec<Option<f32>> = mark
            .rows
            .iter()
            .map(|row| {
                if along_x {
                    scale.position(row.x_value())
                } else {
                    scale.position(row.y_value())
                }
            })
            .collect();
        let extent = [0., extent];
        let packed = transform::dodge(
            &Dataset::new(mark.rows.iter().map(|row| row.row.clone())),
            &positions,
            "dodge",
            config,
            extent,
        );
        for (row, packed_row) in mark.rows.iter_mut().zip(packed.rows()) {
            row.dodge_offset = packed_row.number("dodge").map(|value| value as f32);
        }
        mark.dodge_along_x = along_x;
    }
}

/// Grow a mark's interval channels from its value channel, the way Plot's
/// `barY` grows from zero to `y` and its `rectY`/`cell` marks fill their
/// missing interval ends.
///
/// Plot derives these from the mark's own options, so the same derivation has
/// to run again on a grouped row: the grouping writes the reduced value into
/// `y` after the raw rows already had their intervals derived, and a bar with
/// a `y` but no `y1`/`y2` draws nothing.
fn derive_intervals(
    mark: &Mark,
    x: &Value,
    y: &Value,
    x1: &mut Option<Value>,
    x2: &mut Option<Value>,
    y1: &mut Option<Value>,
    y2: &mut Option<Value>,
) {
    let unset = |start: &Option<Value>, end: &Option<Value>| start.is_none() && end.is_none();
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

fn resolve_row(row: &Table, index: usize, channels: &Channels, mark: &Mark) -> PreparedRow {
    let read = |channel: &Option<crate::table::Channel>| {
        channel
            .as_ref()
            .map(|channel| channel.read(row))
            .unwrap_or(Value::Null)
    };

    let mut x = read(&channels.x);
    let mut y = read(&channels.y);
    let x1 = read(&channels.x1);
    let x2 = read(&channels.x2);
    let y1 = read(&channels.y1);
    let y2 = read(&channels.y2);

    let mut x1 = (!x1.is_null()).then_some(x1);
    let mut x2 = (!x2.is_null()).then_some(x2);
    let mut y1 = (!y1.is_null()).then_some(y1);
    let mut y2 = (!y2.is_null()).then_some(y2);
    derive_intervals(mark, &x, &y, &mut x1, &mut x2, &mut y1, &mut y2);

    // A one-dimensional mark has no second position channel. Plot fills it
    // with the datum's index for `lineX`/`rectX`, and leaves it unset for
    // `dotX`, whose dots all sit in the frame's middle.
    if mark.kind.is_x_only() && mark.kind.cross_axis() == CrossAxis::Index {
        y = Value::Number(index as f64);
    } else if mark.kind.is_y_only() && mark.kind.cross_axis() == CrossAxis::Index {
        x = Value::Number(index as f64);
    }

    let z = channels
        .z
        .as_ref()
        .map(|channel| channel.read(row))
        .filter(|v| !v.is_null());

    PreparedRow {
        row: row.clone(),
        index,
        x,
        y,
        x1,
        x2,
        y1,
        y2,
        z,
        r: channels.r.as_ref().map(|channel| channel.read(row)),
        symbol: channels
            .symbol
            .as_ref()
            .map(|channel| symbol_from_value(&channel.read(row)))
            .filter(|symbol| symbol.is_some())
            .flatten(),
        opacity: channels.opacity.as_ref().map(|channel| channel.read(row)),
        dodge_offset: None,
    }
}

/// A symbol channel's value as a [`Symbol`](crate::style::Symbol): a name
/// (`"circle"`, `"square"`, …) or an index into the symbol set.
fn symbol_from_value(value: &Value) -> Option<crate::style::Symbol> {
    const SYMBOLS: [crate::style::Symbol; 13] = [
        crate::style::Symbol::Circle,
        crate::style::Symbol::Square,
        crate::style::Symbol::Diamond,
        crate::style::Symbol::Triangle,
        crate::style::Symbol::Cross,
        crate::style::Symbol::CrossRotate,
        crate::style::Symbol::Star,
        crate::style::Symbol::Asterisk,
        crate::style::Symbol::Plus,
        crate::style::Symbol::Times,
        crate::style::Symbol::Arrow,
        crate::style::Symbol::TriangleDown,
        crate::style::Symbol::CircleOpen,
    ];

    match value {
        Value::Number(number) => Some(SYMBOLS[(*number as usize) % SYMBOLS.len()]),
        Value::Text(text) => match text.as_ref() {
            "circle" => Some(crate::style::Symbol::Circle),
            "square" => Some(crate::style::Symbol::Square),
            "diamond" => Some(crate::style::Symbol::Diamond),
            "triangle" => Some(crate::style::Symbol::Triangle),
            "cross" => Some(crate::style::Symbol::Cross),
            "star" => Some(crate::style::Symbol::Star),
            "plus" => Some(crate::style::Symbol::Plus),
            "times" => Some(crate::style::Symbol::Times),
            "arrow" => Some(crate::style::Symbol::Arrow),
            _ => None,
        },
        _ => None,
    }
}

fn apply_sort(rows: &mut [PreparedRow], mark: &Mark) {
    let Some(sort) = &mark.sort else {
        return;
    };
    if let Some(field) = &sort.field {
        rows.sort_by(|a, b| {
            let ordering = a.row.value(field).cmp(&b.row.value(field));
            if sort.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    } else {
        // Sorting without a field sorts by the mark's own value: a bar chart's
        // length, which is Plot's default for a sorted bar chart.
        rows.sort_by(|a, _b| {
            let ordering = a.y_value().cmp(a.x_value());
            if sort.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }
    if sort.reverse {
        rows.reverse();
    }
}

fn apply_window(data: &Dataset, mark: &Mark, rows: &mut [PreparedRow]) {
    let Some((k, reducer, anchor)) = mark.window else {
        return;
    };
    // The window rewrites a field in place, so it runs over the whole dataset
    // and then the resolved rows read the rewritten value back.
    let field = match mark.channels.y.as_ref().or(mark.channels.x.as_ref()) {
        Some(crate::table::Channel::Field(field)) => field.clone(),
        _ => return,
    };
    let smoothed = transform::window(data, &field, k, reducer, anchor);
    for (prepared, row) in rows.iter_mut().zip(smoothed.rows()) {
        let value = row.value(&field);
        if mark.channels.y.as_ref().is_some_and(is_field(&field)) {
            prepared.y = value.clone();
        }
        if mark.channels.x.as_ref().is_some_and(is_field(&field)) {
            prepared.x = value;
        }
    }
}

fn is_field(field: &SharedString) -> impl Fn(&crate::table::Channel) -> bool + '_ {
    move |channel| matches!(channel, crate::table::Channel::Field(name) if name == field)
}

fn normalize_rows(rows: &mut [PreparedRow]) {
    let values: Vec<Value> = rows.iter().map(|row| row.y_value().clone()).collect();
    let Some([lo, hi]) = crate::scale::extent(values) else {
        return;
    };
    let span = hi - lo;
    for row in rows.iter_mut() {
        if let Some(value) = row.y_value().as_number() {
            let normalized = if span == 0. { 0.5 } else { (value - lo) / span };
            row.y = Value::Number(normalized);
            row.y1 = None;
            row.y2 = None;
        }
    }
}

/// The value a grouped output reduces, read from one member of the group.
///
/// Plot reads the channel the output writes unless the output names another
/// input, and `y1`/`y2` fall back to `y` when the mark names no interval —
/// Plot's `maybeInput` — so `{y1: "min"}` is the smallest y value even though
/// a bar's interval has already been grown from zero by the time it groups.
fn group_input(row: &PreparedRow, mark: &Mark, output: &crate::mark::GroupOutput) -> Value {
    let name = output.input.as_deref().unwrap_or(&output.name);
    match name {
        // An interval's end falls back to the channel it splits, the way
        // Plot's `maybeInput` does.
        "x1" | "x2" if mark.channels.x1.is_some() || mark.channels.x2.is_some() => row
            .x1
            .as_ref()
            .or(row.x2.as_ref())
            .unwrap_or(&row.x)
            .clone(),
        "y1" | "y2" if mark.channels.y1.is_some() || mark.channels.y2.is_some() => row
            .y1
            .as_ref()
            .or(row.y2.as_ref())
            .unwrap_or(&row.y)
            .clone(),
        "x1" | "x2" => row.x_value().clone(),
        "y1" | "y2" => row.y_value().clone(),
        "x" => row.x_value().clone(),
        "y" => row.y_value().clone(),
        "z" => row.z.clone().unwrap_or(Value::Null),
        // Any other name is a field on the record, which is where a grouped
        // color, label or tooltip value lands.
        field => row.row.value(field),
    }
}

/// Collapse the rows sharing a category, Plot's `groupX`/`groupY`/`groupZ`.
///
/// Every group becomes one row: its category channel keeps the key, its
/// outputs hold the reductions, and every other channel — the radius, the
/// symbol, the record the tooltip reads — carries over from the group's first
/// member. The groups keep the order they first appear in, so a categorical
/// axis reads in data order.
fn collapse_groups(
    rows: Vec<PreparedRow>,
    mark: &Mark,
    config: &crate::mark::GroupConfig,
) -> Vec<PreparedRow> {
    let key = |row: &PreparedRow| match config.axis {
        Some(Axis::X) => row.x_value().clone(),
        Some(Axis::Y) => row.y_value().clone(),
        // `groupZ` subdivides by the series, and a row with no `z` is its
        // own group rather than a missing one.
        None => row.z.clone().unwrap_or(Value::Null),
    };

    let mut groups: Vec<(Value, Vec<PreparedRow>)> = Vec::new();
    for row in rows {
        let key = key(&row);
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }

    groups
        .into_iter()
        .map(|(key, members)| {
            let first = members[0].clone();
            let mut grouped = PreparedRow {
                x: Value::Null,
                y: Value::Null,
                x1: None,
                x2: None,
                y1: None,
                y2: None,
                ..first
            };
            for output in &config.outputs {
                let values: Vec<Value> = members
                    .iter()
                    .map(|row| group_input(row, mark, output))
                    .collect();
                let records: Vec<&Table> = members.iter().map(|row| &row.row).collect();
                let reduced = output.reducer.reduce(&values, &records);
                match &*output.name {
                    "x" => grouped.x = reduced,
                    "y" => grouped.y = reduced,
                    "x1" => grouped.x1 = (!reduced.is_null()).then_some(reduced),
                    "x2" => grouped.x2 = (!reduced.is_null()).then_some(reduced),
                    "y1" => grouped.y1 = (!reduced.is_null()).then_some(reduced),
                    "y2" => grouped.y2 = (!reduced.is_null()).then_some(reduced),
                    "z" => grouped.z = (!reduced.is_null()).then_some(reduced),
                    // Anything else is a field the record gains, so a channel
                    // that reads it — a fill accessor, a text label — sees the
                    // reduced value.
                    field => grouped.row = grouped.row.with(field, reduced),
                }
            }
            // The category channel keeps the key when no output wrote it,
            // which is what makes a grouped dot sit at its own category.
            if grouped.x.is_null() && grouped.x1.is_none() {
                grouped.x = key.clone();
            }
            if grouped.y.is_null() && grouped.y1.is_none() {
                grouped.y = key;
            }
            // The grouping wrote the reduced value into a channel the mark
            // turns into an interval, so a bar grows from zero to the count
            // again. Plot derives the interval from the mark's options after
            // its transforms, not before.
            derive_intervals(
                mark,
                &grouped.x,
                &grouped.y,
                &mut grouped.x1,
                &mut grouped.x2,
                &mut grouped.y1,
                &mut grouped.y2,
            );
            grouped
        })
        .collect()
}

fn reduce_rows(rows: Vec<PreparedRow>, mark: &Mark, reducer: Reducer) -> Vec<PreparedRow> {
    // Grouping by x for a vertical mark and by y for a horizontal one mirrors
    // Plot's `groupX` and `groupY`.
    let group_by_y = matches!(
        mark.kind,
        MarkKind::BarX | MarkKind::AreaX | MarkKind::RuleY | MarkKind::TickY
    );

    let mut groups: Vec<(Value, Vec<PreparedRow>)> = Vec::new();
    for row in rows {
        let key = if group_by_y {
            row.y_value().clone()
        } else {
            row.x_value().clone()
        };
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }

    groups
        .into_iter()
        .map(|(key, members)| {
            let values: Vec<Value> = members
                .iter()
                .map(|row| {
                    if group_by_y {
                        row.x_value().clone()
                    } else {
                        row.y_value().clone()
                    }
                })
                .collect();
            let reduced = reducer.reduce(&values);
            let first = members[0].clone();
            if group_by_y {
                PreparedRow {
                    y: key,
                    x: reduced,
                    x1: None,
                    x2: None,
                    y1: None,
                    y2: None,
                    ..first
                }
            } else {
                PreparedRow {
                    x: key,
                    y: reduced,
                    x1: None,
                    x2: None,
                    y1: None,
                    y2: None,
                    ..first
                }
            }
        })
        .collect()
}

/// Replace rows with one row per bin of the mark's positional field.
///
/// Plot's `Plot.binX({y: "count"}, {x: "value"})`: the bins become the x
/// interval and the count becomes the y value.
fn bin_rows(data: &Dataset, mark: &Mark, rows: &[PreparedRow]) -> Vec<PreparedRow> {
    let Some(bin) = &mark.bin else {
        return rows.to_vec();
    };
    let field = mark
        .channels
        .x
        .as_ref()
        .or(mark.channels.y.as_ref())
        .and_then(|channel| match channel {
            crate::table::Channel::Field(name) => Some(name.clone()),
            _ => None,
        });
    let Some(field) = field else {
        return rows.to_vec();
    };
    // A vertical mark bins along x and counts on y; a horizontal one the
    // other way round. Plot picks this from the mark, not from which channel
    // happens to be set — `rectX` names its field on x yet bins along y. The
    // mark's classification pins the axis for the marks that have a fixed one,
    // and a mark without one bins along whatever channel it names.
    let bin_x = mark
        .kind
        .category()
        .bins_on_x
        .unwrap_or(mark.channels.x.is_some());

    let (bins, _domain) = transform::bin(data, &field, bin.thresholds);
    bins.into_iter()
        .map(|bin| {
            let count = bin.count();
            let representative = bin.rows.first().and_then(|index| rows.get(*index)).cloned();
            let mut row = representative.unwrap_or_else(|| PreparedRow {
                row: Table::empty(),
                index: 0,
                x: Value::Null,
                y: Value::Null,
                x1: None,
                x2: None,
                y1: None,
                y2: None,
                z: None,
                r: None,
                symbol: None,
                opacity: None,
                dodge_offset: None,
            });
            if bin_x {
                // The bin's interval is the x axis, the count is the y value.
                row.x1 = Some(Value::Number(bin.x0));
                row.x2 = Some(Value::Number(bin.x1));
                row.x = Value::Null;
                row.y = Value::Number(count);
                row.y1 = Some(Value::Number(0.));
                row.y2 = Some(Value::Number(count));
            } else {
                // Binned along y: the interval is the y axis, the count grows
                // along x.
                row.y1 = Some(Value::Number(bin.x0));
                row.y2 = Some(Value::Number(bin.x1));
                row.y = Value::Null;
                row.x = Value::Number(count);
                row.x1 = Some(Value::Number(0.));
                row.x2 = Some(Value::Number(count));
            }
            row
        })
        .collect()
}

/// Rewrite every row's y interval from the stacked series.
///
/// Plot stacks two shapes of data: *long* data, where one field names the
/// series (`stack_series("platform")`), and *wide* data, where the mark names
/// one value channel per series (`y1("tablet").y2("desktop")`). Both end in
/// the same series list.
fn stack_rows(rows: Vec<PreparedRow>, mark: &Mark) -> Vec<PreparedRow> {
    let Some(stack) = &mark.stack else {
        return rows;
    };

    // A horizontal mark puts its categories on y and its values on x; a
    // vertical one is the other way round.
    let group_by_y = mark.kind.category().groups_on_y;
    let (category_channel, value_channel) = if group_by_y {
        (&mark.channels.y, &mark.channels.x)
    } else {
        (&mark.channels.x, &mark.channels.y)
    };

    let field_name = |channel: &Option<crate::table::Channel>| match channel {
        Some(crate::table::Channel::Field(name)) => Some(name.clone()),
        _ => None,
    };

    let Some(category_field) = field_name(category_channel) else {
        return rows;
    };
    let data = Dataset::new(rows.iter().map(|row| row.row.clone()));

    let series = match &stack.series_field {
        // Long data: the field names the series, the value channel holds the
        // value.
        Some(field) => {
            let Some(value_field) = field_name(value_channel) else {
                return rows;
            };
            transform::stack_long(&data, &category_field, field, &value_field, stack.offset)
        }
        // Wide data: the caller names the value fields to stack.
        None if !stack.fields.is_empty() => {
            let keys: Vec<&str> = stack.fields.iter().map(|field| field.as_ref()).collect();
            transform::stack_wide(&data, &category_field, &keys, stack.offset)
        }
        None => return rows,
    };

    stack_rows_from_series(rows, mark, &series)
}

/// Apply a stacked series to the rows: the series' points replace the y
/// intervals, and the rows keep their x and original record.
fn stack_rows_from_series(
    rows: Vec<PreparedRow>,
    mark: &Mark,
    series: &[StackedSeries],
) -> Vec<PreparedRow> {
    let group_by_y = mark.kind.category().groups_on_y;
    let mut out: Vec<PreparedRow> = Vec::new();

    for stacked in series {
        for point in &stacked.points {
            // Find the row this point came from so the tooltip keeps its
            // record; a point per (series, category) has one.
            let source = rows
                .iter()
                .find(|row| {
                    let key = if group_by_y {
                        row.y_value()
                    } else {
                        row.x_value()
                    };
                    key.key() == point.x.key()
                        && row.z.as_ref().is_some_and(|z| z.label() == stacked.key)
                })
                .or_else(|| {
                    rows.iter().find(|row| {
                        let key = if group_by_y {
                            row.y_value()
                        } else {
                            row.x_value()
                        };
                        key.key() == point.x.key()
                    })
                })
                .cloned();

            let mut row = source.unwrap_or_else(|| PreparedRow {
                row: Table::empty(),
                index: 0,
                x: point.x.clone(),
                y: Value::Null,
                x1: None,
                x2: None,
                y1: None,
                y2: None,
                z: None,
                r: None,
                symbol: None,
                opacity: None,
                dodge_offset: None,
            });
            if group_by_y {
                row.x1 = Some(Value::Number(point.y0));
                row.x2 = Some(Value::Number(point.y1));
            } else {
                row.y1 = Some(Value::Number(point.y0));
                row.y2 = Some(Value::Number(point.y1));
            }
            row.x = point.x.clone();
            row.z = Some(Value::Text(stacked.key.clone()));
            out.push(row);
        }
    }

    // Keep the stacking order: the series' keys order the shapes, and within
    // each series the x order decides where they land.
    out
}

/// A value channel as text, for a mark's label.
pub fn channel_text(channel: &Option<crate::table::Channel>, row: &Table) -> Option<SharedString> {
    channel.as_ref().map(|channel| channel.read(row).label())
}

/// The default color of a mark at `index` in the mark list.
pub fn mark_color(theme: &crate::theme::PlotTheme, index: usize) -> Hsla {
    theme.palette_color(index)
}

/// The tick count a mark wants on its value axis.
pub fn tick_count(mark: &Mark) -> usize {
    let _ = mark;
    5
}

/// A mark's rows grouped by a categorical field, keeping first-occurrence
/// order — the grouping a box, violin or density mark draws per category.
pub fn group_rows<'a>(
    rows: &'a [PreparedRow],
    field: &dyn Fn(&PreparedRow) -> Value,
) -> Vec<(Value, Vec<&'a PreparedRow>)> {
    let mut groups: Vec<(Value, Vec<&PreparedRow>)> = Vec::new();
    for row in rows {
        let key = field(row);
        if key.is_null() {
            continue;
        }
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(row),
            None => groups.push((key, vec![row])),
        }
    }
    groups
}

/// The anchor a windowed mark uses.
pub fn window_anchor(mark: &Mark) -> Anchor {
    mark.window.map(|(_, _, anchor)| anchor).unwrap_or_default()
}

/// The bin thresholds a mark uses.
pub fn thresholds(mark: &Mark) -> Thresholds {
    mark.bin.map(|bin| bin.thresholds).unwrap_or_default()
}

/// The stack offset a mark uses.
pub fn stack_offset(mark: &Mark) -> StackOffset {
    mark.stack
        .as_ref()
        .map(|stack| stack.offset)
        .unwrap_or_default()
}

#[cfg(test)]
mod stack_tests {
    use super::*;
    use crate::mark::MarkKind;

    fn long_rows() -> Vec<Table> {
        vec![
            Table::empty()
                .with("m", Value::text("a"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("m", Value::text("a"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(30.)),
            Table::empty()
                .with("m", Value::text("b"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(20.)),
            Table::empty()
                .with("m", Value::text("b"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(20.)),
        ]
    }

    #[test]
    fn long_stack_rewrites_intervals() {
        let mark = Mark::new(MarkKind::BarY)
            .x("m")
            .y("v")
            .z("p")
            .stack(StackOffset::Normalize)
            .stack_series("p");
        let prepared = prepare(&Dataset::new(long_rows()), &[mark]);
        let rows = &prepared[0].rows;
        assert_eq!(rows.len(), 4, "one row per (series, category)");
        let a: Vec<(f64, f64)> = rows
            .iter()
            .filter(|row| row.x_value() == &Value::text("a"))
            .map(|row| {
                (
                    row.y1.as_ref().unwrap().as_number().unwrap(),
                    row.y2.as_ref().unwrap().as_number().unwrap(),
                )
            })
            .collect();
        assert_eq!(a, vec![(0., 0.25), (0.25, 1.)]);
    }

    #[test]
    fn wide_stack_reads_the_named_fields() {
        let rows = vec![
            Table::empty()
                .with("m", Value::text("a"))
                .with("x", Value::Number(1.))
                .with("y", Value::Number(3.)),
            Table::empty()
                .with("m", Value::text("b"))
                .with("x", Value::Number(2.))
                .with("y", Value::Number(4.)),
        ];
        let mark = Mark::new(MarkKind::AreaY)
            .x("m")
            .y("x")
            .stack(StackOffset::Zero)
            .stack_fields(["x", "y"]);
        let prepared = prepare(&Dataset::new(rows), &[mark]);
        assert_eq!(prepared[0].rows.len(), 4);
        let top = prepared[0]
            .rows
            .iter()
            .filter(|row| row.x_value() == &Value::text("a"))
            .map(|row| row.y2.as_ref().unwrap().as_number().unwrap())
            .fold(0.0_f64, f64::max);
        assert_eq!(top, 4.);
    }

    fn grouped_rows() -> Vec<Table> {
        vec![
            Table::empty()
                .with("c", Value::text("a"))
                .with("v", Value::Number(1.))
                .with("sex", Value::text("F")),
            Table::empty()
                .with("c", Value::text("a"))
                .with("v", Value::Number(3.))
                .with("sex", Value::text("M")),
            Table::empty()
                .with("c", Value::text("b"))
                .with("v", Value::Number(5.))
                .with("sex", Value::text("F")),
        ]
    }

    #[test]
    fn group_x_counts_rows_per_category() {
        let mark = Mark::new(MarkKind::BarY).x("c").y("v").group_x();
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        assert_eq!(rows.len(), 2, "one row per category");
        assert_eq!(rows[0].x_value(), &Value::text("a"));
        assert_eq!(rows[0].y_value(), &Value::Number(2.));
        assert_eq!(rows[1].x_value(), &Value::text("b"));
        assert_eq!(rows[1].y_value(), &Value::Number(1.));
    }

    #[test]
    fn a_grouped_bar_keeps_its_baseline() {
        // The grouping writes the count into `y` after the raw rows already
        // grew their interval from zero, so the interval has to be derived
        // again for the bar to have a length to draw.
        let mark = Mark::new(MarkKind::BarY).x("c").y("v").group_x();
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        assert_eq!(
            rows[0].y_interval(),
            Some((Value::Number(0.), Value::Number(2.)))
        );
        assert_eq!(
            rows[1].y_interval(),
            Some((Value::Number(0.), Value::Number(1.)))
        );
    }

    #[test]
    fn group_x_reduces_a_named_channel() {
        let mark = Mark::new(MarkKind::BarY)
            .x("c")
            .y("v")
            .group_x()
            .group_output("y", crate::transform::GroupReducer::Sum);
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        // The later output replaces the default count, and sums the y values.
        assert_eq!(rows[0].y_value(), &Value::Number(4.));
        assert_eq!(rows[1].y_value(), &Value::Number(5.));
    }

    #[test]
    fn find_picks_the_row_matching_its_test() {
        // Plot's example: a bar's two ends come from two rows of the same x,
        // chosen by a field rather than by an extreme.
        let mark = Mark::new(MarkKind::BarY)
            .x("c")
            .y("v")
            .group_x()
            .group_output(
                "y1",
                crate::transform::GroupReducer::find(|row| {
                    row.value("sex").label().as_ref() == "F"
                }),
            )
            .group_output(
                "y2",
                crate::transform::GroupReducer::find(|row| {
                    row.value("sex").label().as_ref() == "M"
                }),
            );
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        let a = rows
            .iter()
            .find(|row| row.x_value() == &Value::text("a"))
            .unwrap();
        assert_eq!(a.y1.as_ref().unwrap(), &Value::Number(1.));
        assert_eq!(a.y2.as_ref().unwrap(), &Value::Number(3.));
        // Category "b" has no M row, so its upper end is missing rather than
        // borrowing a neighbour's value.
        let b = rows
            .iter()
            .find(|row| row.x_value() == &Value::text("b"))
            .unwrap();
        assert_eq!(b.y1.as_ref().unwrap(), &Value::Number(5.));
        assert!(b.y2.is_none());
    }

    #[test]
    fn group_z_subdivides_by_series() {
        let mark = Mark::new(MarkKind::BarY).x("c").y("v").z("sex").group_z();
        let prepared = prepare(&Dataset::new(grouped_rows()), &[mark]);
        let rows = &prepared[0].rows;
        // One row per series, each holding the series' count.
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].z.as_ref().unwrap(), &Value::text("F"));
        assert_eq!(rows[0].row.number("fill"), Some(2.));
        assert_eq!(rows[1].z.as_ref().unwrap(), &Value::text("M"));
        assert_eq!(rows[1].row.number("fill"), Some(1.));
    }
}

#[cfg(test)]
mod facet_tests {
    use super::*;
    use crate::facet::Grid;
    use crate::mark::MarkKind;

    #[test]
    fn a_stack_summarizes_within_its_facet() {
        // Two facets, each with both series, so a whole-figure stack would
        // reach 100 in each and a per-facet one reaches the facet's own total.
        let rows = vec![
            Table::empty()
                .with("f", Value::text("one"))
                .with("m", Value::text("a"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("f", Value::text("one"))
                .with("m", Value::text("a"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(30.)),
            Table::empty()
                .with("f", Value::text("two"))
                .with("m", Value::text("a"))
                .with("p", Value::text("x"))
                .with("v", Value::Number(20.)),
            Table::empty()
                .with("f", Value::text("two"))
                .with("m", Value::text("a"))
                .with("p", Value::text("y"))
                .with("v", Value::Number(20.)),
        ];
        let mark = Mark::new(MarkKind::BarY)
            .x("m")
            .y("v")
            .z("p")
            .stack(StackOffset::Normalize)
            .stack_series("p");
        let grid = Grid::from_values(vec![Value::text("one"), Value::text("two")], Vec::new());
        let (facets, populated) =
            prepare_facets(&Dataset::new(rows), &[mark], &grid, Some("f"), None);
        assert_eq!(facets.len(), 2);
        assert_eq!(populated, vec![true, true]);
        for marks in &facets {
            let top = marks[0]
                .rows
                .iter()
                .map(|row| row.y2.as_ref().unwrap().as_number().unwrap())
                .fold(0.0_f64, f64::max);
            // Normalized within the facet: the two series add to one.
            assert_eq!(top, 1.);
        }
    }

    #[test]
    fn an_unfaceted_grid_prepares_every_row_once() {
        let rows = vec![Table::empty().with("x", Value::Number(1.))];
        let mark = Mark::new(MarkKind::Dot).x("x");
        let grid = Grid::from_values(Vec::new(), Vec::new());
        let (facets, populated) = prepare_facets(&Dataset::new(rows), &[mark], &grid, None, None);
        assert_eq!(facets.len(), 1);
        assert_eq!(facets[0][0].rows.len(), 1);
        assert!(populated.is_empty());
    }
}
