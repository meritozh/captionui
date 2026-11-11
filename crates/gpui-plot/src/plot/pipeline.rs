//! The transform pipeline a frame runs: the marks resolved per facet,
//! primed or fresh, then dodged, binned and colored against the frame,
//! with the text dumps the identity harness diffs.

use gpui::{App, Bounds, Pixels, Point, Window};
use gpui_kit::component::ActiveTheme;

use crate::facet::Grid;
use crate::frame::Frame;
use crate::plot::layout::cell_frame;
use crate::plot::{Plot, Prepared};
use crate::render::{self, PreparedMark};
use crate::theme::PlotTheme;

impl Plot {
    /// The transform pipeline both `natural_height` and `prepaint` run: the
    /// marks (their facet scales' intervals applied) resolved in each facet
    /// of the grid, through the scales' `transform`s, with which facets hold
    /// any faceted rows.
    pub(super) fn prepare_marks(&self) -> Prepared {
        let bucketed = self.interval_marks();
        // The facet channels the grid reads, which the partitions then reuse.
        let mut reads = crate::facet::FacetReads::default();
        let grid = self.facet_grid_reading(bucketed.as_deref(), Some(&mut reads));
        let marks = bucketed.as_deref().unwrap_or(&self.marks);
        let resolved = self.resolve_facet_axes(marks, &grid);
        let (mut facets, populated) = render::prepare_facets_reading(
            &self.data,
            resolved.as_deref().unwrap_or(marks),
            &grid,
            self.fx.as_deref(),
            self.fy.as_deref(),
            &reads,
        );
        self.apply_scale_transforms(&mut facets);
        (grid, facets, populated)
    }

    /// Have the next `prepaint` take `prepared`, the result of this plot's
    /// own pipeline, rather than run it again. Sound only while the plot's
    /// marks and data stay as they were, as a retained plot's do.
    pub(crate) fn prime(&self, prepared: Prepared) {
        *self.primed.borrow_mut() = Some(prepared);
    }

    /// Move what `prepaint` resolved by `delta`, for a plot drawn again at
    /// the same size somewhere else (a scrolled list): the pipeline and the
    /// scales are in plot-local pixels, only the frame's bounds are absolute.
    pub(crate) fn translate_prepared(&mut self, delta: Point<Pixels>) {
        if let Some(frame) = &mut self.frame_cache {
            frame.bounds.origin += delta;
            frame.plot.origin += delta;
        }
    }

    /// Everything `prepaint` resolves for a frame at `bounds`: the marks (the
    /// transform pipeline), the frame (the layout), the dodge and hexbin
    /// (which need the frame's pixels), the color scales and the facet grid.
    pub(super) fn resolve_for_paint(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
    ) -> (
        Vec<Vec<PreparedMark>>,
        render::color::PlotColors,
        Frame,
        Grid,
    ) {
        let (mut grid, mut facets, populated) =
            self.primed.take().unwrap_or_else(|| self.prepare_marks());
        grid.set_empty(&populated);
        render::apply_quantity_scales(&mut facets, &self.scales.r, &self.scales.opacity);
        render::apply_symbol_scale(&mut facets, &self.scales.symbol);
        let frame = self.resolve_frame(bounds, theme, window, &facets, &grid);
        // A dodge works in pixels, so it needs the frame resolved first. Every
        // facet is the same size, so one facet's frame packs the same way.
        let dodge_frame = cell_frame(&frame, &grid, 0);
        for marks in facets.iter_mut() {
            render::apply_dodge(marks, &dodge_frame);
        }
        let peak_frame = cell_frame(&frame, &grid, 0);
        render::apply_hexbin(&mut facets, &peak_frame);
        render::apply_quantity_scales(&mut facets, &self.scales.r, &self.scales.opacity);
        for mark in facets.iter_mut().flatten() {
            mark.density_max = render::radial::density_peak(mark, &peak_frame);
        }
        let colors = self.plot_colors(theme, &facets);
        (facets, colors, frame, grid)
    }

