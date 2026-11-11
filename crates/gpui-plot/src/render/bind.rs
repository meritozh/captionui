//! A group's or bin's outputs bound to the mark's channels, and the
//! `$in:` inputs through which an output reads the mark's own channel.

use gpui::SharedString;

use crate::mark::{Channels, Mark};
use crate::table::Table;
use crate::value::Value;

/// The channel names a transform output can read from the mark itself.
const BOUND: [&str; 4] = ["fill", "stroke", "text", "title"];

/// The [`BOUND`] channel an input names as `$in:<name>`, the input
/// [`bind_outputs`] gives an output that reads the mark's own channel.
pub(super) fn bound_slot(input: &str) -> Option<usize> {
    let name = input.strip_prefix("$in:")?;
    BOUND.iter().position(|bound| *bound == name)
}

/// The value a `$in:<name>` input reads from `record`: the original mark's
/// channel of that name (its slot in [`BOUND`], from [`bound_slot`]), else
/// the record's own field of the input's name.
pub(super) fn bound_input(channels: &Channels, slot: usize, input: &str, record: &Table) -> Value {
    let value = match slot {
        0 => channels
            .fill
            .as_ref()
            .and_then(|ch| ch.scaled_value(record)),
        1 => channels
            .stroke
            .as_ref()
            .and_then(|ch| ch.scaled_value(record)),
        2 => channels.text.as_ref().map(|ch| ch.read(record)),
        3 => channels.title.as_ref().map(|ch| ch.read(record)),
        _ => None,
    };
    value.unwrap_or_else(|| record.value(input))
}

/// The mark as its transforms leave it: a group or bin output to `fill`,
/// `stroke`, `text`, `title` (and so on) becomes that channel, read from the
/// output's field, as Plot binds a transform's outputs to the mark.
pub(super) fn bind_outputs(mark: &Mark) -> Option<Mark> {
    let outputs: Vec<SharedString> = mark
        .group
        .iter()
        .flat_map(|g| g.outputs.iter().map(|o| o.name.clone()))
        .chain(
            mark.bin
                .iter()
                .flat_map(|b| b.outputs.iter().map(|o| o.name.clone())),
        )
        .collect();
    let default_fill = mark
        .group
        .as_ref()
        .is_some_and(|g| g.outputs.is_empty() && (g.both || g.axis.is_none()));
    if outputs.is_empty() && !default_fill {
        return None;
    }
    let mut mark = mark.clone();
    // Outputs with no input read the mark's channel of their name, through
    // [`bound_input`].
    let has = |name: &str| match name {
        "fill" => mark
            .channels
            .fill
            .as_ref()
            .is_some_and(|c| c.scaled_value(&Table::empty()).is_some()),
        "stroke" => mark
            .channels
            .stroke
            .as_ref()
            .is_some_and(|c| c.scaled_value(&Table::empty()).is_some()),
        "text" => mark.channels.text.is_some(),
        "title" => mark.channels.title.is_some(),
        _ => false,
    };
    let inputs: Vec<String> = BOUND
        .iter()
        .filter(|n| has(n))
        .map(|n| n.to_string())
        .collect();
    for config in mark
        .group
        .iter_mut()
        .map(|g| &mut g.outputs)
        .chain(mark.bin.iter_mut().map(|b| &mut b.outputs))
    {
        for output in config.iter_mut() {
            if output.input.is_none() && inputs.iter().any(|n| *n == output.name.as_ref()) {
                output.input = Some(format!("$in:{}", output.name).into());
            }
        }
    }
    // Plot's `maybeSubgroup`: groups subdivide by the first of `z`, `fill`
    // and `stroke` that is a channel and not an output. A `z` a color
    // implied goes when an output replaces that color, and the next color
    // channel left as an input implies it instead.
    if mark.implied_z {
        use crate::{style::ColorChannel, table::Channel};
        let implies = |color: &Option<ColorChannel>| match (color, &mark.channels.z) {
            (Some(ColorChannel::Field(a)), Some(Channel::Field(b))) => a == b,
            (Some(ColorChannel::Value(a)), Some(Channel::Accessor(b))) => std::rc::Rc::ptr_eq(a, b),
            _ => false,
        };
        let is_output = |name: &str| {
            outputs.iter().any(|n| n.as_ref() == name) || (default_fill && name == "fill")
        };
        let colors = [
            ("fill", mark.channels.fill.clone()),
            ("stroke", mark.channels.stroke.clone()),
        ];
        if colors.iter().any(|(name, c)| is_output(name) && implies(c)) {
            let mut next = colors.iter().filter(|(name, _)| !is_output(name));
            let z = next.find_map(|(_, c)| match c {
                Some(ColorChannel::Field(f)) => Some(Channel::Field(f.clone())),
                Some(ColorChannel::Value(a)) => Some(Channel::Accessor(a.clone())),
                _ => None,
            });
            mark.channels.z = z;
            mark.implied_z = mark.channels.z.is_some();
        }
    }
    for name in outputs
        .iter()
        .map(|n| n.as_ref())
        .chain(default_fill.then_some("fill"))
    {
        match name {
            "fill" => mark.channels.fill = Some(crate::style::ColorChannel::Field("fill".into())),
            "stroke" => {
                mark.channels.stroke = Some(crate::style::ColorChannel::Field("stroke".into()))
            }
            "text" => mark.channels.text = Some(crate::table::Channel::Field("text".into())),
            "title" => mark.channels.title = Some(crate::table::Channel::Field("title".into())),
            _ => {}
        }
    }
    Some(mark)
}
