//! Plot's `hexbin` transform, run in pixels once the frame is known.

use crate::frame::Frame;
use crate::mark::MarkKind;
use crate::render::{PreparedMark, PreparedRow};
use crate::table::Table;
use crate::value::Value;

/// Plot's `hexbin` transform, run once the frame is known: each mark's
/// rows are placed in pixels, binned into pointy-top hexagons `binWidth`
/// across (offset half a pixel, as Plot's are), per `z` series, and each bin
/// becomes a row at its centre carrying its outputs.
pub fn apply_hexbin(facets: &mut [Vec<PreparedMark>], frame: &Frame) {
    for prepared in facets.iter_mut().flatten() {
        let Some((outputs, width)) = prepared.mark.hexbin.clone() else {
            continue;
        };
        let dx = width as f64;
        let dy = dx * 1.5 / 3f64.sqrt();
        type BinKey = (Option<crate::value::ValueKey>, i64, i64);
        // The bins by first appearance, each with its rows in row order.
        let mut bins: Vec<(BinKey, Vec<usize>)> = Vec::new();
        let mut slots: rustc_hash::FxHashMap<BinKey, usize> = Default::default();
        for (index, row) in prepared.rows.iter().enumerate() {
            let Some(p) = crate::render::basic::position_in(prepared, frame, row) else {
                continue;
            };
            let (px, py) = (p.x.as_f32() as f64, p.y.as_f32() as f64);
            // d3-hexbin's rounding, as Plot's `hbin`.
            let mut py = py / dy;
            let mut pj = py.round();
            let mut px = (px - 0.5) / dx - ((pj as i64) & 1) as f64 / 2.;
            let mut pi = px.round();
            let py1 = py - pj;
            if py1.abs() * 3. > 1. {
                let px1 = px - pi;
                let pi2 = pi + if px < pi { -0.5 } else { 0.5 };
                let pj2 = pj + if py < pj { -1. } else { 1. };
                let px2 = px - pi2;
                let py2 = py - pj2;
                if px1 * px1 + py1 * py1 > px2 * px2 + py2 * py2 {
                    pi = pi2 + if (pj as i64) & 1 == 1 { 0.5 } else { -0.5 };
                    pj = pj2;
                }
            }
            let _ = (&mut px, &mut py);
            let key = (row.z.as_ref().map(Value::key), (pi * 2.) as i64, pj as i64);
            match slots.get(&key) {
                Some(&slot) => bins[slot].1.push(index),
                None => {
                    slots.insert(key.clone(), bins.len());
                    bins.push((key, vec![index]));
                }
            }
        }
        let mark = prepared.mark.clone();
        let rows: Vec<PreparedRow> = bins
            .into_iter()
            .map(|((_, pi2, pj), members)| {
                let (pi, pj) = (pi2 as f64 / 2., pj as f64);
                let cx = (pi + ((pj as i64) & 1) as f64 / 2.) * dx + 0.5;
                let cy = pj * dy;
                let mut row = prepared.rows[members[0]].clone();
                // The bin's size and opacity come from its outputs alone.
                row.r = None;
                row.r_value = None;
                row.opacity = None;
                row.opacity_value = None;
                let records: Vec<&Table> = members.iter().map(|i| &prepared.rows[*i].row).collect();
                for output in &outputs {
                    let channel_value = |r: &PreparedRow| -> Value {
                        match output.name.as_ref() {
                            "fill" => mark
                                .channels
                                .fill
                                .as_ref()
                                .and_then(|c| c.scaled_value(&r.row))
                                .unwrap_or(Value::Null),
                            "stroke" => mark
                                .channels
                                .stroke
                                .as_ref()
                                .and_then(|c| c.scaled_value(&r.row))
                                .unwrap_or(Value::Null),
                            "x" => r.x_value().clone(),
                            "y" => r.y_value().clone(),
                            name => r.row.value(name),
                        }
                    };
                    let values: Vec<Value> = members
                        .iter()
                        .map(|i| match &output.input {
                            Some(field) => prepared.rows[*i].row.value(field),
                            None => channel_value(&prepared.rows[*i]),
                        })
                        .collect();
                    let reduced = output.reducer.reduce(&values, &records);
                    match output.name.as_ref() {
                        "r" => row.r_value = reduced.as_number(),
                        "fillOpacity" | "opacity" => row.opacity_value = reduced.as_number(),
                        _ => {}
                    }
                    row.row = row
                        .row
                        .clone()
                        .with(format!("$hexbin:{}", output.name), reduced);
                }
                row.pixel = Some((cx as f32, cy as f32));
                row
            })
            .collect();
        // The outputs bind to the mark's channels.
        let mut mark = prepared.mark.clone();
        for output in &outputs {
            let field = || format!("$hexbin:{}", output.name);
            match output.name.as_ref() {
                "fill" => {
                    mark.channels.fill = Some(crate::style::ColorChannel::Field(field().into()))
                }
                "stroke" => {
                    mark.channels.stroke = Some(crate::style::ColorChannel::Field(field().into()))
                }
                "text" => mark.channels.text = Some(crate::table::Channel::Field(field().into())),
                "title" => mark.channels.title = Some(crate::table::Channel::Field(field().into())),
                _ => {}
            }
        }
        // A dot through a hexbin is a hexagon, its radius half the bin width
        // unless an output sizes it.
        if mark.kind == MarkKind::Dot {
            if mark.channels.symbol.is_none() {
                mark.style.symbol = crate::style::Symbol::Hexagon;
            }
            if !outputs.iter().any(|o| o.name == "r") && mark.channels.r.is_none() {
                mark.style.radius = width / 2.;
            }
            // Plot's dot fills when a fill output or channel is given.
        }
        prepared.mark = mark;
        prepared.rows = rows;
    }
}
