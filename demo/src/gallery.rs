//! The gallery's chart definitions: one [`Plot`] per example, built from the
//! sample data the way Plot's own examples are built.

use gpui_plot::mark::Curve;
use gpui_plot::{
    ColorRange, Dodge, DodgeAnchor, LegendPosition, MapReducer, Plot, Reducer, Selector,
    StackOffset, Table, Thresholds, Transform, Value,
};

use crate::data;

/// One example in the gallery.
pub struct Example {
    /// The example's title.
    pub title: &'static str,
    /// The Plot documentation section it mirrors.
    pub section: &'static str,
    /// A sentence on what the example demonstrates.
    pub description: &'static str,
    /// The height the example asks for, in pixels.
    pub height: f32,
    /// Builds the plot.
    pub build: fn() -> Plot,
}

/// The gallery's examples, in the order the Plot documentation presents them.
pub fn examples() -> Vec<Example> {
    vec![
        Example {
            title: "Line chart",
            section: "Marks / line",
            description: "A daily temperature series over a year, with an area under it and a moving-average overlay.",
            height: 440.,
            build: line_chart,
        },
        Example {
            title: "Dot plot",
            section: "Marks / dot",
            description: "Two economic series as dots, colored by a categorical series channel.",
            height: 440.,
            build: dot_plot,
        },
        Example {
            title: "Bar chart",
            section: "Marks / barY",
            description: "Categorical sales as vertical bars, with a pinned zero baseline.",
            height: 440.,
            build: bar_chart,
        },
        Example {
            title: "Horizontal bars",
            section: "Marks / barX",
            description: "The same sales as horizontal bars, which suits long category names.",
            height: 440.,
            build: horizontal_bars,
        },
        Example {
            title: "Stacked bars",
            section: "Marks / stack",
            description: "Monthly traffic stacked by platform, with the stack normalized to a share of each month.",
            height: 440.,
            build: stacked_bars,
        },
        Example {
            title: "Stacked area",
            section: "Marks / areaY",
            description: "A wide table stacked into an area chart, each column a series.",
            height: 440.,
            build: stacked_area,
        },
        Example {
            title: "Histogram",
            section: "Transforms / bin",
            description: "A distribution binned into a histogram with the count reducer.",
            height: 440.,
            build: histogram,
        },
        Example {
            title: "Heatmap",
            section: "Marks / cell",
            description: "Activity by hour and day as cells, colored by a continuous range.",
            height: 440.,
            build: heatmap,
        },
        Example {
            title: "Box plot",
            section: "Marks / boxY",
            description: "Four treatment groups summarized as box-and-whisker plots.",
            height: 440.,
            build: box_plot,
        },
        Example {
            title: "Violin plot",
            section: "Marks / violinY",
            description: "The same groups as kernel-density violins, showing the shape of each distribution.",
            height: 440.,
            build: violin_plot,
        },
        Example {
            title: "Density",
            section: "Marks / densityX",
            description: "A kernel density estimate of the temperature series, with the raw ticks beneath.",
            height: 440.,
            build: density_plot,
        },
        Example {
            title: "Hexbin",
            section: "Marks / hexbin",
            description: "A two-dimensional cloud binned into hexagons, a dot plot's answer for dense data.",
            height: 440.,
            build: hexbin_plot,
        },
        Example {
            title: "Pie chart",
            section: "Marks / pie",
            description: "Market share by channel as a pie, the radial layout Plot's pie mark draws.",
            height: 440.,
            build: pie_chart,
        },
        Example {
            title: "Waffle",
            section: "Marks / waffle",
            description: "The same shares as a unit-square waffle, one cell per percent.",
            height: 440.,
            build: waffle_chart,
        },
        Example {
            title: "Rules and ticks",
            section: "Marks / ruleY, tickX",
            description: "Reference rules at round values, with ticks marking each datum.",
            height: 440.,
            build: rules_and_ticks,
        },
        Example {
            title: "Log scale",
            section: "Scales / log",
            description: "A growth series on a logarithmic y scale, where equal ratios take equal heights.",
            height: 440.,
            build: log_scale,
        },
        Example {
            title: "Moving average",
            section: "Transforms / window",
            description: "A noisy series smoothed by a centered window transform.",
            height: 440.,
            build: moving_average,
        },
        Example {
            title: "Labels",
            section: "Marks / text",
            description: "Bars with their values drawn as labels above them.",
            height: 440.,
            build: labelled_bars,
        },
        Example {
            title: "Beeswarm",
            section: "Transforms / dodge",
            description: "One dot per reading, packed into a swarm by the dodgeX transform.",
            height: 160.,
            build: beeswarm,
        },
        Example {
            title: "One-dimensional dots",
            section: "Marks / dotX",
            description: "A distribution laid along the x axis, its dots in the frame's middle.",
            height: 160.,
            build: one_dimensional_dots,
        },
        Example {
            title: "Horizontal histogram",
            section: "Marks / rectX",
            description: "The same data binned into horizontal bars with rectX and bin.",
            height: 440.,
            build: horizontal_histogram,
        },
        Example {
            title: "Nicened domain",
            section: "Scales / nice",
            description: "A domain widened to round values, so its axis reads 0, 25, 50.",
            height: 440.,
            build: nicened_domain,
        },
        Example {
            title: "Annotated peaks",
            section: "Marks / tip",
            description: "A line with tips pinned to its highest and lowest readings.",
            height: 440.,
            build: annotated_peaks,
        },
        Example {
            title: "Selected extremes",
            section: "Transforms / select",
            description: "One dot per series at its highest reading, picked by selectMaxY.",
            height: 440.,
            build: selected_extremes,
        },
        Example {
            title: "Cumulative total",
            section: "Transforms / map",
            description: "A running total folded by mapY, over a series of daily readings.",
            height: 440.,
            build: cumulative_total,
        },
        Example {
            title: "Frame and grid marks",
            section: "Marks / axisX, gridX, frame",
            description: "Axes, grid lines and the frame drawn as marks rather than plot options.",
            height: 440.,
            build: furniture_marks,
        },
        Example {
            title: "Sized dots",
            section: "Legends / size",
            description: "Two series as dots whose radius encodes a value, with a size legend beside them.",
            height: 440.,
            build: sized_dots,
        },
        Example {
            title: "Curves",
            section: "Features / curves",
            description: "The same points through Plot's spline families, from basis to bundle.",
            height: 440.,
            build: curves,
        },
    ]
}

