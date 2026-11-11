//! Plot's `image`: a picture centred on each row's position.

use gpui::{Bounds, point, px};

use crate::render::PreparedMark;

use super::{DrawContext, position};

/// Plot's `image` reads an `src` per row; here the mark carries one
/// [`gpui::RenderImage`] shared across its rows.
///
/// Plot's `image`: each row's image centred on its position in a box
/// `width` × `height` pixels (16 by default, each defaulting to the other),
/// or a circle of radius `r`, fit to the box by `preserveAspectRatio`
/// (`xMidYMid meet` by default; `slice` covers the box and crops).
pub(super) fn draw_image(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let Some(image) = prepared.mark.image.clone() else {
        return;
    };
    if prepared.rows.is_empty() {
        return;
    }
    let style = &prepared.mark.style;
    let natural = image.size(0);
    let (natural_w, natural_h) = (natural.width.0 as f32, natural.height.0 as f32);
    if natural_w <= 0. || natural_h <= 0. {
        return;
    }
    let aspect = style.preserve_aspect_ratio.as_deref().unwrap_or("xMidYMid meet");
    let mut words = aspect.split_whitespace();
    let align = words.next().unwrap_or("xMidYMid");
    let slice = words.next() == Some("slice");
    // SVG's alignment: where the image sits along each axis of the box.
    let along = |axis: &str| {
        if align.contains(&format!("{axis}Min")) {
            0.
        } else if align.contains(&format!("{axis}Max")) {
            1.
        } else {
            0.5
        }
    };
    let (ax, ay) = (along("x"), along("y"));
    let none = align == "none";
    ctx.clipped(|ctx| {
        for row in prepared.rows.iter() {
            let Some(centre) = position(prepared, ctx, row) else {
                continue;
            };
            let r = prepared
                .mark
                .channels
                .r
                .as_ref()
                .map(|channel| channel.read(&row.row));
            let (w, h) = match (r, style.image_size) {
                (Some(r), _) => (r * 2., r * 2.),
                (None, [Some(w), Some(h)]) => (w, h),
                (None, [Some(s), None] | [None, Some(s)]) => (s, s),
                (None, [None, None]) => (16., 16.),
            };
            if !(w > 0. && h > 0.) {
                continue;
            }
            let at = ctx.absolute(centre);
            let origin = point(
                at.x + px(style.dx - w / 2.),
                at.y + px(style.dy - h / 2.),
            );
            let bounds = Bounds::new(origin, gpui::size(px(w), px(h)));
            let image_bounds = if none {
                bounds
            } else {
                let scale = if slice {
                    (w / natural_w).max(h / natural_h)
                } else {
                    (w / natural_w).min(h / natural_h)
                };
                let (iw, ih) = (natural_w * scale, natural_h * scale);
                Bounds::new(
                    point(
                        origin.x + px((w - iw) * ax),
                        origin.y + px((h - ih) * ay),
                    ),
                    gpui::size(px(iw), px(ih)),
                )
            };
            // Plot's `r` clips the image to a circle.
            let corners = match r {
                Some(r) => gpui::Corners::all(px(r)),
                None => Default::default(),
            };
            // `paint_image` returns a Result; a failure paints nothing.
            let _ = ctx
                .window
                .paint_image(bounds, image_bounds, corners, image.clone(), 0, false);
        }
    });
}
