//! The axis labels: the label each axis derives or is given, and the
//! painters for the x, y and facet axes' labels.

use gpui::{App, SharedString, Window};

use crate::facet::Grid;
use crate::frame::{Frame, Insets};
use crate::mark::{Mark, MarkKind};
use crate::plot::Plot;
use crate::render;
use crate::theme::PlotTheme;
use crate::value::Value;

impl Plot {
    /// The facet scales' axis labels, where Plot puts them: an explicit
    /// `label` (or the facet field's name), centred along the axis in the
    /// margin's outer edge (`fy`'s rotated), as Plot labels any band axis.
    pub(super) fn paint_facet_axis_labels(
        &self,
        frame: &Frame,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
        grid: &Grid,
    ) {
        use crate::options::LabelAnchor;
        // The label: an axis mark's own, else the scale's, else the facet
        // field's name, which Plot's `formatAxisLabel` drops when it is a
        // "date", "time" or "year" over a domain of dates.
        let label_of = |options: &crate::options::ScaleOptions,
                        field: &Option<SharedString>,
                        mark: Option<&Mark>,
                        keys: &Option<Vec<crate::value::ValueKey>>|
         -> Option<SharedString> {
            if let Some(label) = mark.and_then(|m| m.axis.label.clone()) {
                return label;
            }
            match &options.label {
                Some(label) => label.clone(),
                None if self.no_labels => None,
                None => field.clone().filter(|label| {
                    let temporal = keys
                        .iter()
                        .flatten()
                        .any(|key| matches!(key.to_value(), Value::Date(_)));
                    !(temporal
                        && ["date", "time", "year"]
                            .iter()
                            .any(|n| label.eq_ignore_ascii_case(n)))
                }),
            }
        };
        let (fx_mark, fy_mark) = (self.facet_axis_mark(true), self.facet_axis_mark(false));
        // An axis mark labels its scale wherever the plot's axes are; the
        // implicit axis only when shown.
        let shown = |options: &crate::options::ScaleOptions, mark: Option<&Mark>| {
            mark.is_some() || (options.axis != Some(None) && !self.no_axes)
        };
        // The facet field: the plot's, else the first mark's own facet field.
        let field = |plot_field: &Option<SharedString>, x: bool| -> Option<SharedString> {
            plot_field.clone().or_else(|| {
                self.marks
                    .iter()
                    .find_map(|m| match if x { &m.fx } else { &m.fy } {
                        Some(crate::table::Channel::Field(name)) => Some(name.clone()),
                        _ => None,
                    })
            })
        };
        let (fx_field, fy_field) = (field(&self.fx, true), field(&self.fy, false));
        let [top, right, bottom, left] = self.plot_margins(grid);
        let insets = Insets {
            top,
            right,
            bottom,
            left,
        };
        // Plot's `formatAxisLabel`: a facet scale is ordinal, so its label
        // has an arrow only when it has an interval.
        let arrow = |options: &crate::options::ScaleOptions, label: &str| {
            options.label_arrow.unwrap_or(true)
                && options.interval.is_some()
                && !label.contains(['↑', '↓', '→', '←'])
        };
        if grid.fx.is_some()
            && shown(&self.scales.fx, fx_mark)
            && let Some(label) = label_of(&self.scales.fx, &fx_field, fx_mark, &grid.fx)
        {
            let options = &self.scales.fx;
            let text = if !arrow(options, &label) {
                label.to_string()
            } else if options.reverse == Some(true) {
                format!("← {label}")
            } else {
                format!("{label} →")
            };
            let top = match fx_mark {
                Some(mark) => {
                    Self::facet_axis_anchor(mark, true) != crate::options::AxisAnchor::Bottom
                }
                None => !self.fx_axis_bottom(),
            };
            // Plot's `axisFx` label: centred by default, `labelOffset` past
            // the plot's edge, by default the margin less three pixels.
            let offset = options
                .label_offset
                .unwrap_or(if top { insets.top } else { insets.bottom } - 3.);
            crate::frame::paint_horizontal_axis_label(
                frame.plot,
                &insets,
                &text,
                top,
                options.label_anchor.unwrap_or(LabelAnchor::Center),
                offset,
                theme,
                window,
                cx,
            );
        }
        if grid.fy.is_some()
            && shown(&self.scales.fy, fy_mark)
            && let Some(label) = label_of(&self.scales.fy, &fy_field, fy_mark, &grid.fy)
        {
            let options = &self.scales.fy;
            let right = match fy_mark {
                Some(mark) => {
                    Self::facet_axis_anchor(mark, false) != crate::options::AxisAnchor::Left
                }
                None => !self.fy_axis_left(),
            };
            let label_anchor = options.label_anchor.unwrap_or(LabelAnchor::Center);
            // An ordinal range runs top to bottom: its values grow
            // downward unless reversed.
            let up = options.reverse == Some(true);
            let text = crate::frame::vertical_axis_label_text(
                &label,
                arrow(options, &label),
                up,
                label_anchor == LabelAnchor::Center,
                right,
            );
            let offset = options
                .label_offset
                .unwrap_or(if right { insets.right } else { insets.left } - 3.);
            crate::frame::paint_vertical_axis_label(
                frame.plot,
                &insets,
                &text,
                right,
                label_anchor,
                offset,
                theme,
                window,
                cx,
            );
        }
    }

