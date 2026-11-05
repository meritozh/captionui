//! Group transform: `transforms/group.md`.

use crate::prelude::*;
use gpui_plot::{GroupConfig, GroupOutput, ScaleOptions};

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

/// Replace a mark's grouping with exactly `outputs`, Plot's
/// `groupX({…outputs}, …)` (or `groupZ` when `axis` is `None`).
fn grouped(mark: impl Into<Mark>, axis: Option<Axis>, outputs: Vec<GroupOutput>) -> Mark {
    let mut mark = mark.into();
    mark.group = Some(GroupConfig { axis, outputs });
    mark
}

/// A `groupX` over a mark with no `y`, kept in one row: the library's
/// grouping writes each group's x key into a missing y too, so the port gives
/// the mark one constant y and carries it through.
fn one_row(mut mark: Mark) -> Mark {
    mark = mark.y(Value::text(""));
    if let Some(group) = &mut mark.group {
        group
            .outputs
            .push(GroupOutput::new("y", GroupReducer::First));
    }
    mark
}

/// Plot's two-dimensional `Plot.group({…}, {x, y})`: the library groups on
/// one axis or on `z`, so the port groups on a `z` naming both categories and
/// carries each group's x and y through as its first value.
fn group_xy(
    mark: impl Into<Mark>,
    x: &'static str,
    y: &'static str,
    mut outputs: Vec<GroupOutput>,
) -> Mark {
    let mut mark = mark.into();
    mark.channels.z = Some(Channel::Accessor(std::rc::Rc::new(move |d: &Table| {
        Value::text(format!("{}\u{0}{}", d.value(x).label(), d.value(y).label()))
    })));
    outputs.push(GroupOutput::new("x", GroupReducer::First));
    outputs.push(GroupOutput::new("y", GroupReducer::First));
    mark.group = Some(GroupConfig {
        axis: None,
        outputs,
    });
    mark
}

/// `Plot.barY(olympians, Plot.groupX({y: "count"}, {x: "sport"}))`
fn e01() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true))
        .mark(bar_y(data::olympians()).x("sport").group_x())
        .mark(rule_y([0.]))
}

/// `Plot.dot(olympians, Plot.groupX({r: "count"}, {x: "sport"}))`
fn e02() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .r(|r| r.range([0., 14.]))
        .mark(one_row(grouped(
            dot(data::olympians()).x("sport").r("r"),
            Some(Axis::X),
            vec![GroupOutput::new("r", GroupReducer::Count)],
        )))
}

/// `Plot.cell(olympians, Plot.groupX({fill: "count"}, {x: "sport"}))`
///
/// Gap: the grouping writes each sport into the missing `y` as well, so the
/// cells run down a diagonal; a constant `y` (as [`one_row`] gives the dot)
/// collapses the grouped cells to slivers.
fn e03() -> Plot {
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.tick_rotate(90.))
        .color(|c| c.scheme("YlGnBu"))
        .mark(grouped(
            cell(data::olympians()).x("sport").fill("count"),
            Some(Axis::X),
            vec![GroupOutput::new("count", GroupReducer::Count)],
        ))
}

/// `Plot.cell(olympians, Plot.groupX({fill: "mode"}, {fill: "sex", x: "sport"}))`
///
/// Gap: the `mode` reducer is missing; each sport's most common sex is
/// computed up front and the group reads it as its first value.
fn e04() -> Plot {
    let olympians = data::olympians();
    let mut counts: Vec<(Value, usize, usize)> = Vec::new();
    for d in olympians.rows() {
        let sport = d.value("sport");
        let female = d.value("sex").label() == "female";
        let slot = match counts.iter().position(|(s, ..)| *s == sport) {
            Some(i) => i,
            None => {
                counts.push((sport, 0, 0));
                counts.len() - 1
            }
        };
        if female {
            counts[slot].1 += 1;
        } else {
            counts[slot].2 += 1;
        }
    }
    let with_mode = Dataset::new(olympians.rows().iter().map(|d| {
        let sport = d.value("sport");
        let (_, female, male) = counts.iter().find(|(s, ..)| *s == sport).unwrap();
        let mode = if female > male { "female" } else { "male" };
        d.clone().with("mode", Value::text(mode))
    }));
    Plot::plot()
        .margin_bottom(80.)
        .x(|x| x.tick_rotate(90.))
        .mark(grouped(
            cell(with_mode).x("sport").fill("mode"),
            Some(Axis::X),
            vec![GroupOutput::new("mode", GroupReducer::First)],
        ))
}

