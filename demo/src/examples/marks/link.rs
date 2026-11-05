//! Link mark: `marks/link.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "link", 1, e01),
        Example {
            height: 140.,
            ..example("marks", "link", 2, e02)
        },
        example("marks", "link", 3, e03),
        example("marks", "link", 4, e04),
    ]
}

fn e01() -> Plot {
    let highlighted: Dataset = data::metros()
        .rows()
        .iter()
        .filter(|d| d.number("highlight").is_some_and(|h| h != 0.))
        .cloned()
        .collect();
    Plot::plot()
        .grid(true)
        .inset(10.)
        .x(|x| x.type_name("log").label("Population"))
        .y(|y| y.label("Inequality").ticks(4))
        .color(|c| {
            c.scheme("BuRd")
                .label("Change in inequality from 1980 to 2015")
                .legend(true)
                .tick_format("+f")
        })
        .mark(
            link(data::metros())
                .x1("POP_1980")
                .y1("R90_10_1980")
                .x2("POP_2015")
                .y2("R90_10_2015")
                .stroke(ColorChannel::number(|d| {
                    d.num("R90_10_2015") - d.num("R90_10_1980")
                }))
                .marker_end("arrow"),
        )
        .mark(
            text(highlighted)
                .x("POP_2015")
                .y("R90_10_2015")
                .text("nyt_display")
                .fill("currentColor")
                .stroke("var(--vp-c-bg)")
                .dy(-8.),
        )
}

/// `Plot.treeLink` and `Plot.treeNode` are not separate transforms here: the
/// `tree` mark draws the same links, nodes and labels in one.
fn e02() -> Plot {
    let gods = Dataset::values_of([
        "Chaos/Gaia/Mountains",
        "Chaos/Gaia/Pontus",
        "Chaos/Gaia/Uranus",
        "Chaos/Eros",
        "Chaos/Erebus",
        "Chaos/Tartarus",
    ]);
    Plot::plot()
        .no_axes()
        .height(120.)
        .inset(20.)
        .margin_right(120.)
        .mark(tree(gods).path(Channel::identity()))
}

/// Without `projection: "equal-earth"`, the sphere and the world's `land`,
/// the San Francisco–Paris links are drawn on linear lon/lat axes.
fn e03() -> Plot {
    let xy = |mark: LinkMark| {
        mark.x1(Value::Number(-122.4194))
            .y1(Value::Number(37.7749))
            .x2(Value::Number(2.3522))
            .y2(Value::Number(48.8566))
    };
    Plot::plot()
        .x(|x| x.domain([-180., 180.]))
        .y(|y| y.domain([-90., 90.]))
        .mark(xy(link(Dataset::length(1))
            .curve(Curve::Linear)
            .stroke("red")))
        .mark(xy(link(Dataset::length(1))
            .marker_start("dot")
            .marker_end("arrow")
            .stroke_width(1.5)))
}

/// The scales' `transform: (d) => d / 1000` is applied in the channels.
fn e04() -> Plot {
    let ks = || Dataset::values_of([0.6, 0.7, 0.8, 0.9, 1.]);
    let k = |d: &Table| d.num(gpui_plot::table::VALUE);
    Plot::plot()
        .aspect_ratio(1.)
        .margin_right(40.)
        .x(|x| x.label("Median annual income (men, thousands)"))
        .y(|y| y.label("Median annual income (women, thousands)"))
        .mark(
            link(ks())
                .x1(Value::Number(0.))
                .y1(Value::Number(0.))
                .x2(Value::Number(102.))
                .y2(Channel::number(move |d| 102. * k(d)))
                .opacity(move |d: &Table| if k(d) == 1. { 1. } else { 0.2 }),
        )
        .mark(
            text(ks())
                .x(Value::Number(102.))
                .y(Channel::number(move |d| 102. * k(d)))
                .text(move |d: &Table| {
                    let k = k(d);
                    Value::text(if k == 1. {
                        "Equal".to_string()
                    } else {
                        format!("{:+.0}%", (k - 1.) * 100.)
                    })
                })
                .text_anchor(TextAnchor::Start)
                .dx(6.),
        )
        .mark(
            dot(data::income_gender())
                .x(Channel::number(|d| d.num("m") / 1000.))
                .y(Channel::number(|d| d.num("f") / 1000.)),
        )
}
