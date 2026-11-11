//! Plot's `rect` and `cell`, and the blending of overlapping rects.

use gpui::{Bounds, Hsla, Pixels, Window, point, px};

use crate::mark::MarkKind;
use crate::paint;
use crate::render::PreparedMark;
use crate::scale::PositionScale;

use super::{Colors, DrawContext};

pub(super) fn draw_rects(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    // With a `mixBlendMode`, the pieces painted so far with the opaque color
    // each shows, so an overlap paints its blend.
    let mut blended = BlendedPieces::new(prepared.mark.style.mix_blend_mode, ctx.theme.background);
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // Under a projection, Plot projects the corners (x1, y1) and
            // (x2, y2) as points, and the rect spans them.
            let projected = ctx.frame.projection.as_ref().map(|projection| {
                let (x_start, x_end) = row.x_interval()?;
                let (y_start, y_end) = row.y_interval()?;
                let a = projection.project(x_start.as_number()?, y_start.as_number()?)?;
                let b = projection.project(x_end.as_number()?, y_end.as_number()?)?;
                Some([a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32])
            });
            // Plot's rect spans the frame along an axis it has no channel
            // on (`rect(…, binX({fill: "count"}, {x}))` fills the height).
            let x_span = match row.x_interval() {
                _ if projected.is_some() => match projected.flatten() {
                    Some([x0, _, x1, _]) => (Some(x0), Some(x1)),
                    None => continue,
                },
                Some((start, end)) => (
                    ctx.frame.x.scale.position(&start),
                    ctx.frame.x.scale.position(&end),
                ),
                None if row.x1.is_none() && row.x2.is_none() => {
                    (Some(0.), Some(ctx.frame.plot.size.width.as_f32()))
                }
                None => continue,
            };
            let y_span = match row.y_interval() {
                _ if projected.is_some() => match projected.flatten() {
                    Some([_, y0, _, y1]) => (Some(y0), Some(y1)),
                    None => continue,
                },
                Some((start, end)) => (
                    ctx.frame.y.scale.position(&start),
                    ctx.frame.y.scale.position(&end),
                ),
                None if row.y1.is_none() && row.y2.is_none() => {
                    (Some(0.), Some(ctx.frame.plot.size.height.as_f32()))
                }
                None => continue,
            };
            let ((Some(x0), Some(x1)), (Some(y0), Some(y1))) = (x_span, y_span) else {
                continue;
            };
            // Plot's `inset`: an explicit inset shrinks every side; a binned
            // rect defaults to half a pixel along the axes it bins, so its
            // neighbours read apart.
            let (bin_x, bin_y) = crate::render::bin_axes(&prepared.mark);
            let inset = |binned: bool| {
                prepared
                    .mark
                    .style
                    .inset
                    .unwrap_or(if binned { 0.5 } else { 0. })
            };
            let shrink = |a: f32, b: f32, inset: f32| {
                let (lo, hi) = (a.min(b), a.max(b));
                let inset = inset.min((hi - lo) / 2.);
                (lo + inset, hi - inset)
            };
            let (x0, x1) = shrink(x0, x1, inset(bin_x));
            let (y0, y1) = shrink(y0, y1, inset(bin_y));
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0), px(y0))),
                ctx.absolute(point(px(x1), px(y1))),
            );
            // Plot's rect fills nothing when given only a stroke.
            if !(prepared.mark.channels.stroke.is_some() && prepared.mark.channels.fill.is_none()) {
                let color = paint::alpha(colors.fill(index), ctx.row_fill_opacity(prepared, row));
                if !blended.paint(rect, color, ctx.window) {
                    paint::paint_rect(rect, prepared.mark.style.corner_radius, color, ctx.window);
                }
            }
            if prepared.mark.channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    prepared.mark.style.corner_radius,
                    prepared.mark.style.stroke_width,
                    colors.stroke(index),
                    ctx.window,
                );
            }
        }
    });
}

/// Rect fills under a CSS `mix-blend-mode`, which GPUI cannot composite: the
/// pieces painted so far, each with the opaque color it shows over the
/// background. A new rect paints its blend over the background, then over
/// every piece it overlaps, in painting order so the topmost piece wins, as
/// the browser composites the mark's group.
struct BlendedPieces {
    mode: Option<crate::mark::BlendMode>,
    background: Hsla,
    pieces: Vec<(Bounds<Pixels>, Hsla)>,
}

