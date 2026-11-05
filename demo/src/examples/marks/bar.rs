//! Bar mark: `marks/bar.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    (1..=9)
        .map(|i| {
            example(
                "marks",
                "bar",
                i,
                [e01, e02, e03, e04, e05, e06, e07, e08, e09][i - 1],
            )
        })
        .collect()
}

/// `Plot.barY(alphabet, {x: "letter", y: "frequency"}).plot()`
fn e01() -> Plot {
    bar_y(data::alphabet()).x("letter").y("frequency").plot()
}

/// `Plot.barY(alphabet, {x: "letter", y: "frequency", sort: {x: "-y"}}).plot()`
fn e02() -> Plot {
    bar_y(data::alphabet())
        .x("letter")
        .y("frequency")
        .sort_domain("x", "-y", false)
        .plot()
}

fn e03() -> Plot {
    Plot::plot()
        .margin_left(60.)
        .x(|x| x.label("Frequency"))
        .y(|y| y.no_label())
        .color(|c| c.legend(true))
        .mark(
            bar_x(data::penguins())
                .y("species")
                .x(Value::Number(1.))
                .inset(0.5)
                .fill("body_mass_g")
                .sort("body_mass_g", false),
        )
        .mark(rule_x([0.]))
}

fn e04() -> Plot {
    Plot::plot()
        .margin_left(130.)
        .no_axes()
        .x(|x| {
            x.axis(AxisAnchor::Top).grid(true).tick_format(|v: &Value| {
                let x = v.as_number().unwrap_or(0.);
                if x < 0. {
                    format!("{} BC", -x)
                } else {
                    format!("{x} AD")
                }
            })
        })
        .mark(
            bar_x(data::civilizations())
                .x1("start")
                .x2("end")
                .y("civilization")
                .sort_domain("y", "x1", false),
        )
        .mark(
            text(data::civilizations())
                .x("start")
                .y("civilization")
                .text("civilization")
                .text_anchor(TextAnchor::End)
                .dx(-3.),
        )
}

fn e05() -> Plot {
    let change = |d: &Table| (d.num("2019") - d.num("2010")) / d.num("2010");
    Plot::plot()
        .no_labels()
        .x(|x| {
            x.axis(AxisAnchor::Top)
                .label("← decrease · Change in population, 2010–2019 (%) · increase →")
                .label_anchor(LabelAnchor::Center)
                .tick_format("+")
                .percent(true)
        })
        .color(|c| c.scheme("PiYg").type_name("ordinal"))
        .mark(
            bar_x(data::us_state_population())
                .y("State")
                .x(Channel::number(change))
                .fill(ColorChannel::number(move |d| change(d).signum()))
                .sort_domain("y", "x", false),
        )
        .mark(grid_x().stroke("white").stroke_opacity(0.5))
        .mark(axis_y().x(Value::Number(0.)))
        .mark(rule_x([0.]))
}

fn e06() -> Plot {
    let timeseries = Dataset::new(
        [
            (2014., 7295.290765),
            (2015., 7379.797139),
            (2016., 7464.022049),
            (2017., 7547.858925),
            (2019., 7713.468100),
            (2020., 7794.798739),
        ]
        .map(|(year, population)| {
            Table::empty()
                .with("year", Value::Number(year))
                .with("population", Value::Number(population))
        }),
    );
    bar_y(timeseries)
        .x("year")
        .y("population")
        .plot()
        .x(|x| x.tick_format("").interval(1.))
}

fn e07() -> Plot {
    Plot::plot()
        .x(|x| x.round(true).tick_format("d"))
        .color(|c| c.scheme("BuRd"))
        .mark(
            bar_x(data::hadcrut())
                .x("year")
                .fill("anomaly")
                .interval(Interval::Step(1.))
                .inset(0.),
        )
}

fn e08() -> Plot {
    Plot::plot()
        .x(|x| x.percent(true))
        .mark(
            bar_x(data::alphabet())
                .x("frequency")
                .fill_opacity(0.3)
                .inset(0.5)
                .stack_x(),
        )
        .mark(
            text_x(data::alphabet())
                .x("frequency")
                .text("letter")
                .stack_x(),
        )
        .mark(rule_x([0., 1.]))
}

fn e09() -> Plot {
    Plot::plot()
        .margin_left(60.)
        .margin_right(60.)
        .no_labels()
        .x(|x| x.label("Frequency"))
        .y(|y| y.padding(0.))
        .mark(
            bar_x(data::penguins())
                .fy("island")
                .y("sex")
                .x(Value::Number(1.))
                .inset(0.5),
        )
        .mark(rule_x([0.]))
}