/// `Plot.barY(olympians, Plot.groupX({y: "count"}, {x: "sport", fill: "sex"}))`
fn e05() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(bar_y(data::olympians()).x("sport").fill("sex").group_x())
        .mark(rule_y([0.]))
}

/// `Plot.barY(olympians, Plot.groupX({y2: "count"}, {x: "sport", fill: "sex", mixBlendMode: "multiply"}))`
///
/// Gap: a `y2` group output (bars overlapping from zero rather than
/// stacking) draws no bar, `groupX` does not subdivide by the `fill` series,
/// and `mixBlendMode` is missing.
fn e06() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(grouped(
            bar_y(data::olympians())
                .x("sport")
                .fill("sex")
                .fill_opacity(0.5),
            Some(Axis::X),
            vec![GroupOutput::new("y2", GroupReducer::Count)],
        ))
        .mark(rule_y([0.]))
}

/// `Plot.barY(olympians, Plot.groupX({y2: "count"}, {x: "sex", fx: "sport", fill: "sex"}))`
///
/// Gap: a `y2` group output draws no bar (see the previous example); since
/// each bar here is its own sex, a `y` count, stacked over one series, is
/// the same chart.
fn e07() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .fx_scale(|fx| fx.padding(0.).no_label().tick_rotate(90.).tick_size(6.))
        .x(|x| x.no_axis().padding_outer(0.2))
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(grouped(
            bar_y(data::olympians()).x("sex").fx("sport").fill("sex"),
            Some(Axis::X),
            vec![GroupOutput::new("y", GroupReducer::Count)],
        ))
        .mark(rule_y([0.]))
}

/// The docs' link chart of each sport's women and men.
///
/// Gap: custom reducer functions (`(D) => d3.sum(D, (d) => d === "female")`)
/// are missing; the counts are computed up front, one row per sport. The
/// library's `link` draws nothing from a lone `x`, so it names `x1` and `x2`.
fn e08() -> Plot {
    let mut sports: Vec<(Value, f64, f64)> = Vec::new();
    for d in data::olympians().rows() {
        let sport = d.value("sport");
        let slot = match sports.iter().position(|(s, ..)| *s == sport) {
            Some(i) => i,
            None => {
                sports.push((sport, 0., 0.));
                sports.len() - 1
            }
        };
        match d.value("sex").label().as_ref() {
            "female" => sports[slot].1 += 1.,
            "male" => sports[slot].2 += 1.,
            _ => {}
        }
    }
    let counts = Dataset::new(sports.into_iter().map(|(sport, female, male)| {
        Table::empty()
            .with("sport", sport)
            .with("female", Value::Number(female))
            .with("male", Value::Number(male))
    }));
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| y.grid(true).label("Frequency"))
        .color(|c| c.type_name("categorical").domain([-1., 1.]).unknown("#aaa"))
        .mark(rule_y([0.]))
        .mark(
            link(counts)
                .x1("sport")
                .x2("sport")
                .y1("female")
                .y2("male")
                .marker_start("dot")
                .marker_end("arrow")
                .stroke(ColorChannel::number(|d| {
                    (d.num("male") - d.num("female")).signum()
                }))
                .stroke_width(2.),
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
        .color(|c| c.label("Median weight (kg)").legend(true).scheme("YlGnBu"))
        .mark(group_xy(
            cell(data::olympians()).x("sport").y("sex").fill("median"),
            "sport",
            "sex",
            vec![GroupOutput::new("median", GroupReducer::Median).input("weight")],
        ))
}

/// The gold-medal axis the docs' last charts share.
fn gold_axis(y: ScaleOptions) -> ScaleOptions {
    y.label("gold").label_arrow(true).reverse(true)
}