    /// A deterministic text dump of what [`Plot::natural_height`] prepares:
    /// the facet grid and which cells the marks populate, the x and y values
    /// the scales read, and every facet's marks with every row's channels
    /// and record, in order. The pipeline identity harness
    /// (`demo/examples/dump.rs`) diffs it across builds.
    #[doc(hidden)]
    pub fn debug_dump(&self, width: f32) -> String {
        use crate::debug_dump as dump;
        use std::fmt::Write;
        let (grid, facets, populated) = self.prepare_marks();
        let mut out = String::new();
        let _ = writeln!(
            out,
            "natural_height({width:?}) = {:?}",
            self.natural_height(width)
        );
        dump::grid(&mut out, &grid, Some(&populated));
        let (x_values, y_values) = (self.x_domain_values(&facets), self.y_domain_values(&facets));
        dump::value_lines(&mut out, "", "x_domain_values", &x_values);
        dump::value_lines(&mut out, "", "y_domain_values", &y_values);
        for (cell, marks) in facets.iter().enumerate() {
            let _ = writeln!(out, "facet {cell}");
            for (index, mark) in marks.iter().enumerate() {
                dump::mark(&mut out, index, mark);
                for (i, row) in mark.rows.iter().enumerate() {
                    dump::row(&mut out, i, row);
                }
            }
        }
        out
    }

    /// [`Plot::debug_dump`] for what `prepaint` resolves at `bounds`: the
    /// frame (its bounds, insets, scales and ticks, and each facet's plot
    /// area), the color scales and legend entries, and every row after the
    /// dodge, hexbin and quantity scales, with the position and the colors it
    /// paints at. The window measures the legend's labels, as in `prepaint`.
    #[doc(hidden)]
    pub fn debug_dump_frame(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> String {
        use crate::debug_dump as dump;
        use std::fmt::Write;
        let theme = PlotTheme::from_theme(cx.theme());
        let (facets, colors, frame, grid) = self.resolve_for_paint(bounds, &theme, window);
        let mut out = String::new();
        dump::grid(&mut out, &grid, None);
        dump::frame(&mut out, &frame);
        let _ = writeln!(out, "color scale={:?}", colors.scale);
        let _ = writeln!(out, "color ordinal={:?}", colors.ordinal);
        let _ = writeln!(out, "color continuous={:?}", colors.continuous);
        for entry in self.legend_entries(&colors, &facets) {
            let _ = writeln!(out, "legend {entry:?}");
        }
        for (cell, marks) in facets.iter().enumerate() {
            let facet_frame = cell_frame(&frame, &grid, cell);
            out.push_str("facet ");
            let _ = write!(out, "{cell} plot=");
            dump::bounds(&mut out, facet_frame.plot);
            out.push('\n');
            for (index, mark) in marks.iter().enumerate() {
                dump::mark(&mut out, index, mark);
                for (i, row) in mark.rows.iter().enumerate() {
                    dump::row(&mut out, i, row);
                    let position = render::basic::position_in(mark, &facet_frame, row);
                    let _ = write!(
                        out,
                        "      at={:?} fill=",
                        position.map(|p| (p.x.as_f32(), p.y.as_f32()))
                    );
                    dump::hsla(&mut out, colors.row_fill(mark, index, row));
                    out.push_str(" series=");
                    dump::hsla(&mut out, colors.series_color(mark, index, row));
                    out.push_str(" identity=");
                    dump::hsla(&mut out, colors.identity(mark, index, row));
                    out.push('\n');
                }
            }
        }
        out
    }

    /// Plot's scale `transform` options, over the resolved marks' channels.
    fn apply_scale_transforms(&self, facets: &mut [Vec<PreparedMark>]) {
        let s = &self.scales;
        render::apply_scale_transforms(facets, &s.x, &s.y, &s.r, &s.opacity);
    }
}
