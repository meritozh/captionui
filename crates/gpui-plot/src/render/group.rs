//! Plot's group transform: the rows sharing a category collapsed into one.

use crate::mark::{Axis, Channels, Mark};
use crate::render::PreparedRow;
use crate::render::bind::{bound_input, bound_slot};
use crate::render::interval::derive_intervals;
use crate::render::resolve::GroupRow;
use crate::table::Table;
use crate::transform;
use crate::value::Value;

/// The value a grouped output reduces, read from one member of the group.
///
/// Plot reads the channel the output writes unless the output names another
/// input, and `y1`/`y2` fall back to `y` when the mark names no interval —
/// Plot's `maybeInput` — so `{y1: "min"}` is the smallest y value even though
/// a bar's interval has already been grown from zero by the time it groups.
pub(super) fn group_input(row: &impl GroupRow, mark: &Mark, name: &str) -> Value {
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
///
/// [`CoreRow`]: super::resolve::CoreRow
pub(super) fn collapse_groups<R: GroupRow>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::MarkKind;
    use crate::render::prepare;
    use crate::table::Dataset;

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
