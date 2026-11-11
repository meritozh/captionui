//! Plot's bin transform (`binX`, `binY` and `bin`): the edges a mark bins
//! over, and its rows replaced with one row per bin.

use gpui::SharedString;

use crate::mark::{Channels, Mark, MarkKind};
use crate::render::PreparedRow;
use crate::render::bind::{bound_input, bound_slot};
use crate::render::resolve::{RowFacts, resolve_core, resolve_row};
use crate::table::{Dataset, Table};
use crate::transform::{self, Reducer};
use crate::value::Value;

/// Which axes a binned mark bins, `(x, y)`: Plot's `binX`, `binY` or `bin`.
///
/// A mark with a fixed bin axis (`rectY` and `barY` bin x, `rectX` and
/// `barX` bin y) bins that one; a `rect` bins every axis it names a field on,
/// so `rect(x: "weight", y: "height")` is Plot's two-dimensional `bin`.
pub fn bin_axes(mark: &Mark) -> (bool, bool) {
    if mark.bin.is_none() {
        return (false, false);
    }
    match mark.kind.category().bins_on_x {
        Some(true) => (true, false),
        Some(false) => (false, true),
        None => {
            let named = |channel: &Option<crate::table::Channel>| {
                matches!(channel, Some(crate::table::Channel::Field(_)))
            };
            match (named(&mark.channels.x), named(&mark.channels.y)) {
                (_, false) => (true, false),
                (false, true) => (false, true),
                (true, true) => (true, true),
            }
        }
    }
}

/// The bin edges a mark lays over `rows` on each of its bin axes.
///
/// A faceted plot pins the edges from the whole data first (see
/// [`pin_bin_edges`]), the way Plot computes its thresholds once and then
/// bins each facet, so the facets' bins line up.
fn bin_edges(
    mark: &Mark,
    rows: &[Table],
) -> (Option<transform::BinEdges>, Option<transform::BinEdges>) {
    let (x_channel, y_channel) = bin_channels(mark);
    let numbers = |channel: Option<&crate::table::Channel>| -> Option<Vec<f64>> {
        let channel = channel?;
        let hint = std::cell::Cell::new(0);
        Some(
            rows.iter()
                .filter_map(|row| channel.read_hinted(row, &hint).as_number())
                .collect(),
        )
    };
    bin_edges_with(mark, || numbers(x_channel), || numbers(y_channel))
}

/// [`bin_edges`] over the numbers each bin channel already read, one per row
/// (`None` where the channel is absent, or the row's value is not a number).
fn bin_edges_of(
    mark: &Mark,
    xs: Option<&[Option<f64>]>,
    ys: Option<&[Option<f64>]>,
) -> (Option<transform::BinEdges>, Option<transform::BinEdges>) {
    let numbers = |numbers: Option<&[Option<f64>]>| -> Option<Vec<f64>> {
        Some(numbers?.iter().flatten().copied().collect())
    };
    bin_edges_with(mark, || numbers(xs), || numbers(ys))
}

/// The edges over each bin axis's numbers, which are only gathered when the
/// mark has no pinned edges on that axis.
fn bin_edges_with(
    mark: &Mark,
    xs: impl FnOnce() -> Option<Vec<f64>>,
    ys: impl FnOnce() -> Option<Vec<f64>>,
) -> (Option<transform::BinEdges>, Option<transform::BinEdges>) {
    let Some(bin) = &mark.bin else {
        return (None, None);
    };
    let edges = |numbers: Option<Vec<f64>>| transform::BinEdges::of(&numbers?, bin.thresholds);
    (
        bin.edges[0].or_else(|| edges(xs())),
        bin.edges[1].or_else(|| edges(ys())),
    )
}

