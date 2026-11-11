//! Plot's box mark: the whisker reducers, and `expand_box`, which draws a box as a
//! rule, a bar, a tick and the outlier dots.

use crate::mark::{Axis, Mark, MarkKind, TransformOptions};
use crate::style::ColorChannel;
use crate::transform::GroupReducer;

/// The quartiles of the numbers among `values`, d3's R-7 quantiles.
fn quartiles(
    values: &[crate::value::Value],
    order: &mut crate::transform::OrderStatistics,
) -> Option<(f64, f64)> {
    Some((order.quantile(values, 0.25)?, order.quantile(values, 0.75)?))
}

/// Plot's `loqr1`: the least value within 1.5 IQR below the first quartile.
pub fn whisker_low(values: &[crate::value::Value]) -> crate::value::Value {
    whisker_low_with(values, &mut Default::default())
}

/// [`whisker_low`], reading the quartiles through `order`, which only ever
/// reads these `values`.
pub(crate) fn whisker_low_with(
    values: &[crate::value::Value],
    order: &mut crate::transform::OrderStatistics,
) -> crate::value::Value {
    let Some((q1, q3)) = quartiles(values, order) else {
        return crate::value::Value::Null;
    };
    let lo = q1 * 2.5 - q3 * 1.5;
    values
        .iter()
        .filter_map(crate::value::Value::as_number)
        .filter(|v| *v >= lo)
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
        .map(crate::value::Value::Number)
        .unwrap_or(crate::value::Value::Null)
}

/// Plot's `hiqr2`: the greatest value within 1.5 IQR above the third.
pub fn whisker_high(values: &[crate::value::Value]) -> crate::value::Value {
    whisker_high_with(values, &mut Default::default())
}

/// [`whisker_high`], reading the quartiles through `order`, which only ever
/// reads these `values`.
pub(crate) fn whisker_high_with(
    values: &[crate::value::Value],
    order: &mut crate::transform::OrderStatistics,
) -> crate::value::Value {
    let Some((q1, q3)) = quartiles(values, order) else {
        return crate::value::Value::Null;
    };
    let hi = q3 * 2.5 - q1 * 1.5;
    values
        .iter()
        .filter_map(crate::value::Value::as_number)
        .filter(|v| *v <= hi)
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
        .map(crate::value::Value::Number)
        .unwrap_or(crate::value::Value::Null)
}

/// Plot's `boxX`/`boxY` as the marks it composes: a rule from whisker to
/// whisker, a bar across the interquartile range filled `#ccc`, a two-pixel
/// tick at the median, and the outliers as dots. Each groups by the category
/// channel (the other axis), or as one group without one.
pub fn expand_box(mark: Mark) -> Vec<Mark> {
    let vertical = mark.kind == MarkKind::BoxY;
    let category = if vertical {
        mark.channels.x.clone()
    } else {
        mark.channels.y.clone()
    };
    let value = if vertical {
        mark.channels.y.clone()
    } else {
        mark.channels.x.clone()
    };
    let value = value.or_else(|| Some(crate::table::Channel::identity()));
    let base = |kind: MarkKind| {
        let mut m = Mark::new(kind);
        m.data = mark.data.clone();
        m.fx = mark.fx.clone();
        m.fy = mark.fy.clone();
        m.filter = mark.filter.clone();
        // Plot's box passes its missing category on as an explicit
        // `null`, so no part reads its values on the category axis.
        let across = category
            .clone()
            .or(Some(crate::table::Channel::Constant(crate::value::Value::Null)));
        if vertical {
            m.channels.x = across;
            m.channels.y = value.clone();
        } else {
            m.channels.y = across;
            m.channels.x = value.clone();
        }
        m.channels.z = category.clone();
        m
    };
    let group = |m: Mark| {
        if category.is_some() {
            if vertical { m.group_x() } else { m.group_y() }
        } else {
            m.group_z()
        }
    };
    let (lo, hi, q1, q3, mid) = if vertical {
        ("y1", "y2", "y1", "y2", "y")
    } else {
        ("x1", "x2", "x1", "x2", "x")
    };
    let input = if vertical { "y" } else { "x" };
    let stroke = mark.channels.stroke.clone();
    let mut rule = group(base(if vertical {
        MarkKind::RuleX
    } else {
        MarkKind::RuleY
    }))
    .group_output(lo, GroupReducer::WhiskerLow)
    .group_output(hi, GroupReducer::WhiskerHigh);
    let mut bar = group(base(if vertical {
        MarkKind::BarY
    } else {
        MarkKind::BarX
    }))
    .group_output(q1, GroupReducer::Quantile(0.25))
    .group_output(q3, GroupReducer::Quantile(0.75));
    let mut tick = group(base(if vertical {
        MarkKind::TickY
    } else {
        MarkKind::TickX
    }))
    .group_output(mid, GroupReducer::Quantile(0.5));
    for m in [&mut rule, &mut bar, &mut tick] {
        if let Some(config) = m.group.as_mut() {
            for output in config.outputs.iter_mut() {
                output.input = Some(input.into());
            }
        }
    }
    bar.channels.fill =
        mark.channels
            .fill
            .clone()
            .or(Some(ColorChannel::Constant(crate::scale::hsla_from_hex(
                0xcccccc,
            ))));
    bar.style.fill_opacity = mark.style.fill_opacity;
    for m in [&mut rule, &mut tick] {
        m.channels.stroke = stroke.clone();
    }
    tick.style.stroke_width = 2.;
    let mut dots = base(MarkKind::Dot);
    dots.outliers = Some(if vertical { Axis::Y } else { Axis::X });
    dots.channels.stroke = stroke;
    dots.channels.r = mark.channels.r.clone();
    // The group reducers read the value channel itself: each part is a
    // fresh mark whose value axis no transform rewrites before grouping.
    vec![rule, bar, tick, dots]
}