/// `Plot.cell(olympians, Plot.group({fill: "count"}, {x: "sport", y: "gold"}))`
fn e11() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(gold_axis)
        .color(|c| c.type_name("sqrt").scheme("YlGnBu"))
        .mark(group_xy(
            cell(data::olympians()).x("sport").y("gold").fill("count"),
            "sport",
            "gold",
            vec![GroupOutput::new("count", GroupReducer::Count)],
        ))
}

/// `Plot.dot(olympians, Plot.group({r: "count"}, {x: "sport", y: "gold"}))`
fn e12() -> Plot {
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| gold_axis(y.type_name("point")))
        .r(|r| r.range([0., 12.]))
        .mark(group_xy(
            dot(data::olympians()).x("sport").y("gold").r("r"),
            "sport",
            "gold",
            vec![GroupOutput::new("r", GroupReducer::Count)],
        ))
}

/// `Plot.dot(olympians, Plot.group({r: "count"}, {x: "sport", y: "gold", stroke: "sex"}))`
fn e13() -> Plot {
    let olympians = data::olympians();
    let mut mark = group_xy(
        dot(olympians).x("sport").y("gold").stroke("sex").r("r"),
        "sport",
        "gold",
        vec![GroupOutput::new("r", GroupReducer::Count)],
    );
    // Plot groups within each stroke series too.
    mark.channels.z = Some(Channel::Accessor(std::rc::Rc::new(|d: &Table| {
        Value::text(format!(
            "{}\u{0}{}\u{0}{}",
            d.value("sport").label(),
            d.value("gold").label(),
            d.value("sex").label()
        ))
    })));
    Plot::plot()
        .margin_bottom(100.)
        .x(sport_axis)
        .y(|y| gold_axis(y.type_name("point")))
        .r(|r| r.range([0., 12.]))
        .mark(mark)
}

/// Each sport's share of athletes, stacked: `Plot.stackX({order: "x",
/// reverse: true}, Plot.groupZ({x: "proportion"}, {z: "sport", …}))`.
///
/// Gap: the `proportion` reducer is missing, so the port counts each sport
/// and stacks with the `normalize` offset, which is the same proportion; the
/// text's `Plot.filter((D) => D.length > 200, …)` is a label accessor that
/// leaves the smaller sports blank.
fn e14() -> Plot {
    let olympians = data::olympians();
    let mut counts: Vec<(Value, usize)> = Vec::new();
    for d in olympians.rows() {
        let sport = d.value("sport");
        match counts.iter_mut().find(|(s, _)| *s == sport) {
            Some((_, n)) => *n += 1,
            None => counts.push((sport, 1)),
        }
    }
    let big: Vec<Value> = counts
        .into_iter()
        .filter(|(_, n)| *n > 200)
        .map(|(s, _)| s)
        .collect();
    let stacked = |mark: Mark| {
        let mut mark = grouped(
            mark,
            None,
            vec![
                GroupOutput::new("x", GroupReducer::Count),
                GroupOutput::new("y", GroupReducer::First),
            ],
        );
        // The library's `groupZ` puts each group on its own y unless an
        // output writes y; one constant y keeps the sports in one stack.
        mark = mark.y(Value::text(""));
        mark = mark
            .stack_x()
            .stack_offset(Offset::Normalize)
            .stack_order(StackOrder::Value)
            .stack_reverse(true);
        mark
    };
    Plot::plot()
        .height(100.)
        .x(|x| x.percent(true))
        .mark(stacked(
            bar_x(olympians.clone())
                .z("sport")
                .fill_opacity(0.2)
                .inset(0.5)
                .into_mark(),
        ))
        .mark(stacked(
            text(olympians)
                .z("sport")
                .text(move |d: &Table| {
                    let sport = d.value("sport");
                    if big.contains(&sport) {
                        sport
                    } else {
                        Value::Null
                    }
                })
                .text_rotate(90.)
                .into_mark(),
        ))
        .mark(rule_x([0., 1.]))
}
