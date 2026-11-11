//! The order `prepare_mark` composes a mark's transforms in.

use crate::mark::{MapReducer, Mark, MarkKind};
use crate::render::{PreparedRow, prepare};
use crate::table::{Dataset, Table};
use crate::transform::{self, Anchor, Reducer, WindowReducer};
use crate::value::Value;

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