fn line_chart() -> Plot {
    let data = data::temperature();
    Plot::new(data.clone())
        .title("Daily temperature")
        .subtitle("A year of readings, with a 30-day mean")
        .area_y(|mark| {
            mark.x("date")
                .y("temperature")
                .fill(theme_color(0.55, 0.65, 0.6, 0.25))
                .stroke_width(0.)
        })
        .line(|mark| mark.x("date").y("temperature").curve(Curve::Natural))
        .line(|mark| {
            mark.x("date")
                .y("temperature")
                .window(
                    30,
                    gpui_plot::WindowReducer::Mean,
                    gpui_plot::Anchor::Middle,
                )
                .stroke(theme_color(0.33, 0.7, 0.45, 1.))
                .stroke_width(2.5)
                .title("30-day mean")
        })
        .y_axis(true)
        .grid(true)
}

fn dot_plot() -> Plot {
    let data = data::economics();
    let long: Vec<Table> = data
        .iter()
        .flat_map(|row| {
            ["unemployment", "inflation"]
                .into_iter()
                .filter_map(|field| {
                    let value = row.number(field)?;
                    Some(
                        Table::empty()
                            .with("date", row.value("date"))
                            .with("value", Value::Number(value))
                            .with("series", Value::text(field)),
                    )
                })
        })
        .collect();

    Plot::new(long)
        .title("Economic indicators")
        .subtitle("Monthly unemployment and inflation")
        .dot(|mark| mark.x("date").y("value").z("series").r(3.5))
        .legend(LegendPosition::Top)
        .y_axis(true)
}

