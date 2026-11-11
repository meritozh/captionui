//! The axes: where the x, y and facet axes sit, the axis marks that
//! replace the implicit ones, the facet axis marks resolved into text
//! marks, and the axes layer painted over the marks.

use gpui::{App, SharedString, Window};

use crate::facet::Grid;
use crate::frame::Frame;
use crate::mark::{Mark, MarkKind};
use crate::plot::Plot;
use crate::plot::layout::cell_frame;
use crate::table::{Dataset, Table};
use crate::theme::PlotTheme;
use crate::value::Value;

impl Plot {
    /// The first `axisFx` (`x`) or `axisFy` mark: with one, the plot draws no
    /// implicit axis for that facet scale, Plot's `hasAxis`.
    pub(super) fn facet_axis_mark(&self, x: bool) -> Option<&Mark> {
        let axis = if x {
            crate::mark::Axis::X
        } else {
            crate::mark::Axis::Y
        };
        self.marks.iter().find(|m| m.facet_axis == Some(axis))
    }

    /// Where a facet axis mark sits: its `anchor`, else Plot's `axisFx` top
    /// or `axisFy` right.
    pub(super) fn facet_axis_anchor(mark: &Mark, x: bool) -> crate::options::AxisAnchor {
        use crate::options::AxisAnchor;
        mark.axis
            .axis
            .flatten()
            .unwrap_or(if x { AxisAnchor::Top } else { AxisAnchor::Right })
    }

    /// The marks with each `axisFx`/`axisFy` resolved against the facet
    /// domain into the text mark Plot's `axisTextKx`/`axisTextKy` make: one
    /// label per facet value, at the facet frame's `frameAnchor` (the axis's
    /// anchor by default), offset by `tickSize + tickPadding` outward, in the
    /// facets along the anchor's edge. `None` when there are none.
    pub(super) fn resolve_facet_axes(&self, marks: &[Mark], grid: &Grid) -> Option<Vec<Mark>> {
        use crate::mark::{FrameAnchor, LineAnchor, TextAnchor};
        use crate::options::AxisAnchor;
        if !marks.iter().any(|m| m.facet_axis.is_some()) {
            return None;
        }
        let resolve = |mark: &Mark, x: bool| -> Mark {
            let mut mark = mark.clone();
            mark.facet_axis = Some(if x {
                crate::mark::Axis::X
            } else {
                crate::mark::Axis::Y
            });
            let anchor = Self::facet_axis_anchor(&mark, x);
            let options = mark.axis.clone();
            let tick_size = options.tick_size.unwrap_or(0.);
            let rotate = options.tick_rotate.unwrap_or(0.);
            let style = &mut mark.style;
            if x {
                let tick_padding = options.tick_padding.unwrap_or(
                    (9. - tick_size).max(3.)
                        + if rotate.abs() >= 10. {
                            4. * rotate.to_radians().cos()
                        } else {
                            0.
                        },
                );
                let bottom = anchor == AxisAnchor::Bottom;
                if !style.text_anchor_set {
                    style.text_anchor = if rotate.abs() >= 10. {
                        if (rotate < 0.) ^ bottom {
                            TextAnchor::Start
                        } else {
                            TextAnchor::End
                        }
                    } else {
                        TextAnchor::Middle
                    };
                    style.text_anchor_set = true;
                }
                if !style.line_anchor_set {
                    style.line_anchor = if rotate.abs() >= 10. {
                        LineAnchor::Middle
                    } else if bottom {
                        LineAnchor::Top
                    } else {
                        LineAnchor::Bottom
                    };
                    style.line_anchor_set = true;
                }
                style
                    .frame_anchor
                    .get_or_insert(if bottom { FrameAnchor::Bottom } else { FrameAnchor::Top });
                style.dy += if bottom {
                    tick_size + tick_padding
                } else {
                    -tick_size - tick_padding
                };
            } else {
                let tick_padding = options.tick_padding.unwrap_or(
                    (9. - tick_size).max(3.)
                        + if rotate.abs() > 60. {
                            4. * rotate.to_radians().cos()
                        } else {
                            0.
                        },
                );
                let left = anchor == AxisAnchor::Left;
                if !style.text_anchor_set {
                    style.text_anchor = if rotate.abs() > 60. {
                        TextAnchor::Middle
                    } else if left {
                        TextAnchor::End
                    } else {
                        TextAnchor::Start
                    };
                    style.text_anchor_set = true;
                }
                if !style.line_anchor_set {
                    style.line_anchor = if rotate > 60. {
                        LineAnchor::Top
                    } else if rotate < -60. {
                        LineAnchor::Bottom
                    } else {
                        LineAnchor::Middle
                    };
                    style.line_anchor_set = true;
                }
                style
                    .frame_anchor
                    .get_or_insert(if left { FrameAnchor::Left } else { FrameAnchor::Right });
                style.dx += if left {
                    -tick_size - tick_padding
                } else {
                    tick_size + tick_padding
                };
            }
            if rotate != 0. {
                style.text_rotate = rotate;
            }
            // The labels: each facet value through the axis's `tickFormat`,
            // else Plot's inferred format (the multi-line one for dates).
            let keys = if x { &grid.fx } else { &grid.fy };
            let values: Vec<Value> = keys
                .iter()
                .flatten()
                .map(crate::value::ValueKey::to_value)
                .collect();
            let millis: Option<Vec<f64>> = values
                .iter()
                .map(|v| match v {
                    Value::Date(_) => v.as_number(),
                    _ => None,
                })
                .collect();
            let labels: Vec<SharedString> = match (&options.tick_format, millis) {
                (Some(format), _) => values.iter().map(|v| format.format(v).into()).collect(),
                (None, Some(millis)) => {
                    crate::ticks::plot_time_labels(&millis, anchor == AxisAnchor::Top)
                        .into_iter()
                        .map(SharedString::from)
                        .collect()
                }
                (None, None) => values
                    .iter()
                    .map(|v| match v {
                        Value::Null => "null".into(),
                        other => other.label(),
                    })
                    .collect(),
            };
            // Plot's `facetAnchor`: an fx axis labels the facets along its
            // edge of the grid, an fy axis those along its side.
            let other = if x { &grid.fy } else { &grid.fx };
            let edge = other.as_ref().and_then(|keys| {
                let first = matches!(anchor, AxisAnchor::Top | AxisAnchor::Left);
                if first { keys.first() } else { keys.last() }.map(crate::value::ValueKey::to_value)
            });
            let (own, cross) = if x {
                ("$facet_x", "$facet_y")
            } else {
                ("$facet_y", "$facet_x")
            };
            let rows: Vec<Table> = values
                .into_iter()
                .zip(labels)
                .map(|(value, label)| {
                    let mut row = Table::empty()
                        .with(own, value)
                        .with("$facet_text", Value::text(label.to_string()));
                    if let Some(edge) = &edge {
                        row = row.with(cross, edge.clone());
                    }
                    row
                })
                .collect();
            mark.data = Some(Dataset::new(rows));
            let field = |name: &str| Some(crate::table::Channel::Field(name.to_string().into()));
            mark.channels.text = field("$facet_text");
            let (fx, fy) = if x {
                (field(own), edge.as_ref().and_then(|_| field(cross)))
            } else {
                (edge.as_ref().and_then(|_| field(cross)), field(own))
            };
            mark.fx = fx;
            mark.fy = fy;
            mark
        };
        Some(
            marks
                .iter()
                .map(|mark| match mark.facet_axis {
                    Some(crate::mark::Axis::X) => resolve(mark, true),
                    Some(_) => resolve(mark, false),
                    None => mark.clone(),
                })
                .collect(),
        )
    }

