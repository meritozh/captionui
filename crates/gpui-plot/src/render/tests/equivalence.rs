//! The pipeline's fast paths, each checked against a naive reading of the
//! raw records.

use crate::facet::Grid;
use crate::mark::{Axis, GroupOutput, Mark, MarkKind, SortConfig};
use crate::prelude::*;
use crate::render::bin::bin_rows;
use crate::render::group::collapse_groups;
use crate::render::interval::derive_intervals;
use crate::render::resolve::{RowFacts, derives_intervals, resolve_row};
use crate::render::{PreparedMark, PreparedRow, prepare, prepare_facets};
use crate::style::ColorChannel;
use crate::table::{Channel, Dataset, Table};
use crate::transform::{Anchor, GroupReducer, Thresholds};
use crate::value::{Value, ValueKey};
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
                GroupReducer::Proportion => Value::Number(members.len() as f64 / rows.len() as f64),
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
        bin(Mark::new(MarkKind::Rect).x("n").y("v").z("g")).bin_output("fill", GroupReducer::Count),
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
        bin(Mark::new(MarkKind::Rect).x("n").y("v").z("g")).bin_output("fill", GroupReducer::Count),
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