fn bar_chart() -> Plot {
    let data = data::sales();
    Plot::new(data)
        .title("Sales by region")
        .bar_y(|mark| mark.x("region").y("sales").corner_radius(4.))
        .y_zero(true)
        .x_axis(true)
        .y_axis(true)
        .grid(false)
}

fn horizontal_bars() -> Plot {
    let data = data::sales();
    Plot::new(data)
        .title("Sales by region")
        .subtitle("Horizontal bars suit long labels")
        .bar_x(|mark| mark.x("sales").y("region").corner_radius(4.))
        .x_zero(true)
        .grid(false)
}

fn stacked_bars() -> Plot {
    let data = data::long_stacked();
    Plot::new(data)
        .title("Monthly visits")
        .subtitle("Stacked by platform, normalized")
        .bar_y(|mark| {
            mark.x("month")
                .y("visits")
                .z("platform")
                .stack(StackOffset::Normalize)
                .stack_series("platform")
        })
        .legend(LegendPosition::Top)
        .y_zero(true)
        .grid(false)
}

fn stacked_area() -> Plot {
    let data = data::stacked();
    Plot::new(data)
        .title("Traffic by device")
        .subtitle("A wide table, one column per series")
        .area_y(|mark| {
            mark.x("month")
                .y("desktop")
                .stack(StackOffset::Zero)
                .stack_fields(["desktop", "mobile", "tablet"])
                .stroke_width(1.)
        })
        .legend(LegendPosition::Top)
        .y_zero(true)
}

fn histogram() -> Plot {
    let data = data::cloud();
    Plot::new(data)
        .title("Distribution of x")
        .subtitle("Binned into a histogram")
        .bar_y(|mark| {
            mark.x("x")
                .y1(Value::Number(0.))
                .y2("x")
                .bin(Thresholds::Count(20))
                .reduce(Reducer::Count)
                .corner_radius(2.)
        })
        .y_zero(true)
        .grid(false)
}

fn heatmap() -> Plot {
    let data = data::heatmap();
    Plot::new(data)
        .title("Activity by hour and day")
        .cell(|mark| {
            mark.x("day").y("hour").fill(|row: &Table| {
                // A sequential ramp from the theme's first chart color.
                let value = row.number("activity").unwrap_or(0.) / 60.;
                ramp(value)
            })
        })
        .x_scale(gpui_plot::frame::ScaleKind::Band)
        .y_scale(gpui_plot::frame::ScaleKind::Band)
        .grid(false)
        .legend(LegendPosition::None)
}

fn box_plot() -> Plot {
    let data = data::distributions();
    Plot::new(data)
        .title("Scores by group")
        .subtitle("Median, quartiles and the full range")
        .box_x(|mark| mark.x("score").y("group"))
        .y_scale(gpui_plot::frame::ScaleKind::Band)
        .grid(false)
}

fn violin_plot() -> Plot {
    let data = data::distributions();
    Plot::new(data)
        .title("Score distributions")
        .subtitle("Kernel density per group")
        .violin_x(|mark| mark.x("score").y("group"))
        .y_scale(gpui_plot::frame::ScaleKind::Band)
        .grid(false)
}

fn density_plot() -> Plot {
    let data = data::temperature();
    Plot::new(data)
        .title("Temperature density")
        .subtitle("A kernel density estimate")
        .density_x(|mark| mark.x("temperature"))
        .tick_x(|mark| mark.x("temperature").stroke_width(1.).opacity(0.25))
        .y_zero(true)
}

fn hexbin_plot() -> Plot {
    let data = data::cloud();
    Plot::new(data)
        .title("Two clusters")
        .subtitle("Hexagonal binning of 400 points")
        .hexbin(|mark| mark.x("x").y("y"))
        .dot(|mark| mark.x("x").y("y").r(1.5).opacity(0.35))
        .grid(false)
}

