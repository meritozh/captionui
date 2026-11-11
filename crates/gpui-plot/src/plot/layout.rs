//! The figure's layout: its natural height, the margins and insets the
//! axes, title, legend and caption reserve, the resolved frame, and the
//! frame each facet cell draws in.

use gpui::{Bounds, Pixels, Window, px};

use crate::facet::Grid;
use crate::frame::{Frame, Insets, ScaleKind};
use crate::mark::MarkKind;
use crate::plot::domain::{marks_use_band, sorted_domain};
use crate::plot::legend::LEGEND_BAND;
use crate::plot::{LegendPosition, Plot, Prepared};
use crate::render::PreparedMark;
use crate::theme::PlotTheme;
use crate::value::Value;

/// The gap between a y-axis label's right edge and the axis line, matching
/// GPUI Base's `TEXT_GAP` for the axis labels it draws.
const AXIS_LABEL_GAP: f32 = 4.;

impl Plot {
    /// The height the figure takes at `width`: the chart's height (its
    /// [`Plot::height`], or Plot's `autoHeight` — 400-odd pixels for a
    /// continuous y, twenty per value for an ordinal one, sixty for a plot
    /// with no y at all, or what `aspectRatio` asks for) plus the title,
    /// legend and caption above and below it.
    pub fn natural_height(&self, width: f32) -> f32 {
        self.natural_height_prepared(width).0
    }

    /// [`Plot::natural_height`] with the pipeline's result it worked from,
    /// which reads neither the width nor the theme.
    pub(crate) fn natural_height_prepared(&self, width: f32) -> (f32, Prepared) {
        let (grid, facets, populated) = self.prepare_marks();
        let [mt, mr, mb, ml] = self.plot_margins(&grid);
        // The x and y values the scales read are gathered only by the
        // branches below that read them, and only as far as they read.
        let nfy = grid.fy.as_ref().map_or(1, |d| d.len().max(1)) as f32;
        let nfx = grid.fx.as_ref().map_or(1, |d| d.len().max(1)) as f32;
        let ordinal = |mut values: &mut dyn Iterator<Item = Value>,
                       options: &crate::options::ScaleOptions| {
            options.r#type.is_some_and(|t| t.is_discrete())
                || (options.r#type.is_none()
                    && Iterator::find(&mut values, |v| !v.is_null())
                        .is_some_and(|v| !v.kind().is_quantitative()))
        };
        let projected = self.projection.as_ref().map(|p| {
            // Plot's `projectionAspectRatio`: a domain's bounds at 100 × 100,
            // else the projection's own; clamped as Plot clamps it.
            let ratio = match &p.domain {
                Some(domain) => {
                    let fitted = crate::geo::GeoProjection {
                        domain: None,
                        ..p.clone()
                    }
                    .fit(100., 100.);
                    fitted
                        .bounds(domain)
                        .map(|[[x0, y0], [x1, y1]]| ((y1 - y0) / (x1 - x0)).clamp(0.2, 5.))
                        .filter(|r| r.is_finite())
                        .unwrap_or(0.618)
                }
                None => p.aspect_ratio().unwrap_or(0.618),
            };
            let far = ((1.1 * nfy - 0.1) / (1.1 * nfx - 0.1)) as f64 * ratio;
            ((width - ml - mr) as f64 * far.clamp(0.1, 10.) + (mt + mb) as f64).round() as f32
        });
        let chart = match (self.height, self.aspect_ratio) {
            (Some(height), _) => height,
            _ if projected.is_some() => projected.unwrap_or(400.),
            (None, Some(ratio)) => {
                let (y_values, x_values) =
                    (self.y_domain_values(&facets), self.x_domain_values(&facets));
                let length = |values: &[Value], options: &crate::options::ScaleOptions| -> f32 {
                    if ordinal(&mut values.iter().cloned(), options) {
                        let n = crate::scale::dedup(values.iter().map(Value::key))
                            .len()
                            .max(1);
                        return n as f32;
                    }
                    let [lo, hi] = options
                        .numeric_domain()
                        .or_else(|| crate::scale::extent(values.iter().cloned()))
                        .unwrap_or([0., 1.]);
                    let t = |v: f64| match options.r#type {
                        Some(crate::options::ScaleType::Log) => v.max(f64::MIN_POSITIVE).log10(),
                        Some(crate::options::ScaleType::Sqrt) => v.sqrt(),
                        _ => v,
                    };
                    (t(hi) - t(lo)).abs().max(f64::MIN_POSITIVE) as f32
                };
                let ratio =
                    length(&y_values, &self.scales.y) / (length(&x_values, &self.scales.x) * ratio);
                let w = (width - ml - mr) / (1.1 * nfx - 0.1).max(1.);
                ratio * w * (1.1 * nfy - 0.1).max(1.) + mt + mb
            }
            (None, None) => {
                // Plot's autoHeight counts a y scale whether or not its axis
                // draws. An axisY or gridY mark reads the y scale even
                // without data; Plot's `ny` is 1 without one, even when fy
                // facets the plot.
                let y_scale = self.y_domain_iter(&facets).any(|v| !v.is_null())
                    || self
                        .marks
                        .iter()
                        .any(|m| matches!(m.kind, MarkKind::AxisY | MarkKind::GridY));
                let has_y =
                    self.y_domain_iter(&facets).next().is_some() || y_scale || grid.fy.is_some();
                // Plot's `ny` counts the y scale alone (one row without one);
                // fy multiplies it.
                let ny = if !y_scale {
                    1.
                } else if ordinal(&mut self.y_domain_iter(&facets), &self.scales.y)
                    // A bar or cell reads its band axis as ordinal, years
                    // and all.
                    || (self.scales.y.r#type.is_none() && marks_use_band(&facets, false))
                {
                    crate::scale::dedup(self.y_domain_iter(&facets).map(|v| v.key()))
                        .len()
                        .max(1) as f32
                } else {
                    (17. / nfy).max(7.)
                };
                let base = if has_y {
                    (ny * nfy).clamp(1., 60.) * 20.
                } else {
                    0.
                };
                base + if grid.fx.is_some() { 30. } else { 0. } + 60.
            }
        };
        // Plot grows its automatic height by whatever explicit top and
        // bottom margins add beyond the default ones.
        let chart = if self.height.is_none() {
            let defaults = {
                let mut plot = self.clone_options();
                plot.margins = [None; 4];
                plot.plot_margins(&grid)
            };
            chart + (mt - defaults[0] + mb - defaults[2]).max(0.)
        } else {
            chart
        };
        // The figure's extras: the title, legend and caption bands.
        let mut extras = 0.;
        if self.title.text.is_some() {
            extras += 13. + 6.;
            if self.title.subtitle.is_some() {
                extras += 18.;
            }
        }
        if self.legend_position() != LegendPosition::None {
            extras += if self.color_is_continuous(&facets) {
                50.
            } else {
                LEGEND_BAND + 13.
            };
        }
        if self.caption.is_some() {
            extras += 18.;
        }
        (chart + extras, (grid, facets, populated))
    }

