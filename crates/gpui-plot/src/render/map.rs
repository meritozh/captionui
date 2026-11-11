//! Plot's `map` transform over a mark's records.

use gpui::SharedString;

use crate::mark::{Mark, MarkKind};
use crate::table::{Dataset, Table};
use crate::transform;
use crate::value::Value;

/// Plot's `map` transform over the mark's records: each target channel's
/// values are mapped per `z` series and written under `$map:<channel>`,
/// and the returned mark reads them there.
pub(super) fn apply_maps(source: &Dataset, mark: &Mark) -> (Dataset, Mark) {
    use crate::mark::MapTarget;
    use crate::style::{ColorChannel, NumberChannel};
    use crate::table::Channel;
    let mut mark = mark.clone();
    let mut rows: Vec<Table> = source.rows().to_vec();
    // The series a map restarts at: Plot's `maybeZ`, the `z` channel (which
    // a color field implies).
    let mut series: Vec<Vec<usize>> = Vec::new();
    let mut slots: std::collections::HashMap<Value, usize> = std::collections::HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        let key = mark
            .channels
            .z
            .as_ref()
            .map(|z| z.read(row))
            .unwrap_or(Value::Null);
        let slot = *slots.entry(key).or_insert_with(|| {
            series.push(Vec::new());
            series.len() - 1
        });
        series[slot].push(index);
    }
    let maps = std::mem::take(&mut mark.maps);
    for (target, mapper) in &maps {
        // Each map call is its own transform, so a later one reads what an
        // earlier one wrote, as Plot's nested `windowY(k, mapY(…))` does;
        // within one call, every input is read before any output is written.
        let inputs = mark.channels.clone();
        // `mapX`/`mapY`: the axis's channels the mark has, else the datum.
        let names: Vec<&str> = match target {
            MapTarget::X | MapTarget::Y => {
                let (names, slots) = if *target == MapTarget::X {
                    (["x", "x1", "x2"], [&inputs.x, &inputs.x1, &inputs.x2])
                } else {
                    (["y", "y1", "y2"], [&inputs.y, &inputs.y1, &inputs.y2])
                };
                let present: Vec<&str> = names
                    .iter()
                    .zip(slots)
                    .filter(|(_, slot)| slot.is_some())
                    .map(|(name, _)| *name)
                    .collect();
                if present.is_empty() {
                    vec![names[0]]
                } else {
                    present
                }
            }
            MapTarget::Channel(name) => vec![name.as_ref()],
        };
        for name in names {
            let c = &inputs;
            // Plot's `maybeInput`: an interval end the mark lacks reads its
            // axis's channel.
            let position = |own: &Option<Channel>, axis: &Option<Channel>| {
                own.clone().or_else(|| axis.clone())
            };
            let position = match name {
                "x" => Some(c.x.clone().unwrap_or_else(Channel::identity)),
                "y" => Some(c.y.clone().unwrap_or_else(Channel::identity)),
                "x1" | "x2" => position(if name == "x1" { &c.x1 } else { &c.x2 }, &c.x),
                "y1" | "y2" => position(if name == "y1" { &c.y1 } else { &c.y2 }, &c.y),
                _ => None,
            };
            let color = match name {
                "fill" => c.fill.as_ref(),
                "stroke" => c.stroke.as_ref(),
                _ => None,
            }
            .filter(|ch| matches!(ch, ColorChannel::Field(_) | ColorChannel::Value(_)));
            let number = match name {
                "r" => c.r.as_ref(),
                "opacity" => c.opacity.as_ref(),
                _ => None,
            }
            .filter(|ch| ch.is_scaled());
            let values: Vec<Value> = if let Some(channel) = position {
                rows.iter().map(|row| channel.read(row)).collect()
            } else if let Some(channel) = color {
                rows.iter()
                    .map(|row| channel.scaled_value(row).unwrap_or(Value::Null))
                    .collect()
            } else if let Some(channel) = number {
                rows.iter()
                    .map(|row| channel.data_value(row).map_or(Value::Null, Value::number))
                    .collect()
            } else {
                // Plot throws on a missing channel; a constant has nothing
                // to map.
                continue;
            };
            let field: SharedString = format!("$map:{name}").into();
            let mut output = vec![Value::Null; rows.len()];
            for members in &series {
                let series_values: Vec<Value> =
                    members.iter().map(|&i| values[i].clone()).collect();
                for (&i, value) in members
                    .iter()
                    .zip(transform::map_values(&series_values, mapper))
                {
                    output[i] = value;
                }
            }
            for (row, value) in rows.iter_mut().zip(output) {
                row.set(&field, value);
            }
            let c = &mut mark.channels;
            match name {
                "x" => c.x = Some(Channel::Field(field)),
                "x1" => c.x1 = Some(Channel::Field(field)),
                "x2" => c.x2 = Some(Channel::Field(field)),
                "y" => c.y = Some(Channel::Field(field)),
                "y1" => c.y1 = Some(Channel::Field(field)),
                "y2" => c.y2 = Some(Channel::Field(field)),
                "fill" => c.fill = Some(ColorChannel::Field(field)),
                "stroke" => c.stroke = Some(ColorChannel::Field(field)),
                "r" => c.r = Some(NumberChannel::Field(field)),
                _ => c.opacity = Some(NumberChannel::Field(field)),
            }
        }
    }
    // Plot's `maybeStackY`/`maybeStackX`: an interval mark given an
    // interval end draws `y1`–`y2` (the missing end from `y`) and drops `y`,
    // so `areaY(map({y1, y2}, {y}))` draws the band alone.
    let c = &mut mark.channels;
    match mark.kind {
        MarkKind::BarY | MarkKind::AreaY | MarkKind::RectY | MarkKind::WaffleY
            if c.y.is_some() && (c.y1.is_some() || c.y2.is_some()) =>
        {
            let y = c.y.take();
            c.y1 = c.y1.take().or_else(|| y.clone());
            c.y2 = c.y2.take().or(y);
        }
        MarkKind::BarX | MarkKind::AreaX | MarkKind::RectX | MarkKind::WaffleX
            if c.x.is_some() && (c.x1.is_some() || c.x2.is_some()) =>
        {
            let x = c.x.take();
            c.x1 = c.x1.take().or_else(|| x.clone());
            c.x2 = c.x2.take().or(x);
        }
        _ => {}
    }
    (Dataset::new(rows), mark)
}