/// The channels a binned mark bins on x and y: [`bin_fields`]' choice, but
/// any channel form, so `x: (d) => Math.log(d.volume)` bins too.
fn bin_channels(
    mark: &Mark,
) -> (
    Option<&crate::table::Channel>,
    Option<&crate::table::Channel>,
) {
    let (x, y) = (mark.channels.x.as_ref(), mark.channels.y.as_ref());
    match bin_axes(mark) {
        (true, true) => (x, y),
        (true, false) => (x.or(y), None),
        (false, true) => (None, y.or(x)),
        (false, false) => (None, None),
    }
}

/// Fix every binned mark's edges over the whole data, before it is split
/// into facets.
pub(super) fn pin_bin_edges(data: &Dataset, marks: &[Mark]) -> Vec<Mark> {
    marks
        .iter()
        .map(|mark| {
            let mut mark = mark.clone();
            if mark.bin.is_some() {
                let source = mark.data.clone().unwrap_or_else(|| data.clone());
                let (x, y) = bin_edges(&mark, source.rows());
                if let Some(bin) = mark.bin.as_mut() {
                    bin.edges = [x, y];
                }
            }
            mark
        })
        .collect()
}

/// One series' bins: the row indices in each `(x, y)` bin cell.
type BinCells = std::collections::BTreeMap<(usize, usize), Vec<usize>>;

/// Replace rows with one row per bin, Plot's `binX`, `binY` and `bin`.
///
/// Rows are binned within their `z` series, so `fill: "sex"` gives each sex
/// its own bins over shared edges. Each bin is reduced into the mark's bin
/// outputs — by default, a one-dimensional bin counts into its length
/// channel. A one-dimensional count is stacked across the series in each bin,
/// Plot's implicit `stackY` for `rectY`; an `opacity` or `fillOpacity` output
/// goes through Plot's default opacity scale, `[0, max] → [0, 1]`.
///
/// An output's `$in:<name>` input reads that channel of `bound`, as a
/// group's does.
///
/// `resolved` says the rows' `x` and `y` are still what the mark's `x` and
/// `y` channels read from their records, which no transform has rewritten.
pub(super) fn bin_rows(
    mark: &Mark,
    rows: &[PreparedRow],
    bound: &Channels,
    resolved: bool,
) -> Vec<PreparedRow> {
    if mark.bin.is_none() {
        return rows.to_vec();
    }
    let (x_channel, y_channel) = bin_channels(mark);
    // Each bin channel's number per row, read once for both the edges and
    // the slots. A field the rows already resolved as their own position
    // (`own`) is read from the row rather than its record: the same value,
    // without a second lookup.
    let numbers = |channel: Option<&crate::table::Channel>,
                   own: Option<&crate::table::Channel>,
                   along_x: bool| {
        channel.map(|channel| {
            let field = matches!(channel, crate::table::Channel::Field(_));
            if resolved && field && own.is_some_and(|own| std::ptr::eq(own, channel)) {
                return rows
                    .iter()
                    .map(|row| if along_x { &row.x } else { &row.y }.as_number())
                    .collect::<Vec<_>>();
            }
            let hint = std::cell::Cell::new(0);
            rows.iter()
                .map(|row| channel.read_hinted(&row.row, &hint).as_number())
                .collect::<Vec<_>>()
        })
    };
    let (xs, ys) = (
        numbers(x_channel, mark.channels.x.as_ref(), true),
        numbers(y_channel, mark.channels.y.as_ref(), false),
    );
    bin_cells(mark, rows, xs, ys, bound)
}