    /// The label an axis shows, Plot's default scale label: the first data
    /// mark's field on that axis, or `Frequency` for the count a bin grows
    /// along it. An explicit label overrides it, and an empty one hides it.
    fn axis_label(&self, x: bool) -> Option<SharedString> {
        let scale = if x { &self.scales.x } else { &self.scales.y };
        if let Some(label) = &scale.label {
            return label.clone();
        }
        if let Some(mark) = self.axis_mark(x)
            && let Some(label) = &mark.axis.label
        {
            return label.clone();
        }
        self.scale_label(x)
    }

    /// The scale's own label, which an axis mark without a `label` shows:
    /// the `label` option, else the one derived from the channels.
    fn scale_label(&self, x: bool) -> Option<SharedString> {
        let scale = if x { &self.scales.x } else { &self.scales.y };
        if let Some(label) = &scale.label {
            return label.clone();
        }
        if self.no_labels {
            return None;
        }
        let label = self.derived_axis_label(x)?;
        // Plot's `formatAxisLabel`: an inferred "date", "time" or "year" on
        // a temporal axis says nothing the ticks don't.
        if ["date", "time", "year"]
            .iter()
            .any(|n| label.eq_ignore_ascii_case(n))
            && self.axis_is_temporal(x)
        {
            return None;
        }
        // Plot's `percent` marks a derived label as a percentage.
        if scale.percent == Some(true) {
            return Some(format!("{label} (%)").into());
        }
        Some(label)
    }

    /// Whether an axis's scale is temporal, from the cached frame.
    fn axis_is_temporal(&self, x: bool) -> bool {
        self.frame_cache
            .as_ref()
            .is_some_and(|f| if x { f.x.temporal } else { f.y.temporal })
    }

    /// The label an axis derives from its channels, before scale options.
    fn derived_axis_label(&self, x: bool) -> Option<SharedString> {
        let explicit = if x { &self.x_label } else { &self.y_label };
        if let Some(label) = explicit {
            return (!label.is_empty()).then(|| label.clone());
        }
        // Plot's `inferScaleLabel`: every channel bound to the scale offers a
        // label (its field's name; an accessor or constant offers none), and
        // the scale takes it only when all agree. A bin's count is
        // "Frequency".
        let field = |channel: Option<&crate::table::Channel>| -> Option<SharedString> {
            match channel {
                Some(crate::table::Channel::Field(name)) if !name.starts_with('$') => {
                    Some(name.clone())
                }
                _ => None,
            }
        };
        let mut labels: Vec<Option<SharedString>> = Vec::new();
        for mark in &self.marks {
            if mark.kind.is_furniture() {
                continue;
            }
            let channels = &mark.channels;
            if let Some(bin) = &mark.bin {
                let (bin_x, bin_y) = render::bin_axes(mark);
                let length = if x { bin_y && !bin_x } else { bin_x && !bin_y };
                if length {
                    let name = if x { "x" } else { "y" };
                    labels.push(
                        match bin.outputs.iter().find(|output| output.name == name) {
                            Some(output)
                                if !matches!(
                                    output.reducer,
                                    crate::transform::GroupReducer::Count
                                ) =>
                            {
                                output.input.clone()
                            }
                            _ => Some("Frequency".into()),
                        },
                    );
                    continue;
                }
                if (x && bin_x && !bin_y) || (!x && bin_y && !bin_x) {
                    labels.push(field(channels.x.as_ref()).or_else(|| field(channels.y.as_ref())));
                    continue;
                }
            }
            // A group's count output is "Frequency" too, including the
            // default count a groupX writes to y.
            if let Some(group) = &mark.group {
                let names: &[&str] = if x {
                    &["x", "x1", "x2"]
                } else {
                    &["y", "y1", "y2"]
                };
                let by_x = group.both || group.axis == Some(crate::mark::Axis::X);
                let by_y = group.both || group.axis == Some(crate::mark::Axis::Y);
                if group.outputs.is_empty() && ((!x && by_x && !by_y) || (x && by_y && !by_x)) {
                    labels.push(Some("Frequency".into()));
                    continue;
                }
                if let Some(output) = group
                    .outputs
                    .iter()
                    .find(|o| names.contains(&o.name.as_ref()))
                {
                    labels.push(match output.reducer {
                        crate::transform::GroupReducer::Count => Some("Frequency".into()),
                        _ => field(if x {
                            channels.x.as_ref()
                        } else {
                            channels.y.as_ref()
                        }),
                    });
                    continue;
                }
            }
            if !x
                && mark
                    .stack
                    .as_ref()
                    .is_some_and(|stack| !stack.fields.is_empty())
            {
                return None;
            }
            let bound: [Option<&crate::table::Channel>; 3] = if x {
                [
                    channels.x.as_ref(),
                    channels.x1.as_ref(),
                    channels.x2.as_ref(),
                ]
            } else {
                [
                    channels.y.as_ref(),
                    channels.y1.as_ref(),
                    channels.y2.as_ref(),
                ]
            };
            for channel in bound.into_iter().flatten() {
                labels.push(field(Some(channel)));
            }
        }
        let mut named = labels.iter().flatten();
        let first = named.next()?.clone();
        if named.any(|label| *label != first) {
            return None;
        }
        Some(first)
    }

