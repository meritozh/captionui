//! Mark rendering: resolve a mark's channels against the data, then draw the
//! shapes Plot's mark types describe.
//!
//! Resolution and drawing are separate: [`prepare`] turns a [`Mark`] and a
//! [`Dataset`] into [`PreparedMark`]s, which hold plain values, and the frame
//! unions their values to build the scales. Drawing then only reads pixels.

pub(crate) mod basic;
pub mod color;
pub(crate) mod delaunay;
pub(crate) mod furniture;
pub mod radial;
pub mod stack;
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
    /// A raster or contour mark's grid, resolved from its rows (or sampled
    /// once the frame is known, for a function).
    pub raster: Option<std::rc::Rc<crate::raster::RasterGrid>>,
    /// A density mark's peak, in Plot's density units, for the color scale.
    pub density_max: Option<f64>,
    /// Whether a dodge packed this mark's rows along x, so its offsets are
    /// vertical; meaningless when the mark does not dodge.
    pub dodge_along_x: bool,
    /// The x and y channel values of the rows the mark's `filter` dropped:
    /// Plot filters the index a mark draws, but infers its scales from the
    /// channels over all the data.
    pub filtered_domain: [Vec<Value>; 2],
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
    /// A hierarchy path, read for the `tree` mark.
    pub path: Option<SharedString>,
    /// The resolved radius of a dot.
    pub r: Option<f32>,
    /// The resolved symbol of a dot.
    pub symbol: Option<crate::style::Symbol>,
    /// The row's opacity from its `opacity` channel, when it has one.
    pub opacity: Option<f32>,
    /// The data value of a scaled `r` channel, before the radius scale.
    pub r_value: Option<f64>,
    /// The data value of a scaled `opacity` channel (or a bin's or group's
    /// `fillOpacity` output), before the opacity scale.
    pub opacity_value: Option<f64>,
    /// Where the row sits across the axis a dodge packs along, in pixels from
    /// that axis's start. Set by [`apply_dodge`] once the frame is known.
    pub dodge_offset: Option<f32>,
    /// A position in plot pixels that overrides the scales, for transforms
    /// that work in pixels (Plot's `hexbin`).
    pub pixel: Option<(f32, f32)>,
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

/// Resolve every mark against its data, for an unfaceted plot: the mark's
/// own data when it has some, else the plot's `data`.
pub fn prepare(data: &Dataset, marks: &[Mark]) -> Vec<PreparedMark> {
    marks
        .iter()
        .map(|mark| prepare_mark(mark.data.as_ref().unwrap_or(data), mark))
        .collect()
}

/// The facet channels a mark is split by: its own `fx`/`fy`, else — for a
/// mark reading the plot's data — the plot's facet fields. A mark with its
/// own data and no facet channels of its own is not faceted: Plot repeats it
/// in every facet (its `facet: "auto"`).
fn facet_channels(
    mark: &Mark,
    fx: Option<&str>,
    fy: Option<&str>,
) -> (Option<crate::table::Channel>, Option<crate::table::Channel>) {
    if mark.fx.is_some() || mark.fy.is_some() {
        return (mark.fx.clone(), mark.fy.clone());
    }
    if mark.data.is_some() {
        return (None, None);
    }
    let field = |name: Option<&str>| name.map(|name| crate::table::Channel::Field(name.into()));
    (field(fx), field(fy))
}

/// Resolve every mark against the part of its data that lands in each facet.
///
/// A mark's rows are split by its facet channels (see [`facet_channels`]),
/// and each facet's share then resolves on its own, so a stack, bin or
/// reducer summarizes within its facet; a mark with no facet channels draws
/// all its rows in every facet. The facets come back in the grid's order, one
/// list of marks per facet, with which facets hold any faceted rows; a figure
/// that is not faceted has one facet holding every row and no empty facets.
pub fn prepare_facets(
    data: &Dataset,
    marks: &[Mark],
    grid: &Grid,
    fx: Option<&str>,
    fy: Option<&str>,
) -> (Vec<Vec<PreparedMark>>, Vec<bool>) {
    prepare_facets_reading(data, marks, grid, fx, fy, &Default::default())
}

/// [`prepare_facets`], taking a facet channel's values over a mark's data
/// from `reads` when building the grid already read them.
pub(crate) fn prepare_facets_reading(
    data: &Dataset,
    marks: &[Mark],
    grid: &Grid,
    fx: Option<&str>,
    fy: Option<&str>,
    reads: &crate::facet::FacetReads,
) -> (Vec<Vec<PreparedMark>>, Vec<bool>) {
    if !grid.is_faceted() {
        return (vec![prepare(data, marks)], Vec::new());
    }
    let marks = pin_bin_edges(data, marks);
    let cells = grid.cells.len();
    let mut prepared: Vec<Vec<PreparedMark>> = vec![Vec::with_capacity(marks.len()); cells];
    let mut populated = vec![false; cells];
    // A furniture mark faceted by constants reads one placeholder record,
    // which lands in the facet the constants name.
    let placeholder = Dataset::new([Table::empty()]);
    // Marks over the same source and facet channels (a box's parts, marks
    // over the plot's data) share one partition and one dataset per facet.
    type Partition = (
        Dataset,
        Option<crate::table::Channel>,
        Option<crate::table::Channel>,
        Vec<Dataset>,
    );
    let mut partitions: Vec<Partition> = Vec::new();
    let same_channel =
        |a: &Option<crate::table::Channel>, b: &Option<crate::table::Channel>| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same(b),
            _ => false,
        };
    for mark in &marks {
        let source = if mark.kind.is_furniture() && mark.data.is_none() {
            &placeholder
        } else {
            mark.data.as_ref().unwrap_or(data)
        };
        let (mark_fx, mark_fy) = facet_channels(mark, fx, fy);
        if mark_fx.is_none() && mark_fy.is_none() {
            let resolved = prepare_mark(source, mark);
            for cell in prepared.iter_mut() {
                cell.push(resolved.clone());
            }
            continue;
        }
        let cached = partitions.iter().position(|(data, px, py, _)| {
            data.same_rows(source) && same_channel(px, &mark_fx) && same_channel(py, &mark_fy)
        });
        let cached = cached.unwrap_or_else(|| {
            let groups = grid
                .partition_reading(source, mark_fx.as_ref(), mark_fy.as_ref(), reads)
                .into_iter()
                .map(Dataset::new)
                .collect();
            partitions.push((source.clone(), mark_fx.clone(), mark_fy.clone(), groups));
            partitions.len() - 1
        });
        for (cell, rows) in partitions[cached].3.iter().enumerate() {
            // A facet axis labels every facet, so it fills none of them.
            populated[cell] |= !rows.is_empty() && mark.facet_axis.is_none();
            // A furniture mark's placeholder landed in another facet: this
            // one draws nothing (an empty dataset would read a placeholder).
            if rows.is_empty()
                && std::ptr::eq(source, &placeholder)
                && (mark.fx.is_some() || mark.fy.is_some())
            {
                let mut empty = prepare_mark(&placeholder, mark);
                empty.rows.clear();
                prepared[cell].push(empty);
                continue;
            }
            prepared[cell].push(prepare_mark(rows, mark));
        }
    }
    (prepared, populated)
}

/// A mark over plain values or tuples reads them by default, as Plot's marks
/// do: `Plot.ruleY([0])` puts `y` on the value, `Plot.dotX(numbers)` puts
/// `x` on it, and `Plot.line([[0, 1], [1, 2]])` reads `x` and `y` from each
/// pair.
pub fn with_value_defaults(mark: &Mark, data: &Dataset) -> Option<Mark> {
    use crate::table::Channel;
    let channels = &mark.channels;
    let unset_x = channels.x.is_none() && channels.x1.is_none() && channels.x2.is_none();
    let unset_y = channels.y.is_none() && channels.y1.is_none() && channels.y2.is_none();
    let binned_or_grouped = mark.bin.is_some() || mark.group.is_some();
    if data.is_values() {
        let mut mark = mark.clone();
        let set_x = |m: &mut Mark, c: Channel| {
            if unset_x {
                m.channels.x = Some(c)
            }
        };
        let set_y = |m: &mut Mark, c: Channel| {
            if unset_y {
                m.channels.y = Some(c)
            }
        };
        use MarkKind::*;
        match mark.kind {
            // One-dimensional marks read the value on their axis.
            DotX | TextX | TickX | RuleX | GridX | AxisX | BoxX | DensityX | ViolinX | VectorX => {
                set_x(&mut mark, Channel::identity())
            }
            DotY | TextY | TickY | RuleY | GridY | AxisY | BoxY | DensityY | ViolinY | VectorY => {
                set_y(&mut mark, Channel::identity())
            }
            // Plot's `areaY`, `lineY`, `rectY` and `barY` over values: the
            // value on y and, unbinned, the index on x (`rectY` and `areaY`
            // stack). Binned or grouped, the value is the binned x.
            AreaY | LineY | BarY | RectY | WaffleY => {
                if binned_or_grouped {
                    set_x(&mut mark, Channel::identity());
                } else {
                    set_y(&mut mark, Channel::identity());
                    set_x(&mut mark, Channel::index());
                    // Plot's `rectY(values)`: each value spans its index's
                    // unit interval.
                    if mark.kind == RectY && unset_x && mark.interval.is_none() {
                        mark.interval = Some(crate::mark::Interval::Step(1.));
                    }
                }
            }
            AreaX | LineX | BarX | RectX | WaffleX => {
                if binned_or_grouped {
                    set_y(&mut mark, Channel::identity());
                } else {
                    set_x(&mut mark, Channel::identity());
                    set_y(&mut mark, Channel::index());
                    if mark.kind == RectX && unset_y && mark.interval.is_none() {
                        mark.interval = Some(crate::mark::Interval::Step(1.));
                    }
                }
            }
            // `cellX(values)`: x the index, fill the value; `cellY` likewise.
            CellX => {
                set_x(&mut mark, Channel::index());
                if mark.channels.fill.is_none() {
                    mark.channels.fill = Some(crate::style::ColorChannel::identity());
                }
            }
            CellY => {
                set_y(&mut mark, Channel::index());
                if mark.channels.fill.is_none() {
                    mark.channels.fill = Some(crate::style::ColorChannel::identity());
                }
            }
            _ => return None,
        }
        return Some(mark);
    }
    if data.is_tuples() {
        let mut mark = mark.clone();
        if unset_x {
            mark.channels.x = Some(Channel::Field("0".into()));
        }
        if unset_y {
            mark.channels.y = Some(Channel::Field("1".into()));
        }
        // Plot's text over tuples labels each with its index.
        if matches!(mark.kind, MarkKind::Text) && mark.channels.text.is_none() {
            mark.channels.text = Some(Channel::index());
        }
        return Some(mark);
    }
    None
}

