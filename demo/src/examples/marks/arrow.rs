//! Arrow mark: `marks/arrow.md`.

use std::collections::HashMap;

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "arrow", 1, e01),
        example("marks", "arrow", 2, e02),
        Example {
            height: 1100.,
            ..example("marks", "arrow", 3, e03)
        },
    ]
}

/// Plot's `bend: true`, 22.5 degrees.
const BEND: f32 = 22.5;

fn e01() -> Plot {
    let metros = data::metros();
    // `filter: "highlight"`, applied to the data.
    let highlighted: Dataset = metros
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
            arrow(metros)
                .x1("POP_1980")
                .y1("R90_10_1980")
                .x2("POP_2015")
                .y2("R90_10_2015")
                .bend(BEND)
                .stroke(ColorChannel::number(|d| {
                    d.num("R90_10_2015") - d.num("R90_10_1980")
                })),
        )
        .mark(
            text(highlighted)
                .x("POP_2015")
                .y("R90_10_2015")
                .text("nyt_display")
                .fill("currentColor")
                .stroke("white")
                .dy(-6.),
        )
}

/// The docs' `markov`: three nodes on a circle and an edge for every pair,
/// weighted by the transition matrix.
fn markov() -> (Dataset, Vec<Table>) {
    let matrix = [[3., 2., 5.], [1., 7., 2.], [1., 1., 8.]];
    // `d3.pointRadial(angle, 100)`: `[100 sin(angle), -100 cos(angle)]`.
    let nodes: Vec<[f64; 2]> = (0..3)
        .map(|i| {
            let angle = ((2 - i) as f64 * std::f64::consts::TAU) / 3.;
            [100. * angle.sin(), -100. * angle.cos()]
        })
        .collect();
    let edges = (0..3)
        .flat_map(|i| {
            let nodes = nodes.clone();
            (0..3).map(move |j| {
                Table::empty()
                    .with("x1", Value::Number(nodes[i][0]))
                    .with("y1", Value::Number(nodes[i][1]))
                    .with("x2", Value::Number(nodes[j][0]))
                    .with("y2", Value::Number(nodes[j][1]))
                    .with("value", Value::Number(matrix[i][j]))
            })
        })
        .collect();
    (Dataset::tuples(nodes), edges)
}

fn e02() -> Plot {
    let (nodes, edges) = markov();
    let labels: Dataset = nodes
        .rows()
        .iter()
        .zip(["A", "B", "C"])
        .map(|(node, label)| node.clone().with("label", Value::text(label)))
        .collect();
    let mut plot = Plot::plot()
        .inset(60.)
        .aspect_ratio(1.)
        .no_axes()
        .mark(dot(nodes).x("0").y("1").r(40.));
    // `strokeWidth: ([,, value]) => value` is a channel in Plot; the port
    // draws an arrow mark per edge to vary it.
    for edge in &edges {
        plot = plot.mark(
            arrow(vec![edge.clone()])
                .x1("x1")
                .y1("y1")
                .x2("x2")
                .y2("y2")
                .bend(BEND)
                .stroke_width(edge.num("value") as f32)
                .head_length(24.)
                .inset(48.),
        );
    }
    plot.mark(text(labels).x("0").y("1").text("label").dy(12.))
        .mark(
            text(edges)
                .x(Channel::number(|d| {
                    (d.num("x1") + d.num("x2")) / 2. + (d.num("y1") - d.num("y2")) * 0.15
                }))
                .y(Channel::number(|d| {
                    (d.num("y1") + d.num("y2")) / 2. - (d.num("x1") - d.num("x2")) * 0.15
                }))
                .text("value"),
        )
}

fn e03() -> Plot {
    let json = data::json("miserables.json");
    let nodes: Dataset = json["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|node| {
            Table::empty()
                .with(
                    "id",
                    Value::text(node["id"].as_str().unwrap_or_default().to_string()),
                )
                .with(
                    "group",
                    Value::Number(node["group"].as_f64().unwrap_or(f64::NAN)),
                )
        })
        .collect();
    let groups: HashMap<String, f64> = nodes
        .rows()
        .iter()
        .map(|d| {
            (
                d.text("id").map(|s| s.to_string()).unwrap_or_default(),
                d.num("group"),
            )
        })
        .collect();
    // `samegroup({source, target})`: the shared group, or null, kept as a
    // field so the mark can both color and sort by it.
    let links: Dataset = json["links"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|link| {
            let source = link["source"].as_str().unwrap_or_default().to_string();
            let target = link["target"].as_str().unwrap_or_default().to_string();
            let samegroup = match (groups.get(&source), groups.get(&target)) {
                (Some(a), Some(b)) if a == b => Value::Number(*a),
                _ => Value::Null,
            };
            Table::empty()
                .with("source", Value::text(source))
                .with("target", Value::text(target))
                .with("samegroup", samegroup)
        })
        .collect();
    Plot::plot()
        .height(1080.)
        .margin_left(100.)
        .no_axes()
        .x(|x| x.domain([0., 1.]))
        .color(|c| c.domain((0..10).map(|i| i as f64)).unknown("#ccc"))
        .mark(
            dot(nodes.clone())
                .x(Value::Number(0.))
                .y("id")
                .fill("group")
                .sort_domain("y", "fill", false),
        )
        .mark(
            text(nodes)
                .x(Value::Number(0.))
                .y("id")
                .text("id")
                .text_anchor(TextAnchor::End)
                .dx(-6.)
                .fill("group"),
        )
        .mark(
            arrow(links)
                .x(Value::Number(0.))
                .y1("source")
                .y2("target")
                // `sweep: "-y"` has no equivalent; the bend alone curves
                // each arc by its own direction.
                .bend(90.)
                .head_length(0.)
                .stroke("samegroup")
                .sort("samegroup", false)
                .reverse(),
        )
}