fn pie_chart() -> Plot {
    let data = data::market_share();
    Plot::new(data)
        .title("Market share")
        .pie(|mark| mark.y("share").z("channel"))
        .legend(LegendPosition::Right)
        .grid(false)
        .x_axis(false)
        .y_axis(false)
}

fn waffle_chart() -> Plot {
    let data = data::market_share();
    Plot::new(data)
        .title("Market share")
        .subtitle("One cell per percent")
        .waffle(|mark| mark.y("share").z("channel"))
        .legend(LegendPosition::Top)
        .grid(false)
        .x_axis(false)
        .y_axis(false)
}

fn rules_and_ticks() -> Plot {
    let data = data::noisy_series();
    Plot::new(data)
        .title("Reference rules")
        .subtitle("Round values marked across the plot")
        .tick_x(|mark| mark.x("date").opacity(0.18))
        .line(|mark| mark.x("date").y("value"))
        .rule_y(|mark| {
            mark.y(Value::Number(80.))
                .stroke(theme_color(0.6, 0.6, 0.5, 0.7))
        })
        .rule_y(|mark| {
            mark.y(Value::Number(120.))
                .stroke(theme_color(0.0, 0.6, 0.5, 0.7))
        })
        .y_axis(true)
}

fn log_scale() -> Plot {
    let data = data::economics();
    Plot::new(data)
        .title("Growth on a log scale")
        .subtitle("Equal ratios take equal heights")
        .line(|mark| mark.x("date").y("gdp"))
        .dot(|mark| mark.x("date").y("gdp").r(2.))
        .y_transform(Transform::Log)
        .y_axis(true)
}

fn moving_average() -> Plot {
    let data = data::noisy_series();
    Plot::new(data)
        .title("Noisy series")
        .subtitle("Raw values and a 7-day centered mean")
        .dot(|mark| mark.x("date").y("value").r(2.).opacity(0.4))
        .line(|mark| {
            mark.x("date")
                .y("value")
                .window(7, gpui_plot::WindowReducer::Mean, gpui_plot::Anchor::Middle)
                .stroke_width(2.5)
                .title("7-day mean")
        })
        .y_axis(true)
}

fn labelled_bars() -> Plot {
    let data = data::sales();
    Plot::new(data)
        .title("Sales by region")
        .subtitle("Bars labelled with their values")
        .bar_y(|mark| mark.x("region").y("sales").corner_radius(4.))
        .text(|mark| {
            mark.x("region")
                .y("sales")
                .text("sales")
                .dy(-10.)
                .stroke(theme_color(0.0, 0.0, 0.25, 1.))
        })
        .y_zero(true)
        .grid(false)
}

fn beeswarm() -> Plot {
    let data = data::noisy_series();
    Plot::new(data)
        .title("Values as a swarm")
        .subtitle("One dot per reading, packed by dodgeX")
        .dot_x(|mark| {
            mark.x("value").r(3.).dodge(Dodge {
                anchor: DodgeAnchor::Middle,
                radius: 4.,
                padding: 1.5,
            })
        })
        .x_nice(true)
        .y_axis(false)
        .grid(false)
}

fn one_dimensional_dots() -> Plot {
    let data = data::distributions();
    Plot::new(data)
        .title("Scores")
        .subtitle("A one-dimensional distribution, all dots in the frame's middle")
        .dot_x(|mark| mark.x("score").r(2.5).opacity(0.6))
        .x_nice(true)
        .y_axis(false)
        .grid(false)
}

fn horizontal_histogram() -> Plot {
    let data = data::cloud();
    Plot::new(data)
        .title("Distribution of x")
        .subtitle("Binned into horizontal bars with rectX")
        .rect_x(|mark| {
            mark.x("x")
                .x1(Value::Number(0.))
                .x2("x")
                .bin(Thresholds::Count(20))
                .reduce(Reducer::Count)
        })
        .x_zero(true)
        .grid(false)
}

