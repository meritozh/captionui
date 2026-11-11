//! Plot's `text`, and the wrapping of a label to a line width.

use gpui::{Hsla, SharedString, TextAlign, Window, point, px};

use crate::paint::{self, Label};
use crate::render::PreparedMark;
use crate::value::Value;

use super::{Colors, DrawContext, position};

pub(super) fn draw_texts(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    use crate::mark::{LineAnchor, TextAnchor};
    let style = &prepared.mark.style;
    let mut font = gpui::font(ctx.theme.font_family.clone());
    if style.italic {
        font.style = gpui::FontStyle::Italic;
    }
    let weight = style
        .font_weight
        .map(|w| gpui::FontWeight(w as f32))
        .unwrap_or(gpui::FontWeight::NORMAL);
    // Plot's text paints its glyphs in `fill`, `currentColor` by default; a
    // stroke alone (a halo) does not color the glyphs.
    let colors = Colors::resolve(prepared, ctx);
    let size = px(style.font_size.unwrap_or(10.));
    let (dx, dy) = (style.dx, style.dy);
    let values = prepared
        .rows
        .first()
        .is_some_and(|row| row.row.get(crate::table::VALUE).is_some());

    // Text is not clipped to the plot: Plot draws a label whole even where it
    // overhangs the frame.
    for (index, row) in prepared.rows.iter().enumerate() {
        let Some(position) = position(prepared, ctx, row) else {
            continue;
        };
        let color = if prepared.mark.channels.fill.is_some() {
            colors.fill(index)
        } else {
            ctx.colors.default_fill(prepared, ctx.mark_index)
        };
        let color = paint::alpha(color, ctx.row_fill_opacity(prepared, row));
        let text = match &prepared.mark.channels.text {
            Some(channel) => channel.read(&row.row),
            None if values => row.row.value(crate::table::VALUE),
            None => Value::Null,
        };
        let text = match (&text, &prepared.mark.text_format) {
            (Value::Null, _) => continue,
            (value, Some(format)) => SharedString::from(format.format(value)),
            (value, None) => value.label(),
        };
        if text.is_empty() {
            continue;
        }
        // Plot splits on newlines and, given a `lineWidth` in ems, wraps.
        let mut lines: Vec<String> = Vec::new();
        for line in text.split('\n') {
            match style.line_width {
                Some(ems) => lines.extend(wrap(line, ems, size.as_f32(), ctx.window, &font)),
                None => lines.push(line.to_string()),
            }
        }
        let line_height = size.as_f32() * 1.2;
        let block = line_height * lines.len() as f32;
        let origin = ctx.absolute(position);
        // Plot's text infers its anchors from a `frameAnchor` on the
        // frame's edge: "top-left" reads start-aligned and hangs from the top.
        use crate::mark::FrameAnchor as F;
        let frame_anchor = style.frame_anchor;
        let line_anchor = match (style.line_anchor_set, frame_anchor) {
            (false, Some(F::Top | F::TopLeft | F::TopRight)) => LineAnchor::Top,
            (false, Some(F::Bottom | F::BottomLeft | F::BottomRight)) => LineAnchor::Bottom,
            _ => style.line_anchor,
        };
        let text_anchor = match (style.text_anchor_set, frame_anchor) {
            (false, Some(F::Left | F::TopLeft | F::BottomLeft)) => TextAnchor::Start,
            (false, Some(F::Right | F::TopRight | F::BottomRight)) => TextAnchor::End,
            _ => style.text_anchor,
        };
        let top = match line_anchor {
            LineAnchor::Top => origin.y.as_f32() + dy,
            LineAnchor::Middle => origin.y.as_f32() + dy - block / 2.,
            LineAnchor::Bottom => origin.y.as_f32() + dy - block,
        };
        let align = match text_anchor {
            TextAnchor::Start => TextAlign::Left,
            TextAnchor::Middle => TextAlign::Center,
            TextAnchor::End => TextAlign::Right,
        };
        // Plot's `rotate`: the label turns about its anchor point.
        if style.text_rotate.abs() > 0.5 {
            let anchor = point(px(origin.x.as_f32() + dx), px(origin.y.as_f32() + dy));
            paint::paint_rotated_text(
                &lines.join(" "),
                size.as_f32(),
                color,
                &ctx.theme.font_family,
                anchor,
                style.text_rotate,
                align,
                ctx.window,
                ctx.cx,
            );
            continue;
        }
        for (i, line) in lines.into_iter().enumerate() {
            let y_line = top + i as f32 * line_height;
            // Plot's `textStroke`: a halo, painted as the glyphs offset in a
            // ring around their place, under the glyphs themselves.
            if let Some(halo) = style.text_stroke {
                let halo_label = Label::new(
                    line.clone().into(),
                    size,
                    halo,
                    weight,
                    font.clone(),
                    ctx.window,
                );
                let w = style.text_stroke_width / 2.;
                for k in 0..8 {
                    let a = std::f32::consts::TAU * k as f32 / 8.;
                    let y = y_line + (line_height - halo_label.height.as_f32()) / 2. + w * a.sin();
                    halo_label.paint_at(
                        point(px(origin.x.as_f32() + dx + w * a.cos()), px(y)),
                        align,
                        None,
                        ctx.window,
                        ctx.cx,
                    );
                }
            }
            let label = Label::new(line.into(), size, color, weight, font.clone(), ctx.window);
            let y = y_line + (line_height - label.height.as_f32()) / 2.;
            label.paint_at(
                point(px(origin.x.as_f32() + dx), px(y)),
                align,
                None,
                ctx.window,
                ctx.cx,
            );
        }
    }
}

/// Wrap `text` to `ems` em-widths, at spaces, as Plot's `lineWidth` does.
pub(super) fn wrap(
    text: &str,
    ems: f32,
    size: f32,
    window: &mut Window,
    font: &gpui::Font,
) -> Vec<String> {
    let limit = ems * size;
    let measure = |text: &str, window: &mut Window| {
        Label::new(
            text.to_string().into(),
            px(size),
            Hsla::black(),
            gpui::FontWeight::NORMAL,
            font.clone(),
            window,
        )
        .width
        .as_f32()
    };
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if !current.is_empty() && measure(&candidate, window) > limit {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    lines.push(current);
    lines
}
