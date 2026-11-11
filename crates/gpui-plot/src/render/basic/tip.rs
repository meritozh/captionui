//! Plot's `tip`: a box with a pointer, around a row's text, at the datum.

use gpui::{Hsla, Pixels, Point, TextAlign, point, px};

use crate::paint::{self, Label};
use crate::render::PreparedMark;

use super::text::wrap;
use super::{DrawContext, position};

/// Plot's `tip`: a box around the row's text with a pointer at the datum,
/// on the side `anchor` names or else the side that fits.
///
/// As in Plot, the box fills with the background and strokes in the
/// foreground (a `fill` or `stroke` overrides either), the text reads in the
/// foreground at `lineHeight` 1, wraps at `lineWidth` (20 ems), and sits
/// `textPadding` (8px) inside the box, whose pointer is `pointerSize` (12px).
/// Without a `title` (or text) the tip lists the row's channels as bold
/// name, value pairs.
pub(super) fn draw_tips(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    use crate::mark::FrameAnchor as A;
    let style = &prepared.mark.style;
    let font = gpui::font(ctx.theme.font_family.clone());
    let size = style.font_size.unwrap_or(ctx.theme.label_size.as_f32());
    let r = style.text_padding.unwrap_or(8.);
    let m = style.pointer_size.unwrap_or(12.);
    let line_width = style.line_width.unwrap_or(20.);
    let ink = ctx.theme.mark_color;
    let defaults = crate::mark::MarkStyle::default();
    let (figure_width, figure_height) = (
        ctx.frame.bounds.size.width.as_f32(),
        ctx.frame.bounds.size.height.as_f32(),
    );

    // Tips are not clipped to the plot: a tip on the topmost point sits above
    // it, and Plot draws the whole label.
    for row in prepared.rows.iter() {
        let Some(placed) = position(prepared, ctx, row) else {
            continue;
        };
        // Each line is an optional bold name and its value.
        let mut lines: Vec<(Option<String>, String)> = Vec::new();
        let title = prepared
            .mark
            .channels
            .text
            .as_ref()
            .or(prepared.mark.channels.title.as_ref());
        if let Some(channel) = title {
            let text = channel.read(&row.row);
            if text.is_null() {
                continue;
            }
            for line in text.label().split('\n') {
                for wrapped in wrap(line, line_width, size, ctx.window, &font) {
                    lines.push((None, wrapped));
                }
            }
        } else {
            let channels = &prepared.mark.channels;
            for (name, channel, value) in [
                ("x", &channels.x, row.x_value()),
                ("y", &channels.y, row.y_value()),
            ] {
                if channel.is_none() || value.is_null() {
                    continue;
                }
                let label = match channel {
                    Some(crate::table::Channel::Field(field)) => field.to_string(),
                    _ => name.to_string(),
                };
                lines.push((Some(label), format!(" {}", value.label())));
            }
        }
        if lines.iter().all(|(l, v)| l.is_none() && v.is_empty()) {
            continue;
        }
        let shaped: Vec<(Option<Label>, Label)> = lines
            .into_iter()
            .map(|(label, value)| {
                let label = label.map(|label| {
                    Label::new(
                        label.into(),
                        px(size),
                        ink,
                        gpui::FontWeight::BOLD,
                        font.clone(),
                        ctx.window,
                    )
                });
                let value = Label::new(
                    value.into(),
                    px(size),
                    ink,
                    gpui::FontWeight::NORMAL,
                    font.clone(),
                    ctx.window,
                );
                (label, value)
            })
            .collect();
        let n = shaped.len() as f32;
        // The text's bounding box, rounded for crisp edges as Plot does: the
        // lines sit one em apart, and the last carries its descent.
        let w = shaped
            .iter()
            .map(|(label, value)| {
                label.as_ref().map_or(0., |l| l.width.as_f32()) + value.width.as_f32()
            })
            .fold(0., f32::max)
            .round();
        let h = (size * (n - 1. + 1.15)).round();

        let at = ctx.absolute(point(
            px(placed.x.as_f32().round() + style.dx),
            px(placed.y.as_f32().round() + style.dy),
        ));
        let anchor = style.tip_anchor.unwrap_or_else(|| {
            let x = at.x.as_f32() - ctx.frame.bounds.origin.x.as_f32();
            let y = at.y.as_f32() - ctx.frame.bounds.origin.y.as_f32();
            let fit_left = x + w + m + r * 2. < figure_width;
            let fit_right = x - w - m - r * 2. > 0.;
            let fit_top = y + h + m + r * 2. < figure_height;
            let fit_bottom = y - h - m - r * 2. > 0.;
            match (fit_left && fit_right, fit_top && fit_bottom) {
                (true, true) => A::Bottom,
                (true, false) if fit_bottom => A::Bottom,
                (true, false) => A::Top,
                (false, true) if fit_left => A::Left,
                (false, true) => A::Right,
                _ if (fit_left || fit_right) && (fit_top || fit_bottom) => {
                    match (fit_bottom, fit_left) {
                        (true, true) => A::BottomLeft,
                        (true, false) => A::BottomRight,
                        (false, true) => A::TopLeft,
                        (false, false) => A::TopRight,
                    }
                }
                _ => A::Bottom,
            }
        });

        // Plot's `getPath`, and its `getTextTranslate` for the text block.
        let (bw, bh, m2) = (w + r * 2., h + r * 2., m / 2.);
        let outline: Vec<(f32, f32)> = match anchor {
            A::Middle => vec![
                (-bw / 2., -bh / 2.),
                (bw / 2., -bh / 2.),
                (bw / 2., bh / 2.),
                (-bw / 2., bh / 2.),
            ],
            A::TopLeft => vec![(0., 0.), (m2, m2), (bw, m2), (bw, m2 + bh), (0., m2 + bh)],
            A::Top => vec![
                (0., 0.),
                (m2, m2),
                (bw / 2., m2),
                (bw / 2., m2 + bh),
                (-bw / 2., m2 + bh),
                (-bw / 2., m2),
                (-m2, m2),
            ],
            A::TopRight => vec![(0., 0.), (-m2, m2), (-bw, m2), (-bw, m2 + bh), (0., m2 + bh)],
            A::Right => vec![
                (0., 0.),
                (-m2, -m2),
                (-m2, -bh / 2.),
                (-m2 - bw, -bh / 2.),
                (-m2 - bw, bh / 2.),
                (-m2, bh / 2.),
                (-m2, m2),
            ],
            A::BottomLeft => vec![(0., 0.), (m2, -m2), (bw, -m2), (bw, -m2 - bh), (0., -m2 - bh)],
            A::Bottom => vec![
                (0., 0.),
                (m2, -m2),
                (bw / 2., -m2),
                (bw / 2., -m2 - bh),
                (-bw / 2., -m2 - bh),
                (-bw / 2., -m2),
                (-m2, -m2),
            ],
            A::BottomRight => vec![
                (0., 0.),
                (-m2, -m2),
                (-bw, -m2),
                (-bw, -m2 - bh),
                (0., -m2 - bh),
            ],
            A::Left => vec![
                (0., 0.),
                (m2, -m2),
                (m2, -bh / 2.),
                (m2 + bw, -bh / 2.),
                (m2 + bw, bh / 2.),
                (m2, bh / 2.),
                (m2, m2),
            ],
        };
        let (tx, ty) = match anchor {
            A::Middle => (-w / 2., h / 2.),
            A::TopLeft => (r, m2 + r),
            A::Top => (-w / 2., m2 + r),
            A::TopRight => (-w - r, m2 + r),
            A::Right => (-m2 - w - r, h / 2.),
            A::BottomLeft => (r, -m2 - r),
            A::Bottom => (-w / 2., -m2 - r),
            A::BottomRight => (-w - r, -m2 - r),
            A::Left => (r + m2, h / 2.),
        };
        // Plot's `getLineOffset`: a top anchor hangs the first baseline 0.94em
        // below the text's origin; every other anchor (its regex test is
        // always truthy) rests the last baseline 0.29em above it.
        let first_baseline = match anchor {
            A::Top | A::TopLeft | A::TopRight => 0.94 * size,
            _ => (-0.29 - n + 1.) * size,
        };

        let to_point = |(x, y): (f32, f32)| point(at.x + px(x), at.y + px(y));
        let polygon: Vec<Point<Pixels>> = outline.iter().copied().map(to_point).collect();
        let fill = match &prepared.mark.channels.fill {
            Some(channel) => ctx.colors.channel(channel, &row.row),
            None if style.fill != defaults.fill => style.fill,
            None => ctx.theme.background,
        };
        let stroke = match &prepared.mark.channels.stroke {
            Some(channel) => ctx.colors.channel(channel, &row.row),
            None if style.stroke != defaults.stroke => style.stroke,
            None => ink,
        };
        // Plot's `drop-shadow(0 3px 4px rgba(0,0,0,0.2))`, as stacked offsets
        // that fade with distance.
        for offset in 1..=4 {
            let shadow: Vec<Point<Pixels>> = polygon
                .iter()
                .map(|p| point(p.x, p.y + px(offset as f32)))
                .collect();
            paint::paint_fill(&shadow, paint::alpha(Hsla::black(), 0.05), ctx.window);
        }
        paint::paint_fill(&polygon, fill, ctx.window);
        let mut closed = polygon.clone();
        closed.push(polygon[0]);
        paint::paint_stroke(&closed, style.stroke_width.max(1.), stroke, ctx.window);

        for (i, (label, value)) in shaped.iter().enumerate() {
            let baseline = ty + first_baseline + i as f32 * size;
            // A label's top sits about 0.85em above its baseline.
            let top = at.y + px(baseline - 0.85 * size);
            let mut left = at.x + px(tx);
            if let Some(label) = label {
                label.paint_at(point(left, top), TextAlign::Left, None, ctx.window, ctx.cx);
                left += label.width;
            }
            value.paint_at(point(left, top), TextAlign::Left, None, ctx.window, ctx.cx);
        }
    }
}
