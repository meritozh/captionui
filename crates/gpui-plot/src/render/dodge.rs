//! Plot's `dodgeX` and `dodgeY`, packed in pixels once the frame is known.

use crate::frame::Frame;
use crate::render::PreparedMark;
use crate::transform;

/// Pack a dodging mark's rows into a beeswarm, Plot's `dodgeX`/`dodgeY`.
///
/// The transform works in pixels, so it runs after the frame has resolved the
/// scales: each row's position on the axis the data varies on becomes an
/// offset across the other axis, which the mark then reads instead of its
/// own cross-axis value.
pub fn apply_dodge(prepared: &mut [PreparedMark], frame: &Frame) {
    for mark in prepared.iter_mut() {
        let Some(config) = mark.mark.dodge else {
            continue;
        };
        // The dodge packs along the axis the mark's data varies on, and writes
        // the offset across it: a `dotX` varies on x and spreads vertically,
        // a `dotY` varies on y and spreads horizontally.
        // A mark with only an x (Plot's `dodgeY` on an image or dot) varies
        // on x too.
        let along_x = mark.mark.kind.is_x_only()
            || (mark.mark.channels.x.is_some() && mark.mark.channels.y.is_none());
        // The positions come from the axis the data varies on; the packed
        // offsets land on the *other* axis, so the extent is that one's.
        let (scale, extent) = if along_x {
            (&frame.x.scale, frame.plot.size.height.as_f32())
        } else {
            (&frame.y.scale, frame.plot.size.width.as_f32())
        };
        // Plot's dodge with an `r` channel sorts the dots by descending
        // radius, unless the mark sorts them itself, so the big ones pack
        // first (and draw beneath the small ones).
        let varying = mark.rows.iter().any(|row| row.r.is_some());
        if varying && mark.mark.sort.is_none() {
            mark.rows
                .sort_by(|a, b| b.r.unwrap_or(0.).total_cmp(&a.r.unwrap_or(0.)));
        }
        let positions: Vec<Option<f32>> = mark
            .rows
            .iter()
            .map(|row| {
                if along_x {
                    scale.position(row.x_value())
                } else {
                    scale.position(row.y_value())
                }
            })
            .collect();
        // Each dot's own radius, else the dodge's `r`, else the mark's.
        let constant = if config.radius != transform::Dodge::default().radius {
            config.radius
        } else {
            mark.mark.style.radius
        };
        let radii: Vec<f32> = mark.rows.iter().map(|row| row.r.unwrap_or(constant)).collect();
        let order: Vec<usize> = (0..positions.len()).collect();
        let packed = transform::dodge_positions(
            &positions,
            &radii,
            &order,
            config.anchor,
            config.padding,
            [0., extent],
        );
        for (row, packed) in mark.rows.iter_mut().zip(packed) {
            row.dodge_offset = packed;
        }
        mark.dodge_along_x = along_x;
    }
}
