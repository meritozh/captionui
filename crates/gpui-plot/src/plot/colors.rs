//! The plot's color scales over the resolved marks, and whether its
//! color scale is a continuous ramp.

use crate::plot::Plot;
use crate::render::{self, PreparedMark};
use crate::scale::ColorRange;
use crate::theme::PlotTheme;

impl Plot {
    /// Whether the plot's color scale is a continuous ramp, as the scale
    /// `plot_colors` builds would be, without building it.
    pub(super) fn color_is_continuous(&self, facets: &[Vec<PreparedMark>]) -> bool {
        let theme = PlotTheme {
            palette: ColorRange::observable10_colors(),
            ..PlotTheme::from_theme(&gpui_kit::component::Theme::default())
        };
        render::color::color_scale_is_continuous(&theme, facets, &self.scales.color)
    }

    /// The plot's color scales over the resolved marks.
    pub(super) fn plot_colors(
        &self,
        theme: &PlotTheme,
        facets: &[Vec<PreparedMark>],
    ) -> render::color::PlotColors {
        let color = self.color_field.as_ref().zip(self.color_range.as_ref());
        render::color::PlotColors::new(theme, facets, color, &self.scales.color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Table;
    use crate::value::Value;

    #[test]
    fn color_continuity_is_what_the_built_scale_says() {
        use crate::options::ScaleType;
        let theme = PlotTheme {
            palette: ColorRange::observable10_colors(),
            ..PlotTheme::from_theme(&gpui_kit::component::Theme::default())
        };
        let row = |i: usize| {
            let date = crate::value::Value::Date(
                chrono::DateTime::from_timestamp_millis(i as i64 * 86_400_000).unwrap(),
            );
            Table::empty()
                .with("x", Value::Number(i as f64))
                .with("n", Value::Number(i as f64 * 1.5 - 3.))
                .with("t", Value::text(["a", "b", "c"][i % 3]))
                .with("css", Value::text(["red", "#00f", "steelblue"][i % 3]))
                .with(
                    "mixed",
                    if i == 3 {
                        Value::text("odd")
                    } else {
                        Value::Number(i as f64)
                    },
                )
                .with("b", Value::Bool(i.is_multiple_of(2)))
                .with("d", date)
                .with("nulls", Value::Null)
        };
        let data = || (0..8).map(row).collect::<Vec<_>>();
        let fields = ["n", "t", "css", "mixed", "b", "d", "nulls", "missing"];
        let types = [
            None,
            Some(ScaleType::Linear),
            Some(ScaleType::Log),
            Some(ScaleType::Ordinal),
            Some(ScaleType::Categorical),
            Some(ScaleType::Band),
            Some(ScaleType::Point),
            Some(ScaleType::Diverging),
            Some(ScaleType::Threshold),
            Some(ScaleType::Quantize),
            Some(ScaleType::Quantile),
            Some(ScaleType::Identity),
            Some(ScaleType::Utc),
        ];
        let domains: [Option<Vec<Value>>; 5] = [
            None,
            Some(vec![]),
            Some(vec![Value::Number(0.), Value::Number(9.)]),
            Some(vec![
                Value::Number(0.),
                Value::Number(4.),
                Value::Number(9.),
            ]),
            Some(vec![Value::text("a"), Value::Null]),
        ];
        let mut checked = 0;
        for field in fields {
            for kind in types {
                for domain in &domains {
                    for colors in [None, Some(2), Some(3)] {
                        for transform in [false, true] {
                            let plot = Plot::new(data())
                                .dot(|m| m.x("x").y("x").fill(field))
                                .line(|m| m.x("x").y("n").stroke(field));
                            let mut options = crate::options::ScaleOptions::default();
                            if transform {
                                // Negative numbers become NaN.
                                options = options.transform(f64::sqrt);
                            }
                            options.r#type = kind;
                            options.domain = domain.clone();
                            options.colors = colors.map(|n| vec![gpui::red(); n]);
                            let options = &options;
                            let (_, facets, _) = plot.prepare_marks();
                            let built =
                                render::color::PlotColors::new(&theme, &facets, None, options)
                                    .scale
                                    .is_some_and(|scale| !scale.is_ordinal());
                            let inferred =
                                render::color::color_scale_is_continuous(&theme, &facets, options);
                            assert_eq!(
                                inferred, built,
                                "{field} {kind:?} {domain:?} {colors:?} {transform}"
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 1000);
    }

    #[test]
    fn the_color_option_colors_the_marks() {
        let data = [1., 5.].map(|v| {
            Table::empty()
                .with("x", Value::Number(v))
                .with("y", Value::Number(v))
                .with("v", Value::Number(v))
        });
        let ramp = ColorRange::Continuous(vec![gpui::black(), gpui::white()]);
        let plot = Plot::new(data)
            .dot(|mark| mark.x("x").y("y"))
            .color_by("v", ramp);
        let facets = vec![render::prepare(plot.data(), plot.marks())];
        let theme = PlotTheme::from_theme(&gpui_kit::component::Theme::default());
        let colors = plot.plot_colors(&theme, &facets);
        let mark = &facets[0][0];
        let (low, high) = (
            colors.row_fill(mark, 0, &mark.rows[0]),
            colors.row_fill(mark, 0, &mark.rows[1]),
        );
        assert_ne!(low, high, "the two ends of the domain differ in color");
        assert_ne!(low, theme.mark_color);
    }
}
