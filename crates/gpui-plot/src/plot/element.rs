//! The plot as a GPUI Kit plot element: the prepaint, paint, appear,
//! hover and tooltip phases.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, ParentElement, Pixels, Point, Window, px,
};
use gpui_kit::base::plot::{Plot as PlotTrait, PlotAppear, PlotElement, PlotHover, TooltipState};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::plot::tooltip::{CrossLine, Dot, Tooltip};

use crate::frame::Frame;
use crate::plot::Plot;
use crate::plot::hover::{hover_targets, hoverable, interval_rect};
use crate::plot::layout::cell_frame;
use crate::render::{self, basic::DrawContext};
use crate::theme::PlotTheme;

impl PlotTrait for Plot {
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        // Resolve everything the later phases read here, once per frame: the
        // marks (the transform pipeline), the frame (the layout), the dodge
        // (which needs the frame's pixels) and the color scales. `paint`,
        // `tooltip_state` and `tooltip` then share one consistent view.
        let theme = PlotTheme::from_theme(cx.theme());
        let (facets, colors, frame, grid) = self.resolve_for_paint(bounds, &theme, window);
        self.facets_cache = Some(Rc::new(facets));
        self.colors_cache = Some(Rc::new(colors));
        self.frame_cache = Some(frame);
        self.facet_cache = Some(grid);
        vec![]
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let theme = PlotTheme::from_theme(cx.theme());
        let (Some(facets), Some(frame), Some(grid), Some(colors)) = (
            self.facets_cache.clone(),
            self.frame_cache.clone(),
            self.facet_cache.clone(),
            self.colors_cache.clone(),
        ) else {
            return;
        };

        let appear = self.appear_progress;