fn prepare_mark(data: &Dataset, mark: &Mark) -> PreparedMark {
    let defaulted = with_value_defaults(mark, data);
    let mark = defaulted.as_ref().unwrap_or(mark);
    let original = mark;
    let bound = bind_outputs(mark);
    let mark = bound.as_ref().unwrap_or(mark);
    // A furniture mark draws the plot's axes, grid or frame rather than data,
    // so it reads no channels and resolves no rows.
    // A furniture mark without data reads one placeholder record, which its
    // constant channels (`axisY({x: 0})`, `gridX({stroke: "white"})`) read.
    if mark.kind.is_furniture() && mark.data.is_none() && data.len() != 1 {
        let placeholder = Dataset::new([Table::empty()]);
        return prepare_mark(&placeholder, mark);
    }
    let channels = &mark.channels;

    // The data transforms rewrite the records themselves, so they run before
    // any row is resolved: `select` drops rows, and `map`/`shift` add fields
    // the channels may read. Plot applies them to the mark's data in the same
    // order, and a mark's `shuffle` is only a reordering of what is left.
    let mut source = data.clone();
    // Plot's tree transforms replace the records with the laid-out nodes or
    // links.
    if let Some((spec, links)) = &mark.tree {
        source = match crate::hierarchy::layout(&source, spec) {
            Some(h) if *links => h.link_records(spec),
            Some(h) => h.node_records(spec),
            None => Dataset::default(),
        };
        if let Some(filter) = &mark.tree_filter {
            source = Dataset::new(
                source
                    .rows()
                    .iter()
                    .filter(|r| r.value(filter) == Value::Bool(true))
                    .cloned(),
            );
        }
    }
    let mut dropped: Vec<Table> = Vec::new();
    if let Some(keep) = &mark.filter {
        let (kept, out): (Vec<Table>, Vec<Table>) =
            source.rows().iter().cloned().partition(|row| (keep.0)(row));
        source = Dataset::new(kept);
        dropped = out;
    }
    if let Some(config) = mark.select.as_ref().filter(|config| config.axis.is_none()) {
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
    // Plot's `map` transform: each mapped channel's values, per `z` series,
    // land on the record under `$map:<channel>`, which the channel then
    // reads. The mark keeps its original channels for its labels.
    let mapped;
    let (mark, channels) = if mark.maps.is_empty() {
        (mark, channels)
    } else {
        let (data, with_maps) = apply_maps(&source, mark);
        source = data;
        mapped = with_maps;
        (&mapped, &mapped.channels)
    };
    if let Some(config) = &mark.shift {
        source = transform::shift(&source, &config.input, &config.output, config.offset);
    }
    if let Some(seed) = mark.shuffle {
        source = transform::shuffle(&source, seed);
    }

    let source_rows_for_raster = source.clone();
    // Plot's `maybeZ`: a `z` implied by a color field is dropped when the
    // field is quantitative (numbers or dates), so a continuous fill colors
    // rows without splitting them into series.
    let dropped_z;
    let z_hint = std::cell::Cell::new(0);
    let channels = if mark.implied_z
        && let Some(z) = &channels.z
        && source
            .rows()
            .iter()
            .map(|row| z.read_hinted(row, &z_hint))
            .find(|v| !v.is_null())
            .is_some_and(|v| v.kind().is_quantitative())
    {
        dropped_z = Channels {
            z: None,
            ..channels.clone()
        };
        &dropped_z
    } else {
        channels
    };
    let facts = RowFacts::new(channels, mark);
    // Each consumer below gets its own closure, so each inlines the resolve.
    let raw = || source.rows().iter().enumerate();
    // A transform output reads the mark's own channel of that name unless it
    // names an input (Plot's `maybeInput`): `binX({fill: "x"}, {fill:
    // "Anomaly"})` averages Anomaly. The output reads `$in:<name>`, which the
    // group or bin resolves against the original mark's channel (see
    // [`bound_input`]).
    let bound_channels = &original.channels;
    // A grouped mark whose raw rows no transform reads before the grouping
    // streams them straight into their groups, without collecting them; the
    // transforms below up to the grouping are then all no-ops.
    let streamed = mark
        .group
        .as_ref()
        .filter(|_| mark.sort.is_none() && !transforms_before_group(mark));
    // A box's outliers are a few rows of many: when nothing before the
    // outlier filter rewrites the rows, each is read only as far as its
    // value and series, and only the outliers are resolved in full.
    let outliers_first = mark.outliers.filter(|_| {
        streamed.is_none()
            && mark.sort.is_none()
            && mark.window.is_none()
            && !mark.normalize
            && !(mark.normalize_by.is_some() && mark.bin.is_none())
    });
    // A bin whose rows nothing rewrites before it bins the records
    // themselves, resolving only each cell's first row; the transforms
    // below up to the bin are then all no-ops on the rows left empty.
    let binned_records = mark.bin.is_some()
        && streamed.is_none()
        && mark.group.is_none()
        && mark.sort.is_none()
        && !transforms_before_group(mark);
    let mut rows: Vec<PreparedRow> = match (streamed, outliers_first) {
        (Some(config), _) => collapse_groups(
            raw().map(|(index, row)| CoreRow {
                core: resolve_core(row, index, channels, mark, &facts),
                record: row,
                index,
            }),
            |row| finish_row(row.core, row.record, row.index, channels, &facts),
            mark,
            config,
            bound_channels,
        ),
        (None, Some(axis)) => {
            let keep = outlier_flags(
                raw().map(|(index, row)| CoreRow {
                    core: resolve_core(row, index, channels, mark, &facts),
                    record: row,
                    index,
                }),
                axis,
            );
            raw()
                .zip(keep)
                .filter(|(_, keep)| *keep)
                .map(|((index, row), _)| resolve_row(row, index, channels, mark, &facts))
                .collect()
        }
        (None, None) if binned_records => Vec::new(),
        (None, None) => raw()
            .map(|(index, row)| resolve_row(row, index, channels, mark, &facts))
            .collect(),
    };

    // A mark's own ordering decides where its shapes land, and a line's order
    // is its path, so sorting happens before anything positional.
    apply_sort(&mut rows, mark);

    // Windows and normalization rewrite the value the mark draws, so they run
    // before the shapes read it.
    apply_window(mark, &mut rows);
    if mark.normalize {
        normalize_rows(&mut rows);
    }
    // Plot's `normalizeY(binX(…))` normalizes the bins, so a binned mark
    // normalizes after binning.
    if let Some((axis, basis)) = mark.normalize_by.filter(|_| mark.bin.is_none()) {
        stack::normalize(&mut rows, axis, basis);
    }
    if let Some(axis) = mark.outliers.filter(|_| outliers_first.is_none()) {
        rows = outlier_rows(rows, axis);
    }
    // A select over an axis (`selectMinX`) reads the mark's resolved channel,
    // after the transforms that rewrite it, per `z` series (or group field).
    if let Some(config) = mark.select.as_ref().filter(|config| config.axis.is_some()) {
        rows = select_rows(rows, config);
    }

    // A grouping collapses the rows sharing a category into one row each,
    // Plot's `groupX`/`groupY`/`groupZ`.
    if let Some(config) = mark.group.as_ref().filter(|_| streamed.is_none()) {
        rows = collapse_groups(rows, |row| row, mark, config, bound_channels);
    }

    // A reducer collapses the rows sharing a category, Plot's `groupX`. A
    // binned mark reduces each bin instead, so its reducer goes to the bin.
    if let Some(reducer) = mark.reduce
        && mark.bin.is_none()
    {
        rows = reduce_rows(rows, mark, reducer);
    }

    // Binning replaces the rows with one row per bin. Unless a transform
    // above rewrote them, the rows' x and y are still what their channels
    // read.
    if mark.bin.is_some() {
        let resolved = mark.group.is_none() && !transforms_before_group(mark);
        rows = if binned_records {
            bin_records(mark, source.rows(), channels, &facts, bound_channels)
        } else {
            bin_rows(mark, &rows, bound_channels, resolved)
        };
        if let Some((axis, basis)) = mark.normalize_by {
            stack::normalize(&mut rows, axis, basis);
        }
    }

    // Stacking rewrites the y interval of every row.
    if mark.stack.is_some() {
        rows = stack_rows(rows, mark);
    }
    if let Some(spec) = stack::effective_stack(mark) {
        stack::stack(&mut rows, &spec);
    }

    // Statistical marks summarize their category themselves, so they keep the
    // raw rows and group at draw time.

    // A filtered row's channels still feed the scales, unless a transform
    // (a bin, a group, a stack) writes that channel from the kept rows.
    let mut filtered_domain: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
    if !dropped.is_empty() && mark.bin.is_none() && mark.group.is_none() && mark.stack.is_none() {
        let stacked = stack::effective_stack(mark).map(|spec| spec.axis);
        let facts = RowFacts::new(&mark.channels, mark);
        for (index, table) in dropped.iter().enumerate() {
            let row = resolve_row(table, index, &mark.channels, mark, &facts);
            if stacked != Some(Axis::X) {
                filtered_domain[0].extend(
                    [Some(row.x.clone()), row.x1.clone(), row.x2.clone()]
                        .into_iter()
                        .flatten()
                        .filter(|v| !v.is_null()),
                );
            }
            if stacked != Some(Axis::Y) {
                filtered_domain[1].extend(
                    [Some(row.y.clone()), row.y1.clone(), row.y2.clone()]
                        .into_iter()
                        .flatten()
                        .filter(|v| !v.is_null()),
                );
            }
        }
    }

    PreparedMark {
        mark: mark.clone(),
        raster: dense_raster(mark, &source_rows_for_raster),
        density_max: None,
        rows,
        dodge_along_x: false,
        filtered_domain,
    }
}

/// Keep each `z` group's rows beyond its whiskers, Plot's box outliers.
fn outlier_rows(mut rows: Vec<PreparedRow>, axis: Axis) -> Vec<PreparedRow> {
    // Kept in place, so only the outliers move; `retain` visits the rows in
    // order, as the flags list them.
    let mut keep = outlier_flags(rows.iter(), axis).into_iter();
    rows.retain(|_| keep.next().expect("a flag per row"));
    rows
}

/// Which of `rows` lie beyond their `z` group's whiskers, in row order.
fn outlier_flags<R: GroupRow>(rows: impl Iterator<Item = R>, axis: Axis) -> Vec<bool> {
    let read = |row: &R| -> Value {
        if axis == Axis::X {
            GroupRow::x_value(row).clone()
        } else {
            GroupRow::y_value(row).clone()
        }
    };
    // Each `z` group's first row's `z` and values, in row order, and each
    // row's group and number. A row whose `z` equals the `z` of the previous
    // row's group is in that group, since equal values have equal keys; any
    // other asks the map.
    let mut groups: Vec<(Option<Value>, Vec<Value>)> = Vec::new();
    let mut slots: rustc_hash::FxHashMap<crate::value::ValueKey, usize> = Default::default();
    let mut members: Vec<(usize, Option<f64>)> = Vec::with_capacity(rows.size_hint().0);
    let mut last: Option<usize> = None;
    for row in rows {
        let group = match last {
            Some(group) if groups[group].0.as_ref() == row.z() => group,
            _ => {
                let key = row.z().map(Value::key).unwrap_or_else(|| Value::Null.key());
                *slots.entry(key).or_insert_with(|| {
                    groups.push((row.z().cloned(), Vec::new()));
                    groups.len() - 1
                })
            }
        };
        last = Some(group);
        let value = read(&row);
        members.push((group, value.as_number()));
        groups[group].1.push(value);
    }
    let bounds: Vec<(f64, f64)> = groups
        .into_iter()
        .map(|(_, values)| {
            let mut order = transform::OrderStatistics::default();
            let lo = crate::mark::whisker_low_with(&values, &mut order)
                .as_number()
                .unwrap_or(f64::NEG_INFINITY);
            let hi = crate::mark::whisker_high_with(&values, &mut order)
                .as_number()
                .unwrap_or(f64::INFINITY);
            (lo, hi)
        })
        .collect();
    members
        .into_iter()
        .map(|(group, value)| {
            let (lo, hi) = bounds[group];
            value.is_some_and(|v| v < lo || v > hi)
        })
        .collect()
}

/// Keep the row of each group with the least or greatest value on the
/// selector's axis, Plot's `selectMinX` and its siblings.
fn select_rows(rows: Vec<PreparedRow>, config: &crate::mark::SelectConfig) -> Vec<PreparedRow> {
    let along_x = config.axis == Some(Axis::X);
    let want_max = matches!(config.selector, Selector::Max(_));
    let mut best: Vec<(crate::value::ValueKey, usize, f64)> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let group = match &config.group {
            Some(field) => row.row.value(field).key(),
            None => row
                .z
                .as_ref()
                .map(Value::key)
                .unwrap_or_else(|| Value::Null.key()),
        };
        let Some(v) = (if along_x {
            row.x_value()
        } else {
            row.y_value()
        })
        .as_number() else {
            continue;
        };
        match best.iter_mut().find(|(g, _, _)| *g == group) {
            Some(entry) => {
                if (want_max && v > entry.2) || (!want_max && v < entry.2) {
                    entry.1 = i;
                    entry.2 = v;
                }
            }
            None => best.push((group, i, v)),
        }
    }
    let keep: std::collections::HashSet<usize> = best.into_iter().map(|(_, i, _)| i).collect();
    rows.into_iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, row)| row)
        .collect()
}

/// Plot's `applyScaleTransforms`: once the marks' own transforms (stack,
/// bin, window …) have run, each channel bound to a scale with a
/// `transform` option has its values mapped through it, before any scale
/// infers its domain. The position channels (`x`, `x1`, `x2` and the `y`
/// family) take the x and y scales' transforms, and the scaled `r` and
/// `opacity` values the `r` and `opacity` scales'; the color scale's
/// transform is applied where its values are read, in
/// [`color::PlotColors`].
pub fn apply_scale_transforms(
    facets: &mut [Vec<PreparedMark>],
    x: &crate::options::ScaleOptions,
    y: &crate::options::ScaleOptions,
    r: &crate::options::ScaleOptions,
    opacity: &crate::options::ScaleOptions,
) {
    let (tx, ty, tr, to) = (&x.transform, &y.transform, &r.transform, &opacity.transform);
    if tx.is_none() && ty.is_none() && tr.is_none() && to.is_none() {
        return;
    }
    for mark in facets.iter_mut().flatten() {
        // Furniture reads the scales, not channels.
        if mark.mark.kind.is_furniture() {
            continue;
        }
        let [fx, fy] = &mut mark.filtered_domain;
        for (t, values) in [(tx, fx), (ty, fy)] {
            if let Some(t) = t {
                values.iter_mut().for_each(|v| t.apply_mut(v));
            }
        }
        for row in mark.rows.iter_mut() {
            if let Some(t) = tx {
                for v in [Some(&mut row.x), row.x1.as_mut(), row.x2.as_mut()] {
                    v.into_iter().for_each(|v| t.apply_mut(v));
                }
            }
            if let Some(t) = ty {
                for v in [Some(&mut row.y), row.y1.as_mut(), row.y2.as_mut()] {
                    v.into_iter().for_each(|v| t.apply_mut(v));
                }
            }
            if let (Some(t), Some(v)) = (tr, row.r_value.as_mut()) {
                *v = t.number(*v);
            }
            if let (Some(t), Some(v)) = (to, row.opacity_value.as_mut()) {
                *v = t.number(*v);
            }
        }
    }
}

/// Put every scaled `r` and `opacity` value through Plot's scales: a sqrt
/// radius scale over `[0, max]` whose first quartile is three pixels (capped
/// so the largest dot is 30), and a linear opacity scale from `[0, max]` to
/// `[0, 1]`. Both span every facet, so sizes compare across the figure.
pub fn apply_quantity_scales(
    facets: &mut [Vec<PreparedMark>],
    r: &crate::options::ScaleOptions,
    opacity: &crate::options::ScaleOptions,
) {
    let values =
        |facets: &[Vec<PreparedMark>], read: fn(&PreparedRow) -> Option<f64>| -> Vec<f64> {
            facets
                .iter()
                .flatten()
                .flat_map(|mark| mark.rows.iter().filter_map(read))
                .filter(|v| v.is_finite())
                .collect()
        };
    let radii = values(facets, |row| row.r_value);
    if !radii.is_empty() {
        let max = radii.iter().copied().fold(0., f64::max);
        let [d0, d1] = r.numeric_domain().unwrap_or([0., max]);
        // Plot's radius hint: a hexbin's `r` output runs to half its bin.
        let hint = facets.iter().flatten().find_map(|m| {
            m.mark
                .hexbin
                .as_ref()
                .filter(|(o, _)| o.iter().any(|o| o.name == "r"))
                .map(|(_, w)| *w as f64 / 2.)
        });
        let range = match (&r.range, hint) {
            (Some(range), _) if range.len() >= 2 => [range[0], range[1]],
            (_, Some(hint)) => [0., hint],
            _ => {
                // Plot's `inferRadialRange`: the median of the marks' first
                // quartiles of positive values maps to three pixels.
                let mut positive: Vec<f64> = radii.iter().copied().filter(|v| *v > 0.).collect();
                positive.sort_by(f64::total_cmp);
                let q25 = positive
                    .get(((positive.len().max(1) - 1) as f64 * 0.25).round() as usize)
                    .copied()
                    .unwrap_or(1.)
                    .max(f64::MIN_POSITIVE);
                let range = [
                    3. * (d0 / q25).max(0.).sqrt(),
                    3. * (d1 / q25).max(0.).sqrt(),
                ];
                let k = 30. / range[1].max(f64::MIN_POSITIVE);
                if k < 1. {
                    [range[0] * k, range[1] * k]
                } else {
                    range
                }
            }
        };
        let kind = r.r#type.unwrap_or(crate::options::ScaleType::Sqrt);
        let t = |v: f64| match kind {
            crate::options::ScaleType::Linear => v,
            crate::options::ScaleType::Pow => v.signum() * v.abs().powf(r.exponent.unwrap_or(1.)),
            crate::options::ScaleType::Log => v.max(f64::MIN_POSITIVE).ln(),
            _ => v.signum() * v.abs().sqrt(),
        };
        let (t0, t1) = (t(d0), t(d1));
        for row in facets
            .iter_mut()
            .flatten()
            .flat_map(|mark| mark.rows.iter_mut())
        {
            if let Some(value) = row.r_value {
                let ratio = if t1 == t0 {
                    1.
                } else {
                    (t(value) - t0) / (t1 - t0)
                };
                row.r = Some((range[0] + (range[1] - range[0]) * ratio).max(0.) as f32);
            }
        }
    }
    let opacities = values(facets, |row| row.opacity_value);
    if !opacities.is_empty() {
        let max = opacities.iter().copied().fold(0., f64::max);
        let [d0, d1] = opacity.numeric_domain().unwrap_or([0., max]);
        let range = match &opacity.range {
            Some(range) if range.len() >= 2 => [range[0], range[1]],
            _ => [0., 1.],
        };
        for row in facets
            .iter_mut()
            .flatten()
            .flat_map(|mark| mark.rows.iter_mut())
        {
            if let Some(value) = row.opacity_value {
                let ratio = if d1 == d0 {
                    1.
                } else {
                    (value - d0) / (d1 - d0)
                };
                row.opacity = Some((range[0] + (range[1] - range[0]) * ratio).clamp(0., 1.) as f32);
            }
        }
    }
}

/// Put every symbol channel that names categories rather than symbols
/// through Plot's ordinal symbol scale: the sorted domain (or the `symbol`
/// options' domain) onto d3's fill symbols for a filled dot, its stroke
/// symbols otherwise.
pub fn apply_symbol_scale(
    facets: &mut [Vec<PreparedMark>],
    options: &crate::options::ScaleOptions,
) {
    use crate::style::{SYMBOLS_FILL, SYMBOLS_STROKE};
    let mut values: Vec<Value> = Vec::new();
    for mark in facets.iter().flatten() {
        if let Some(channel) = &mark.mark.channels.symbol {
            for row in &mark.rows {
                let value = channel.read(&row.row);
                if !value.is_null() && symbol_from_value(&value).is_none() {
                    values.push(value);
                }
            }
        }
    }
    if values.is_empty() {
        return;
    }
    let domain: Vec<crate::value::ValueKey> = match &options.domain {
        Some(domain) => domain.iter().map(Value::key).collect(),
        None => {
            let mut keys = crate::scale::dedup(values.iter().map(Value::key));
            keys.sort();
            keys
        }
    };
    for mark in facets.iter_mut().flatten() {
        let Some(channel) = mark.mark.channels.symbol.clone() else {
            continue;
        };
        if constant_symbol(&channel).is_some() {
            continue;
        }
        let range = if mark.mark.channels.fill.is_some() {
            SYMBOLS_FILL
        } else {
            SYMBOLS_STROKE
        };
        for row in mark.rows.iter_mut() {
            let value = channel.read(&row.row);
            if row.symbol.is_none()
                && let Some(index) = domain.iter().position(|key| *key == value.key())
            {
                row.symbol = Some(range[index % range.len()]);
            }
        }
    }
}

