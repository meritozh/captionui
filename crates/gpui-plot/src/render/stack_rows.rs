//! A mark's explicit stack over long or wide data, its `stack_series` or
//! `stack_fields`.

use crate::mark::Mark;
use crate::render::PreparedRow;
use crate::table::{Dataset, Table};
use crate::transform::{self, StackedSeries};
use crate::value::Value;

/// Rewrite every row's y interval from the stacked series.
///
/// Plot stacks two shapes of data: *long* data, where one field names the
/// series (`stack_series("platform")`), and *wide* data, where the mark names
/// one value channel per series (`y1("tablet").y2("desktop")`). Both end in
/// the same series list.
pub(super) fn stack_rows(rows: Vec<PreparedRow>, mark: &Mark) -> Vec<PreparedRow> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::MarkKind;
    use crate::prelude::*;
    use crate::render::prepare;
    use crate::transform::StackOffset;

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
}
