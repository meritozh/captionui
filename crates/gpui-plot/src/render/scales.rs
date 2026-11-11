//! The passes over every facet's prepared marks that put channels through
//! scales: the scale transforms, and the radius, opacity and symbol scales.

use crate::render::resolve::symbol_from_value;
use crate::render::{PreparedMark, PreparedRow, constant_symbol};
use crate::value::Value;

/// Plot's `applyScaleTransforms`: once the marks' own transforms (stack,
/// bin, window …) have run, each channel bound to a scale with a
/// `transform` option has its values mapped through it, before any scale
/// infers its domain. The position channels (`x`, `x1`, `x2` and the `y`
/// family) take the x and y scales' transforms, and the scaled `r` and
/// `opacity` values the `r` and `opacity` scales'; the color scale's
/// transform is applied where its values are read, in
/// [`color::PlotColors`].
///
/// [`color::PlotColors`]: super::color::PlotColors
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