    /// Where the x axis sits, or `None` when the plot draws none: Plot's
    /// `x.axis`, the plot's `axis: null`, and whether any mark reads x.
    pub(crate) fn x_axis_anchor(&self) -> Option<crate::options::AxisAnchor> {
        if self.projection.is_some()
            && self.scales.x.axis.is_none()
            && self.axis_mark(true).is_none()
        {
            return None;
        }
        if let Some(mark) = self.axis_mark(true) {
            return Some(
                mark.axis
                    .axis
                    .flatten()
                    .unwrap_or(crate::options::AxisAnchor::Bottom),
            );
        }
        if !self.x_axis {
            return None;
        }
        match self.scales.x.axis {
            Some(anchor) => anchor,
            None if self.no_axes => None,
            None => self
                .reads_axis(true)
                .then_some(crate::options::AxisAnchor::Bottom),
        }
    }

    /// Where the y axis sits, or `None` when the plot draws none.
    pub(crate) fn y_axis_anchor(&self) -> Option<crate::options::AxisAnchor> {
        if self.projection.is_some()
            && self.scales.y.axis.is_none()
            && self.axis_mark(false).is_none()
        {
            return None;
        }
        if let Some(mark) = self.axis_mark(false) {
            return Some(
                mark.axis
                    .axis
                    .flatten()
                    .unwrap_or(crate::options::AxisAnchor::Left),
            );
        }
        if !self.y_axis {
            return None;
        }
        match self.scales.y.axis {
            Some(anchor) => anchor,
            None if self.no_axes => None,
            None => self
                .reads_axis(false)
                .then_some(crate::options::AxisAnchor::Left),
        }
    }

