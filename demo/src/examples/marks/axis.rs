//! Axis mark: `marks/axis.md`.

use gpui_plot::ScaleOptions;

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "axis", 1, e01),
        example("marks", "axis", 2, e02),
        example("marks", "axis", 3, e03),
        example("marks", "axis", 4, e04),
        example("marks", "axis", 5, e05),
        example("marks", "axis", 6, e06),
        example("marks", "axis", 7, e07),
        example("marks", "axis", 8, e08),
        example("marks", "axis", 9, e09),
        Example {
            height: 60.,
            ..example("marks", "axis", 10, e10)
        },
        Example {
            height: 60.,
            ..example("marks", "axis", 11, e11)
        },
        Example {
            height: 60.,
            ..example("marks", "axis", 12, e12)
        },
        Example {
            height: 40.,
            ..example("marks", "axis", 13, e13)
        },
        Example {
            height: 640.,
            ..example("marks", "axis", 14, e14)
        },
        Example {
            height: 80.,
            ..example("marks", "axis", 15, e15)
        },
        example("marks", "axis", 16, e16),
    ]
}

/// `d3.ticks(0, 1, 10)`
fn ticks_0_1() -> Vec<Value> {
    (0..=10).map(|i| Value::Number(i as f64 / 10.)).collect()
}

/// The x domain `Plot.axisX(data)` takes from its ticks; an axis mark's
/// ticks don't reach the scale, so the port pins it.
fn ticks_domain(x: ScaleOptions) -> ScaleOptions {
    x.domain([0., 1.])
}

fn e01() -> Plot {
    Plot::plot()
        .x(|x| x.percent(true).grid(true).axis(AxisAnchor::Both))
        .mark(bar_x(data::alphabet()).x("frequency").y("letter"))
        .mark(rule_x([0.]))
}

fn e02() -> Plot {
    Plot::plot()
        .x(|x| x.percent(true))
        .mark(axis_x().axis_anchor(AxisAnchor::Top))
        .mark(axis_x().axis_anchor(AxisAnchor::Bottom).no_axis_label())
        .mark(bar_x(data::alphabet()).x("frequency").y("letter"))
        .mark(
            // `interval: 1` in percent units: a line every percentage point.
            grid_x()
                .tick_interval(Interval::Step(0.01))
                .stroke("white")
                .stroke_opacity(0.5),
        )
        .mark(rule_x([0.]))
}

fn e03() -> Plot {
    let responses = Dataset::new(
        [
            ("Family in feud with Zucker\u{ad}bergs", 0.17),
            ("Committed 671 birthdays to memory", 0.19),
            ("Ex is doing too well", 0.10),
            ("High school friends all dead now", 0.15),
            ("Discovered how to “like” things mentally", 0.27),
            ("Not enough politics", 0.12),
        ]
        .map(|(name, value)| {
            Table::empty()
                .with("name", Value::text(name))
                .with("value", Value::Number(value))
        }),
    );
    Plot::plot()
        // The axis's `marginBottom: 40`, as the plot's.
        .margin_bottom(40.)
        .y(|y| y.percent(true))
        .mark(axis_x().no_axis_label().line_width(8.))
        .mark(axis_y().axis_label("Responses (%)"))
        .mark(bar_y(responses).x("name").y("value"))
        .mark(rule_y([0.]))
}

