//! Hit-testing for the pointer: the rows it can land on, an interval
//! mark's body, and which marks are hoverable at all.

use gpui::{Bounds, Pixels, point, px};

use crate::frame::Frame;
use crate::mark::MarkKind;
use crate::render::PreparedMark;

/// Every `(facet, mark, row)` a pointer can land on, in the order
/// `tooltip_state` searches them. Both `tooltip_state` and `tooltip` build
/// this list, so the state's index means the same thing to each.
///
/// A facet draws its marks in the order the marks were declared, and the
/// facets come in the grid's own order, which is the order `paint` draws in.
pub(super) fn hover_targets(facets: &[Vec<PreparedMark>]) -> Vec<(usize, usize, usize)> {
    facets
        .iter()
        .enumerate()
        .flat_map(|(cell, prepared)| {
            prepared
                .iter()
                .enumerate()
                .flat_map(move |(mark_index, mark)| {
                    (0..mark.rows.len()).map(move |row_index| (cell, mark_index, row_index))
                })
        })
        .collect()
}

/// The pixel rectangle a row's body spans, so the pointer can hit an interval
/// mark — a bar, rect or cell — across its whole extent, not just its anchor.
///
/// Both ends of each interval are projected; a mark with no x interval uses its
/// x value, and likewise y, falling back to no rect when a channel does not
/// scale. The rectangle is in the facet frame's plot-local pixels.
pub(super) fn interval_rect(
    mark: &PreparedMark,
    row: &crate::render::PreparedRow,
    frame: &Frame,
) -> Option<Bounds<Pixels>> {
    let cat = mark.mark.kind.category();
    // Only a mark that paints a body has one to hover: a row with an interval
    // (a bar's length, a rect's extent) or a band-filling mark (a cell). A dot
    // or a line vertex is hit by its point instead, so a padded rectangle
    // around it cannot steal the hover from a nearer datum.
    let has_interval = row.x1.is_some() || row.x2.is_some() || row.y1.is_some() || row.y2.is_some();
    if !(has_interval || (cat.band_x && cat.band_y)) || cat.furniture {
        return None;
    }
    let x = |v: &crate::value::Value| frame.x.scale.position(v);
    let y = |v: &crate::value::Value| frame.y.scale.position(v);
    let x0 = row.x1.as_ref().or(Some(&row.x)).and_then(x);
    let x1 = row.x2.as_ref().or(Some(&row.x)).and_then(x);
    let y0 = row.y1.as_ref().or(Some(&row.y)).and_then(y);
    let y1 = row.y2.as_ref().or(Some(&row.y)).and_then(y);
    let (Some(x0), Some(x1), Some(y0), Some(y1)) = (x0, x1, y0, y1) else {
        return None;
    };
    let (mut x0, mut x1) = (x0.min(x1), x0.max(x1));
    let (mut y0, mut y1) = (y0.min(y1), y0.max(y1));
    // A length mark spans a single value on its other axis (a vertical bar one
    // x, a horizontal bar one y). On a band axis the body is the band's width,
    // centred on that value; elsewhere it is padded a little so it is still
    // hoverable.
    let widen = |lo: &mut f32, hi: &mut f32, band: f32| {
        if *hi - *lo <= 1. {
            let half = if band > 1. { band / 2. } else { 6. };
            let centre = (*lo + *hi) / 2.;
            *lo = centre - half;
            *hi = centre + half;
        }
    };
    widen(&mut x0, &mut x1, frame.x.scale.band_width());
    widen(&mut y0, &mut y1, frame.y.scale.band_width());
    Some(Bounds::new(
        point(px(x0), px(y0)),
        gpui::size(px(x1 - x0), px(y1 - y0)),
    ))
}

/// Whether the pointer can hover a mark's rows.
///
/// A pie, arc or waffle lays its rows out in polar or grid space around the
/// plot's center, not at their x/y positions, so a Cartesian hit-test would
/// land on meaningless spots; they are not hoverable rather than wrong.
pub(super) fn hoverable(mark: &PreparedMark) -> bool {
    !matches!(
        mark.mark.kind,
        MarkKind::Pie | MarkKind::Arc | MarkKind::Waffle
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::ScaleKind;
    use crate::plot::Plot;
    use crate::plot::domain::marks_use_band;
    use crate::render;
    use crate::table::Table;
    use crate::value::Value;

    /// A frame laid out over a plot's own marks, without a window.
    fn frame_for(plot: &Plot, facets: &[Vec<PreparedMark>]) -> Frame {
        let mut options = plot.frame_options.clone();
        if marks_use_band(facets, true) {
            options.x_kind = Some(ScaleKind::Band);
        }
        Frame::resolve(
            Bounds::new(point(px(0.), px(0.)), gpui::size(px(400.), px(200.))),
            &options,
            &plot.x_domain_values(facets),
            &plot.y_domain_values(facets),
        )
    }

    #[test]
    fn one_dimensional_marks_have_a_hover_point() {
        // A barcode's rows have no y; the hit-test must place them where the
        // painter does (mid-height) instead of dropping them.
        let data = [1., 2., 3.].map(|v| Table::empty().with("v", Value::Number(v)));
        let plot = Plot::barcode(data, "v");
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let frame = frame_for(&plot, &facets);
        let mark = &facets[0][0];
        for row in &mark.rows {
            assert!(render::basic::position_in(mark, &frame, row).is_some());
        }
    }

    #[test]
    fn a_row_missing_its_channel_value_is_not_placed() {
        // d3.autoType reads "NaN" as missing; Plot filters the row out
        // rather than drawing it at the frame's middle.
        let data = [Value::Number(1.), Value::Null, Value::Number(3.)]
            .map(|y| Table::empty().with("x", Value::Number(1.)).with("y", y));
        let plot = Plot::plot().mark(crate::marks::dot(data.to_vec()).x("x").y("y"));
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let frame = frame_for(&plot, &facets);
        let mark = &facets[0][0];
        let placed: Vec<bool> = mark
            .rows
            .iter()
            .map(|row| render::basic::position_in(mark, &frame, row).is_some())
            .collect();
        assert_eq!(placed, [true, false, true]);
    }

    #[test]
    fn a_band_bar_is_hoverable_across_its_width() {
        let data = ["a", "b", "c"].map(|x| {
            Table::empty()
                .with("x", Value::text(x))
                .with("y", Value::Number(4.))
        });
        let plot = Plot::new(data).bar_y(|mark| mark.x("x").y("y"));
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let frame = frame_for(&plot, &facets);
        let mark = &facets[0][0];
        let rect = interval_rect(mark, &mark.rows[0], &frame).expect("a bar has a body");
        let band = frame.x.scale.band_width();
        assert!(band > 12.);
        assert!((rect.size.width.as_f32() - band).abs() < 0.5);
    }

    #[test]
    fn a_dot_has_no_hover_body() {
        let data = [Table::empty()
            .with("x", Value::Number(1.))
            .with("y", Value::Number(1.))];
        let plot = Plot::new(data).dot(|mark| mark.x("x").y("y"));
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let frame = frame_for(&plot, &facets);
        let mark = &facets[0][0];
        assert!(interval_rect(mark, &mark.rows[0], &frame).is_none());
    }
}