    /// The resolved layout: the insets the frame reserves.
    ///
    /// Plot's margins follow the axes and legend it draws, so the plot shrinks
    /// to fit them. A faceted figure repeats its axis gutters inside every
    /// facet, so the axis insets are returned separately from the figure's
    /// own margins for the title, legend and facet labels.
    fn insets(
        &self,
        theme: &PlotTheme,
        window: &mut Window,
        facets: &[Vec<PreparedMark>],
        grid: &Grid,
        width: f32,
    ) -> (Insets, Insets) {
        let margins = self.plot_margins(grid);
        let _ = AXIS_LABEL_GAP;
        let mut insets = Insets {
            top: margins[0],
            right: margins[1],
            bottom: margins[2],
            left: margins[3],
        };
        // Plot draws a facet's axes at the grid's edges, in the margins, so a
        // facet's own band holds only its marks.
        let facet_insets = Insets::default();
        if let Some(caption) = &self.caption {
            let _ = caption;
            insets.bottom += theme.label_size.as_f32() + 8.;
        }

        if self.title.text.is_some() {
            insets.top += theme.title_size.as_f32() + 6.;
            if self.title.subtitle.is_some() {
                insets.top += theme.label_size.as_f32() + 8.;
            }
        }

        let colors = self.plot_colors(theme, facets);
        let legend_entries = self.legend_entries(&colors, facets);
        if self.legend_position() != LegendPosition::None && !legend_entries.is_empty() {
            // The band is the height the legend's own layout takes at this
            // width: a heading, wrapped rows and quantity rows all count.
            let (_, band) = self.legend_layout(theme, window, &legend_entries, width);
            match self.legend_position() {
                LegendPosition::Top => insets.top += band,
                LegendPosition::Bottom => insets.bottom += band,
                LegendPosition::Left => {
                    insets.left += self.legend_width(theme, window, &legend_entries)
                }
                LegendPosition::Right => {
                    insets.right += self.legend_width(theme, window, &legend_entries)
                }
                LegendPosition::None => {}
            }
        }

        (insets, facet_insets)
    }