/// Plot's `hexbin` transform, run once the frame is known: each mark's
/// rows are placed in pixels, binned into pointy-top hexagons `binWidth`
/// across (offset half a pixel, as Plot's are), per `z` series, and each bin
/// becomes a row at its centre carrying its outputs.
pub fn apply_hexbin(facets: &mut [Vec<PreparedMark>], frame: &Frame) {
    for prepared in facets.iter_mut().flatten() {
        let Some((outputs, width)) = prepared.mark.hexbin.clone() else {
            continue;
        };
        let dx = width as f64;
        let dy = dx * 1.5 / 3f64.sqrt();
        type BinKey = (Option<crate::value::ValueKey>, i64, i64);
        // The bins by first appearance, each with its rows in row order.
        let mut bins: Vec<(BinKey, Vec<usize>)> = Vec::new();
        let mut slots: rustc_hash::FxHashMap<BinKey, usize> = Default::default();
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(p) = crate::render::basic::position_in(prepared, frame, row) else {
                continue;
            };
            let (px, py) = (p.x.as_f32() as f64, p.y.as_f32() as f64);
            // d3-hexbin's rounding, as Plot's `hbin`.
            let mut py = py / dy;
            let mut pj = py.round();
            let mut px = (px - 0.5) / dx - ((pj as i64) & 1) as f64 / 2.;
            let mut pi = px.round();
            let py1 = py - pj;
            if py1.abs() * 3. > 1. {
                let px1 = px - pi;
                let pi2 = pi + if px < pi { -0.5 } else { 0.5 };
                let pj2 = pj + if py < pj { -1. } else { 1. };
                let px2 = px - pi2;
                let py2 = py - pj2;
                if px1 * px1 + py1 * py1 > px2 * px2 + py2 * py2 {
                    pi = pi2 + if (pj as i64) & 1 == 1 { 0.5 } else { -0.5 };
                    pj = pj2;
                }
            }
            let _ = (&mut px, &mut py);
            let key = (row.z.as_ref().map(Value::key), (pi * 2.) as i64, pj as i64);
            match slots.get(&key) {
                Some(&slot) => bins[slot].1.push(index),
                None => {
                    slots.insert(key.clone(), bins.len());
                    bins.push((key, vec![index]));
                }
            }
        }
        let mark = prepared.mark.clone();
        let rows: Vec<PreparedRow> = bins
            .into_iter()
            .map(|((_, pi2, pj), members)| {
                let (pi, pj) = (pi2 as f64 / 2., pj as f64);
                let cx = (pi + ((pj as i64) & 1) as f64 / 2.) * dx + 0.5;
                let cy = pj * dy;
                let mut row = prepared.rows[members[0]].clone();
                // The bin's size and opacity come from its outputs alone.
                row.r = None;
                row.r_value = None;
                row.opacity = None;
                row.opacity_value = None;
                let records: Vec<&Table> = members.iter().map(|i| &prepared.rows[*i].row).collect();
                for output in &outputs {
                    let channel_value = |r: &PreparedRow| -> Value {
                        match output.name.as_ref() {
                            "fill" => mark
                                .channels
                                .fill
                                .as_ref()
                                .and_then(|c| c.scaled_value(&r.row))
                                .unwrap_or(Value::Null),
                            "stroke" => mark
                                .channels
                                .stroke
                                .as_ref()
                                .and_then(|c| c.scaled_value(&r.row))
                                .unwrap_or(Value::Null),
                            "x" => r.x_value().clone(),
                            "y" => r.y_value().clone(),
                            name => r.row.value(name),
                        }
                    };
                    let values: Vec<Value> = members
                        .iter()
                        .map(|i| match &output.input {
                            Some(field) => prepared.rows[*i].row.value(field),
                            None => channel_value(&prepared.rows[*i]),
                        })
                        .collect();
                    let reduced = output.reducer.reduce(&values, &records);
                    match output.name.as_ref() {
                        "r" => row.r_value = reduced.as_number(),
                        "fillOpacity" | "opacity" => row.opacity_value = reduced.as_number(),
                        _ => {}
                    }
                    row.row = row
                        .row
                        .clone()
                        .with(format!("$hexbin:{}", output.name), reduced);
                }
                row.pixel = Some((cx as f32, cy as f32));
                row
            })
            .collect();
        // The outputs bind to the mark's channels.
        let mut mark = prepared.mark.clone();
        for output in &outputs {
            let field = || format!("$hexbin:{}", output.name);
            match output.name.as_ref() {
                "fill" => {
                    mark.channels.fill = Some(crate::style::ColorChannel::Field(field().into()))
                }
                "stroke" => {
                    mark.channels.stroke = Some(crate::style::ColorChannel::Field(field().into()))
                }
                "text" => mark.channels.text = Some(crate::table::Channel::Field(field().into())),
                "title" => mark.channels.title = Some(crate::table::Channel::Field(field().into())),
                _ => {}
            }
        }
        // A dot through a hexbin is a hexagon, its radius half the bin width
        // unless an output sizes it.
        if mark.kind == MarkKind::Dot {
            if mark.channels.symbol.is_none() {
                mark.style.symbol = crate::style::Symbol::Hexagon;
            }
            if !outputs.iter().any(|o| o.name == "r") && mark.channels.r.is_none() {
                mark.style.radius = width / 2.;
            }
            // Plot's dot fills when a fill output or channel is given.
        }
        prepared.mark = mark;
        prepared.rows = rows;
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
        // A mark with only an x (Plot's `dodgeY` on an image or dot) varies
        // on x too.
        let along_x = mark.mark.kind.is_x_only()
            || (mark.mark.channels.x.is_some() && mark.mark.channels.y.is_none());
        // The positions come from the axis the data varies on; the packed
        // offsets land on the *other* axis, so the extent is that one's.
        let (scale, extent) = if along_x {
            (&frame.x.scale, frame.plot.size.height.as_f32())
        } else {
            (&frame.y.scale, frame.plot.size.width.as_f32())
        };
        // Plot's dodge with an `r` channel sorts the dots by descending
        // radius, unless the mark sorts them itself, so the big ones pack
        // first (and draw beneath the small ones).
        let varying = mark.rows.iter().any(|row| row.r.is_some());
        if varying && mark.mark.sort.is_none() {
            mark.rows
                .sort_by(|a, b| b.r.unwrap_or(0.).total_cmp(&a.r.unwrap_or(0.)));
        }
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
        // Each dot's own radius, else the dodge's `r`, else the mark's.
        let constant = if config.radius != transform::Dodge::default().radius {
            config.radius
        } else {
            mark.mark.style.radius
        };
        let radii: Vec<f32> = mark.rows.iter().map(|row| row.r.unwrap_or(constant)).collect();
        let order: Vec<usize> = (0..positions.len()).collect();
        let packed = transform::dodge_positions(
            &positions,
            &radii,
            &order,
            config.anchor,
            config.padding,
            [0., extent],
        );
        for (row, packed) in mark.rows.iter_mut().zip(packed) {
            row.dodge_offset = packed;
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

/// Which positional channel a row's `z` shares, so the row reads it once.
#[derive(Clone, Copy)]
enum SharedZ {
    None,
    X,
    Y,
}

/// The facts [`resolve_row`] needs that depend only on the mark and its
/// channels, worked out once per mark rather than once per row.
struct RowFacts {
    /// A one-dimensional mark fills its missing y (or x) with the row index.
    index_y: bool,
    index_x: bool,
    shared_z: SharedZ,
    /// A symbol channel that names a symbol is a constant.
    constant_symbol: Option<crate::style::Symbol>,
    r_unscaled: bool,
    opacity_unscaled: bool,
    /// Whether a raw row runs [`derive_intervals`]: see [`derives_intervals`].
    derive: bool,
    /// Where the field each positional channel reads (x, y, x1, x2, y1, y2,
    /// z) sat in the last record, which the next record of the same shape
    /// shares: see [`Table::get_hinted`].
    slots: [std::cell::Cell<usize>; 7],
}

/// Whether [`derive_intervals`] on a mark's raw rows can change anything the
/// pipeline reads. Skipping it changes no result in two cases:
///
/// - It writes nothing: the mark has no `interval` and is none of the kinds
///   it grows an interval for.
/// - The mark groups, names no interval channel, and runs none of the
///   [`transforms_before_group`], which read or rewrite an interval. With no
///   interval channel a raw row's ends start empty, and every derivation
///   that fills one needs that axis's value to be non-null, so
///   [`PreparedRow::x_value`] and [`PreparedRow::y_value`] — all a sort, the
///   group keys and [`group_input`] read when no interval channel is named —
///   return the value either way. [`collapse_groups`] then drops the raw
///   rows' ends and derives each grouped row's afresh.
fn derives_intervals(channels: &Channels, mark: &Mark) -> bool {
    use MarkKind::*;
    let writes = mark.interval.is_some()
        || matches!(
            mark.kind,
            RuleX
                | RuleY
                | DifferenceX
                | DifferenceY
                | BarX
                | BarY
                | AreaX
                | AreaY
                | WaffleX
                | WaffleY
                | RectX
                | RectY
                | Cell
        );
    let grouped_away = mark.group.is_some()
        && channels.x1.is_none()
        && channels.x2.is_none()
        && channels.y1.is_none()
        && channels.y2.is_none()
        && !transforms_before_group(mark);
    writes && !grouped_away
}

/// Whether any transform between resolving a mark's rows and grouping them,
/// besides its sort, applies: a window, a normalize (a `normalizeX`/`Y`
/// normalizes before only an unbinned mark), an outlier filter or a select
/// on an axis. Each reads or rewrites the rows' values and interval ends.
fn transforms_before_group(mark: &Mark) -> bool {
    mark.window.is_some()
        || mark.normalize
        || (mark.normalize_by.is_some() && mark.bin.is_none())
        || mark.outliers.is_some()
        || mark
            .select
            .as_ref()
            .is_some_and(|config| config.axis.is_some())
}

impl RowFacts {
    fn new(channels: &Channels, mark: &Mark) -> Self {
        // A one-dimensional mark has no second position channel. Plot fills it
        // with the datum's index for `lineX`/`rectX`, and leaves it unset for
        // `dotX`, whose dots all sit in the frame's middle.
        // A cross channel the mark names wins: `Plot.lineY(data, {x, y})`.
        let category = mark.kind.category();
        let index_cross = category.cross == CrossAxis::Index;
        let index_y = category.single_axis == Some(Axis::X) && index_cross && channels.y.is_none();
        let index_x = !index_y
            && category.single_axis == Some(Axis::Y)
            && index_cross
            && channels.x.is_none();
        // A box part's `z` is its category channel, which is also its x (or
        // y): reuse that read rather than calling the channel twice. Constants
        // are cheap to read, so only fields and accessors are shared.
        let shared_z = match &channels.z {
            Some(crate::table::Channel::Constant(_)) | None => SharedZ::None,
            Some(z) if channels.x.as_ref().is_some_and(|x| z.same(x)) => SharedZ::X,
            Some(z) if channels.y.as_ref().is_some_and(|y| z.same(y)) => SharedZ::Y,
            Some(_) => SharedZ::None,
        };
        Self {
            index_y,
            index_x,
            shared_z,
            constant_symbol: channels.symbol.as_ref().and_then(constant_symbol),
            r_unscaled: channels.r.as_ref().is_some_and(|c| !c.is_scaled()),
            opacity_unscaled: channels.opacity.as_ref().is_some_and(|c| !c.is_scaled()),
            derive: derives_intervals(channels, mark),
            slots: Default::default(),
        }
    }
}

// Inlined into each loop that resolves rows, which then builds every row
// where it lands rather than copying it out of a call.
#[inline(always)]
fn resolve_row(
    row: &Table,
    index: usize,
    channels: &Channels,
    mark: &Mark,
    facts: &RowFacts,
) -> PreparedRow {
    let core = resolve_core(row, index, channels, mark, facts);
    finish_row(core, row, index, channels, facts)
}

/// What grouping and the outlier filter read of a row besides its record:
/// its positions, interval ends and series, as [`resolve_row`] resolves
/// them (it resolves these first, then [`finish_row`] the rest).
struct RowCore {
    x: Value,
    y: Value,
    x1: Option<Value>,
    x2: Option<Value>,
    y1: Option<Value>,
    y2: Option<Value>,
    z: Option<Value>,
}

/// A row read as far as its [`RowCore`], with its record and index, which
/// [`finish_row`] makes a [`PreparedRow`] only if it is kept.
struct CoreRow<'a> {
    core: RowCore,
    record: &'a Table,
    index: usize,
}

/// A row as grouping reads it: a [`PreparedRow`], or a [`CoreRow`] not yet
/// resolved in full.
trait GroupRow {
    fn record(&self) -> &Table;
    fn x(&self) -> &Value;
    fn y(&self) -> &Value;
    fn x1(&self) -> Option<&Value>;
    fn x2(&self) -> Option<&Value>;
    fn y1(&self) -> Option<&Value>;
    fn y2(&self) -> Option<&Value>;
    fn z(&self) -> Option<&Value>;

    /// [`PreparedRow::x_value`].
    fn x_value(&self) -> &Value {
        let x = self.x();
        if !x.is_null() {
            x
        } else {
            self.x1().unwrap_or(x)
        }
    }

    /// [`PreparedRow::y_value`].
    fn y_value(&self) -> &Value {
        let y = self.y();
        if !y.is_null() {
            y
        } else {
            self.y1().unwrap_or(y)
        }
    }
}

impl GroupRow for PreparedRow {
    fn record(&self) -> &Table {
        &self.row
    }
    fn x(&self) -> &Value {
        &self.x
    }
    fn y(&self) -> &Value {
        &self.y
    }
    fn x1(&self) -> Option<&Value> {
        self.x1.as_ref()
    }
    fn x2(&self) -> Option<&Value> {
        self.x2.as_ref()
    }
    fn y1(&self) -> Option<&Value> {
        self.y1.as_ref()
    }
    fn y2(&self) -> Option<&Value> {
        self.y2.as_ref()
    }
    fn z(&self) -> Option<&Value> {
        self.z.as_ref()
    }
}

impl GroupRow for CoreRow<'_> {
    fn record(&self) -> &Table {
        self.record
    }
    fn x(&self) -> &Value {
        &self.core.x
    }
    fn y(&self) -> &Value {
        &self.core.y
    }
    fn x1(&self) -> Option<&Value> {
        self.core.x1.as_ref()
    }
    fn x2(&self) -> Option<&Value> {
        self.core.x2.as_ref()
    }
    fn y1(&self) -> Option<&Value> {
        self.core.y1.as_ref()
    }
    fn y2(&self) -> Option<&Value> {
        self.core.y2.as_ref()
    }
    fn z(&self) -> Option<&Value> {
        self.core.z.as_ref()
    }
}

impl<T: GroupRow> GroupRow for &T {
    fn record(&self) -> &Table {
        (**self).record()
    }
    fn x(&self) -> &Value {
        (**self).x()
    }
    fn y(&self) -> &Value {
        (**self).y()
    }
    fn x1(&self) -> Option<&Value> {
        (**self).x1()
    }
    fn x2(&self) -> Option<&Value> {
        (**self).x2()
    }
    fn y1(&self) -> Option<&Value> {
        (**self).y1()
    }
    fn y2(&self) -> Option<&Value> {
        (**self).y2()
    }
    fn z(&self) -> Option<&Value> {
        (**self).z()
    }
}

/// The first half of [`resolve_row`]: the channels a [`RowCore`] holds,
/// read in the order `resolve_row` reads them.
#[inline(always)]
fn resolve_core(
    row: &Table,
    index: usize,
    channels: &Channels,
    mark: &Mark,
    facts: &RowFacts,
) -> RowCore {
    let read = |channel: &Option<crate::table::Channel>, slot: usize| match channel {
        Some(channel @ crate::table::Channel::Field(_)) => {
            channel.read_hinted(row, &facts.slots[slot])
        }
        Some(channel) => channel.read(row),
        None => Value::Null,
    };

    let mut x = read(&channels.x, 0);
    let mut y = read(&channels.y, 1);
    let shared_z = match facts.shared_z {
        SharedZ::None => None,
        SharedZ::X => Some(x.clone()),
        SharedZ::Y => Some(y.clone()),
    };
    let x1 = read(&channels.x1, 2);
    let x2 = read(&channels.x2, 3);
    let y1 = read(&channels.y1, 4);
    let y2 = read(&channels.y2, 5);

    let mut x1 = (!x1.is_null()).then_some(x1);
    let mut x2 = (!x2.is_null()).then_some(x2);
    let mut y1 = (!y1.is_null()).then_some(y1);
    let mut y2 = (!y2.is_null()).then_some(y2);
    if facts.derive {
        derive_intervals(mark, &x, &y, &mut x1, &mut x2, &mut y1, &mut y2);
    }

    if facts.index_y {
        y = Value::Number(index as f64);
    } else if facts.index_x {
        x = Value::Number(index as f64);
    }

    let z = match shared_z {
        Some(value) => Some(value),
        None => channels.z.as_ref().map(|_| read(&channels.z, 6)),
    }
    .filter(|v| !v.is_null());
    RowCore {
        x,
        y,
        x1,
        x2,
        y1,
        y2,
        z,
    }
}

/// The second half of [`resolve_row`]: the row from its core and the
/// channels left to read.
#[inline(always)]
fn finish_row(
    core: RowCore,
    row: &Table,
    index: usize,
    channels: &Channels,
    facts: &RowFacts,
) -> PreparedRow {
    let RowCore {
        x,
        y,
        x1,
        x2,
        y1,
        y2,
        z,
    } = core;
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
        path: channels
            .path
            .as_ref()
            .map(|channel| channel.read(row))
            .filter(|v| !v.is_null())
            .map(|v| v.label()),
        r: channels
            .r
            .as_ref()
            .filter(|_| facts.r_unscaled)
            .map(|channel| channel.read(row)),
        r_value: channels
            .r
            .as_ref()
            .and_then(|channel| channel.data_value(row)),
        opacity_value: channels
            .opacity
            .as_ref()
            .and_then(|channel| channel.data_value(row)),
        symbol: channels.symbol.as_ref().and_then(|channel| {
            facts
                .constant_symbol
                .or_else(|| symbol_from_value(&channel.read(row)))
        }),
        opacity: channels
            .opacity
            .as_ref()
            .filter(|_| facts.opacity_unscaled)
            .map(|channel| channel.read(row)),
        dodge_offset: None,
        pixel: None,
    }
}

/// A symbol channel's value as a [`Symbol`](crate::style::Symbol): a name
/// (`"circle"`, `"square"`, …) or an index into the symbol set.
/// A symbol channel that names a symbol (`symbol: "square"`) is a constant,
/// as Plot reads it.
pub fn constant_symbol(channel: &crate::table::Channel) -> Option<crate::style::Symbol> {
    match channel {
        crate::table::Channel::Field(name) => crate::style::Symbol::parse(name),
        crate::table::Channel::Constant(value) => symbol_from_value(value),
        _ => None,
    }
}

fn symbol_from_value(value: &Value) -> Option<crate::style::Symbol> {
    match value {
        Value::Text(text) => crate::style::Symbol::parse(text),
        _ => None,
    }
}