fn nicened_domain() -> Plot {
    // A series that runs from 3.4 to 96.2: the frame widens it to 0..100.
    let data: Vec<Table> = (0..12)
        .map(|index| {
            let value = 3.4 + (index as f64) * (92.8 / 11.);
            Table::empty()
                .with("step", Value::Number(index as f64))
                .with("value", Value::Number((value * 10.).round() / 10.))
        })
        .collect();
    Plot::new(data)
        .title("A nicened domain")
        .subtitle("Data spans 3.4 to 96.2; the axis reads 0, 25, 50, 75, 100")
        .bar_y(|mark| mark.x("step").y("value").corner_radius(3.))
        .y_zero(true)
        .y_nice(true)
        .y_ticks(5)
        .grid(false)
}

fn selected_extremes() -> Plot {
    let data = data::economics();
    let long: Vec<Table> = data
        .iter()
        .flat_map(|row| {
            ["unemployment", "inflation"]
                .into_iter()
                .filter_map(|field| {
                    let value = row.number(field)?;
                    Some(
                        Table::empty()
                            .with("date", row.value("date"))
                            .with("value", Value::Number(value))
                            .with("series", Value::text(field)),
                    )
                })
        })
        .collect();

    Plot::new(long)
        .title("Economic indicators")
        .subtitle("The highest reading of each series, picked by selectMaxY")
        .line(|mark| mark.x("date").y("value").z("series").stroke_width(1.))
        .dot(|mark| {
            mark.x("date")
                .y("value")
                .z("series")
                .r(5.)
                .select(Selector::Max("value".into()), Some("series"))
        })
        .text(|mark| {
            mark.x("date")
                .y("value")
                .text("value")
                .dy(-14.)
                .dx(-6.)
                .select(Selector::Max("value".into()), Some("series"))
        })
        .inset(0.04)
        .legend(LegendPosition::Top)
        .y_axis(true)
}

fn cumulative_total() -> Plot {
    let data = data::noisy_series();
    Plot::new(data)
        .title("Noisy series")
        .subtitle("A running total, folded by mapY")
        .area_y(|mark| {
            mark.x("date")
                .y("total")
                .map("value", "total", MapReducer::Sum)
                .stroke_width(0.)
        })
        .line(|mark| {
            mark.x("date")
                .y("total")
                .map("value", "total", MapReducer::Sum)
        })
        .y_zero(true)
}

fn furniture_marks() -> Plot {
    let data = data::temperature();
    Plot::new(data)
        .title("Daily temperature")
        .subtitle("Grid, axes and frame drawn as marks")
        .grid_y(|mark| mark)
        .grid_x(|mark| mark)
        .frame_mark(|mark| mark.stroke(theme_color(0.0, 0.0, 0.45, 1.)))
        .axis_x(|mark| mark.stroke(theme_color(0.0, 0.0, 0.45, 1.)))
        .axis_y(|mark| mark.stroke(theme_color(0.0, 0.0, 0.45, 1.)))
        .line(|mark| mark.x("date").y("temperature").curve(Curve::Natural))
        .grid(false)
        .y_axis(true)
        .x_axis(true)
}

fn annotated_peaks() -> Plot {
    let data = data::noisy_series();
    let high = data
        .iter()
        .map(|row| row.number("value").unwrap_or(0.))
        .fold(f64::MIN, f64::max);
    let low = data
        .iter()
        .map(|row| row.number("value").unwrap_or(0.))
        .fold(f64::MAX, f64::min);

    let mut plot = Plot::new(data.clone())
        .title("Noisy series")
        .subtitle("Tips pinned to the extremes")
        .line(|mark| mark.x("date").y("value"));
    for (value, name) in [(high, "peak"), (low, "trough")] {
        // The tip names a fixed point, so its channels are constants: the
        // mark reads them from the source row rather than from the datum.
        let tip_date = data
            .iter()
            .find(|row| row.number("value") == Some(value))
            .map(|row| row.value("date"))
            .unwrap_or(Value::Null);
        let tip_name = Value::text(name);
        plot = plot.tip(move |mark| {
            mark.x(tip_date.clone())
                .y(Value::Number(value))
                .text(tip_name.clone())
        });
    }
    for (value, _name) in [(high, "peak"), (low, "trough")] {
        let row = data
            .iter()
            .find(|row| row.number("value") == Some(value))
            .cloned()
            .unwrap_or_default();
        let date = row.value("date");
        plot = plot.dot(|mark| {
            mark.x(date.clone())
                .y(Value::Number(value))
                .r(4.)
                .stroke(theme_color(0.0, 0.0, 0.3, 1.))
        });
    }
    plot.y_axis(true)
}