fn e04() -> Plot {
    // `Plot.barX([42, 17, 32], {y: [...]})`: the values with their labels.
    let fruit = Dataset::new(
        [(42., "🍌 banana"), (17., "🍎 apple"), (32., "🍐 pear")].map(|(value, name)| {
            Table::empty()
                .with("value", Value::Number(value))
                .with("name", Value::text(name))
        }),
    );
    Plot::plot()
        .margin_top(0.)
        .margin_left(4.)
        .x(|x| x.ticks(4).label("Yield (kg)"))
        .mark(bar_x(fruit).x("value").y("name"))
        .mark(
            axis_y()
                .text_anchor(TextAnchor::Start)
                .fill("white")
                .dx(14.),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .inset(10.)
        .mark(frame().fill("#eaeaea"))
        .mark(grid_y().stroke("white").stroke_opacity(1.))
        .mark(grid_x().stroke("white").stroke_opacity(1.))
        .mark(line(data::aapl()).x("Date").y("Close").stroke("black"))
}

fn e06() -> Plot {
    Plot::plot()
        .margin_left(0.)
        // `insetLeft: 36` reserves space for the inset labels; only a
        // symmetric inset is available.
        .x(|x| x.no_label().round(true))
        .y(|y| y.round(true))
        .mark(grid_y().dash([0.75, 2.]).stroke_opacity(1.))
        .mark(
            axis_y()
                .tick_size(0.)
                .dx(38.)
                .dy(-6.)
                .line_anchor(LineAnchor::Bottom)
                // `(d, i, _) => (i === _.length - 1 ? `$${d}` : d)`: the tick
                // formatter does not see the tick's index.
                .tick_format(|v: &Value| format_number("", v.as_number().unwrap_or(0.))),
        )
        .mark(rule_y([0.]))
        .mark(line(data::aapl()).x("Date").y("Close").marker_end("dot"))
}

fn e07() -> Plot {
    Plot::plot()
        .mark(rule_y([0.]))
        .mark(axis_x().tick_interval(Interval::parse("3 months").expect("an interval")))
        .mark(grid_x())
        .mark(line(data::aapl()).x("Date").y("Close"))
}

fn e08() -> Plot {
    let aapl = data::aapl();
    let rows = aapl.rows();
    let slice: Dataset = rows[rows.len() - 340..rows.len() - 10]
        .iter()
        .cloned()
        .collect();
    Plot::plot()
        // `nice: d3.utcWeek` has no equivalent.
        .x(|x| x.round(true))
        .y(|y| y.inset(6.))
        .mark(frame().fill("currentColor").fill_opacity(0.1))
        .mark(frame().frame_anchor(FrameAnchor::Bottom))
        .mark(
            axis_x()
                .tick_interval(Interval::parse("year").expect("an interval"))
                .tick_size(28.)
                .tick_padding(-11.)
                .tick_format("  %Y")
                .text_anchor(TextAnchor::Start),
        )
        .mark(
            axis_x()
                .tick_interval(Interval::parse("month").expect("an interval"))
                .tick_size(16.)
                .tick_padding(-11.)
                .tick_format("  %b")
                .text_anchor(TextAnchor::Start),
        )
        .mark(
            grid_x()
                .tick_interval(Interval::parse("week").expect("an interval"))
                .stroke("white")
                .stroke_opacity(1.),
        )
        .mark(line(slice).x("Date").y("Close").curve(Curve::Step))
}

fn e09() -> Plot {
    // `x: (y) => aapl.find((d) => d.Close >= y)?.Date` places each grid line
    // and tick at the date the price first reached it; the port's axis and
    // grid can't read the tick value, so they span the frame.
    Plot::plot()
        .margin_right(0.)
        .mark(rule_y([0.]))
        .mark(line(data::aapl()).x("Date").y("Close"))
        .mark(grid_y())
        .mark(axis_y())
}

/// `Plot.axisX(d3.ticks(0, 1, 10), {color: "red"}).plot()`
fn e10() -> Plot {
    axis_x()
        .tick_values(ticks_0_1())
        .color("red")
        .plot()
        .x(ticks_domain)
}

/// `Plot.axisX(d3.ticks(0, 1, 10), {stroke: Plot.identity, strokeWidth: 3, tickSize: 10}).plot()`
fn e11() -> Plot {
    // `stroke: Plot.identity` reads each tick's value as its color.
    axis_x()
        .tick_values(ticks_0_1())
        .stroke_width(3.)
        .tick_size(10.)
        .plot()
        .x(ticks_domain)
}

/// `Plot.axisX(d3.ticks(0, 1, 10), {fill: "red"}).plot()`
fn e12() -> Plot {
    axis_x()
        .tick_values(ticks_0_1())
        .fill("red")
        .plot()
        .x(ticks_domain)
}

fn e13() -> Plot {
    // `style: "background: #777;"` and the text halo
    // (`textStroke`, `textStrokeWidth`, `textStrokeOpacity`) have no
    // equivalent.
    Plot::plot()
        .height(40.)
        .x(|x| x.domain([0., 100.]))
        .mark(axis_x().fill("black").stroke("white"))
}

fn e14() -> Plot {
    // The docs' `anchor` and `facetAnchor` inputs, at their defaults:
    // "bottom" and "auto". `Plot.axisFx({color: "blue", anchor: "top"})` has
    // no mark form; the fx axis is the plot's.
    Plot::plot()
        .margin_right(80.)
        .grid(true)
        .fx_scale(|fx| fx.axis(AxisAnchor::Top))
        .mark(frame())
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fx("sex")
                .fy("species"),
        )
        .mark(axis_x().color("red").axis_anchor(AxisAnchor::Bottom))
}

fn e15() -> Plot {
    let axis = |anchor: AxisAnchor, label: &'static str, label_anchor: LabelAnchor| {
        axis_x()
            .axis_anchor(anchor)
            .axis_label(label)
            .axis_label_anchor(label_anchor)
    };
    Plot::plot()
        .height(80.)
        .grid(true)
        .x(|x| x.type_name("linear"))
        .mark(axis(AxisAnchor::Top, "top-left", LabelAnchor::Start))
        .mark(axis(AxisAnchor::Top, "top-center", LabelAnchor::Center).tick_values(Vec::new()))
        .mark(axis(AxisAnchor::Top, "top-right", LabelAnchor::End).tick_values(Vec::new()))
        .mark(axis(AxisAnchor::Bottom, "bottom-left", LabelAnchor::Start))
        .mark(
            axis(AxisAnchor::Bottom, "bottom-center", LabelAnchor::Center).tick_values(Vec::new()),
        )
        .mark(axis(AxisAnchor::Bottom, "bottom-right", LabelAnchor::End).tick_values(Vec::new()))
}

fn e16() -> Plot {
    let axis = |anchor: AxisAnchor, label: &'static str, label_anchor: LabelAnchor| {
        axis_y()
            .axis_anchor(anchor)
            .axis_label(label)
            .axis_label_anchor(label_anchor)
    };
    Plot::plot()
        .grid(true)
        .y(|y| y.type_name("linear"))
        .mark(axis(AxisAnchor::Left, "left-top", LabelAnchor::Start))
        .mark(axis(AxisAnchor::Left, "left-center", LabelAnchor::Center).tick_values(Vec::new()))
        .mark(axis(AxisAnchor::Left, "left-bottom", LabelAnchor::End).tick_values(Vec::new()))
        .mark(axis(AxisAnchor::Right, "right-top", LabelAnchor::Start))
        .mark(axis(AxisAnchor::Right, "right-center", LabelAnchor::Center).tick_values(Vec::new()))
        .mark(axis(AxisAnchor::Right, "right-bottom", LabelAnchor::End).tick_values(Vec::new()))
}
