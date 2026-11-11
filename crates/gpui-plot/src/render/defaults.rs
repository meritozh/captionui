//! The channels a mark over plain values or tuples reads by default.

use crate::mark::{Mark, MarkKind};
use crate::table::Dataset;

/// A mark over plain values or tuples reads them by default, as Plot's marks
/// do: `Plot.ruleY([0])` puts `y` on the value, `Plot.dotX(numbers)` puts
/// `x` on it, and `Plot.line([[0, 1], [1, 2]])` reads `x` and `y` from each
/// pair.
pub fn with_value_defaults(mark: &Mark, data: &Dataset) -> Option<Mark> {
    use crate::table::Channel;
    let channels = &mark.channels;
    let unset_x = channels.x.is_none() && channels.x1.is_none() && channels.x2.is_none();
    let unset_y = channels.y.is_none() && channels.y1.is_none() && channels.y2.is_none();
    let binned_or_grouped = mark.bin.is_some() || mark.group.is_some();
    if data.is_values() {
        let mut mark = mark.clone();
        let set_x = |m: &mut Mark, c: Channel| {
            if unset_x {
                m.channels.x = Some(c)
            }
        };
        let set_y = |m: &mut Mark, c: Channel| {
            if unset_y {
                m.channels.y = Some(c)
            }
        };
        use MarkKind::*;
        match mark.kind {
            // One-dimensional marks read the value on their axis.
            DotX | TextX | TickX | RuleX | GridX | AxisX | BoxX | DensityX | ViolinX | VectorX => {
                set_x(&mut mark, Channel::identity())
            }
            DotY | TextY | TickY | RuleY | GridY | AxisY | BoxY | DensityY | ViolinY | VectorY => {
                set_y(&mut mark, Channel::identity())
            }
            // Plot's `areaY`, `lineY`, `rectY` and `barY` over values: the
            // value on y and, unbinned, the index on x (`rectY` and `areaY`
            // stack). Binned or grouped, the value is the binned x.
            AreaY | LineY | BarY | RectY | WaffleY => {
                if binned_or_grouped {
                    set_x(&mut mark, Channel::identity());
                } else {
                    set_y(&mut mark, Channel::identity());
                    set_x(&mut mark, Channel::index());
                    // Plot's `rectY(values)`: each value spans its index's
                    // unit interval.
                    if mark.kind == RectY && unset_x && mark.interval.is_none() {
                        mark.interval = Some(crate::mark::Interval::Step(1.));
                    }
                }
            }
            AreaX | LineX | BarX | RectX | WaffleX => {
                if binned_or_grouped {
                    set_y(&mut mark, Channel::identity());
                } else {
                    set_x(&mut mark, Channel::identity());
                    set_y(&mut mark, Channel::index());
                    if mark.kind == RectX && unset_y && mark.interval.is_none() {
                        mark.interval = Some(crate::mark::Interval::Step(1.));
                    }
                }
            }
            // `cellX(values)`: x the index, fill the value; `cellY` likewise.
            CellX => {
                set_x(&mut mark, Channel::index());
                if mark.channels.fill.is_none() {
                    mark.channels.fill = Some(crate::style::ColorChannel::identity());
                }
            }
            CellY => {
                set_y(&mut mark, Channel::index());
                if mark.channels.fill.is_none() {
                    mark.channels.fill = Some(crate::style::ColorChannel::identity());
                }
            }
            _ => return None,
        }
        return Some(mark);
    }
    if data.is_tuples() {
        let mut mark = mark.clone();
        if unset_x {
            mark.channels.x = Some(Channel::Field("0".into()));
        }
        if unset_y {
            mark.channels.y = Some(Channel::Field("1".into()));
        }
        // Plot's text over tuples labels each with its index.
        if matches!(mark.kind, MarkKind::Text) && mark.channels.text.is_none() {
            mark.channels.text = Some(Channel::index());
        }
        return Some(mark);
    }
    None
}