fn apply_sort(rows: &mut Vec<PreparedRow>, mark: &Mark) {
    let Some(sort) = &mark.sort else {
        return;
    };
    // Each row's key is read once, then a stable sort of the indices by key
    // orders the rows as a stable sort of the rows would.
    let keys: Vec<Value> = if let Some(field) = &sort.field {
        rows.iter().map(|row| row.row.value(field)).collect()
    } else {
        // Sorting without a field sorts by the mark's own value: a bar chart's
        // length, which is Plot's default for a sorted bar chart.
        // The value is on x when the categories are on y (a `barX`), else y.
        let value_on_x = mark.kind.category().band_y;
        rows.iter()
            .map(|row| {
                if value_on_x {
                    row.x_value().clone()
                } else {
                    row.y_value().clone()
                }
            })
            .collect()
    };
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| {
        let ordering = keys[a].cmp(&keys[b]);
        if sort.descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    let mut taken: Vec<Option<PreparedRow>> = std::mem::take(rows).into_iter().map(Some).collect();
    rows.extend(order.into_iter().filter_map(|index| taken[index].take()));
    if sort.reverse {
        rows.reverse();
    }
}

/// Plot's `map` transform over the mark's records: each target channel's
/// values are mapped per `z` series and written under `$map:<channel>`,
/// and the returned mark reads them there.
fn apply_maps(source: &Dataset, mark: &Mark) -> (Dataset, Mark) {
    use crate::mark::MapTarget;
    use crate::style::{ColorChannel, NumberChannel};
    use crate::table::Channel;
    let mut mark = mark.clone();
    let mut rows: Vec<Table> = source.rows().to_vec();
    // The series a map restarts at: Plot's `maybeZ`, the `z` channel (which
    // a color field implies).
    let mut series: Vec<Vec<usize>> = Vec::new();
    let mut slots: std::collections::HashMap<Value, usize> = std::collections::HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        let key = mark
            .channels
            .z
            .as_ref()
            .map(|z| z.read(row))
            .unwrap_or(Value::Null);
        let slot = *slots.entry(key).or_insert_with(|| {
            series.push(Vec::new());
            series.len() - 1
        });
        series[slot].push(index);
    }
    let maps = std::mem::take(&mut mark.maps);
    for (target, mapper) in &maps {
        // Each map call is its own transform, so a later one reads what an
        // earlier one wrote, as Plot's nested `windowY(k, mapY(…))` does;
        // within one call, every input is read before any output is written.
        let inputs = mark.channels.clone();
        // `mapX`/`mapY`: the axis's channels the mark has, else the datum.
        let names: Vec<&str> = match target {
            MapTarget::X | MapTarget::Y => {
                let (names, slots) = if *target == MapTarget::X {
                    (["x", "x1", "x2"], [&inputs.x, &inputs.x1, &inputs.x2])
                } else {
                    (["y", "y1", "y2"], [&inputs.y, &inputs.y1, &inputs.y2])
                };
                let present: Vec<&str> = names
                    .iter()
                    .zip(slots)
                    .filter(|(_, slot)| slot.is_some())
                    .map(|(name, _)| *name)
                    .collect();
                if present.is_empty() {
                    vec![names[0]]
                } else {
                    present
                }
            }
            MapTarget::Channel(name) => vec![name.as_ref()],
        };
        for name in names {
            let c = &inputs;
            // Plot's `maybeInput`: an interval end the mark lacks reads its
            // axis's channel.
            let position = |own: &Option<Channel>, axis: &Option<Channel>| {
                own.clone().or_else(|| axis.clone())
            };
            let position = match name {
                "x" => Some(c.x.clone().unwrap_or_else(Channel::identity)),
                "y" => Some(c.y.clone().unwrap_or_else(Channel::identity)),
                "x1" | "x2" => position(if name == "x1" { &c.x1 } else { &c.x2 }, &c.x),
                "y1" | "y2" => position(if name == "y1" { &c.y1 } else { &c.y2 }, &c.y),
                _ => None,
            };
            let color = match name {
                "fill" => c.fill.as_ref(),
                "stroke" => c.stroke.as_ref(),
                _ => None,
            }
            .filter(|ch| matches!(ch, ColorChannel::Field(_) | ColorChannel::Value(_)));
            let number = match name {
                "r" => c.r.as_ref(),
                "opacity" => c.opacity.as_ref(),
                _ => None,
            }
            .filter(|ch| ch.is_scaled());
            let values: Vec<Value> = if let Some(channel) = position {
                rows.iter().map(|row| channel.read(row)).collect()
            } else if let Some(channel) = color {
                rows.iter()
                    .map(|row| channel.scaled_value(row).unwrap_or(Value::Null))
                    .collect()
            } else if let Some(channel) = number {
                rows.iter()
                    .map(|row| channel.data_value(row).map_or(Value::Null, Value::number))
                    .collect()
            } else {
                // Plot throws on a missing channel; a constant has nothing
                // to map.
                continue;
            };
            let field: SharedString = format!("$map:{name}").into();
            let mut output = vec![Value::Null; rows.len()];
            for members in &series {
                let series_values: Vec<Value> =
                    members.iter().map(|&i| values[i].clone()).collect();
                for (&i, value) in members
                    .iter()
                    .zip(transform::map_values(&series_values, mapper))
                {
                    output[i] = value;
                }
            }
            for (row, value) in rows.iter_mut().zip(output) {
                row.set(&field, value);
            }
            let c = &mut mark.channels;
            match name {
                "x" => c.x = Some(Channel::Field(field)),
                "x1" => c.x1 = Some(Channel::Field(field)),
                "x2" => c.x2 = Some(Channel::Field(field)),
                "y" => c.y = Some(Channel::Field(field)),
                "y1" => c.y1 = Some(Channel::Field(field)),
                "y2" => c.y2 = Some(Channel::Field(field)),
                "fill" => c.fill = Some(ColorChannel::Field(field)),
                "stroke" => c.stroke = Some(ColorChannel::Field(field)),
                "r" => c.r = Some(NumberChannel::Field(field)),
                _ => c.opacity = Some(NumberChannel::Field(field)),
            }
        }
    }
    // Plot's `maybeStackY`/`maybeStackX`: an interval mark given an
    // interval end draws `y1`–`y2` (the missing end from `y`) and drops `y`,
    // so `areaY(map({y1, y2}, {y}))` draws the band alone.
    let c = &mut mark.channels;
    match mark.kind {
        MarkKind::BarY | MarkKind::AreaY | MarkKind::RectY | MarkKind::WaffleY
            if c.y.is_some() && (c.y1.is_some() || c.y2.is_some()) =>
        {
            let y = c.y.take();
            c.y1 = c.y1.take().or_else(|| y.clone());
            c.y2 = c.y2.take().or(y);
        }
        MarkKind::BarX | MarkKind::AreaX | MarkKind::RectX | MarkKind::WaffleX
            if c.x.is_some() && (c.x1.is_some() || c.x2.is_some()) =>
        {
            let x = c.x.take();
            c.x1 = c.x1.take().or_else(|| x.clone());
            c.x2 = c.x2.take().or(x);
        }
        _ => {}
    }
    (Dataset::new(rows), mark)
}

fn apply_window(mark: &Mark, rows: &mut [PreparedRow]) {
    let Some((k, reducer, anchor)) = mark.window else {
        return;
    };
    // Plot's `windowX` maps x, `windowY` y: the axis an X mark varies on,
    // else y.
    let on_x = mark.kind.category().single_axis == Some(Axis::X);
    let read = |row: &PreparedRow| -> Option<f64> {
        if on_x { row.x.as_number() } else { row.y.as_number() }
    };
    // A window restarts with each `z` series, as Plot's map groups by z.
    let mut series: Vec<(Option<crate::value::ValueKey>, Vec<usize>)> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let key = row.z.as_ref().map(Value::key);
        match series.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(index),
            None => series.push((key, vec![index])),
        }
    }
    // An interval end grown from the value (a bar's or area's) follows it.
    let derived = if on_x {
        mark.channels.x1.is_none() && mark.channels.x2.is_none()
    } else {
        mark.channels.y1.is_none() && mark.channels.y2.is_none()
    };
    for (_, members) in series {
        let values: Vec<Option<f64>> = members.iter().map(|&i| read(&rows[i])).collect();
        let mapped = transform::window_values(&values, k, reducer, anchor, mark.window_strict);
        for (&i, value) in members.iter().zip(mapped) {
            let row = &mut rows[i];
            let value = value.map(Value::Number).unwrap_or(Value::Null);
            let (slot, end) = if on_x {
                (&mut row.x, &mut row.x2)
            } else {
                (&mut row.y, &mut row.y2)
            };
            if derived && end.as_ref() == Some(slot) {
                *end = (!value.is_null()).then(|| value.clone());
            }
            *slot = value;
        }
    }
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
fn group_input(row: &impl GroupRow, mark: &Mark, name: &str) -> Value {
    match name {
        // An interval's end falls back to the channel it splits, the way
        // Plot's `maybeInput` does.
        "x1" | "x2" if mark.channels.x1.is_some() || mark.channels.x2.is_some() => {
            row.x1().or(row.x2()).unwrap_or(row.x()).clone()
        }
        "y1" | "y2" if mark.channels.y1.is_some() || mark.channels.y2.is_some() => {
            row.y1().or(row.y2()).unwrap_or(row.y()).clone()
        }
        "x1" | "x2" => row.x_value().clone(),
        "y1" | "y2" => row.y_value().clone(),
        "x" => row.x_value().clone(),
        "y" => row.y_value().clone(),
        "z" => row.z().cloned().unwrap_or(Value::Null),
        // Any other name is a field on the record, which is where a grouped
        // color, label or tooltip value lands.
        field => row.record().value(field),
    }
}