    /// Paint the axis labels where Plot puts them: the y label in the top
    /// margin at the figure's left as `↑ label`, the x label in the bottom
    /// margin at the plot's right as `label →`. A categorical axis has no
    /// direction, so no arrow.
    ///
    /// Each axis mark labels itself, at its own anchor and `labelAnchor`,
    /// with its `label` or else the scale's; without axis marks the plot's
    /// own axes are labelled.
    pub(super) fn paint_axis_labels(
        &self,
        frame: &Frame,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::options::{AxisAnchor, LabelAnchor};
        let continuous = |scale: &crate::scale::PositionScale| {
            matches!(scale, crate::scale::PositionScale::Continuous(_))
        };
        let grid = self
            .facet_cache
            .clone()
            .unwrap_or_else(|| self.facet_grid());
        let [top, right, bottom, left] = self.plot_margins(&grid);
        let insets = Insets {
            top,
            right,
            bottom,
            left,
        };
        let plot = frame.plot;
        // The labels each axis draws: its anchor, label and label anchor.
        let specs = |x: bool| -> Vec<(AxisAnchor, SharedString, Option<LabelAnchor>)> {
            let kind = if x { MarkKind::AxisX } else { MarkKind::AxisY };
            let scale = if x { &self.scales.x } else { &self.scales.y };
            let marks: Vec<&Mark> = self.marks.iter().filter(|m| m.kind == kind).collect();
            if marks.is_empty() {
                let anchor = if x {
                    self.x_axis_anchor()
                } else {
                    self.y_axis_anchor()
                };
                return anchor
                    .zip(self.axis_label(x))
                    .map(|(anchor, label)| (anchor, label, scale.label_anchor))
                    .into_iter()
                    .collect();
            }
            let default = if x {
                AxisAnchor::Bottom
            } else {
                AxisAnchor::Left
            };
            marks
                .into_iter()
                .filter_map(|mark| {
                    let label = match &mark.axis.label {
                        Some(label) => label.clone(),
                        None => self.scale_label(x),
                    }?;
                    Some((
                        mark.axis.axis.flatten().unwrap_or(default),
                        label,
                        mark.axis.label_anchor.or(scale.label_anchor),
                    ))
                })
                .collect()
        };
        for (anchor, label, label_anchor) in specs(false) {
            // Plot's `formatAxisLabel`: an ordinal scale has no direction, so
            // no arrow unless it has an interval.
            let ordinal = !continuous(&frame.y.scale);
            let arrow = self.scales.y.label_arrow.unwrap_or(true)
                && (!ordinal || self.scales.y.interval.is_some())
                && !label.contains(['↑', '↓', '→', '←']);
            let reverse = self
                .scales
                .y
                .reverse
                .unwrap_or(self.frame_options.y_reverse);
            // An ordinal y range runs top to bottom, so its values grow
            // downward unless reversed.
            let up = reverse == ordinal;
            let right = anchor == AxisAnchor::Right;
            // Plot centres an ordinal scale's label, else puts it on top.
            let label_anchor = label_anchor.unwrap_or(if ordinal {
                LabelAnchor::Center
            } else {
                LabelAnchor::End
            });
            let text = crate::frame::vertical_axis_label_text(
                &label,
                arrow,
                up,
                label_anchor == LabelAnchor::Center,
                right,
            );
            let offset = self
                .scales
                .y
                .label_offset
                .unwrap_or(if right { insets.right } else { insets.left } - 3.);
            crate::frame::paint_vertical_axis_label(
                plot,
                &insets,
                &text,
                right,
                label_anchor,
                offset,
                theme,
                window,
                cx,
            );
        }
        for (anchor, label, label_anchor) in specs(true) {
            let arrow = self.scales.x.label_arrow.unwrap_or(true)
                && (continuous(&frame.x.scale) || self.scales.x.interval.is_some())
                && !label.contains(['↑', '↓', '→', '←']);
            let forward = !self
                .scales
                .x
                .reverse
                .unwrap_or(self.frame_options.x_reverse);
            let text = if !arrow {
                label.to_string()
            } else if forward {
                format!("{label} →")
            } else {
                format!("← {label}")
            };
            let top = anchor == AxisAnchor::Top;
            // A band scale centres its label, as Plot does; a continuous one
            // puts it at the axis's end.
            let label_anchor = label_anchor.unwrap_or(if continuous(&frame.x.scale) {
                LabelAnchor::End
            } else {
                LabelAnchor::Center
            });
            let offset = self
                .scales
                .x
                .label_offset
                .unwrap_or(if top { insets.top } else { insets.bottom } - 3.);
            crate::frame::paint_horizontal_axis_label(
                plot,
                &insets,
                &text,
                top,
                label_anchor,
                offset,
                theme,
                window,
                cx,
            );
        }
    }
}
