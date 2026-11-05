//! Group transform: `transforms/group.md`.

use crate::prelude::*;
use gpui_plot::ScaleOptions;

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "group", 1, e01),
        Example {
            height: 180.,
            ..example("transforms", "group", 2, e02)
        },
        Example {
            height: 180.,
            ..example("transforms", "group", 3, e03)
        },
        Example {
            height: 180.,
            ..example("transforms", "group", 4, e04)
        },
        example("transforms", "group", 5, e05),
        example("transforms", "group", 6, e06),
        example("transforms", "group", 7, e07),
        example("transforms", "group", 8, e08),
        Example {
            height: 640.,
            ..example("transforms", "group", 9, e09)
        },
        example("transforms", "group", 10, e10),
        example("transforms", "group", 11, e11),
        example("transforms", "group", 12, e12),
        example("transforms", "group", 13, e13),
        Example {
            height: 120.,
            ..example("transforms", "group", 14, e14)
        },
    ]
}

/// The x axis every sport chart shares: `{label: null, tickRotate: 90}`.
fn sport_axis(x: ScaleOptions) -> ScaleOptions {
    x.no_label().tick_rotate(90.)
}

/// `Plot.barY(olympians, Plot.groupX({y: "count"}, {x: "sport"}))`
fn e01() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true))
        .mark(
            bar_y(data::olympians())
                .x("sport")
                .group_x()
                .group_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `Plot.dot(olympians, Plot.groupX({r: "count"}, {x: "sport"}))`
fn e02() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .r(|r| r.range([0., 14.]))
        .mark(
            dot(data::olympians())
                .x("sport")
                .group_x()
                .group_output("r", GroupReducer::Count),
        )
}

/// `Plot.cell(olympians, Plot.groupX({fill: "count"}, {x: "sport"}))`
fn e03() -> Plot {
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.tick_rotate(90.))
        .color(|c| c.scheme("YlGnBu"))
        .mark(
            cell(data::olympians())
                .x("sport")
                .group_x()
                .group_output("fill", GroupReducer::Count),
        )
}

/// `Plot.cell(olympians, Plot.groupX({fill: "mode"}, {fill: "sex", x: "sport"}))`
fn e04() -> Plot {
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.tick_rotate(90.))
        .mark(
            cell(data::olympians())
                .fill("sex")
                .x("sport")
                .group_x()
                .group_output("fill", GroupReducer::Mode),
        )
}

