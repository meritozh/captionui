//! Waffle mark <VersionBadge version="0.6.16" pr="2040" />: `marks/waffle.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "waffle", 1, e01),
        example("marks", "waffle", 2, e02),
        example("marks", "waffle", 3, e03),
        example("marks", "waffle", 4, e04),
        Example {
            height: 260.,
            ..example("marks", "waffle", 5, e05)
        },
        Example {
            height: 240.,
            ..example("marks", "waffle", 6, e06)
        },
    ]
}

/// Plot's `groupZ({y: "count"}, …)`: one group per series, counting rows
/// into y.
fn group_z_count<M: From<Mark> + Into<Mark>>(mark: M) -> M {
    let mut mark: Mark = mark.into();
    mark.group = Some(GroupConfig {
        axis: None,
        outputs: vec![GroupOutput::new("y", GroupReducer::Count)],
    });
    mark.into()
}

/// The docs' `survey`.
fn survey() -> Dataset {
    Dataset::new(
        [
            ("don’t go out after dark", 96.),
            ("do no activities other than school", 89.),
            (
                "engage in political discussion and social movements, including online",
                10.,
            ),
            (
                "would like to do activities but are prevented by safety concerns",
                73.,
            ),
        ]
        .map(|(question, yes)| {
            Table::empty()
                .with("question", Value::text(question))
                .with("yes", Value::Number(yes))
        }),
    )
}

/// `Plot.waffleY([212, 207, 315, 11], {x: ["apples", "bananas", "oranges",
/// "pears"]}).plot({height: 420})`; the parallel `x` array is zipped into
/// the data.
fn e01() -> Plot {
    let fruit = Dataset::new(
        [212., 207., 315., 11.]
            .into_iter()
            .zip(["apples", "bananas", "oranges", "pears"])
            .map(|(value, name)| {
                Table::empty()
                    .with("value", Value::Number(value))
                    .with("x", Value::text(name))
            }),
    );
    // Plot's array channels carry no name, so its axes go unlabelled.
    waffle_y(fruit)
        .x("x")
        .y("value")
        .plot()
        .height(420.)
        .x(|x| x.no_label())
        .y(|y| y.no_label())
}

/// `Plot.waffleY(olympians, Plot.groupX({y: "count"}, {x: "sex"})).plot({x: {label: null}})`
fn e02() -> Plot {
    waffle_y(data::olympians())
        .x("sex")
        .group_x()
        .plot()
        .x(|x| x.no_label())
}

/// The docs' unit picker starts at 10.
fn e03() -> Plot {
    Plot::plot()
        .fx_scale(|fx| fx.interval("5 years").no_label())
        .mark(group_z_count(
            waffle_y(data::olympians()).fx("date_of_birth").unit(10.),
        ))
}

fn e04() -> Plot {
    Plot::plot()
        .fx_scale(|fx| fx.interval(10.))
        .color(|c| c.legend(true))
        .mark(group_z_count(
            waffle_y(data::olympians())
                .fill("sex")
                .sort("sex", false)
                .fx("weight")
                .unit(10.),
        ))
}

/// Plot's `axisFx({lineWidth: 10, anchor: "bottom", dy: 20})` and `rx:
/// "100%"` have no counterpart: the fx axis is the default one at the bottom
/// and the cells are rounded by a large corner radius.
fn e05() -> Plot {
    Plot::plot()
        .no_axes()
        .x(|x| x.no_label())
        .y(|y| y.no_label())
        .height(260.)
        .margin_top(20.)
        .margin_bottom(70.)
        .title("Subdued")
        .subtitle("Of 120 surveyed Syrian teenagers:")
        .fx_scale(|fx| fx.axis(AxisAnchor::Bottom).no_label())
        .mark(
            waffle_y(Dataset::length(1))
                .y(Value::Number(120.))
                .fill_opacity(0.4)
                .corner_radius(100.),
        )
        .mark(
            waffle_y(survey())
                .fx("question")
                .y("yes")
                .corner_radius(100.)
                .fill("orange"),
        )
        .mark(
            text(survey())
                .fx("question")
                .text(|d: &Table| Value::text(format!("{}%", (d.num("yes") / 120. * 100.).round())))
                .frame_anchor(FrameAnchor::Bottom)
                .line_anchor(LineAnchor::Top)
                .dy(6.)
                .fill("orange")
                .font_size(24.)
                .font_weight(700),
        )
}

/// The docs' apples slider starts at 512.
fn e06() -> Plot {
    let apples = Dataset::new([Table::empty()
        .with("value", Value::Number(512.))
        .with("y", Value::text("apples"))]);
    // Plot's array channels carry no name, so its axes go unlabelled.
    waffle_x(apples)
        .y("y")
        .x("value")
        .plot()
        .height(240.)
        .x(|x| x.no_label())
        .y(|y| y.no_label())
}
