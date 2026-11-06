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
            text(data::metros())
                .filter(|d| d.number("highlight").is_some_and(|h| h != 0.))
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
/// ```js
/// Plot.plot({axis: null, height: 120, inset: 20, insetRight: 120, marks: [
///   Plot.link(gods, Plot.treeLink({stroke: "node:internal"})),
///   Plot.dot(gods, Plot.treeNode({fill: "node:internal"})),
///   Plot.text(gods, Plot.treeNode({text: "node:name", stroke: "var(--vp-c-bg)", fill: "currentColor", dx: 6}))
/// ]})
/// ```
fn e02() -> Plot {
    let gods = || {
        Dataset::values_of(
            [
                "Chaos/Gaia/Mountains",
                "Chaos/Gaia/Pontus",
                "Chaos/Gaia/Uranus",
                "Chaos/Eros",
                "Chaos/Erebus",
                "Chaos/Tartarus",
            ]
            .map(Value::text),
        )
    };
    Plot::plot()
        .no_axes()
        .height(120.)
        .inset(20.)
        .inset_right(120.)
        .mark(
            link(gods())
                .tree_link(TreeSpec::default())
                .stroke("node:internal"),
        )
        .mark(
            dot(gods())
                .tree_node(TreeSpec::default())
                .fill("node:internal"),
        )
        .mark(
            text(gods())
                .tree_node(TreeSpec::default())
                .text(Channel::Field("node:name".into()))
                .text_stroke("white")
                .fill("currentColor")
                .dx(6.),
        )
}

fn e03() -> Plot {
    let xy = |mark: LinkMark| {
        mark.x1(Value::Number(-122.4194))
            .y1(Value::Number(37.7749))
            .x2(Value::Number(2.3522))
            .y2(Value::Number(48.8566))
    };
    Plot::plot()
        .projection("equal-earth")
        .mark(geo(gpui_plot::geo::sphere()))
        .mark(geo(data::land()).fill("currentColor").fill_opacity(0.3))
        .mark(xy(link(Dataset::length(1))
            .curve(Curve::Linear)
            .stroke("red")))
        .mark(xy(link(Dataset::length(1))
            .marker_start("dot")
            .marker_end("arrow")
            .stroke_width(1.5)))
}

fn e04() -> Plot {
    let ks = || Dataset::values_of([0.6, 0.7, 0.8, 0.9, 1.]);
    let k = |d: &Table| d.num(gpui_plot::table::VALUE);
    Plot::plot()
        .aspect_ratio(1.)
        .margin_right(40.)
        .x(|x| {
            x.label("Median annual income (men, thousands)")
                .transform(|d| d / 1000.)
        })
        .y(|y| {
            y.label("Median annual income (women, thousands)")
                .transform(|d| d / 1000.)
        })
        .mark(
            link(ks())
                .x1(Value::Number(0.))
                .y1(Value::Number(0.))
                .x2(Value::Number(102000.))
                .y2(Channel::number(move |d| 102000. * k(d)))
                .opacity(move |d: &Table| if k(d) == 1. { 1. } else { 0.2 }),
        )
        .mark(
            text(ks())
                .x(Value::Number(102000.))
                .y(Channel::number(move |d| 102000. * k(d)))
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
        .mark(dot(data::income_gender()).x("m").y("f"))
}