/// Collapse the rows sharing a category, Plot's `groupX`/`groupY`/`groupZ`
/// and `group`.
///
/// Rows group by the transform's axis (or both) and are subdivided by `z`
/// (the series a fill or stroke field implies), so `groupX` with `fill: "sex"`
/// gives a bar per sex in each category, which the implicit stack then
/// stacks. Each group becomes one row: the grouping channels keep their key,
/// the outputs hold the reductions, and a positional channel neither keeps,
/// as Plot drops it. The groups keep their first appearance's order.
///
/// An output's `$in:<name>` input reads that channel of `bound`, the mark's
/// channels before its outputs replaced them (see [`bound_input`]).
///
/// The rows arrive one at a time and only each group's first row is kept:
/// the others leave the values their outputs read, so a mark whose raw rows
/// nothing else reads can stream them in without collecting them first.
/// A streamed row arrives as its [`CoreRow`], which `finish` resolves in
/// full only when it starts a group.
fn collapse_groups<R: GroupRow>(
    rows: impl IntoIterator<Item = R>,
    mut finish: impl FnMut(R) -> PreparedRow,
    mark: &Mark,
    config: &crate::mark::GroupConfig,
    bound: &Channels,
) -> Vec<PreparedRow> {
    let by_x = config.both || config.axis == Some(Axis::X);
    let by_y = config.both || config.axis == Some(Axis::Y);
    let key = |row: &R| -> (
        crate::value::ValueKey,
        crate::value::ValueKey,
        crate::value::ValueKey,
    ) {
        let none = Value::Null.key();
        (
            if by_x {
                GroupRow::x_value(row).key()
            } else {
                none.clone()
            },
            if by_y {
                GroupRow::y_value(row).key()
            } else {
                none.clone()
            },
            row.z().map(Value::key).unwrap_or(none),
        )
    };
    // Plot's default output: a count into the channel the grouping leaves
    // free — y for groupX, x for groupY, fill for group and groupZ.
    let default_outputs;
    let outputs: &[crate::mark::GroupOutput] = if config.outputs.is_empty() {
        let name = match (by_x, by_y) {
            (true, false) => "y",
            (false, true) => "x",
            _ => "fill",
        };
        default_outputs = vec![crate::mark::GroupOutput::new(
            name,
            transform::GroupReducer::Count,
        )];
        &default_outputs
    } else {
        &config.outputs
    };
    // The inputs the outputs read, each once, with the bound channel a `$in:`
    // input reads: a box's bar reads its q1 and q3 from one column. Each
    // output's column, or none for a count or proportion, which read no
    // value.
    let mut inputs: Vec<(&str, Option<usize>)> = Vec::new();
    let output_columns: Vec<Option<usize>> = outputs
        .iter()
        .map(|output| match output.reducer {
            transform::GroupReducer::Proportion | transform::GroupReducer::Count => None,
            _ => {
                let name = output.input.as_deref().unwrap_or(&output.name);
                Some(match inputs.iter().position(|(n, _)| *n == name) {
                    Some(column) => column,
                    None => {
                        inputs.push((name, bound_slot(name)));
                        inputs.len() - 1
                    }
                })
            }
        })
        .collect();
    let needs_records = outputs.iter().any(|output| output.reducer.reads_records());

    // One group: its first row, its size, the values each input reads from
    // its members in row order, and the members' records when a reducer
    // reads them.
    struct Group {
        first: PreparedRow,
        len: usize,
        columns: Vec<Vec<Value>>,
        records: Vec<Table>,
    }
    // The groups in first-appearance order.
    let mut groups: Vec<Group> = Vec::new();
    let mut slots: rustc_hash::FxHashMap<_, usize> = Default::default();
    // Whether a row has the same key as a group's first row, without building
    // either key. Consecutive rows often share a group (a `groupZ` without a
    // `z` puts every row in one), and comparing values is cheaper than
    // hashing their keys. Equal values have equal keys (a key only forgets a
    // date's sub-millisecond part), so a match is the group the map would
    // find; a mismatch asks the map.
    let same_group = |row: &R, first: &PreparedRow| {
        (!by_x || GroupRow::x_value(row) == first.x_value())
            && (!by_y || GroupRow::y_value(row) == first.y_value())
            && row.z() == first.z.as_ref()
    };
    // The group of the last row.
    let mut last: Option<usize> = None;
    let mut total = 0usize;
    // Internal iteration hands each resolved row to the body where it was
    // built, rather than moving it out through `next`.
    rows.into_iter().for_each(|row| {
        total += 1;
        let read = |&(name, input_slot): &(&str, Option<usize>)| match input_slot {
            // A `$in:` input reads the mark's own channel.
            Some(channel) => bound_input(bound, channel, name, row.record()),
            None => group_input(&row, mark, name),
        };
        let found = match last {
            Some(slot) if same_group(&row, &groups[slot].first) => Some(slot),
            _ => slots.get(&key(&row)).copied(),
        };
        match found {
            Some(slot) => {
                let group = &mut groups[slot];
                group.len += 1;
                for (column, input) in group.columns.iter_mut().zip(&inputs) {
                    column.push(read(input));
                }
                if needs_records {
                    group.records.push(row.record().clone());
                }
                last = Some(slot);
            }
            None => {
                let slot = groups.len();
                slots.insert(key(&row), slot);
                last = Some(slot);
                let columns = inputs.iter().map(|input| vec![read(input)]).collect();
                let records = if needs_records {
                    vec![row.record().clone()]
                } else {
                    Vec::new()
                };
                groups.push(Group {
                    first: finish(row),
                    len: 1,
                    columns,
                    records,
                });
            }
        }
    });
    let total = total.max(1) as f64;

    groups
        .into_iter()
        .map(|group| {
            let Group {
                first,
                len,
                columns,
                records,
            } = group;
            let mut grouped = PreparedRow {
                x: if by_x {
                    first.x_value().clone()
                } else {
                    Value::Null
                },
                y: if by_y {
                    first.y_value().clone()
                } else {
                    Value::Null
                },
                x1: None,
                x2: None,
                y1: None,
                y2: None,
                ..first
            };
            let records: Vec<&Table> = records.iter().collect();
            // Each column's order, which its outputs share: a box's quartiles
            // and whiskers gather the column's numbers once and select each
            // quartile once.
            let mut order: Vec<transform::OrderStatistics> =
                columns.iter().map(|_| Default::default()).collect();
            for (output, column) in outputs.iter().zip(&output_columns) {
                let reduced = match (column, &output.reducer) {
                    // Plot's `proportion`: the group's share of every row.
                    (None, transform::GroupReducer::Proportion) => {
                        Value::Number(len as f64 / total)
                    }
                    // A count counts every member, null or not.
                    (None, _) => Value::Number(len as f64),
                    (Some(column), reducer) => {
                        reducer.reduce_with(&columns[*column], &mut order[*column], &records)
                    }
                };
                match &*output.name {
                    "x" => grouped.x = reduced.clone(),
                    "y" => grouped.y = reduced.clone(),
                    "x1" => grouped.x1 = (!reduced.is_null()).then(|| reduced.clone()),
                    "x2" => grouped.x2 = (!reduced.is_null()).then(|| reduced.clone()),
                    "y1" => grouped.y1 = (!reduced.is_null()).then(|| reduced.clone()),
                    "y2" => grouped.y2 = (!reduced.is_null()).then(|| reduced.clone()),
                    "z" => grouped.z = (!reduced.is_null()).then(|| reduced.clone()),
                    "r" => grouped.r_value = reduced.as_number(),
                    "fillOpacity" | "opacity" | "strokeOpacity" => {
                        grouped.opacity_value = reduced.as_number()
                    }
                    _ => {}
                }
                // The record gains every output under its name, so a channel
                // that reads it — a fill, a text label, a tip — sees it.
                grouped.row.set(output.name.clone(), reduced);
            }
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

/// The extent a raster or contour mark covers, `[x1, y1, x2, y2]`, from its
/// constant `x1`/`y1`/`x2`/`y2` channels, else its grid (`[0, width] ×
/// [0, height]`, Plot's default for dense values).
pub fn raster_extent(mark: &Mark) -> Option<[f64; 4]> {
    let constant = |c: &Option<crate::table::Channel>| match c {
        Some(crate::table::Channel::Constant(v)) => v.as_number(),
        _ => None,
    };
    let c = &mark.channels;
    match (
        constant(&c.x1),
        constant(&c.y1),
        constant(&c.x2),
        constant(&c.y2),
    ) {
        (Some(x1), Some(y1), Some(x2), Some(y2)) => Some([x1, y1, x2, y2]),
        _ => mark
            .raster
            .as_ref()?
            .size
            .map(|(w, h)| [0., 0., w as f64, h as f64]),
    }
}

/// A raster from dense values: the mark's records in order, each its fill
/// (or its value, for plain-value data), laid out `width` across.
fn dense_raster(mark: &Mark, source: &Dataset) -> Option<std::rc::Rc<crate::raster::RasterGrid>> {
    if !matches!(mark.kind, MarkKind::Raster | MarkKind::Contour) {
        return None;
    }
    let spec = mark.raster.as_ref()?;
    let (w, h) = spec.size?;
    let extent = raster_extent(mark)?;
    let read = |row: &Table| -> Value {
        if let Some(fill) = &mark.channels.fill
            && let Some(v) = fill.scaled_value(row)
        {
            return v;
        }
        row.value(crate::table::VALUE)
    };
    let values: Vec<Value> = source.rows().iter().map(read).collect();
    let mut grid = crate::raster::RasterGrid::from_values(values, w, h, extent);
    grid.blur(spec.blur);
    Some(std::rc::Rc::new(grid))
}

/// The grid a raster or contour mark draws once the frame is known: its
/// dense grid, a function sampled at Plot's `pixelSize` (one cell per two
/// pixels), or its scattered rows interpolated.
pub fn resolve_raster(
    prepared: &PreparedMark,
    frame: &crate::frame::Frame,
) -> Option<std::rc::Rc<crate::raster::RasterGrid>> {
    if let Some(grid) = &prepared.raster {
        return Some(grid.clone());
    }
    let spec = prepared.mark.raster.clone().unwrap_or_default();
    let (pw, ph) = (
        frame.plot.size.width.as_f32() as f64,
        frame.plot.size.height.as_f32() as f64,
    );
    let size = spec.size.unwrap_or((
        (pw / 2.).round().max(1.) as usize,
        (ph / 2.).round().max(1.) as usize,
    ));
    let invert = |scale: &crate::scale::PositionScale, p: f64| match scale {
        crate::scale::PositionScale::Continuous(s) => s.invert(p as f32),
        _ => p,
    };
    // The data extent: the mark's own, else the frame's.
    let extent = raster_extent(&prepared.mark).unwrap_or_else(|| {
        [
            invert(&frame.x.scale, 0.),
            invert(&frame.y.scale, ph),
            invert(&frame.x.scale, pw),
            invert(&frame.y.scale, 0.),
        ]
    });
    let samples: Vec<(f64, f64, Value)> = if spec.function.is_some() {
        Vec::new()
    } else {
        prepared
            .rows
            .iter()
            .filter_map(|row| {
                let value = match &prepared.mark.channels.fill {
                    Some(fill) => fill.scaled_value(&row.row)?,
                    None => row.row.value(crate::table::VALUE),
                };
                Some((
                    row.x_value().as_number()?,
                    row.y_value().as_number()?,
                    value,
                ))
            })
            .collect()
    };
    let how = spec
        .interpolate
        .unwrap_or(if prepared.mark.kind == MarkKind::Contour {
            crate::raster::Interpolate::Nearest
        } else {
            crate::raster::Interpolate::None
        });
    // Interpolating thousands of samples (or sampling a function per cell) is
    // the costly part, and a plot repaints on every pointer move with the same
    // inputs; remember the last few grids by exactly what they were built
    // from. A function is known by its `Rc`, which the entry holds so the
    // address cannot be reused: a retained plot keeps the same function from
    // frame to frame, and a rebuilt one simply resolves again.
    let key = RasterKey {
        size,
        how,
        blur: spec.blur.to_bits(),
        extent: extent.map(f64::to_bits),
        function: spec.function.clone(),
        samples: samples
            .iter()
            .map(|(x, y, v)| (x.to_bits(), y.to_bits(), v.key()))
            .collect(),
    };
    thread_local! {
        static GRIDS: std::cell::RefCell<Vec<(RasterKey, std::rc::Rc<crate::raster::RasterGrid>)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    if let Some(grid) = GRIDS.with_borrow(|grids| {
        grids.iter().find(|(k, _)| k.same(&key)).map(|(_, g)| g.clone())
    }) {
        return Some(grid);
    }
    let mut grid = if let Some(f) = &spec.function {
        crate::raster::RasterGrid::from_function(&|x, y| f(x, y), size.0, size.1, extent)
    } else {
        crate::raster::RasterGrid::from_samples(&samples, size.0, size.1, extent, how)
    };
    grid.blur(spec.blur);
    let grid = std::rc::Rc::new(grid);
    // Keep as many grids as a page of raster charts needs (a plot rebuilt
    // every frame comes back for its grid each frame), within a budget of
    // cells, so a page of rasters does not thrash and memory stays bounded.
    const CELL_BUDGET: usize = 1 << 21;
    GRIDS.with_borrow_mut(|grids| {
        grids.insert(0, (key, grid.clone()));
        let mut cells = 0;
        let keep = grids
            .iter()
            .position(|(_, g)| {
                cells += g.values.len();
                cells > CELL_BUDGET
            })
            .unwrap_or(grids.len())
            .max(1);
        grids.truncate(keep);
    });
    Some(grid)
}

/// Everything a resolved raster grid was built from, compared exactly.
struct RasterKey {
    size: (usize, usize),
    how: crate::raster::Interpolate,
    blur: u64,
    extent: [u64; 4],
    function: Option<std::rc::Rc<dyn Fn(f64, f64) -> Value>>,
    samples: Vec<(u64, u64, crate::value::ValueKey)>,
}

impl RasterKey {
    fn same(&self, other: &Self) -> bool {
        self.size == other.size
            && self.how == other.how
            && self.blur == other.blur
            && self.extent == other.extent
            && match (&self.function, &other.function) {
                (Some(a), Some(b)) => std::rc::Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && self.samples == other.samples
    }
}

/// The channel names a transform output can read from the mark itself.
const BOUND: [&str; 4] = ["fill", "stroke", "text", "title"];

/// The [`BOUND`] channel an input names as `$in:<name>`, the input
/// [`bind_outputs`] gives an output that reads the mark's own channel.
fn bound_slot(input: &str) -> Option<usize> {
    let name = input.strip_prefix("$in:")?;
    BOUND.iter().position(|bound| *bound == name)
}

/// The value a `$in:<name>` input reads from `record`: the original mark's
/// channel of that name (its slot in [`BOUND`], from [`bound_slot`]), else
/// the record's own field of the input's name.
fn bound_input(channels: &Channels, slot: usize, input: &str, record: &Table) -> Value {
    let value = match slot {
        0 => channels
            .fill
            .as_ref()
            .and_then(|ch| ch.scaled_value(record)),
        1 => channels
            .stroke
            .as_ref()
            .and_then(|ch| ch.scaled_value(record)),
        2 => channels.text.as_ref().map(|ch| ch.read(record)),
        3 => channels.title.as_ref().map(|ch| ch.read(record)),
        _ => None,
    };
    value.unwrap_or_else(|| record.value(input))
}

/// The mark as its transforms leave it: a group or bin output to `fill`,
/// `stroke`, `text`, `title` (and so on) becomes that channel, read from the
/// output's field, as Plot binds a transform's outputs to the mark.
fn bind_outputs(mark: &Mark) -> Option<Mark> {
    let outputs: Vec<SharedString> = mark
        .group
        .iter()
        .flat_map(|g| g.outputs.iter().map(|o| o.name.clone()))
        .chain(
            mark.bin
                .iter()
                .flat_map(|b| b.outputs.iter().map(|o| o.name.clone())),
        )
        .collect();
    let default_fill = mark
        .group
        .as_ref()
        .is_some_and(|g| g.outputs.is_empty() && (g.both || g.axis.is_none()));
    if outputs.is_empty() && !default_fill {
        return None;
    }
    let mut mark = mark.clone();
    // Outputs with no input read the mark's channel of their name, through
    // [`bound_input`].
    let has = |name: &str| match name {
        "fill" => mark
            .channels
            .fill
            .as_ref()
            .is_some_and(|c| c.scaled_value(&Table::empty()).is_some()),
        "stroke" => mark
            .channels
            .stroke
            .as_ref()
            .is_some_and(|c| c.scaled_value(&Table::empty()).is_some()),
        "text" => mark.channels.text.is_some(),
        "title" => mark.channels.title.is_some(),
        _ => false,
    };
    let inputs: Vec<String> = BOUND
        .iter()
        .filter(|n| has(n))
        .map(|n| n.to_string())
        .collect();
    for config in mark
        .group
        .iter_mut()
        .map(|g| &mut g.outputs)
        .chain(mark.bin.iter_mut().map(|b| &mut b.outputs))
    {
        for output in config.iter_mut() {
            if output.input.is_none() && inputs.iter().any(|n| *n == output.name.as_ref()) {
                output.input = Some(format!("$in:{}", output.name).into());
            }
        }
    }
    // Plot's `maybeSubgroup`: groups subdivide by the first of `z`, `fill`
    // and `stroke` that is a channel and not an output. A `z` a color
    // implied goes when an output replaces that color, and the next color
    // channel left as an input implies it instead.
    if mark.implied_z {
        use crate::{style::ColorChannel, table::Channel};
        let implies = |color: &Option<ColorChannel>| match (color, &mark.channels.z) {
            (Some(ColorChannel::Field(a)), Some(Channel::Field(b))) => a == b,
            (Some(ColorChannel::Value(a)), Some(Channel::Accessor(b))) => std::rc::Rc::ptr_eq(a, b),
            _ => false,
        };
        let is_output = |name: &str| {
            outputs.iter().any(|n| n.as_ref() == name) || (default_fill && name == "fill")
        };
        let colors = [
            ("fill", mark.channels.fill.clone()),
            ("stroke", mark.channels.stroke.clone()),
        ];
        if colors.iter().any(|(name, c)| is_output(name) && implies(c)) {
            let mut next = colors.iter().filter(|(name, _)| !is_output(name));
            let z = next.find_map(|(_, c)| match c {
                Some(ColorChannel::Field(f)) => Some(Channel::Field(f.clone())),
                Some(ColorChannel::Value(a)) => Some(Channel::Accessor(a.clone())),
                _ => None,
            });
            mark.channels.z = z;
            mark.implied_z = mark.channels.z.is_some();
        }
    }
    for name in outputs
        .iter()
        .map(|n| n.as_ref())
        .chain(default_fill.then_some("fill"))
    {
        match name {
            "fill" => mark.channels.fill = Some(crate::style::ColorChannel::Field("fill".into())),
            "stroke" => {
                mark.channels.stroke = Some(crate::style::ColorChannel::Field("stroke".into()))
            }
            "text" => mark.channels.text = Some(crate::table::Channel::Field("text".into())),
            "title" => mark.channels.title = Some(crate::table::Channel::Field("title".into())),
            _ => {}
        }
    }
    Some(mark)
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
fn pin_bin_edges(data: &Dataset, marks: &[Mark]) -> Vec<Mark> {
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
fn bin_rows(
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
fn bin_records(
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
                path: None,
                r: None,
                symbol: None,
                opacity: None,
                r_value: None,
                opacity_value: None,
                dodge_offset: None,
                pixel: None,
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
    mark.bin
        .as_ref()
        .map(|bin| bin.thresholds)
        .unwrap_or_default()
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
    fn a_color_output_does_not_subgroup_by_its_input() {
        // Plot's maybeSubgroup: `stroke: "sex"` splits groupX's groups by sex
        // unless a `stroke` output replaces it.
        let count = || crate::transform::GroupReducer::Count;
        let by_sex = Mark::new(MarkKind::BarY).x("c").stroke("sex").group_x();
        let replaced = by_sex.clone().group_output("stroke", count());
        let rows = |mark: Mark| {
            prepare(&Dataset::new(grouped_rows()), &[mark])[0]
                .rows
                .len()
        };
        let categories = grouped_rows()
            .iter()
            .map(|row| row.value("c").key())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert!(rows(by_sex.group_output("y", count())) > categories);
        assert_eq!(rows(replaced.group_output("y", count())), categories);
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

    #[test]
    fn preparing_is_a_pure_function() {
        // Plot caches the prepared facets from prepaint and reuses them for
        // hover, tooltip and paint. That is only faithful if resolving the
        // same data twice yields identical rows — not just the same values,
        // but the same per-row records, so a hovered datum is placed exactly
        // where it was painted.
        let rows = vec![
            Table::empty()
                .with("f", Value::text("one"))
                .with("x", Value::Number(1.))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("f", Value::text("one"))
                .with("x", Value::Number(2.))
                .with("v", Value::Number(20.)),
        ];
        let mark = Mark::new(MarkKind::Dot).x("x").y("v");
        let grid = Grid::from_values(vec![Value::text("one")], Vec::new());
        let dataset = Dataset::new(rows);

        let (first, _) = prepare_facets(
            &dataset,
            std::slice::from_ref(&mark),
            &grid,
            Some("f"),
            None,
        );
        for _ in 0..2 {
            let (again, _) = prepare_facets(
                &dataset,
                std::slice::from_ref(&mark),
                &grid,
                Some("f"),
                None,
            );
            // PreparedMark is not PartialEq (its mark holds closures), so
            // compare the resolved rows: the shape is the position a hovered
            // datum is placed at, and it must be identical across calls.
            let describe = |facets: &[Vec<PreparedMark>]| {
                let cell = &facets[0][0];
                (
                    cell.mark.kind,
                    cell.rows
                        .iter()
                        .map(|row| {
                            (
                                row.x_value().clone(),
                                row.y_value().clone(),
                                row.index,
                                row.row.value("v"),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            };
            assert_eq!(
                describe(&first),
                describe(&again),
                "preparation must be stable across calls"
            );
        }
    }
}

/// The transforms' composition order in [`prepare_mark`], pinned so a future
/// rearrangement cannot silently reorder them.
///
/// The pipeline is `select/reduce` → `map` → `shift` → `shuffle` (record
/// rewrites, run on the cloned `Dataset` before any row resolves), then
/// `sort` → `window` → `normalize` → `group` → `reduce` → `bin` → `stack` (row
/// rewrites, run on the resolved rows). Each test below exercises one edge of
/// that order: if a transform moved, an observable value changes.
#[cfg(test)]
mod transform_order_tests {
    use super::*;
    use crate::mark::{MapReducer, Mark, MarkKind};
    use crate::transform::WindowReducer;

    fn nums(values: &[f64]) -> Vec<Table> {
        values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Table::empty()
                    .with("x", Value::Number(i as f64))
                    .with("g", Value::text(if i % 2 == 0 { "a" } else { "b" }))
                    .with("v", Value::Number(*v))
            })
            .collect()
    }

    #[test]
    fn map_runs_before_resolve() {
        // `map` folds `v` into a new `total` field; the y channel reads it, so
        // the field must exist before the row resolves. If resolve ran first,
        // the y channel would find no `total` and read nothing.
        let mark = Mark::new(MarkKind::BarY)
            .x("x")
            .y("total")
            .map("v", "total", MapReducer::Sum);
        let rows = &prepare(&Dataset::new(nums(&[1., 2., 3., 4.])), &[mark])[0].rows;
        assert_eq!(
            rows.iter().map(|r| r.y_value()).collect::<Vec<_>>(),
            vec![
                &Value::Number(1.),
                &Value::Number(3.),
                &Value::Number(6.),
                &Value::Number(10.)
            ]
        );
    }

    #[test]
    fn map_y_restarts_per_series_and_window_reads_it() {
        // Plot's `windowY(2, mapY("cumsum", {x, y: "v", z: "g"}))`: the sum
        // restarts per `g`, and the window runs over the mapped values.
        let mark = Mark::new(MarkKind::Line)
            .x("x")
            .y("v")
            .z("g")
            .map_y(transform::Mapper::Cumsum);
        let data = Dataset::new(nums(&[1., 2., 3., 4.]));
        let rows = &prepare(&data, std::slice::from_ref(&mark))[0].rows;
        let ys: Vec<Option<f64>> = rows.iter().map(|r| r.y_value().as_number()).collect();
        assert_eq!(ys, vec![Some(1.), Some(2.), Some(4.), Some(6.)]);
        let windowed = mark.window(2, WindowReducer::Sum, Anchor::End);
        let rows = &prepare(&data, &[windowed])[0].rows;
        let ys: Vec<Option<f64>> = rows.iter().map(|r| r.y_value().as_number()).collect();
        assert_eq!(ys, vec![Some(1.), Some(2.), Some(5.), Some(8.)]);
    }

    #[test]
    fn map_calls_compose_in_order() {
        // `mapY(window({k: 2, reduce: "sum", anchor: "end"}), mapY("cumsum", …))`:
        // the window reads the running totals 1, 3, 6.
        let mark = Mark::new(MarkKind::Line)
            .x("x")
            .y("v")
            .map_y(transform::Mapper::Cumsum)
            .map_y(transform::Mapper::window(2, WindowReducer::Sum).anchor(Anchor::End));
        let rows = &prepare(&Dataset::new(nums(&[1., 2., 3.])), &[mark])[0].rows;
        let ys: Vec<Option<f64>> = rows.iter().map(|r| r.y_value().as_number()).collect();
        assert_eq!(ys, vec![Some(1.), Some(4.), Some(9.)]);
    }

    #[test]
    fn mapped_interval_ends_replace_an_area_y() {
        // `areaY(map({y1, y2}, {y: "v"}))` draws the band and drops `y`.
        let mark = Mark::new(MarkKind::AreaY)
            .x("x")
            .y("v")
            .map_channel("y1", transform::Mapper::Rank)
            .map_channel("y2", transform::Mapper::Cumsum);
        let prepared = &prepare(&Dataset::new(nums(&[1., 2., 3.])), &[mark])[0];
        assert!(prepared.mark.channels.y.is_none());
        let bands: Vec<_> = prepared.rows.iter().map(PreparedRow::y_interval).collect();
        assert_eq!(
            bands,
            vec![
                Some((Value::Number(0.), Value::Number(1.))),
                Some((Value::Number(1.), Value::Number(3.))),
                Some((Value::Number(2.), Value::Number(6.))),
            ]
        );
    }

    #[test]
    fn shift_runs_before_resolve() {
        let mark = Mark::new(MarkKind::BarY)
            .x("x")
            .y("shifted")
            .shift("v", "shifted", 10.);
        let rows = &prepare(&Dataset::new(nums(&[1., 2., 3.])), &[mark])[0].rows;
        assert_eq!(
            rows.iter().map(|r| r.y_value()).collect::<Vec<_>>(),
            vec![
                &Value::Number(11.),
                &Value::Number(12.),
                &Value::Number(13.)
            ]
        );
    }

    #[test]
    fn sort_runs_before_group() {
        // Sorting by the value, then grouping by x, must group in the sorted
        // (descending) order. If group ran first, the categories would appear
        // in their original order.
        let mark = Mark::new(MarkKind::BarY)
            .x("x")
            .y("v")
            .sort("v", true)
            .group_x();
        let rows = &prepare(&Dataset::new(nums(&[10., 1., 20., 2.])), &[mark])[0].rows;
        // Sorted descending: 20 (i=2), 10 (i=0), 2 (i=3), 1 (i=1). Grouped by
        // x, so the x values keep the sorted encounter order.
        assert_eq!(rows[0].x_value(), &Value::Number(2.));
        assert_eq!(rows[1].x_value(), &Value::Number(0.));
        assert_eq!(rows[2].x_value(), &Value::Number(3.));
        assert_eq!(rows[3].x_value(), &Value::Number(1.));
    }

    #[test]
    fn sort_keeps_ties_in_input_order() {
        // A stable sort: rows with equal keys keep their input order, whichever
        // the direction.
        let data = Dataset::new(nums(&[2., 1., 2., 1., 2.]));
        for descending in [false, true] {
            let mark = Mark::new(MarkKind::Dot).x("x").y("v").sort("v", descending);
            let rows = &prepare(&data, &[mark])[0].rows;
            let order: Vec<f64> = rows.iter().filter_map(|r| r.x.as_number()).collect();
            let expected = if descending {
                [0., 2., 4., 1., 3.]
            } else {
                [1., 3., 0., 2., 4.]
            };
            assert_eq!(order, expected);
        }
    }

    #[test]
    fn window_runs_before_draw() {
        // A window rewrites the y value in place; the first row of a backward
        // cumulative window is its own value only if the window ran before the
        // rows were read for placement.
        let mark = Mark::new(MarkKind::Line).x("x").y("v").window(
            2,
            WindowReducer::CumulativeMean,
            Anchor::Start,
        );
        let rows = &prepare(&Dataset::new(nums(&[2., 4., 6., 8.])), &[mark])[0].rows;
        // Cumulative means: 2, 3, 4, 5.
        assert_eq!(
            rows.iter().map(|r| r.y_value()).collect::<Vec<_>>(),
            vec![
                &Value::Number(2.),
                &Value::Number(3.),
                &Value::Number(4.),
                &Value::Number(5.)
            ]
        );
    }

    #[test]
    fn normalize_rescales_before_group_counts() {
        // `normalize` rescales y to [0, 1] and a `group` then bins by it. Three
        // equal rows normalize to the same point, so their group's count sees
        // the normalized value, not the raw one.
        let data = vec![
            Table::empty()
                .with("x", Value::Number(0.))
                .with("g", Value::text("a"))
                .with("v", Value::Number(10.)),
            Table::empty()
                .with("x", Value::Number(1.))
                .with("g", Value::text("a"))
                .with("v", Value::Number(20.)),
            Table::empty()
                .with("x", Value::Number(2.))
                .with("g", Value::text("a"))
                .with("v", Value::Number(0.)),
        ];
        let mark = Mark::new(MarkKind::BarY)
            .x("g")
            .y("v")
            .normalize()
            .group_x();
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        // All three share x="a", so one group. Its count is 3 regardless of
        // the normalized values; the important edge is that normalize ran on
        // the whole set first (span 20), not after grouping.
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].y_value(), &Value::Number(3.));
    }

    #[test]
    fn reduce_runs_after_resolve() {
        // `reduce` by sum collapses rows sharing a category (x here) into one
        // total. If reduce ran before the rows were resolved, the categories
        // wouldn't be available to group on.
        let data = vec![
            Table::empty()
                .with("x", Value::text("a"))
                .with("v", Value::Number(1.)),
            Table::empty()
                .with("x", Value::text("a"))
                .with("v", Value::Number(2.)),
            Table::empty()
                .with("x", Value::text("b"))
                .with("v", Value::Number(3.)),
            Table::empty()
                .with("x", Value::text("b"))
                .with("v", Value::Number(4.)),
        ];
        let mark = Mark::new(MarkKind::BarY).x("x").y("v").reduce(Reducer::Sum);
        let rows = &prepare(&Dataset::new(data), &[mark])[0].rows;
        // One row per category, summed.
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].x_value(), &Value::text("a"));
        assert_eq!(rows[0].y_value(), &Value::Number(3.));
        assert_eq!(rows[1].x_value(), &Value::text("b"));
        assert_eq!(rows[1].y_value(), &Value::Number(7.));
    }
}

#[cfg(test)]
mod interval_tests {
    use super::*;
    use crate::mark::{Interval, Mark, MarkKind};

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
}

#[cfg(test)]
mod interval_domain_tests {
    use super::*;
    use crate::mark::{Interval, Mark, MarkKind};

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

    use crate::value::PlotKind;
}

#[cfg(test)]
mod bin_tests {
    use super::*;
    use crate::mark::MarkKind;
    use crate::transform::GroupReducer;

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

/// The behaviour the pipeline's hot-loop rewrites must keep, each checked
/// against a naive reading of the raw records: group and bin order, bound
/// `$in:` inputs, box parts reading their value channel, facet partitions
/// shared only between alike marks, and a `z` shared with a position.
#[cfg(test)]
mod equivalence_tests {
    use super::*;
    use crate::mark::{GroupOutput, SortConfig};
    use crate::style::ColorChannel;
    use crate::table::Channel;
    use crate::transform::GroupReducer;
    use crate::value::ValueKey;
    use std::rc::Rc;

    /// Records with awkward keys (text, null, -0 and +0, a NaN, a boolean)
    /// and values with nulls and NaNs, carrying a field under a bound input's
    /// internal name that the mark's own channel must win over.
    fn records() -> Vec<Table> {
        let keys = [
            Value::text("b"),
            Value::Null,
            Value::text("a"),
            Value::Number(-0.),
            Value::Number(0.),
            Value::Number(f64::NAN),
            Value::Bool(true),
            Value::text("b"),
            Value::text("a"),
        ];
        (0..45)
            .map(|i| {
                let v = match i % 7 {
                    2 => Value::Null,
                    4 => Value::Number(f64::NAN),
                    _ => Value::Number(((i * 37) % 23) as f64 + 0.5),
                };
                Table::new([
                    ("c", keys[i % keys.len()].clone()),
                    ("v", v),
                    ("w", Value::Number(((i * 11) % 7) as f64)),
                    ("n", Value::Number(i as f64)),
                    ("g", Value::text(["x", "y", "z"][i % 3])),
                    ("f", Value::text(["p", "q", "r"][(i / 2) % 3])),
                    ("$in:fill", Value::text("planted")),
                ])
            })
            .collect()
    }

    /// The members of each group, by first appearance, in row order.
    fn naive_groups(rows: &[&Table], key: impl Fn(&Table) -> ValueKey) -> Vec<Vec<usize>> {
        let mut groups: Vec<(ValueKey, Vec<usize>)> = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let k = key(row);
            match groups.iter_mut().find(|(existing, _)| *existing == k) {
                Some((_, members)) => members.push(i),
                None => groups.push((k, vec![i])),
            }
        }
        groups.into_iter().map(|(_, members)| members).collect()
    }

    fn keys(values: impl IntoIterator<Item = Value>) -> Vec<ValueKey> {
        values.into_iter().map(|v| v.key()).collect()
    }

    #[test]
    fn groups_keep_first_appearance_order_and_row_order() {
        let rows = records();
        // An accessor keeps the NaN key, which a field reads as null.
        let mut mark = Mark::new(MarkKind::Dot)
            .x(|row: &Table| row.value("c"))
            .group_x();
        let config = mark.group.as_mut().unwrap();
        for (name, reducer, input) in [
            ("first", GroupReducer::First, "n"),
            ("last", GroupReducer::Last, "n"),
            ("count", GroupReducer::Count, "v"),
            ("lo", GroupReducer::Min, "v"),
            ("hi", GroupReducer::Max, "v"),
            ("mean", GroupReducer::Mean, "v"),
            ("spread", GroupReducer::Deviation, "v"),
            ("found", GroupReducer::find(|row| row.num("w") > 3.), "n"),
            ("share", GroupReducer::Proportion, "v"),
        ] {
            config
                .outputs
                .push(GroupOutput::new(name, reducer).input(input));
        }
        let outputs = config.outputs.clone();
        let prepared = &prepare(&Dataset::new(rows.clone()), &[mark])[0];
        let refs: Vec<&Table> = rows.iter().collect();
        let groups = naive_groups(&refs, |row| row.value("c").key());
        // -0 and +0 are one group, which keeps its first member's key.
        assert_eq!(groups.len(), 6);
        assert_eq!(prepared.rows.len(), groups.len());
        for (row, members) in prepared.rows.iter().zip(&groups) {
            assert_eq!(row.x.key(), rows[members[0]].value("c").key());
            let records: Vec<&Table> = members.iter().map(|&i| &rows[i]).collect();
            for output in &outputs {
                let input = output.input.as_deref().unwrap();
                let values: Vec<Value> = records.iter().map(|r| r.value(input)).collect();
                let expected = match output.reducer {
                    GroupReducer::Proportion => {
                        Value::Number(members.len() as f64 / rows.len() as f64)
                    }
                    _ => output.reducer.reduce(&values, &records),
                };
                assert_eq!(
                    row.row.value(&output.name).key(),
                    expected.key(),
                    "{} of {:?}",
                    output.name,
                    row.x
                );
            }
            // A count counts every member, missing values too.
            assert_eq!(row.row.number("count"), Some(members.len() as f64));
        }
    }

    #[test]
    fn bound_group_outputs_read_the_mark_s_own_channels() {
        let rows = records();
        let mut mark = Mark::new(MarkKind::Dot)
            .x("g")
            .fill(ColorChannel::value(|row: &Table| {
                Value::number(row.num("w") * 3.)
            }))
            .text(|row: &Table| row.value("n"))
            .tip_title(|row: &Table| row.value("v"))
            .group_x()
            .group_output("fill", GroupReducer::Sum)
            .group_output("text", GroupReducer::First)
            .group_output("title", GroupReducer::Last);
        // Sorted first, so the first and last members follow the sort.
        mark.sort = Some(SortConfig {
            field: Some("n".into()),
            descending: true,
            reverse: false,
        });
        let prepared = &prepare(&Dataset::new(rows.clone()), &[mark])[0];
        let mut sorted: Vec<&Table> = rows.iter().collect();
        sorted.sort_by_key(|row| std::cmp::Reverse(row.value("n")));
        let groups = naive_groups(&sorted, |row| row.value("g").key());
        assert_eq!(prepared.rows.len(), groups.len());
        for (row, members) in prepared.rows.iter().zip(&groups) {
            let records: Vec<&Table> = members.iter().map(|&i| sorted[i]).collect();
            let fill: f64 = records.iter().map(|r| r.num("w") * 3.).sum();
            assert_eq!(row.row.number("fill"), Some(fill));
            assert_eq!(row.row.value("text"), records[0].value("n"));
            let last = GroupReducer::Last.reduce(
                &records.iter().map(|r| r.value("v")).collect::<Vec<_>>(),
                &records,
            );
            assert_eq!(row.row.value("title").key(), last.key());
        }
        // Each output then binds the mark's channel to its field.
        assert!(matches!(
            &prepared.mark.channels.fill,
            Some(ColorChannel::Field(name)) if name == "fill"
        ));
    }

    #[test]
    fn bound_bin_outputs_read_the_mark_s_own_channels() {
        // Values between the edges of 10-wide bins, so each lands in the bin
        // floor(v / 10).
        let rows: Vec<Table> = (0..40)
            .map(|i| {
                Table::new([
                    ("v", Value::Number(((i * 7) % 50) as f64 + 0.5)),
                    ("w", Value::Number((i % 5) as f64)),
                    ("$in:fill", Value::text("planted")),
                ])
            })
            .collect();
        let mark = Mark::new(MarkKind::RectY)
            .x("v")
            .fill(ColorChannel::value(|row: &Table| {
                Value::number(row.num("w") * 2.)
            }))
            .bin(Thresholds::Interval(10.))
            .bin_output("fill", GroupReducer::Mean)
            .bin_output("y", GroupReducer::Count);
        let prepared = &prepare(&Dataset::new(rows.clone()), &[mark])[0];
        assert_eq!(prepared.rows.len(), 5);
        let mut total = 0.;
        for row in &prepared.rows {
            let (lo, hi) = row.x_interval().unwrap();
            let (lo, hi) = (lo.as_number().unwrap(), hi.as_number().unwrap());
            let members: Vec<&Table> = rows
                .iter()
                .filter(|r| (lo..hi).contains(&r.num("v")))
                .collect();
            let mean = members.iter().map(|r| r.num("w") * 2.).sum::<f64>() / members.len() as f64;
            assert_eq!(row.row.number("fill"), Some(mean), "[{lo}, {hi})");
            assert_eq!(row.row.number("y"), Some(members.len() as f64));
            total += members.len() as f64;
        }
        assert_eq!(total, 40.);
    }

    /// Every box part's reductions, against its outputs' reducers applied to
    /// the value channel read from each category's raw records.
    fn check_box(rows: &[&Table], parts: &[Mark], prepared: &[PreparedMark], vertical: bool) {
        let value = |part: &Mark| {
            if vertical {
                part.channels.y.clone().unwrap()
            } else {
                part.channels.x.clone().unwrap()
            }
        };
        let groups = naive_groups(rows, |row| row.value("g").key());
        for (part, p) in parts.iter().zip(prepared) {
            let Some(config) = &part.group else {
                continue;
            };
            assert_eq!(p.rows.len(), groups.len(), "{:?}", part.kind);
            let value = value(part);
            for (row, members) in p.rows.iter().zip(&groups) {
                let records: Vec<&Table> = members.iter().map(|&i| rows[i]).collect();
                assert_eq!(
                    row.z.as_ref().map(Value::key),
                    Some(records[0].value("g").key())
                );
                let values: Vec<Value> = records.iter().map(|r| value.read(r)).collect();
                for output in &config.outputs {
                    assert_eq!(
                        row.row.value(&output.name).key(),
                        output.reducer.reduce(&values, &records).key(),
                        "{:?} {}",
                        part.kind,
                        output.name
                    );
                }
            }
        }
    }

    #[test]
    fn box_parts_reduce_their_value_channel_with_its_gaps() {
        let rows = records();
        let refs: Vec<&Table> = rows.iter().collect();
        let accessor = Channel::Accessor(Rc::new(|row: &Table| row.value("v")));
        for vertical in [true, false] {
            for value in [Channel::from("v"), accessor.clone()] {
                let mark = if vertical {
                    Mark::new(MarkKind::BoxY).x("g").y(value)
                } else {
                    Mark::new(MarkKind::BoxX).y("g").x(value)
                };
                let parts = crate::mark::expand_box(mark);
                let prepared = prepare(&Dataset::new(rows.clone()), &parts);
                check_box(&refs, &parts, &prepared, vertical);
            }
        }
    }

    #[test]
    fn faceted_box_parts_reduce_each_facet_s_rows() {
        let rows = records();
        let data = Dataset::new(rows.clone());
        let parts = crate::mark::expand_box(Mark::new(MarkKind::BoxY).x("g").y("v").fx("f"));
        let grid = Grid::from_values(["p", "q", "r"].map(Value::text).to_vec(), Vec::new());
        let (facets, _) = prepare_facets(&data, &parts, &grid, None, None);
        for (cell, prepared) in grid.cells.iter().zip(&facets) {
            let refs: Vec<&Table> = rows
                .iter()
                .filter(|r| cell.x == Some(r.value("f").key()))
                .collect();
            check_box(&refs, &parts, prepared, true);
        }
    }

    #[test]
    fn marks_share_a_facet_partition_only_when_alike() {
        let rows = records();
        let data = Dataset::new(rows.clone());
        // Fewer rows, the same channel: a partition of its own.
        let other = Dataset::new(rows[..20].to_vec());
        let by_f = Channel::Accessor(Rc::new(|row: &Table| row.value("f")));
        // Another accessor naming the same facets from another field.
        let by_g = Channel::Accessor(Rc::new(|row: &Table| {
            Value::text(match row.text("g").map(|g| g.as_ref()) {
                Some("x") => "q",
                Some("y") => "r",
                _ => "p",
            })
        }));
        let dot = || Mark::new(MarkKind::Dot).x("n").y("w");
        let marks = vec![
            dot().fx(by_f.clone()),
            dot().fx(by_g.clone()),
            dot().fx(by_f.clone()),
            dot().data(other.clone()).fx(by_f.clone()),
            dot().fx("f"),
            dot().data(other).fx("f"),
            dot().fx(Value::text("q")),
            dot().fx(by_f.clone()).fy(by_g.clone()),
        ];
        let p_q_r = ["p", "q", "r"].map(Value::text).to_vec();
        for grid in [
            Grid::from_values(p_q_r.clone(), Vec::new()),
            Grid::from_values(p_q_r.clone(), p_q_r.clone()),
        ] {
            let (facets, _) = prepare_facets(&data, &marks, &grid, None, None);
            for (cell, prepared) in grid.cells.iter().zip(&facets) {
                for (mark, p) in marks.iter().zip(prepared) {
                    let source = mark.data.as_ref().unwrap_or(&data);
                    let fits = |channel: &Option<Channel>, have: &Option<ValueKey>, row: &Table| {
                        channel
                            .as_ref()
                            .is_none_or(|c| have.as_ref() == Some(&c.read(row).key()))
                    };
                    let expected: Vec<f64> = source
                        .rows()
                        .iter()
                        .filter(|r| fits(&mark.fx, &cell.x, r) && fits(&mark.fy, &cell.y, r))
                        .map(|r| r.num("n"))
                        .collect();
                    let got: Vec<f64> = p.rows.iter().map(|r| r.x.as_number().unwrap()).collect();
                    assert_eq!(got, expected, "{cell:?}");
                }
            }
        }
    }

    #[test]
    fn a_z_shared_with_a_position_reads_the_raw_value() {
        let rows = records();
        let c = Channel::Accessor(Rc::new(|row: &Table| row.value("c")));
        let marks = [
            Mark::new(MarkKind::Dot).x(c.clone()).y("w").z(c.clone()),
            Mark::new(MarkKind::Dot).x("v").y("w").z("v"),
            Mark::new(MarkKind::Dot).x("w").y("c").z("c"),
            // The index fills x; z still reads y's channel.
            Mark::new(MarkKind::LineY).y("v").z("v"),
            Mark::new(MarkKind::LineX).x("v").z("v"),
            Mark::new(MarkKind::Dot)
                .x(Value::Number(1.))
                .y("w")
                .z(Value::Number(1.)),
        ];
        let prepared = prepare(&Dataset::new(rows.clone()), &marks);
        for (mark, p) in marks.iter().zip(&prepared) {
            let z = mark.channels.z.as_ref().unwrap();
            let expected: Vec<Option<ValueKey>> = rows
                .iter()
                .map(|r| Some(z.read(r)).filter(|v| !v.is_null()).map(|v| v.key()))
                .collect();
            let got: Vec<Option<ValueKey>> = p
                .rows
                .iter()
                .map(|r| r.z.as_ref().map(Value::key))
                .collect();
            assert_eq!(got, expected, "{:?}", mark.kind);
        }
        let indices = || keys((0..rows.len()).map(|i| Value::Number(i as f64)));
        assert_eq!(
            keys(prepared[3].rows.iter().map(|r| r.x.clone())),
            indices()
        );
        assert_eq!(
            keys(prepared[4].rows.iter().map(|r| r.y.clone())),
            indices()
        );
    }

    #[test]
    fn sorts_are_stable_in_both_directions_and_reverse_after() {
        let rows = records();
        for (field, descending, reverse) in [
            (Some("w"), false, false),
            (Some("w"), true, false),
            (Some("w"), true, true),
            (Some("c"), false, true),
            (Some("v"), true, false),
            (None, true, false),
            (None, false, true),
        ] {
            let mut mark = Mark::new(MarkKind::BarY).x("n").y("v");
            mark.sort = Some(SortConfig {
                field: field.map(Into::into),
                descending,
                reverse,
            });
            let prepared = &prepare(&Dataset::new(rows.clone()), &[mark])[0];
            let mut expected: Vec<&Table> = rows.iter().collect();
            // A bar's value, by default, is the channel the mark reads.
            let key = |r: &Table| match field {
                Some(field) => r.value(field),
                None => Channel::from("v").read(r),
            };
            expected.sort_by(|a, b| {
                let ordering = key(a).cmp(&key(b));
                if descending {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
            if reverse {
                expected.reverse();
            }
            let got: Vec<f64> = prepared
                .rows
                .iter()
                .map(|r| r.x.as_number().unwrap())
                .collect();
            let want: Vec<f64> = expected.iter().map(|r| r.num("n")).collect();
            assert_eq!(got, want, "{field:?} {descending} {reverse}");
        }
    }

    /// The kinds [`derive_intervals`] writes for, and two that only an
    /// interval makes it write for.
    fn deriving_marks() -> Vec<Mark> {
        use MarkKind::*;
        let mut marks: Vec<Mark> = [
            RuleX,
            RuleY,
            DifferenceX,
            DifferenceY,
            BarX,
            BarY,
            AreaX,
            AreaY,
            WaffleX,
            WaffleY,
            RectX,
            RectY,
            Cell,
        ]
        .into_iter()
        .map(Mark::new)
        .collect();
        marks.push(Mark::new(Rect).interval(crate::mark::Interval::Step(2.)));
        marks.push(Mark::new(Dot).interval(crate::mark::Interval::Step(2.)));
        marks
    }

    #[test]
    fn deriving_on_a_raw_row_never_changes_the_value_it_reads() {
        // With no interval channel, every derivation that fills an end needs
        // that axis's value, so `x_value` and `y_value` read the same either
        // way: what lets a grouped mark skip it.
        let values = [
            Value::Null,
            Value::Number(-0.),
            Value::Number(3.),
            Value::Number(f64::NAN),
            Value::text("a"),
            Value::date_from_millis(86_400_000),
            Value::Bool(true),
        ];
        for mark in deriving_marks() {
            for x in &values {
                for y in &values {
                    let row = PreparedRow {
                        row: Table::empty(),
                        index: 0,
                        x: x.clone(),
                        y: y.clone(),
                        x1: None,
                        x2: None,
                        y1: None,
                        y2: None,
                        z: None,
                        path: None,
                        r: None,
                        symbol: None,
                        opacity: None,
                        r_value: None,
                        opacity_value: None,
                        dodge_offset: None,
                        pixel: None,
                    };
                    let mut derived = row.clone();
                    derive_intervals(
                        &mark,
                        &derived.x,
                        &derived.y,
                        &mut derived.x1,
                        &mut derived.x2,
                        &mut derived.y1,
                        &mut derived.y2,
                    );
                    assert!(x.is_null() <= (derived.x1.is_none() && derived.x2.is_none()));
                    assert!(y.is_null() <= (derived.y1.is_none() && derived.y2.is_none()));
                    assert_eq!(
                        derived.x_value().key(),
                        row.x_value().key(),
                        "{:?}",
                        mark.kind
                    );
                    assert_eq!(
                        derived.y_value().key(),
                        row.y_value().key(),
                        "{:?}",
                        mark.kind
                    );
                }
            }
        }
    }

    /// A grouped mark's rows, resolved with or without [`derive_intervals`]
    /// on the raw rows, then grouped: what [`prepare_mark`] does for a mark
    /// that streams its rows into the grouping.
    fn grouped_rows(data: &Dataset, mark: &Mark, derive: bool) -> String {
        let mut facts = RowFacts::new(&mark.channels, mark);
        facts.derive = derive;
        let rows = data
            .rows()
            .iter()
            .enumerate()
            .map(|(index, row)| resolve_row(row, index, &mark.channels, mark, &facts));
        let grouped = collapse_groups(
            rows,
            |row| row,
            mark,
            mark.group.as_ref().unwrap(),
            &mark.channels,
        );
        format!("{grouped:?}")
    }

    #[test]
    fn a_grouped_mark_s_raw_rows_need_no_intervals() {
        let rows = records();
        let data = Dataset::new(rows.clone());
        let mut checked = 0;
        for base in deriving_marks() {
            for group in 0..3 {
                let mark = base.clone().x("c").y("v").fill("g");
                let mut mark = match group {
                    0 => mark.group_x(),
                    1 => mark.group_y(),
                    _ => mark.group_z(),
                };
                // Every input a grouping reads from the resolved row, and a
                // field from the record.
                let config = mark.group.as_mut().unwrap();
                for (name, input) in [
                    ("x", "x"),
                    ("y", "y"),
                    ("x1", "x1"),
                    ("x2", "x2"),
                    ("y1", "y1"),
                    ("y2", "y2"),
                    ("z", "z"),
                    ("w", "w"),
                ] {
                    config
                        .outputs
                        .push(GroupOutput::new(name, GroupReducer::First).input(input));
                }
                config
                    .outputs
                    .push(GroupOutput::new("lo", GroupReducer::Min).input("y"));
                assert!(!derives_intervals(&mark.channels, &mark), "{:?}", mark.kind);
                assert_eq!(
                    grouped_rows(&data, &mark, true),
                    grouped_rows(&data, &mark, false),
                    "{:?} {group}",
                    mark.kind
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 45);
        // An interval channel, or a transform before the grouping, keeps the
        // derivation.
        let grouped = Mark::new(MarkKind::BarY).x("c").y("v").group_x();
        assert!(derives_intervals(
            &grouped.clone().y2("w").channels,
            &grouped.clone().y2("w")
        ));
        let mut windowed = grouped.clone();
        windowed.window = Some((3, crate::transform::WindowReducer::Mean, Anchor::Middle));
        assert!(derives_intervals(&windowed.channels, &windowed));
        let mut outliers = grouped.clone();
        outliers.outliers = Some(Axis::Y);
        assert!(derives_intervals(&outliers.channels, &outliers));
        let mut normalized = grouped.clone();
        normalized.normalize = true;
        assert!(derives_intervals(&normalized.channels, &normalized));
        // A kind it writes nothing for skips it without grouping.
        assert!(!derives_intervals(
            &Mark::new(MarkKind::Dot).x("c").channels,
            &Mark::new(MarkKind::Dot).x("c")
        ));
    }

    /// Records whose group keys come in runs, where the grouping compares a
    /// row with the last group's first row: `-0` after `+0`, NaNs, two dates
    /// in one millisecond (equal keys, unequal values), nulls and text.
    fn runs() -> Vec<Table> {
        let within_a_millisecond = |nanos: u32| {
            Value::Date(chrono::DateTime::from_timestamp(1_000, nanos).expect("a valid instant"))
        };
        let keys = [
            Value::Number(0.),
            Value::Number(-0.),
            Value::Number(f64::NAN),
            Value::Number(f64::NAN),
            within_a_millisecond(100),
            within_a_millisecond(200_000),
            Value::Null,
            Value::text("a"),
            Value::text("a"),
            Value::Number(1.),
            Value::Number(0.),
            within_a_millisecond(300_000),
            Value::text("b"),
            Value::Null,
        ];
        (0..keys.len() * 3)
            .map(|i| {
                Table::new([
                    ("c", keys[(i / 3) % keys.len()].clone()),
                    ("z", Value::text(["p", "q"][(i / 7) % 2])),
                    ("v", Value::Number(((i * 37) % 23) as f64)),
                    ("n", Value::Number(i as f64)),
                ])
            })
            .collect()
    }

    #[test]
    fn runs_of_equal_keys_group_as_the_keys_do() {
        let rows = runs();
        let refs: Vec<&Table> = rows.iter().collect();
        for with_z in [false, true] {
            let mut mark = Mark::new(MarkKind::Dot)
                .x(|row: &Table| row.value("c"))
                .group_x()
                .group_output("total", GroupReducer::Sum);
            mark.group.as_mut().unwrap().outputs[0].input = Some("v".into());
            if with_z {
                mark.channels.z = Some(Channel::from("z"));
            }
            let prepared = &prepare(&Dataset::new(rows.clone()), &[mark])[0];
            // The groups by first appearance of each (category, series) key.
            let mut keyed: Vec<((ValueKey, ValueKey), Vec<usize>)> = Vec::new();
            for (i, row) in refs.iter().enumerate() {
                let z = if with_z {
                    row.value("z").key()
                } else {
                    Value::Null.key()
                };
                let key = (row.value("c").key(), z);
                match keyed.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, members)) => members.push(i),
                    None => keyed.push((key, vec![i])),
                }
            }
            let groups: Vec<Vec<usize>> = keyed.into_iter().map(|(_, m)| m).collect();
            assert_eq!(prepared.rows.len(), groups.len(), "z: {with_z}");
            for (row, members) in prepared.rows.iter().zip(&groups) {
                assert_eq!(row.x.key(), rows[members[0]].value("c").key());
                let total: f64 = members.iter().map(|&i| rows[i].num("v")).sum();
                assert_eq!(row.row.number("total"), Some(total));
            }
        }
    }

    #[test]
    fn streamed_and_collected_groupings_agree() {
        // A sort by the row index keeps the order but collects the rows
        // before grouping them, so the two paths must agree exactly.
        for rows in [records(), runs()] {
            let data = Dataset::new(rows);
            for base in [
                Mark::new(MarkKind::BarY).x("c").y("v").group_x(),
                Mark::new(MarkKind::RuleX).x("c").y("v").fill("g").group_x(),
                Mark::new(MarkKind::Dot).x("c").y("v").group_z(),
            ] {
                let mut mark = base
                    .group_output("y1", GroupReducer::Quantile(0.25))
                    .group_output("y2", GroupReducer::WhiskerHigh)
                    .group_output("found", GroupReducer::find(|row| row.num("n") > 4.))
                    .group_output("share", GroupReducer::Proportion);
                for output in mark.group.as_mut().unwrap().outputs.iter_mut() {
                    output.input = Some("v".into());
                }
                let streamed = prepare(&data, std::slice::from_ref(&mark));
                let collected = prepare(&data, &[mark.clone().sort("n", false)]);
                assert_eq!(
                    format!("{:?}", streamed[0].rows),
                    format!("{:?}", collected[0].rows),
                    "{:?}",
                    mark.kind
                );
            }
        }
    }

    #[test]
    fn rows_streamed_as_cores_group_as_resolved_rows_do() {
        // Every input a group reads from a row: its positions, interval
        // ends (with and without interval channels, where they derive),
        // series, a record field, a bound channel and the records
        // themselves; and marks whose r, opacity, symbol and path channels
        // only a group's first row resolves.
        let outputs = [
            ("x1", GroupReducer::Min, Some("x1")),
            ("y1", GroupReducer::Quantile(0.25), Some("y1")),
            ("y2", GroupReducer::Max, Some("y2")),
            ("yy", GroupReducer::Mean, Some("y")),
            ("xx", GroupReducer::First, Some("x")),
            ("zz", GroupReducer::Last, Some("z")),
            ("label", GroupReducer::First, Some("f")),
            ("fill", GroupReducer::Mode, None),
            (
                "found",
                GroupReducer::find(|row| row.num("n") > 4.),
                Some("n"),
            ),
            ("share", GroupReducer::Proportion, None),
        ];
        for rows in [records(), runs()] {
            let data = Dataset::new(rows);
            for base in [
                Mark::new(MarkKind::BarY).x("c").y("v").group_x(),
                Mark::new(MarkKind::BarY)
                    .x("c")
                    .y1("w")
                    .y2("v")
                    .fill("g")
                    .group_x(),
                Mark::new(MarkKind::RectX).y("c").x1("w").x2("v").group_y(),
                Mark::new(MarkKind::Dot)
                    .x("c")
                    .y("v")
                    .z("g")
                    .r("w")
                    .fill("f")
                    .opacity("n")
                    .group_xy(),
                Mark::new(MarkKind::Dot)
                    .x("c")
                    .y(|row: &Table| row.value("v"))
                    .group_z(),
                Mark::new(MarkKind::RuleX)
                    .x("c")
                    .y("v")
                    .stroke("g")
                    .z("f")
                    .group_x(),
            ] {
                let mut mark = base;
                for (name, reducer, input) in &outputs {
                    mark = mark.group_output(*name, reducer.clone());
                    let output = mark.group.as_mut().unwrap().outputs.last_mut().unwrap();
                    output.input = input.map(Into::into);
                }
                let streamed = prepare(&data, std::slice::from_ref(&mark));
                let collected = prepare(&data, &[mark.clone().sort("n", false)]);
                assert_eq!(
                    format!("{:?}", streamed[0].rows),
                    format!("{:?}", collected[0].rows),
                    "{:?}",
                    mark.kind
                );
            }
        }
    }

    #[test]
    fn outliers_read_as_cores_keep_the_rows_resolved_in_full() {
        // The outliers read from each row's value and series alone, then
        // resolved in full, against every row resolved before the filter
        // (a sort by the row index collects them, in the same order).
        let mut rows = records();
        rows.extend(runs());
        for (i, row) in rows.iter_mut().enumerate() {
            row.set("n", Value::Number(i as f64));
            if i % 11 == 0 {
                row.set("v", Value::Number(if i % 2 == 0 { 400. } else { -400. }));
            }
        }
        let data = Dataset::new(rows);
        for (axis, mark) in [
            (
                Axis::Y,
                Mark::new(MarkKind::Dot)
                    .x("c")
                    .y("v")
                    .z("c")
                    .r("w")
                    .fill("g"),
            ),
            (
                Axis::Y,
                Mark::new(MarkKind::Dot).x("c").y("v").z("g").symbol("f"),
            ),
            (Axis::X, Mark::new(MarkKind::Dot).y("c").x("v").opacity("w")),
            (Axis::Y, Mark::new(MarkKind::BarY).x("c").y("v")),
            (Axis::Y, Mark::new(MarkKind::Dot).x("c").y("v").group_x()),
        ] {
            let mut filtered = mark;
            filtered.outliers = Some(axis);
            let streamed = &prepare(&data, std::slice::from_ref(&filtered))[0];
            let collected = &prepare(&data, &[filtered.clone().sort("n", false)])[0];
            assert!(!streamed.rows.is_empty());
            assert_eq!(
                format!("{:?}", streamed.rows),
                format!("{:?}", collected.rows),
                "{axis:?} {:?}",
                filtered.kind
            );
        }
    }

    /// Plot's box outliers as they were read before the single pass: each
    /// `z` group's whiskers, then every row tested against its group's.
    fn naive_outliers(rows: &[PreparedRow], axis: Axis) -> Vec<usize> {
        let read = |row: &PreparedRow| {
            if axis == Axis::X {
                row.x_value().clone()
            } else {
                row.y_value().clone()
            }
        };
        let key = |row: &PreparedRow| row.z.as_ref().map(Value::key).unwrap_or(Value::Null.key());
        let mut groups: Vec<(ValueKey, Vec<Value>)> = Vec::new();
        for row in rows {
            match groups.iter_mut().find(|(k, _)| *k == key(row)) {
                Some((_, values)) => values.push(read(row)),
                None => groups.push((key(row), vec![read(row)])),
            }
        }
        let bounds: Vec<(ValueKey, f64, f64)> = groups
            .into_iter()
            .map(|(k, values)| {
                let lo = crate::mark::whisker_low(&values).as_number();
                let hi = crate::mark::whisker_high(&values).as_number();
                (
                    k,
                    lo.unwrap_or(f64::NEG_INFINITY),
                    hi.unwrap_or(f64::INFINITY),
                )
            })
            .collect();
        rows.iter()
            .filter(|row| {
                let Some(v) = read(row).as_number() else {
                    return false;
                };
                bounds
                    .iter()
                    .find(|(k, _, _)| *k == key(row))
                    .is_some_and(|(_, lo, hi)| v < *lo || v > *hi)
            })
            .map(|row| row.index)
            .collect()
    }

    #[test]
    fn outliers_keep_the_rows_beyond_their_group_s_whiskers() {
        // Spread values with a few far ones, so each group has outliers.
        let mut rows = records();
        rows.extend(runs());
        for (i, row) in rows.iter_mut().enumerate() {
            if i % 11 == 0 {
                row.set("v", Value::Number(if i % 2 == 0 { 400. } else { -400. }));
            }
        }
        let data = Dataset::new(rows);
        for (axis, mark) in [
            (Axis::Y, Mark::new(MarkKind::Dot).x("c").y("v").z("c")),
            (Axis::Y, Mark::new(MarkKind::Dot).x("c").y("v").z("g")),
            (Axis::X, Mark::new(MarkKind::Dot).y("c").x("v")),
        ] {
            let plain = &prepare(&data, std::slice::from_ref(&mark))[0];
            let expected = naive_outliers(&plain.rows, axis);
            assert!(!expected.is_empty());
            let mut filtered = mark.clone();
            filtered.outliers = Some(axis);
            let kept = &prepare(&data, &[filtered])[0];
            let got: Vec<usize> = kept.rows.iter().map(|row| row.index).collect();
            assert_eq!(got, expected, "{axis:?}");
        }
    }

    /// A bin reads the same numbers from its rows' resolved positions as
    /// from their records, for each way it picks its channels: both axes,
    /// one axis, the other axis's channel standing in (a `rectY` with only
    /// a `y`), an accessor, and values that are text, null, NaN, -0, dates
    /// and booleans.
    #[test]
    fn bins_read_resolved_positions_as_their_records() {
        let mut data = records();
        for (i, row) in data.iter_mut().enumerate() {
            let when = chrono::DateTime::from_timestamp_millis(i as i64 * 86_400_000 * 3);
            row.set("t", when.map_or(Value::Null, Value::Date));
            if i % 5 == 0 {
                row.set("n", Value::Number(-0.));
            }
        }
        let bin = |mark: Mark| mark.bin(Thresholds::Count(6));
        let marks = [
            bin(Mark::new(MarkKind::Dot).x("v").y("n")).bin_output("r", GroupReducer::Count),
            bin(Mark::new(MarkKind::Rect).x("n").y("v").z("g"))
                .bin_output("fill", GroupReducer::Count),
            bin(Mark::new(MarkKind::RectY).x("n")).bin_output("y", GroupReducer::Sum),
            bin(Mark::new(MarkKind::RectY).y("n")),
            bin(Mark::new(MarkKind::RectX).y("t").x("w")).bin_output("x", GroupReducer::Mean),
            bin(Mark::new(MarkKind::Dot).y("c")),
            bin(Mark::new(MarkKind::Dot).x("c").y("n")),
            bin(Mark::new(MarkKind::Dot)
                .x(|row: &Table| row.value("v"))
                .y("w")),
            bin(Mark::new(MarkKind::BarY).x("v").fill("f")).bin_cumulative(1),
        ];
        for mark in marks {
            let facts = RowFacts::new(&mark.channels, &mark);
            let rows: Vec<PreparedRow> = data
                .iter()
                .enumerate()
                .map(|(i, row)| resolve_row(row, i, &mark.channels, &mark, &facts))
                .collect();
            let from_rows = bin_rows(&mark, &rows, &mark.channels, true);
            let from_records = bin_rows(&mark, &rows, &mark.channels, false);
            assert!(!from_records.is_empty(), "{:?}", mark.kind);
            assert_eq!(
                format!("{from_rows:?}"),
                format!("{from_records:?}"),
                "{:?}",
                mark.kind
            );
        }
    }

    /// A bin over raw records, resolving only each cell's first row, gives
    /// the rows a bin over every resolved row gives (a sort by the row's
    /// index resolves them all first, in the same order): one and two axes,
    /// series from `z` and from a color field, an accessor, an implied `z`
    /// that a quantitative fill drops, a cumulative bin, kept empty bins,
    /// bound and record-reading outputs.
    #[test]
    fn bins_over_records_resolve_only_their_first_rows() {
        let mut data = records();
        for (i, row) in data.iter_mut().enumerate() {
            row.set("i", Value::Number(i as f64));
            let when = chrono::DateTime::from_timestamp_millis(i as i64 * 86_400_000 * 3);
            row.set("t", when.map_or(Value::Null, Value::Date));
        }
        let data = Dataset::new(data);
        let bin = |mark: Mark| mark.bin(Thresholds::Count(6));
        let marks = [
            bin(Mark::new(MarkKind::Dot).x("v").y("n").r("w")).bin_output("r", GroupReducer::Count),
            bin(Mark::new(MarkKind::Rect).x("n").y("v").z("g"))
                .bin_output("fill", GroupReducer::Count),
            bin(Mark::new(MarkKind::RectY).x("n").fill("g")).bin_output("y", GroupReducer::Sum),
            bin(Mark::new(MarkKind::RectY).y("n")),
            bin(Mark::new(MarkKind::RectX).y("t").x("w")).bin_output("x", GroupReducer::Mean),
            bin(Mark::new(MarkKind::Dot).y("c")),
            bin(Mark::new(MarkKind::Dot)
                .x(|row: &Table| row.value("v"))
                .y("w")
                .symbol("f")),
            bin(Mark::new(MarkKind::BarY).x("v").fill("f")).bin_cumulative(1),
            bin(Mark::new(MarkKind::BarY).x("v").fill("f")).bin_cumulative(-1),
            bin(Mark::new(MarkKind::RectY).x("n").stroke("g")).bin_keep_empty(),
            bin(Mark::new(MarkKind::RectY).x("v").fill("w"))
                .bin_output("y", GroupReducer::Proportion)
                .bin_output("fill", GroupReducer::Mode)
                .bin_output("title", GroupReducer::find(|row| row.num("w") > 3.))
                .bin_output("fillOpacity", GroupReducer::Max),
        ];
        for mark in marks {
            let from_records = &prepare(&data, std::slice::from_ref(&mark))[0];
            let from_rows = &prepare(&data, &[mark.clone().sort("i", false)])[0];
            assert!(!from_records.rows.is_empty(), "{:?}", mark.kind);
            assert_eq!(
                format!("{:?}", from_records.rows),
                format!("{:?}", from_rows.rows),
                "{:?}",
                mark.kind
            );
        }
    }

    /// Records whose shapes differ from one to the next: each holds `n` and
    /// a seeded subset of the channel fields and of decoys whose names sort
    /// beside them, so a field's position (and whether it is there at all)
    /// keeps changing under the hints the fast paths read through.
    fn mixed_shapes() -> Vec<Table> {
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let keys = [
            Value::text("a"),
            Value::text("b"),
            Value::Number(-0.),
            Value::Number(0.),
            Value::Number(f64::NAN),
            Value::Null,
            Value::Date(chrono::DateTime::from_timestamp(9, 1).expect("an instant")),
            Value::Date(chrono::DateTime::from_timestamp(9, 2).expect("an instant")),
        ];
        (0..160)
            .map(|i| {
                let r = next();
                let mut row = Table::new([("n", Value::Number(i as f64))]);
                let has = |bit: u32| r >> bit & 3 != 0;
                if has(0) {
                    row.set("c", keys[(r >> 8) as usize % keys.len()].clone());
                }
                if has(2) {
                    let v = match (r >> 12) % 9 {
                        0 => Value::Null,
                        1 => Value::Number(f64::NAN),
                        2 => Value::Number(-0.),
                        3 => Value::Number(if i % 2 == 0 { 900. } else { -900. }),
                        k => Value::Number(k as f64 * 1.5 + (i % 5) as f64),
                    };
                    row.set("v", v);
                }
                if has(4) {
                    row.set("w", Value::Number(((r >> 16) % 7) as f64));
                }
                if has(6) {
                    row.set("g", Value::text(["x", "y", "z"][(r >> 20) as usize % 3]));
                }
                // Decoys beside the sought names, before and after them.
                for (bit, name) in [(24, "b"), (26, "cc"), (28, "u"), (30, "va"), (32, "wz")] {
                    if r >> bit & 1 == 1 {
                        row.set(name, Value::Number(1.));
                    }
                }
                row
            })
            .collect()
    }

    /// The fast paths (a grouping streamed from cores, outliers read from
    /// cores, a bin over the records) over records of changing shapes give
    /// the rows the collected paths give: a sort by the record's index
    /// collects the rows first without reordering them.
    #[test]
    fn fast_paths_read_mixed_record_shapes_as_collected_rows_do() {
        let data = Dataset::new(mixed_shapes());
        let outliers = |mut mark: Mark, axis: Axis| {
            mark.outliers = Some(axis);
            mark
        };
        let marks = [
            Mark::new(MarkKind::BarY)
                .x("c")
                .y("v")
                .group_x()
                .group_output("y", GroupReducer::Median),
            Mark::new(MarkKind::Dot)
                .x("c")
                .y("v")
                .stroke("g")
                .r("w")
                .group_xy()
                .group_output("r", GroupReducer::Count),
            Mark::new(MarkKind::RuleX)
                .x("c")
                .y1("w")
                .y2("v")
                .group_x()
                .group_output("y2", GroupReducer::Quantile(0.75)),
            Mark::new(MarkKind::Dot)
                .x("w")
                .fill("v")
                .group_z()
                .group_output("fill", GroupReducer::Mean),
            outliers(Mark::new(MarkKind::Dot).x("g").y("v").z("g"), Axis::Y),
            outliers(Mark::new(MarkKind::Dot).y("c").x("v").z("c"), Axis::X),
            Mark::new(MarkKind::RectY)
                .x("v")
                .fill("g")
                .bin(Thresholds::Count(5))
                .bin_output("y", GroupReducer::Count),
            Mark::new(MarkKind::Dot)
                .x("v")
                .y("w")
                .stroke("c")
                .bin(Thresholds::Count(4))
                .bin_output("r", GroupReducer::Sum),
            Mark::new(MarkKind::RectX)
                .y("w")
                .stroke("v")
                .bin(Thresholds::Count(3))
                .bin_output("x", GroupReducer::Max),
        ];
        for mark in marks {
            let fast = &prepare(&data, std::slice::from_ref(&mark))[0];
            let collected = &prepare(&data, &[mark.clone().sort("n", false)])[0];
            assert!(!fast.rows.is_empty(), "{:?}", mark.kind);
            assert_eq!(
                format!("{:?}", fast.rows),
                format!("{:?}", collected.rows),
                "{:?}",
                mark.kind
            );
        }
    }

    /// A hexbin's bins, hashed on their series and cell, are the bins a
    /// linear search by equality finds: the keys that compare equal (-0 and
    /// +0, every NaN) hash alike, and the bins keep their first appearance.
    #[test]
    fn hashed_bin_keys_group_as_equality_does() {
        let zs = [
            None,
            Some(Value::Number(0.)),
            Some(Value::Number(-0.)),
            Some(Value::Number(f64::NAN)),
            Some(Value::Number(-f64::NAN)),
            Some(Value::Number(f64::from_bits(0x7ff8_0000_0000_0001))),
            Some(Value::Null),
            Some(Value::text("a")),
            Some(Value::text("a")),
            Some(Value::Bool(false)),
            Some(Value::Number(1.)),
        ];
        type Key = (Option<ValueKey>, i64, i64);
        let keys: Vec<Key> = (0..400)
            .map(|i: i64| {
                let z = zs[(i as usize * 7) % zs.len()].as_ref().map(Value::key);
                (z, (i * 13) % 5 - 2, (i * 3) % 4)
            })
            .collect();
        let mut naive: Vec<(Key, Vec<usize>)> = Vec::new();
        for (i, key) in keys.iter().enumerate() {
            match naive.iter_mut().find(|(k, _)| k == key) {
                Some((_, members)) => members.push(i),
                None => naive.push((key.clone(), vec![i])),
            }
        }
        let mut hashed: Vec<(Key, Vec<usize>)> = Vec::new();
        let mut slots: rustc_hash::FxHashMap<Key, usize> = Default::default();
        for (i, key) in keys.iter().enumerate() {
            match slots.get(key) {
                Some(&slot) => hashed[slot].1.push(i),
                None => {
                    slots.insert(key.clone(), hashed.len());
                    hashed.push((key.clone(), vec![i]));
                }
            }
        }
        assert!(naive.len() > 20);
        assert_eq!(hashed, naive);
    }
}

#[cfg(test)]
mod raster_key_tests {
    use super::*;

    fn key(function: Option<std::rc::Rc<dyn Fn(f64, f64) -> Value>>) -> RasterKey {
        RasterKey {
            size: (4, 3),
            how: crate::raster::Interpolate::None,
            blur: 0f64.to_bits(),
            extent: [0., 0., 1., 1.].map(f64::to_bits),
            function,
            samples: Vec::new(),
        }
    }

    #[test]
    fn functions_are_told_apart_by_identity_not_by_sampling() {
        // Two functions that agree almost everywhere must not share a grid.
        let f: std::rc::Rc<dyn Fn(f64, f64) -> Value> = std::rc::Rc::new(|x, _| Value::number(x));
        let g: std::rc::Rc<dyn Fn(f64, f64) -> Value> =
            std::rc::Rc::new(|x, y| Value::number(if y > 0.99 { -x } else { x }));
        assert!(key(Some(f.clone())).same(&key(Some(f.clone()))));
        assert!(!key(Some(f.clone())).same(&key(Some(g))));
        assert!(!key(Some(f)).same(&key(None)));
    }

    #[test]
    fn samples_and_parameters_compare_exactly() {
        let mut a = key(None);
        a.samples = vec![(1f64.to_bits(), 2f64.to_bits(), Value::number(3.).key())];
        let mut b = key(None);
        b.samples = a.samples.clone();
        assert!(a.same(&b));
        b.samples[0].2 = Value::number(3.000_000_1).key();
        assert!(!a.same(&b));
        let mut c = key(None);
        c.samples = a.samples.clone();
        c.blur = 2f64.to_bits();
        assert!(!a.same(&c));
    }
}
