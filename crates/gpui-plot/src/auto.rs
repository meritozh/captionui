//! The `auto` mark: pick a mark type from the data's shape, Plot's `Plot.auto`.
//!
//! Plot's `auto` chooses the mark that "best represents the given dimensions
//! of the data": two quantitative dimensions scatter, a monotonic dimension
//! against a numeric one lines, one quantitative dimension histograms, one
//! ordinal dimension bars. These heuristics mirror Plot's `autoSpec`, scoped to
//! the cases this library composes most cleanly — a dot, a line, a bar, a
//! histogram or a rule. The finer reducer-driven heatmap and density branches
//! are left to explicit marks, which a caller reaches just as easily.

use crate::plot::Plot;
use crate::table::{Dataset, Table};
use crate::value::PlotKind;

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
        // x ordinal, y quantitative → vertical bars.
        (Some(_), Some(yk)) if yk.is_quantitative() => plot.bar_y(|mark| {
            let mark = mark.x(x.unwrap()).y(y.unwrap());
            if let Some(field) = &color {
                mark.z(field.as_str())
            } else {
                mark
            }
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
    values.windows(2).all(|pair| pair[0] < pair[1])
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
}
