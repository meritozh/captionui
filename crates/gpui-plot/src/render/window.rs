//! Plot's `window` transform over a mark's resolved rows.

use crate::mark::{Axis, Mark};
use crate::render::PreparedRow;
use crate::transform;
use crate::value::Value;

pub(super) fn apply_window(mark: &Mark, rows: &mut [PreparedRow]) {
    let Some((k, reducer, anchor)) = mark.window else {
        return;
    };
    // Plot's `windowX` maps x, `windowY` y: the axis an X mark varies on,
    // else y.
    let on_x = mark.kind.category().single_axis == Some(Axis::X);
    let read = |row: &PreparedRow| -> Option<f64> {
        if on_x { row.x.as_number() } else { row.y.as_number() }
    };
    // A window restarts with each `z` series, as Plot's map groups by z.
    let mut series: Vec<(Option<crate::value::ValueKey>, Vec<usize>)> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let key = row.z.as_ref().map(Value::key);
        match series.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push(index),
            None => series.push((key, vec![index])),
        }
    }
    // An interval end grown from the value (a bar's or area's) follows it.
    let derived = if on_x {
        mark.channels.x1.is_none() && mark.channels.x2.is_none()
    } else {
        mark.channels.y1.is_none() && mark.channels.y2.is_none()
    };
    for (_, members) in series {
        let values: Vec<Option<f64>> = members.iter().map(|&i| read(&rows[i])).collect();
        let mapped = transform::window_values(&values, k, reducer, anchor, mark.window_strict);
        for (&i, value) in members.iter().zip(mapped) {
            let row = &mut rows[i];
            let value = value.map(Value::Number).unwrap_or(Value::Null);
            let (slot, end) = if on_x {
                (&mut row.x, &mut row.x2)
            } else {
                (&mut row.y, &mut row.y2)
            };
            if derived && end.as_ref() == Some(slot) {
                *end = (!value.is_null()).then(|| value.clone());
            }
            *slot = value;
        }
    }
}