fn sized_dots() -> Plot {
    let data = data::economics();
    let long: Vec<Table> = data
        .iter()
        .flat_map(|row| {
            ["unemployment", "inflation"]
                .into_iter()
                .filter_map(|field| {
                    let value = row.number(field)?;
                    Some(
                        Table::empty()
                            .with("date", row.value("date"))
                            .with("value", Value::Number(value))
                            .with("series", Value::text(field)),
                    )
                })
        })
        .collect();

    Plot::new(long)
        .title("Economic indicators")
        .subtitle("Dot size encodes the reading's distance from 5")
        .dot(|mark| {
            mark.x("date").y("value").z("series").r(|row: &Table| {
                let value = row.number("value").unwrap_or(0.);
                (1.5 + (value - 5.).abs() * 0.7) as f32
            })
        })
        .legend(LegendPosition::Right)
        .y_axis(true)
}

fn curves() -> Plot {
    let data = data::curve_points();
    // One line per curve, offset vertically so all of them stay readable; each
    // line draws its own series, which gives it its own palette color.
    const CURVES: [(&str, Curve); 10] = [
        ("natural", Curve::Natural),
        ("basis", Curve::Basis),
        ("cardinal", Curve::Cardinal),
        ("catmull-rom", Curve::CatmullRom),
        ("monotone-x", Curve::MonotoneX),
        ("bump-x", Curve::BumpX),
        ("bundle", Curve::Bundle),
        ("linear", Curve::Linear),
        ("step", Curve::Step),
        ("step-after", Curve::StepAfter),
    ];
    let mut plot = Plot::new(data.clone())
        .title("Curves")
        .subtitle("One line per interpolation, offset for comparison")
        .dot(|mark| mark.x("step").y("value").r(2.5));
    for (index, (name, curve)) in CURVES.into_iter().enumerate() {
        let offset = index as f64 * 12.;
        plot = plot.line(move |mark| {
            mark.x("step")
                .y(move |row: &Table| Value::Number(row.number("value").unwrap_or(0.) + offset))
                .curve(curve)
                .z(Value::text(name))
                .stroke_width(1.5)
                .title(name)
        });
    }
    plot.legend(LegendPosition::Top)
        .legend_title("interpolation")
        .grid(false)
}

/// A color from HSLA components, for marks that want a specific hue rather
/// than the theme's palette.
fn theme_color(h: f32, s: f32, l: f32, a: f32) -> gpui_kit::Hsla {
    gpui_kit::Hsla { h, s, l, a }
}

/// A sequential ramp for a heatmap, from the theme's blue to a warm accent.
fn ramp(value: f64) -> gpui_kit::Hsla {
    let value = value.clamp(0., 1.);
    let from = gpui_kit::Hsla {
        h: 0.58,
        s: 0.55,
        l: 0.93,
        a: 1.,
    };
    let to = gpui_kit::Hsla {
        h: 0.58,
        s: 0.7,
        l: 0.35,
        a: 1.,
    };
    gpui_kit::Hsla {
        h: from.h + (to.h - from.h) * value as f32,
        s: from.s + (to.s - from.s) * value as f32,
        l: from.l + (to.l - from.l) * value as f32,
        a: 1.,
    }
}

/// The `color` option's ranges, for a caller that wants one by name.
pub fn range_by_name(name: &str) -> ColorRange {
    match name {
        "blue-red" => ColorRange::blue_red(),
        _ => ColorRange::observable10(),
    }
}