    /// Plot's margins, `[top, right, bottom, left]`: the space the axes ask
    /// for around the plot, or the explicit `margin*` options.
    pub(super) fn plot_margins(&self, grid: &Grid) -> [f32; 4] {
        // Plot's margins come from its axes: a bottom x axis wants 30 pixels
        // below and 20 at either side; a left y axis 40 to the left and 20
        // above and below. A top x axis or a right y axis move theirs, and the
        // facet axes (fx on top, fy on the right) add theirs. An explicit
        // `margin*` option wins.
        use crate::options::AxisAnchor;
        // Each axis mark brings its own margins; without any, the plot's axis.
        let anchors = |x: bool| -> Vec<AxisAnchor> {
            let kind = if x { MarkKind::AxisX } else { MarkKind::AxisY };
            let default = if x { AxisAnchor::Bottom } else { AxisAnchor::Left };
            let marks: Vec<AxisAnchor> = self
                .marks
                .iter()
                .filter(|m| m.kind == kind)
                .map(|m| m.axis.axis.flatten().unwrap_or(default))
                .collect();
            if marks.is_empty() {
                if x { self.x_axis_anchor() } else { self.y_axis_anchor() }
                    .into_iter()
                    .collect()
            } else if (if x { self.x_axis_anchor() } else { self.y_axis_anchor() }).is_none() {
                Vec::new()
            } else {
                marks
            }
        };
        let mut margins = [0.0_f32; 4];
        let mut want = |side: usize, value: f32| margins[side] = margins[side].max(value);
        for anchor in anchors(true) {
            want(1, 20.);
            want(3, 20.);
            match anchor {
                AxisAnchor::Top => want(0, 30.),
                AxisAnchor::Both => {
                    want(0, 30.);
                    want(2, 30.);
                }
                _ => want(2, 30.),
            }
        }
        for anchor in anchors(false) {
            want(0, 20.);
            want(2, 20.);
            match anchor {
                AxisAnchor::Right => want(1, 40.),
                AxisAnchor::Both => {
                    want(1, 40.);
                    want(3, 40.);
                }
                _ => want(3, 40.),
            }
        }
        // A facet axis brings the margins of Plot's `axisFx` (20 either side)
        // and `axisFy` (20 above and below) too.
        if grid.fx.is_some() {
            // An `axisFx` mark brings its own margins in place of the
            // implicit axis's.
            let anchors: Vec<AxisAnchor> = match self.facet_axis_mark(true) {
                Some(_) => self
                    .marks
                    .iter()
                    .filter(|m| m.facet_axis == Some(crate::mark::Axis::X))
                    .map(|m| Self::facet_axis_anchor(m, true))
                    .collect(),
                None if self.scales.fx.axis != Some(None) && !self.no_axes => {
                    vec![if self.fx_axis_bottom() {
                        AxisAnchor::Bottom
                    } else {
                        AxisAnchor::Top
                    }]
                }
                None => Vec::new(),
            };
            for anchor in anchors {
                want(if anchor == AxisAnchor::Bottom { 2 } else { 0 }, 30.);
                want(1, 20.);
                want(3, 20.);
            }
        }
        if grid.fy.is_some() {
            let anchors: Vec<AxisAnchor> = match self.facet_axis_mark(false) {
                Some(_) => self
                    .marks
                    .iter()
                    .filter(|m| m.facet_axis == Some(crate::mark::Axis::Y))
                    .map(|m| Self::facet_axis_anchor(m, false))
                    .collect(),
                None if self.scales.fy.axis != Some(None) && !self.no_axes => {
                    vec![if self.fy_axis_left() {
                        AxisAnchor::Left
                    } else {
                        AxisAnchor::Right
                    }]
                }
                None => Vec::new(),
            };
            for anchor in anchors {
                want(if anchor == AxisAnchor::Left { 3 } else { 1 }, 40.);
                want(0, 20.);
                want(2, 20.);
            }
        }
        for (side, explicit) in self.margins.iter().enumerate() {
            if let Some(margin) = explicit {
                margins[side] = *margin;
            }
        }
        margins
    }

