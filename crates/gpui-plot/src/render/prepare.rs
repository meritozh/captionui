//! Marks resolved against their data, facet by facet: `PreparedMark`,
//! `PreparedRow`, and the pipeline that runs a mark's transforms.

use gpui::SharedString;

use crate::facet::Grid;
use crate::mark::{Axis, Channels, MapReducer, Mark};
use crate::render::bin::{bin_records, bin_rows, pin_bin_edges};
use crate::render::bind::bind_outputs;
use crate::render::group::collapse_groups;
use crate::render::map::apply_maps;
use crate::render::normalize::normalize_rows;
use crate::render::raster::dense_raster;
use crate::render::reduce::reduce_rows;
use crate::render::resolve::{
    CoreRow, RowFacts, finish_row, resolve_core, resolve_row, transforms_before_group,
};
use crate::render::select::{outlier_flags, outlier_rows, select_rows};
use crate::render::sort::apply_sort;
use crate::render::stack;
use crate::render::stack_rows::stack_rows;
use crate::render::window::apply_window;
use crate::render::with_value_defaults;
use crate::table::{Dataset, Table};
use crate::transform::{self, Selector};
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
    ///
    /// [`apply_dodge`]: super::apply_dodge
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facet::Grid;
    use crate::mark::MarkKind;
    use crate::transform::StackOffset;

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