        // The title sits above the plot, inside the figure's top margin.
        self.paint_title(bounds, &theme, window, cx);
        self.paint_caption(bounds, &theme, window, cx);
        // The legend sits in the margin its position reserves, inside the
        // figure's bounds (already shrunk to the plot's `height`).
        self.paint_legend(frame.bounds, &theme, window, cx, &facets, &colors);
        // The grid goes under the marks.
        // Plot's grid belongs to its axis: an axis drawn with `grid` (its own
        // or the plot's) draws its lines; `axis: null` takes both away.
        let (x_grid, y_grid) = (
            self.scales.x.grid.unwrap_or(self.grid) && self.x_axis_anchor().is_some(),
            self.scales.y.grid.unwrap_or(self.grid) && self.y_axis_anchor().is_some(),
        );
        if x_grid || y_grid {
            crate::frame::paint_grid(&frame, &theme, window, x_grid, y_grid, &grid);
        }
        let no_axes = self.no_axes;
        let (fx_grid, fy_grid) = (
            (self.scales.fx.grid == Some(true) && !no_axes).then_some(&self.scales.fx),
            (self.scales.fy.grid == Some(true) && !no_axes).then_some(&self.scales.fy),
        );
        if fx_grid.is_some() || fy_grid.is_some() {
            crate::frame::paint_facet_grid(&frame, &theme, window, &grid, fx_grid, fy_grid);
        }
        // The marks. Every mark shares one set of color scales, so a series
        // keeps its color across marks, and across facets.
        let mut row_offset = 0;
        for (cell, marks) in facets.iter().enumerate() {
            let facet_frame = cell_frame(&frame, &grid, cell);
            // Plot skips a facet that holds no data: it keeps the slot, and
            // the facet axis, but draws no marks inside it.
            let skip = grid.is_faceted() && grid.is_empty(cell);
            for (index, mark) in marks.iter().enumerate() {
                let offset = row_offset;
                // The row offset counts every facet's rows, drawn or not, so it
                // matches the flattened list the hover indexes.
                row_offset += mark.rows.len();
                if skip {
                    continue;
                }
                let mut ctx = DrawContext {
                    frame: &facet_frame,
                    theme: &theme,
                    mark_index: index,
                    appear,
                    hover_progress: self.hover_progress,
                    hovered: self.hover,
                    row_offset: offset,
                    colors: &colors,
                    clip: mark.mark.style.clip,
                    window,
                    cx,
                };
                // Which renderer family paints the mark, read from its single
                // classification table rather than a hand-maintained list here.
                match mark.mark.kind.category().family {
                    crate::mark::MarkFamily::Radial => render::radial::draw(mark, &mut ctx),
                    // The Delaunay marks build their geometry from the
                    // projected points, so they draw through the frame's
                    // pixel space rather than the row shapes.
                    crate::mark::MarkFamily::Delaunay => render::delaunay::draw(mark, &mut ctx),
                    crate::mark::MarkFamily::Basic => render::basic::draw(mark, &mut ctx),
                }
            }
        }
        // The axes sit on top of the marks, clipped to the figure as Plot's
        // SVG clips them: a long rotated tick label stops at its edge.
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            self.paint_axes_layer(&frame, &grid, &theme, window, cx)
        });
    }

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn interactive(&self) -> bool {
        self.interaction.enabled
    }

    fn appear(&mut self, appear: PlotAppear, _window: &mut Window, _cx: &mut App) {
        self.appear_progress = appear.progress();
    }

    /// A fixed generation: a plot draws its data in once, the first time it
    /// paints, and never again.
    ///
    /// The generation is the key the appear's element state is kept under, so
    /// a constant one means the motion runs on the plot's first frame and the
    /// state remembers it as finished. A generation that changed with the data
    /// would replay the draw-in whenever the caller rebuilt the plot — on
    /// every hover, every resize — and the marks would flicker away and back.
    fn appear_generation(&self) -> Option<u64> {
        Some(0)
    }

    fn hover(&mut self, hover: Option<&PlotHover>, _window: &mut Window, _cx: &mut App) {
        // The state's index addresses the flattened row list, which both
        // `tooltip_state` and `tooltip` build the same way.
        self.hover = hover.map(|hover| hover.state().index);
        self.hover_progress = hover.map_or(0., |hover| hover.progress());
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _cx: &App,
    ) -> Option<TooltipState> {
        if !self.interaction.enabled {
            return None;
        }
        let frame = self.frame_cache.as_ref()?;
        let grid = self.facet_cache.as_ref()?;
        let facets = self.facets_cache.as_ref()?;

        // Ignore the axis gutters: hovering a label shows nothing.
        let absolute = position + bounds.origin;
        if !frame.plot.contains(&absolute) {
            return None;
        }
        let local = absolute - frame.plot.origin;
        let cell_frames: Vec<Frame> = (0..facets.len())
            .map(|cell| cell_frame(frame, grid, cell))
            .collect();

        // The datum under the pointer. A mark that draws an interval — a bar,
        // a rect, a cell — is hoverable across its whole body, so the pointer
        // selects it when it lies inside that rectangle (Plot's `nearest`)
        // rather than only near its anchor point; among overlapping bodies the
        // last-painted, topmost one wins. A mark without an interval (a dot, a
        // line vertex) is picked by distance to the point it was painted at.
        let targets = hover_targets(facets);
        let mut contained: Option<(usize, Point<Pixels>)> = None;
        let mut best: Option<(usize, Point<Pixels>, f32)> = None;
        for (flat, &(cell, mark_index, row_index)) in targets.iter().enumerate() {
            if grid.is_faceted() && grid.is_empty(cell) {
                continue;
            }
            let mark = &facets[cell][mark_index];
            if !hoverable(mark) {
                continue;
            }
            let Some(row) = mark.rows.get(row_index) else {
                continue;
            };
            let facet_frame = &cell_frames[cell];
            // Everything is lifted from the facet's plot-local pixels into the
            // whole figure's plot coordinates, where `local` lives.
            let lift = facet_frame.plot.origin - frame.plot.origin;
            let Some(point) = render::basic::position_in(mark, facet_frame, row) else {
                continue;
            };
            let point = point + lift;
            if interval_rect(mark, row, facet_frame).is_some_and(|r| (r + lift).contains(&local)) {
                contained = Some((flat, point));
            }
            let dx = point.x.as_f32() - local.x.as_f32();
            let dy = point.y.as_f32() - local.y.as_f32();
            let distance = dx * dx + dy * dy;
            if best.as_ref().is_none_or(|(_, _, best)| distance < *best) {
                best = Some((flat, point, distance));
            }
        }
        let (flat, hit) = contained.or_else(|| best.map(|(flat, point, _)| (flat, point)))?;
        let (cell, mark_index, row_index) = targets[flat];
        let facet_frame = &cell_frames[cell];
        let lift = facet_frame.plot.origin - frame.plot.origin;
        let mark = &facets[cell][mark_index];
        let row = mark.rows.get(row_index)?;

        // One dot per series at the hovered datum: the rows of the same mark
        // that share its position on the independent axis. A bar's body is
        // already highlighted, so an interval mark marks only its own datum.
        let dots: Vec<Point<Pixels>> = if interval_rect(mark, row, facet_frame).is_some() {
            vec![hit]
        } else {
            let along_y = mark.mark.kind.is_y_only();
            let key = |other: &render::PreparedRow| {
                if along_y {
                    other.y_value().key()
                } else {
                    other.x_value().key()
                }
            };
            let hovered = key(row);
            let dots: Vec<Point<Pixels>> = mark
                .rows
                .iter()
                .filter(|other| key(other) == hovered)
                .filter_map(|other| render::basic::position_in(mark, facet_frame, other))
                .map(|point| point + lift)
                .collect();
            if dots.is_empty() { vec![hit] } else { dots }
        };

        // The tooltip overlay is placed at the element's own origin (its
        // top-left, including the title and legend margins), so the crosshair
        // and hovered dots must be in the element's coordinates, not the
        // plot's. `frame.absolute` lifts a plot-local point up by the plot's
        // offset within the figure.
        let hit = frame.absolute(hit);
        let dots = dots.into_iter().map(|p| frame.absolute(p)).collect();

        Some(TooltipState::new(flat, hit, dots))
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let theme = PlotTheme::from_theme(cx.theme());
        let facets = self.facets_cache.as_ref()?;
        let colors = self.colors_cache.as_ref()?;
        let frame = self.frame_cache.as_ref()?;
        let grid = self.facet_cache.as_ref()?;

        // The state carries the index the pointer resolved to, in the same
        // flattened row list `tooltip_state` searched.
        let (cell, mark_index, row_index) = *hover_targets(facets).get(state.index)?;
        let mark = facets.get(cell)?.get(mark_index)?;
        let row = mark.rows.get(row_index)?;

        // The swatch and dots take the color the mark painted this row in.
        let color = colors.identity(mark, mark_index, row);

        // The box hugs the cursor and carries the crosshair and hovered dots;
        // the body is either a custom formatter's freeform element or the
        // default title-and-rows.
        let mut tooltip = Tooltip::new(cursor, bounds.size).gap(px(8.));
        if let Some(formatter) = &self.tooltip_formatter {
            // A custom formatter replaces the default body, but the crosshair
            // and dots still render and the box still follows the cursor: the
            // returned element becomes the box's freeform base.
            let element = formatter(&row.row, window, cx);
            tooltip.extend([element]);
        } else {
            let title = mark
                .mark
                .channels
                .title
                .as_ref()
                .map(|channel| channel.read(&row.row).label())
                .unwrap_or_else(|| row.x_value().label());

            tooltip = tooltip.title(title).row(
                color,
                mark.mark
                    .title
                    .clone()
                    .unwrap_or_else(|| mark.mark.kind.name().into()),
                row.y_value().label(),
            );

            // Add the channels a tooltip conventionally reads: the series and
            // any other fields the mark named.
            if let Some(z) = &row.z {
                tooltip = tooltip.plain_row("series", z.label());
            }
            for name in ["group", "name", "label"] {
                if let Some(value) = row.row.get(name)
                    && !value.is_null()
                {
                    tooltip = tooltip.plain_row(name, value.label());
                }
            }
        }

        // The crosshair follows the chart's axes, as Plot's crosshair draws a
        // rule for each position channel: both lines on a two-axis chart, a
        // vertical one when only x is read (`dotX`), a horizontal one when
        // only y is (`tickY`), and none for a chart with neither (a map).
        let axes = (self.reads_axis(true), self.reads_axis(false));
        if self.interaction.crosshair && axes != (false, false) {
            // The crosshair spans the hovered facet's plot area only, not the
            // title, legend or axis gutters around it.
            let facet_frame = cell_frame(frame, grid, cell);
            let plot = facet_frame.plot;
            let top = (plot.origin.y - bounds.origin.y).as_f32();
            let left = (plot.origin.x - bounds.origin.x).as_f32();
            let line = CrossLine::new(state.cross_line)
                .span(top, plot.size.height.as_f32())
                .h_span(left, plot.size.width.as_f32());
            let line = match axes {
                (true, true) => line.both(),
                (false, true) => line.horizontal(),
                _ => line,
            };
            tooltip = tooltip.cross_line(line.band(px(6.)));
        }
        if self.interaction.highlight {
            let dots: Vec<Dot> = state
                .dots
                .iter()
                .map(|dot| {
                    Dot::new(*dot)
                        .size(px(8.))
                        .halo(px(14.))
                        .fill(color)
                        .stroke(theme.background)
                })
                .collect();
            tooltip = tooltip.dots(dots);
        }

        Some(tooltip.into_any_element())
    }
}

impl IntoElement for Plot {
    type Element = PlotElement<Self>;

    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}