    /// Plot's facet axis placement: fx on top, unless the x axis is absent
    /// or on top, when it goes to the bottom.
    pub(super) fn fx_axis_bottom(&self) -> bool {
        match self.scales.fx.axis {
            Some(Some(crate::options::AxisAnchor::Bottom)) => true,
            Some(Some(_)) => false,
            _ => matches!(
                self.x_axis_anchor(),
                None | Some(crate::options::AxisAnchor::Top)
            ),
        }
    }

    /// fy on the right, unless the y axis is absent or on the right.
    pub(super) fn fy_axis_left(&self) -> bool {
        match self.scales.fy.axis {
            Some(Some(crate::options::AxisAnchor::Left)) => true,
            Some(Some(_)) => false,
            _ => matches!(
                self.y_axis_anchor(),
                None | Some(crate::options::AxisAnchor::Right)
            ),
        }
    }

    /// The first axis mark for x (or y), Plot's `axisX`: when a plot has one,
    /// it draws no implicit axis of its own for that scale.
    pub(super) fn axis_mark(&self, x: bool) -> Option<&Mark> {
        let kind = if x { MarkKind::AxisX } else { MarkKind::AxisY };
        self.marks.iter().find(|mark| mark.kind == kind)
    }

    /// Paint the axes over the marks: each facet's at the grid's outer
    /// edges (Plot's facet anchors), the facet axes, then the axis labels.
    pub(super) fn paint_axes_layer(
        &self,
        frame: &Frame,
        grid: &Grid,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
    ) {
        let x_style = self
            .x_axis_anchor()
            .filter(|_| self.axis_mark(true).is_none())
            .map(|anchor| crate::frame::AxisStyle::new(anchor, &self.scales.x));
        let y_style = self
            .y_axis_anchor()
            .filter(|_| self.axis_mark(false).is_none())
            .map(|anchor| crate::frame::AxisStyle::new(anchor, &self.scales.y));
        if grid.is_faceted() {
            for cell in 0..grid.cells.len() {
                if grid.is_empty(cell) {
                    continue;
                }
                crate::frame::paint_axes(
                    &cell_frame(frame, grid, cell),
                    theme,
                    window,
                    cx,
                    x_style,
                    y_style,
                    &crate::frame::AxisEdges::from_cell(cell, grid),
                );
            }
            // Plot's facet axes: hidden by `axis: null` or the facet scale's
            // own `axis: null`, labelled through its tickFormat.
            // An `axisFx`/`axisFy` mark replaces the implicit axis.
            let show = |options: &crate::options::ScaleOptions, x: bool| {
                !self.no_axes && options.axis != Some(None) && self.facet_axis_mark(x).is_none()
            };
            crate::frame::paint_facet_labels(
                frame,
                theme,
                window,
                cx,
                grid,
                show(&self.scales.fx, true).then_some(&self.scales.fx),
                show(&self.scales.fy, false).then_some(&self.scales.fy),
                self.fx_axis_bottom(),
                self.fy_axis_left(),
            );
            self.paint_facet_axis_labels(frame, theme, window, cx, grid);
        } else {
            crate::frame::paint_axes(
                frame,
                theme,
                window,
                cx,
                x_style,
                y_style,
                &crate::frame::AxisEdges::default(),
            );
        }
        self.paint_axis_labels(frame, theme, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn a_facet_axis_mark_labels_each_facet_and_replaces_the_implicit_axis() {
        use crate::mark::{FrameAnchor, LineAnchor, TextAnchor};
        let data = vec![
            Table::empty()
                .with("q", Value::text("a"))
                .with("y", Value::Number(1.)),
            Table::empty()
                .with("q", Value::text("b"))
                .with("y", Value::Number(2.)),
        ];
        let plot = Plot::new(data).mark(
            crate::marks::axis_fx()
                .axis_anchor(crate::options::AxisAnchor::Bottom)
                .dy(20.),
        );
        let plot = plot.fx("q");
        let grid = plot.facet_grid();
        let marks = plot
            .resolve_facet_axes(&plot.marks, &grid)
            .expect("an axis mark resolves");
        let axis = &marks[0];
        let rows = axis.data.as_ref().expect("one row per facet").rows();
        let texts: Vec<_> = rows.iter().map(|r| r.value("$facet_text").label()).collect();
        assert_eq!(texts, ["a", "b"]);
        // Plot's axisTextKx at the bottom: hanging below the frame, its dy
        // grown by the default nine-pixel padding.
        assert_eq!(axis.style.frame_anchor, Some(FrameAnchor::Bottom));
        assert_eq!(axis.style.line_anchor, LineAnchor::Top);
        assert_eq!(axis.style.text_anchor, TextAnchor::Middle);
        assert_eq!(axis.style.dy, 29.);
        // The mark brings a bottom margin in place of the implicit top one.
        let [top, _, bottom, _] = plot.plot_margins(&grid);
        assert_eq!((top, bottom), (0., 30.));
    }
}
