//! The `auto` mark: pick a mark type from the data's shape, Plot's `Plot.auto`.
//!
//! Plot's `auto` chooses the mark that "best represents the given dimensions
//! of the data": two quantitative dimensions scatter, a monotonic dimension
//! against a numeric one lines, one quantitative dimension histograms, one
//! ordinal dimension bars. These heuristics mirror Plot's `autoSpec`, scoped to
//! the cases this library composes most cleanly — a dot, a line, a bar, a
//! histogram or a rule. The finer reducer-driven heatmap and density branches
//! are left to explicit marks, which a caller reaches just as easily.

use gpui::Hsla;

use crate::plot::Plot;
use crate::table::{Dataset, Table};
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
        .color(
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
    let xslots = ((x1 - x0) / xw).round() as usize;
    let yslots = ((y1 - y0) / yw).round() as usize;
    let mut grid = vec![0usize; (xslots + 1) * (yslots + 1)];
    for (px, py) in &points {
        let xi = (((px - x0) / xw).floor() as usize).min(xslots);
        let yi = (((py - y0) / yw).floor() as usize).min(yslots);
        grid[xi + yi * (xslots + 1)] += 1;
    }
    let mut cells = Vec::new();
    for yi in 0..=yslots {
        for xi in 0..=xslots {
            let count = grid[xi + yi * (xslots + 1)];
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
}