/// [`bin_rows`] over a mark's raw records, which nothing before the bin
/// rewrites: each row is read only as far as its bin numbers and series,
/// and only a cell's first row is resolved in full.
pub(super) fn bin_records(
    mark: &Mark,
    records: &[Table],
    channels: &Channels,
    facts: &RowFacts,
    bound: &Channels,
) -> Vec<PreparedRow> {
    let (x_channel, y_channel) = bin_channels(mark);
    // A bin channel that is the row's own field channel reads the value the
    // row resolved, as `bin_rows` reads it from a resolved row.
    let own = |channel: Option<&crate::table::Channel>, own: Option<&crate::table::Channel>| {
        channel.map(|channel| {
            matches!(channel, crate::table::Channel::Field(_))
                && own.is_some_and(|own| std::ptr::eq(own, channel))
        })
    };
    let (own_x, own_y) = (
        own(x_channel, mark.channels.x.as_ref()),
        own(y_channel, mark.channels.y.as_ref()),
    );
    let (mut xs, mut ys) = (
        own_x.map(|_| Vec::with_capacity(records.len())),
        own_y.map(|_| Vec::with_capacity(records.len())),
    );
    let mut z = Vec::with_capacity(records.len());
    let (x_hint, y_hint) = (std::cell::Cell::new(0), std::cell::Cell::new(0));
    for (index, record) in records.iter().enumerate() {
        let core = resolve_core(record, index, channels, mark, facts);
        let number = |own: Option<bool>,
                      channel: Option<&crate::table::Channel>,
                      value: &Value,
                      hint: &std::cell::Cell<usize>| match (own, channel) {
            (Some(true), _) => value.as_number(),
            (_, Some(channel)) => channel.read_hinted(record, hint).as_number(),
            _ => None,
        };
        if let Some(xs) = xs.as_mut() {
            xs.push(number(own_x, x_channel, &core.x, &x_hint));
        }
        if let Some(ys) = ys.as_mut() {
            ys.push(number(own_y, y_channel, &core.y, &y_hint));
        }
        z.push(core.z.as_ref().map(Value::key));
    }
    let rows = RecordBins {
        records,
        z,
        channels,
        mark,
        facts,
    };
    bin_cells(mark, &rows, xs, ys, bound)
}

/// What [`bin_cells`] reads of the rows it bins: resolved rows, or records
/// read only as far as their series ([`RecordBins`]).
trait BinRows {
    fn len(&self) -> usize;
    fn record(&self, index: usize) -> &Table;
    fn z_key(&self, index: usize) -> Option<crate::value::ValueKey>;
    /// The row resolved in full, which a cell starts from.
    fn row(&self, index: usize) -> PreparedRow;
}

impl BinRows for [PreparedRow] {
    fn len(&self) -> usize {
        <[PreparedRow]>::len(self)
    }
    fn record(&self, index: usize) -> &Table {
        &self[index].row
    }
    fn z_key(&self, index: usize) -> Option<crate::value::ValueKey> {
        self[index].z.as_ref().map(|z| z.key())
    }
    fn row(&self, index: usize) -> PreparedRow {
        self[index].clone()
    }
}

/// Raw records with each one's series key, resolved on demand as
/// [`resolve_row`] resolves them.
struct RecordBins<'a> {
    records: &'a [Table],
    z: Vec<Option<crate::value::ValueKey>>,
    channels: &'a Channels,
    mark: &'a Mark,
    facts: &'a RowFacts,
}

impl BinRows for RecordBins<'_> {
    fn len(&self) -> usize {
        self.records.len()
    }
    fn record(&self, index: usize) -> &Table {
        &self.records[index]
    }
    fn z_key(&self, index: usize) -> Option<crate::value::ValueKey> {
        self.z[index].clone()
    }
    fn row(&self, index: usize) -> PreparedRow {
        resolve_row(
            &self.records[index],
            index,
            self.channels,
            self.mark,
            self.facts,
        )
    }
}