    /// Resolve the frame for `bounds`, including the margins the axes and
    /// title need.
    ///
    /// A faceted figure lays its x and y scales out inside one facet, so the
    /// frame is told that facet's size up front; the plot's own bounds still
    /// hold the whole grid.
    pub(super) fn resolve_frame(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        facets: &[Vec<PreparedMark>],
        grid: &Grid,
    ) -> Frame {
        let mut options = self.frame_options.clone();
        options.x_options = self.scales.x.clone();
        options.y_options = self.scales.y.clone();
        // A mark's `sort` option orders an ordinal domain the scale left open.
        if options.x_options.domain.is_none() {
            options.x_options.domain = sorted_domain(facets, "x");
        }
        if options.y_options.domain.is_none() {
            options.y_options.domain = sorted_domain(facets, "y");
        }
        let (insets, facet_insets) =
            self.insets(theme, window, facets, grid, bounds.size.width.as_f32());
        options.insets = insets;
        options.facet_insets = facet_insets;
        // A mark that draws a band on an axis — a bar's width, a cell's
        // extent — needs a band scale there, not a point scale: a point
        // scale puts the first and last category on the plot's edges, and
        // their bands would hang off the plot and be clipped in half. Plot
        // infers `band` for barX/barY/cell and `point` for everything else.
        if options.x_kind.is_none() && marks_use_band(facets, true) {
            options.x_kind = Some(ScaleKind::Band);
        }
        if options.y_kind.is_none() && marks_use_band(facets, false) {
            options.y_kind = Some(ScaleKind::Band);
        }
        // A plot may ask for its own height, Plot's `height`: the height of
        // its chart, below whatever title and legend the figure adds. The
        // element fills its container, so the request shrinks the figure
        // to that rather than growing it.
        let margins = self.plot_margins(grid);
        let extras = (options.insets.top - margins[0]) + (options.insets.bottom - margins[2]);
        let bounds = match self.height {
            Some(height) if height > 0. => Bounds {
                size: gpui::size(
                    bounds.size.width,
                    px(height + extras).min(bounds.size.height.max(px(height + extras))),
                ),
                ..bounds
            },
            _ => bounds,
        };
        let bounds = match self.width {
            Some(width) if width > 0. && px(width) < bounds.size.width => Bounds {
                size: gpui::size(px(width), bounds.size.height),
                ..bounds
            },
            _ => bounds,
        };
        // The grid divides the plot the insets leave, so one facet's size
        // follows from the plot's size, the number of facets and the axes each
        // facet draws inside its own band.
        let plot_width =
            (bounds.size.width.as_f32() - options.insets.left - options.insets.right).max(1.);
        let plot_height =
            (bounds.size.height.as_f32() - options.insets.top - options.insets.bottom).max(1.);
        options.facet_size = grid.is_faceted().then(|| {
            let band_width = grid.facet_width(plot_width);
            let band_height = grid.facet_height(plot_height);
            [
                (band_width - facet_insets.left - facet_insets.right).max(1.),
                (band_height - facet_insets.top - facet_insets.bottom).max(1.),
            ]
        });
        let mut frame = Frame::resolve(
            bounds,
            &options,
            &self.x_domain_values(facets),
            &self.y_domain_values(facets),
        );
        // Plot's projection `inset` defaults to the plot's own `inset`.
        frame.projection = self.projection.as_ref().map(|p| {
            if p.inset == 0. && self.frame_options.inset != 0. {
                p.clone()
                    .inset(self.frame_options.inset)
                    .fit(frame.layout[0] as f64, frame.layout[1] as f64)
            } else {
                p.fit(frame.layout[0] as f64, frame.layout[1] as f64)
            }
        });
        frame
    }
}

/// The frame one facet cell draws in: the figure's frame moved to the cell's
/// marks area. An unfaceted figure has one cell, the whole plot.
pub(super) fn cell_frame(frame: &Frame, grid: &Grid, cell: usize) -> Frame {
    if grid.is_faceted() {
        frame.facet(crate::frame::facet_marks(
            grid.facet(frame.plot, cell),
            frame,
        ))
    } else {
        frame.clone()
    }
}