impl BlendedPieces {
    fn new(mode: Option<crate::mark::BlendMode>, background: Hsla) -> Self {
        Self {
            mode,
            background: paint::alpha(background, 1.),
            pieces: Vec::new(),
        }
    }

    /// `color` blended onto the opaque `backdrop`, opaque.
    fn blend(mode: crate::mark::BlendMode, backdrop: Hsla, color: Hsla) -> Hsla {
        let (b, s) = (backdrop.to_rgb(), color.to_rgb());
        let a = s.a;
        let mix = |b: f32, s: f32| (1. - a) * b + a * mode.channel(b, s);
        Hsla::from(gpui::Rgba {
            r: mix(b.r, s.r),
            g: mix(b.g, s.g),
            b: mix(b.b, s.b),
            a: 1.,
        })
    }

    /// Paint `rect` blended, returning whether the mark blends at all.
    fn paint(&mut self, rect: Bounds<Pixels>, color: Hsla, window: &mut Window) -> bool {
        let Some(mode) = self.mode else {
            return false;
        };
        let base = Self::blend(mode, self.background, color);
        paint::paint_rect(rect, 0., base, window);
        let mut added = vec![(rect, base)];
        for (piece, shown) in &self.pieces {
            if !piece.intersects(&rect) {
                continue;
            }
            let overlap = piece.intersect(&rect);
            if overlap.size.width <= px(0.) || overlap.size.height <= px(0.) {
                continue;
            }
            let over = Self::blend(mode, *shown, color);
            paint::paint_rect(overlap, 0., over, window);
            added.push((overlap, over));
        }
        self.pieces.extend(added);
        true
    }
}

pub(super) fn draw_cells(prepared: &PreparedMark, ctx: &mut DrawContext<'_>) {
    let colors = Colors::resolve(prepared, ctx);
    ctx.clipped(|ctx| {
        for (index, row) in prepared.rows.iter().enumerate() {
            // A one-dimensional cell covers the whole frame across the axis
            // it does not read, since that axis has no bands to fill. Its
            // extent starts at the frame's edge, not at the datum's index.
            // A cell without a y spans the frame's height (`cellX`, or a
            // `cell` given only x), one without an x its width.
            let no_y = prepared.mark.kind == MarkKind::CellX || row.y_value().is_null();
            let no_x = prepared.mark.kind == MarkKind::CellY || row.x_value().is_null();
            let (plot_w, plot_h) = (
                ctx.frame.plot.size.width.as_f32(),
                ctx.frame.plot.size.height.as_f32(),
            );
            let (x0, width) = if no_x {
                (plot_w / 2., plot_w)
            } else {
                let Some(x) = ctx.frame.x.scale.position(row.x_value()) else {
                    continue;
                };
                (x, cell_extent(&ctx.frame.x.scale))
            };
            let (y0, height) = if no_y {
                (plot_h / 2., plot_h)
            } else {
                let Some(y) = ctx.frame.y.scale.position(row.y_value()) else {
                    continue;
                };
                (y, cell_extent(&ctx.frame.y.scale))
            };
            let inset = prepared.mark.style.inset.unwrap_or(0.);
            let (hw, hh) = ((width / 2. - inset).max(0.), (height / 2. - inset).max(0.));
            // A band's position is its center, so the cell straddles it.
            let rect = paint::rect_between(
                ctx.absolute(point(px(x0 - hw), px(y0 - hh))),
                ctx.absolute(point(px(x0 + hw), px(y0 + hh))),
            );
            // Like Plot's rect, a cell given only a stroke fills nothing,
            // and outlines itself in that stroke.
            let channels = &prepared.mark.channels;
            if !(channels.stroke.is_some() && channels.fill.is_none()) {
                let color = colors.fill(index);
                paint::paint_rect(
                    rect,
                    0.,
                    paint::alpha(color, ctx.row_fill_opacity(prepared, row)),
                    ctx.window,
                );
            }
            if channels.stroke.is_some() {
                paint::paint_rect_outline(
                    rect,
                    0.,
                    prepared.mark.style.stroke_width,
                    paint::alpha(colors.stroke(index), ctx.row_stroke_opacity(prepared, row)),
                    ctx.window,
                );
            }
        }
    });
}

/// The width a cell covers along an axis: a band's width, or a step for a
/// continuous axis.
fn cell_extent(scale: &PositionScale) -> f32 {
    match scale {
        PositionScale::Band(band) => band.band_width(),
        PositionScale::Point(point_scale) => {
            let step = point_scale.step();
            if step > 0. { step } else { 1. }
        }
        PositionScale::Continuous(_) => 1.,
    }
}