/// [`bin_rows`] from each bin channel's number per row.
fn bin_cells(
    mark: &Mark,
    rows: &(impl BinRows + ?Sized),
    xs: Option<Vec<Option<f64>>>,
    ys: Option<Vec<Option<f64>>>,
    bound: &Channels,
) -> Vec<PreparedRow> {
    let Some(bin) = &mark.bin else {
        return Vec::new();
    };
    let (bin_x, bin_y) = bin_axes(mark);
    let (x_edges, y_edges) = bin_edges_of(mark, xs.as_deref(), ys.as_deref());
    if (bin_x && x_edges.is_none()) || (bin_y && y_edges.is_none()) {
        return Vec::new();
    }

    // The outputs: the mark's own, or a count into the length channel. A
    // mark-level `reduce` stays the length's reducer, as before bins had
    // outputs of their own.
    let length_name = if bin_x { "y" } else { "x" };
    let outputs: Vec<crate::mark::GroupOutput> = if bin.outputs.is_empty() {
        if bin_x && bin_y {
            vec![crate::mark::GroupOutput::new(
                "fillOpacity",
                transform::GroupReducer::Count,
            )]
        } else {
            let reducer = match mark.reduce {
                None | Some(Reducer::Count) => transform::GroupReducer::Count,
                Some(Reducer::Sum) => transform::GroupReducer::Sum,
                Some(Reducer::Mean) => transform::GroupReducer::Mean,
                Some(Reducer::Min) => transform::GroupReducer::Min,
                Some(Reducer::Max) => transform::GroupReducer::Max,
                Some(Reducer::Median) => transform::GroupReducer::Median,
                Some(_) => transform::GroupReducer::Count,
            };
            vec![crate::mark::GroupOutput::new(length_name, reducer)]
        }
    } else {
        bin.outputs.clone()
    };

    // Group the rows by series, then by bin cell. The series keep their
    // sorted order, the order the color scale and the stack use. A series
    // gathers its cells in a hash map, then sorts them once.
    type Gathering = rustc_hash::FxHashMap<(usize, usize), Vec<usize>>;
    let mut gathering: std::collections::BTreeMap<
        Option<crate::value::ValueKey>,
        (usize, Gathering),
    > = std::collections::BTreeMap::new();
    for index in 0..rows.len() {
        let slot = |numbers: &Option<Vec<Option<f64>>>, edges: Option<transform::BinEdges>| match (
            numbers, edges,
        ) {
            (Some(numbers), Some(edges)) => {
                let value = numbers[index].filter(|v| v.is_finite())?;
                Some(edges.slot(value))
            }
            _ => Some(0),
        };
        let (Some(xi), Some(yi)) = (slot(&xs, x_edges), slot(&ys, y_edges)) else {
            continue;
        };
        gathering
            .entry(rows.z_key(index))
            .or_insert_with(|| (index, Gathering::default()))
            .1
            .entry((xi, yi))
            .or_default()
            .push(index);
    }
    let mut series: std::collections::BTreeMap<Option<crate::value::ValueKey>, (usize, BinCells)> =
        gathering
            .into_iter()
            .map(|(key, (first, cells))| (key, (first, cells.into_iter().collect())))
            .collect();

    // Plot's `cumulative` and `filter: null` see every bin, empty or not:
    // a cumulative bin gathers its predecessors' rows (or, negative, its
    // successors'), and only then are the empty bins dropped, unless the
    // filter is null.
    if bin.keep_empty || bin.cumulative != 0 {
        let slots = |edges: Option<transform::BinEdges>, binned: bool| {
            if binned {
                edges.map_or(1, |e| e.count)
            } else {
                1
            }
        };
        let (nx, ny) = (slots(x_edges, bin_x), slots(y_edges, bin_y));
        for (_, cells) in series.values_mut() {
            for xi in 0..nx {
                for yi in 0..ny {
                    cells.entry((xi, yi)).or_default();
                }
            }
            if bin.cumulative != 0 && bin_x != bin_y {
                let mut keys: Vec<(usize, usize)> = cells.keys().copied().collect();
                if bin.cumulative < 0 {
                    keys.reverse();
                }
                for pair in keys.windows(2) {
                    let before = cells[&pair[0]].clone();
                    if let Some(cell) = cells.get_mut(&pair[1]) {
                        cell.extend(before);
                    }
                }
            }
            if !bin.keep_empty {
                cells.retain(|_, members| !members.is_empty());
            }
        }
    }

    // Each output's input: its input field, else the channel it outputs to
    // (the mark's own `y` for `{y: "sum"}`), else a field of its name.
    // An input the mark binds (`$in:fill`) reads the original mark's channel.
    type Input<'a> = (
        Option<SharedString>,
        Option<usize>,
        Option<&'a crate::table::Channel>,
    );
    let inputs: Vec<Input> = outputs
        .iter()
        .map(|output| {
            let channel = match output.name.as_ref() {
                "y" if !bin_y => mark.channels.y.as_ref(),
                "x" if !bin_x => mark.channels.x.as_ref(),
                _ => None,
            };
            let input = output.input.clone().or_else(|| match output.name.as_ref() {
                "x" | "y" => None,
                _ => Some(output.name.clone()),
            });
            let slot = input.as_deref().and_then(bound_slot);
            (input, slot, channel)
        })
        .collect();
    let needs_records = outputs.iter().any(|output| output.reducer.reads_records());
    // A bin outputs its extent (`x1`, `x2`) and, for the marks that read a
    // point (a dot, a text), its midpoint as `x`.
    let point_mark = !matches!(
        mark.kind,
        MarkKind::Rect
            | MarkKind::RectX
            | MarkKind::RectY
            | MarkKind::BarX
            | MarkKind::BarY
            | MarkKind::Cell
    );
    let mut binned: Vec<PreparedRow> = Vec::new();
    // The running top (and bottom, for negative values) of each
    // one-dimensional bin's stack.
    let mut stack_tops: std::collections::HashMap<usize, (f64, f64)> =
        std::collections::HashMap::new();
    for (first, cells) in series.values() {
        for (&(xi, yi), members) in cells {
            // An empty bin (kept by `filter: null`) takes its series' first
            // row, as Plot reads z, fill and stroke from the group's first.
            let mut row = rows.row(members.first().copied().unwrap_or(*first));
            let records: Vec<&Table> = if needs_records {
                members.iter().map(|index| rows.record(*index)).collect()
            } else {
                Vec::new()
            };
            if let Some(edges) = x_edges.filter(|_| bin_x) {
                let (x0, x1) = edges.bounds(xi);
                row.x = if point_mark {
                    Value::Number((x0 + x1) / 2.)
                } else {
                    Value::Null
                };
                row.x1 = Some(Value::Number(x0));
                row.x2 = Some(Value::Number(x1));
            }
            if let Some(edges) = y_edges.filter(|_| bin_y) {
                let (y0, y1) = edges.bounds(yi);
                row.y = if point_mark {
                    Value::Number((y0 + y1) / 2.)
                } else {
                    Value::Null
                };
                row.y1 = Some(Value::Number(y0));
                row.y2 = Some(Value::Number(y1));
            }
            for (output, (input, slot, channel)) in outputs.iter().zip(&inputs) {
                let reduced = match output.reducer {
                    transform::GroupReducer::Proportion => {
                        Value::Number(members.len() as f64 / rows.len().max(1) as f64)
                    }
                    // A count counts every member, null or not.
                    transform::GroupReducer::Count => Value::Number(members.len() as f64),
                    _ => {
                        let values: Vec<Value> = members
                            .iter()
                            .map(|index| {
                                let record = rows.record(*index);
                                match (input, channel) {
                                    (Some(field), _) => match slot {
                                        Some(slot) => bound_input(bound, *slot, field, record),
                                        None => record.value(field),
                                    },
                                    (None, Some(channel)) => channel.read(record),
                                    (None, None) => Value::Null,
                                }
                            })
                            .collect();
                        output.reducer.reduce(&values, &records)
                    }
                };
                let number = reduced.as_number().unwrap_or(0.);
                match output.name.as_ref() {
                    "y" if bin_x && !bin_y => {
                        let (up, down) = stack_tops.entry(xi).or_insert((0., 0.));
                        let base = if number < 0. { down } else { up };
                        row.y = Value::Number(number);
                        row.y1 = Some(Value::Number(*base));
                        row.y2 = Some(Value::Number(*base + number));
                        *base += number;
                    }
                    "x" if bin_y && !bin_x => {
                        let (up, down) = stack_tops.entry(yi).or_insert((0., 0.));
                        let base = if number < 0. { down } else { up };
                        row.x = Value::Number(number);
                        row.x1 = Some(Value::Number(*base));
                        row.x2 = Some(Value::Number(*base + number));
                        *base += number;
                    }
                    "y" => row.y = reduced.clone(),
                    "x" => row.x = reduced.clone(),
                    // An explicit end (`{y2: "count"}`) is unstacked: the bar
                    // runs from zero, or from a `y1` output, to it.
                    "y1" => row.y1 = Some(reduced.clone()),
                    "y2" => {
                        row.y2 = Some(reduced.clone());
                        if !outputs.iter().any(|o| o.name.as_ref() == "y1") {
                            row.y1 = Some(Value::Number(0.));
                        }
                    }
                    "x1" => row.x1 = Some(reduced.clone()),
                    "x2" => {
                        row.x2 = Some(reduced.clone());
                        if !outputs.iter().any(|o| o.name.as_ref() == "x1") {
                            row.x1 = Some(Value::Number(0.));
                        }
                    }
                    "fillOpacity" | "opacity" => row.opacity_value = Some(number),
                    "r" => row.r_value = Some(number),
                    _ => {}
                }
                row.row.set(output.name.clone(), reduced);
            }
            binned.push(row);
        }
    }

    binned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::MarkKind;
    use crate::prelude::*;
    use crate::render::{apply_quantity_scales, apply_scale_transforms, prepare};
    use crate::transform::{GroupReducer, Thresholds};

    fn athletes() -> Dataset {
        // Two series over weights 40..=99, heights two values apart.
        Dataset::new((0..120).map(|i| {
            Table::empty()
                .with(
                    "sex",
                    Value::text(if i % 2 == 0 { "female" } else { "male" }),
                )
                .with("weight", Value::Number(40. + (i / 2) as f64))
                .with("height", Value::Number(if i % 4 < 2 { 1.6 } else { 1.9 }))
        }))
    }

    #[test]
    fn a_fill_field_bins_and_stacks_each_series() {
        let mark: Mark = Mark::new(MarkKind::RectY)
            .x("weight")
            .fill("sex")
            .bin_output("y", GroupReducer::Count);
        let prepared = prepare(&athletes(), &[mark]);
        let rows = &prepared[0].rows;
        // Both series share the bins' edges.
        let edges = |z: &str| -> Vec<(Value, Value)> {
            rows.iter()
                .filter(|row| row.z.as_ref().map(Value::label).as_deref() == Some(z))
                .map(|row| row.x_interval().unwrap())
                .collect()
        };
        assert_eq!(edges("female"), edges("male"));
        // Each bin's male rect stacks on its female rect, the counts summing
        // to every row.
        let total: f64 = rows.iter().filter_map(|row| row.y.as_number()).sum();
        assert_eq!(total, 120.);
        let top = rows
            .iter()
            .filter_map(|row| row.y2.as_ref()?.as_number())
            .fold(0., f64::max);
        let tallest_bin = rows
            .iter()
            .filter(|row| row.y1.as_ref().and_then(Value::as_number) == Some(0.))
            .count();
        assert!(top > 0. && tallest_bin > 0);
        for row in rows
            .iter()
            .filter(|row| row.z.as_ref().unwrap().label() == "male")
        {
            assert!(
                row.y1.as_ref().unwrap().as_number().unwrap() > 0.,
                "male stacks on female"
            );
        }
    }

    #[test]
    fn a_group_output_reads_the_mark_s_own_channel() {
        // `groupX({fill: "mean"}, {x: "height", fill: "weight"})` averages
        // each height's weights, and the bar's fill then reads the output.
        let mark = Mark::new(MarkKind::BarY)
            .x("height")
            .fill("weight")
            .group_x()
            .group_output("fill", GroupReducer::Mean);
        let rows = &prepare(&athletes(), &[mark])[0].rows;
        let fills: Vec<(Value, Value)> = rows
            .iter()
            .map(|row| (row.x.clone(), row.row.value("fill")))
            .collect();
        assert_eq!(
            fills,
            vec![
                (Value::Number(1.6), Value::Number(69.)),
                (Value::Number(1.9), Value::Number(70.)),
            ]
        );
    }

    #[test]
    fn a_bin_output_reads_the_mark_s_own_channel() {
        let mark: Mark = Mark::new(MarkKind::RectY)
            .x("weight")
            .fill("height")
            .bin_output("fill", GroupReducer::Mean);
        let rows = &prepare(&athletes(), &[mark])[0].rows;
        assert!(!rows.is_empty());
        for row in rows {
            let mean = row.row.value("fill").as_number().unwrap();
            assert!((1.6..=1.9).contains(&mean), "{mean}");
        }
    }

    #[test]
    fn scale_transforms_map_channels_after_the_mark_transforms() {
        use crate::options::ScaleOptions;
        // A stacked bar: the stack runs in data units, then the y scale's
        // transform maps its interval, as Plot's applyScaleTransforms.
        let mark = Mark::new(MarkKind::BarY).x("sex").y("weight").stack_y();
        let mut facets = vec![prepare(&athletes(), &[mark])];
        let raw = facets.clone();
        let y = ScaleOptions::default().transform(|d| d / 1000.);
        let none = ScaleOptions::default();
        apply_scale_transforms(&mut facets, &none, &y, &none, &none);
        for (a, b) in raw[0][0].rows.iter().zip(&facets[0][0].rows) {
            let (a1, a2) = a.y_interval().unwrap();
            let (b1, b2) = b.y_interval().unwrap();
            assert_eq!(b1.as_number().unwrap(), a1.as_number().unwrap() / 1000.);
            assert_eq!(b2.as_number().unwrap(), a2.as_number().unwrap() / 1000.);
            // The x scale has no transform, so its ordinal values stay.
            assert_eq!(a.x, b.x);
        }
    }

    #[test]
    fn a_rect_naming_x_and_y_bins_both() {
        let mark: Mark = Mark::new(MarkKind::Rect)
            .x("weight")
            .y("height")
            .fill("sex")
            .bin_output("fillOpacity", GroupReducer::Count);
        assert_eq!(bin_axes(&mark), (true, true));
        let mut facets = vec![prepare(&athletes(), &[mark])];
        apply_quantity_scales(&mut facets, &Default::default(), &Default::default());
        let rows = &facets[0][0].rows;
        assert!(rows.iter().all(|row| row.x1.is_some() && row.y1.is_some()));
        // The opacity scale runs [0, max] → [0, 1].
        let max = rows.iter().filter_map(|row| row.opacity).fold(0., f32::max);
        assert_eq!(max, 1.);
        assert_eq!(
            rows.iter()
                .map(|row| row.row.number("fillOpacity").unwrap())
                .sum::<f64>(),
            120.
        );
    }

    #[test]
    fn auto_thresholds_follow_scotts_rule_on_nice_edges() {
        let values: Vec<f64> = (0..1000).map(|i| (i % 100) as f64 + 31.).collect();
        let edges = transform::BinEdges::of(&values, Thresholds::Auto).unwrap();
        // Scott: 3.49·σ·n^(-1/3) ≈ 10 → ten bins of a nice width 10 from 30.
        assert_eq!(edges.width, 10.);
        assert_eq!(edges.start, 30.);
        assert_eq!(edges.slot(130.), edges.count - 1);
    }

    #[test]
    fn a_color_field_implies_the_series() {
        let mark: Mark = Mark::new(MarkKind::Dot)
            .x("weight")
            .y("height")
            .stroke("sex");
        assert!(
            matches!(&mark.channels.z, Some(crate::table::Channel::Field(name)) if name == "sex")
        );
    }
}