/// `Plot.barY(olympians, Plot.groupX({y: "count"}, {x: "sport", fill: "sex"}))`
fn e05() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(
            bar_y(data::olympians())
                .x("sport")
                .fill("sex")
                .group_x()
                .group_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `Plot.barY(olympians, Plot.groupX({y2: "count"}, {x: "sport", fill: "sex", mixBlendMode: "multiply"}))`
///
/// Gap: a `y2` group output on a bar draws zero-height bars (the bar's
/// interval never reaches down to zero), and `mixBlendMode` is missing.
fn e06() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(
            bar_y(data::olympians())
                .x("sport")
                .fill("sex")
                .group_x()
                .group_output("y2", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `Plot.barY(olympians, Plot.groupX({y2: "count"}, {x: "sex", fx: "sport", fill: "sex"}))`
///
/// Gap: a `y2` group output draws zero-height bars (see the previous
/// example); since each bar here is its own sex, a stacked `y` count is the
/// same chart.
fn e07() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .fx_scale(|fx| fx.padding(0.).no_label().tick_rotate(90.).tick_size(6.))
        .x(|x| x.no_axis().padding_outer(0.2))
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(
            bar_y(data::olympians())
                .x("sex")
                .fx("sport")
                .fill("sex")
                .group_x()
                .group_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `(D) => d3.sum(D, (d) => d === sex)`: how many of a group's values are
/// `sex`.
fn count_of(values: &[Value], sex: &str) -> f64 {
    values.iter().filter(|v| v.label() == sex).count() as f64
}

/// The docs' link chart of each sport's women and men.
///
/// Gap: the color scale has no `transform: Math.sign`, so the stroke reducer
/// returns the sign of the difference itself.
fn e08() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true).label("Frequency"))
        .color(|c| {
            c.type_name("categorical")
                .domain([-1., 1.])
                .unknown("#aaa")
        })
        .mark(rule_y([0.]))
        .mark(
            link(data::olympians())
                .x("sport")
                .y1("sex")
                .y2("sex")
                .marker_start("dot")
                .marker_end("arrow")
                .stroke("sex")
                .stroke_width(2.)
                .group_x()
                .group_output(
                    "y1",
                    GroupReducer::custom(|values, _| Value::Number(count_of(values, "female"))),
                )
                .group_output(
                    "y2",
                    GroupReducer::custom(|values, _| Value::Number(count_of(values, "male"))),
                )
                .group_output(
                    "stroke",
                    GroupReducer::custom(|values, _| {
                        let d = count_of(values, "male") - count_of(values, "female");
                        Value::Number(d.signum() * f64::from(u8::from(d != 0.)))
                    }),
                ),
        )
}

/// `Plot.barX(olympians, Plot.groupY({x: "count"}, {y: "sport", sort: {y: "x"}}))`
fn e09() -> Plot {
    Plot::plot()
        .margin_left(100.)
        .x(|x| x.grid(true))
        .y(|y| y.no_label())
        .mark(
            bar_x(data::olympians())
                .y("sport")
                .group_y()
                .group_output("x", GroupReducer::Count)
                .sort_domain("y", "x", false),
        )
        .mark(rule_x([0.]))
}

/// `Plot.cell(olympians, Plot.group({fill: "median"}, {fill: "weight", x: "sport", y: "sex"}))`
fn e10() -> Plot {
    Plot::plot()
        .margin_bottom(80.)
        .x(sport_axis)
        .y(|y| y.no_label())
        .color(|c| {
            c.label("Median weight (kg)")
                .legend(true)
                .scheme("YlGnBu")
        })
        .mark(
            cell(data::olympians())
                .fill("weight")
                .x("sport")
                .y("sex")
                .group_xy()
                .group_output("fill", GroupReducer::Median),
        )
}

/// The gold-medal axis the docs' last charts share.
fn gold_axis(y: ScaleOptions) -> ScaleOptions {
    y.label("gold")
        .label_anchor(LabelAnchor::End) // Plot's "top"
        .label_arrow(true)
        .reverse(true)
}

/// `Plot.cell(olympians, Plot.group({fill: "count"}, {x: "sport", y: "gold"}))`
fn e11() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(gold_axis)
        .color(|c| c.type_name("sqrt").scheme("YlGnBu"))
        .mark(
            cell(data::olympians())
                .x("sport")
                .y("gold")
                .group_xy()
                .group_output("fill", GroupReducer::Count),
        )
}

/// `Plot.dot(olympians, Plot.group({r: "count"}, {x: "sport", y: "gold"}))`
fn e12() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| gold_axis(y.type_name("point")))
        .r(|r| r.range([0., 12.]))
        .mark(
            dot(data::olympians())
                .x("sport")
                .y("gold")
                .group_xy()
                .group_output("r", GroupReducer::Count),
        )
}

/// `Plot.dot(olympians, Plot.group({r: "count"}, {x: "sport", y: "gold", stroke: "sex"}))`
fn e13() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| gold_axis(y.type_name("point")))
        .r(|r| r.range([0., 12.]))
        .mark(
            dot(data::olympians())
                .x("sport")
                .y("gold")
                .stroke("sex")
                .group_xy()
                .group_output("r", GroupReducer::Count),
        )
}

/// Each sport's share of athletes, stacked: `Plot.stackX({order: "x",
/// reverse: true}, Plot.groupZ({x: "proportion"}, {z: "sport", …}))`.
///
/// Gap: `Plot.filter((D) => D.length > 200, …)` filters the groups, and the
/// mark's `filter` sees only records; the text output's reducer leaves the
/// smaller sports blank instead.
fn e14() -> Plot {
    let stacked = |mark: Mark| {
        mark.stack_x()
            .stack_order(StackOrder::Value)
            .stack_reverse(true)
    };
    Plot::plot()
        .height(100.)
        .x(|x| x.percent(true))
        .mark(stacked(
            bar_x(data::olympians())
                .z("sport")
                .fill_opacity(0.2)
                .inset(0.5)
                .group_z()
                .group_output("x", GroupReducer::Proportion)
                .into_mark(),
        ))
        .mark(stacked(
            text(data::olympians())
                .z("sport")
                .text("sport")
                .text_rotate(90.)
                .group_z()
                .group_output("x", GroupReducer::Proportion)
                .group_output(
                    "text",
                    GroupReducer::custom(|values, records| {
                        if values.len() > 200 && records.len() > 200 {
                            values.first().cloned().unwrap_or(Value::Null)
                        } else {
                            Value::Null
                        }
                    }),
                )
                .into_mark(),
        ))
        .mark(rule_x([0., 1.]))
}
