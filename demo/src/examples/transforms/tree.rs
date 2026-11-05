//! Tree transform: `transforms/tree.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![Example {
        height: 100.,
        ..example("transforms", "tree", 1, e01)
    }]
}

/// The docs' `gods`.
const GODS: [&str; 6] = [
    "Chaos/Gaia/Mountains",
    "Chaos/Gaia/Pontus",
    "Chaos/Gaia/Uranus",
    "Chaos/Eros",
    "Chaos/Erebus",
    "Chaos/Tartarus",
];

/// `Plot.link(gods, Plot.treeLink())`, `Plot.dot(gods, Plot.treeNode())` and
/// `Plot.text(gods, Plot.treeNode({text: "node:name", dx: 6}))`.
///
/// Gap: the `treeLink` and `treeNode` transforms are missing (the library's
/// `tree` mark lays its tree top-down and draws no labels), so the port lays
/// the paths out with the library's tidy tree and feeds ordinary `link`,
/// `dot` and `text` marks: depth across, as Plot's tree runs. (Plot's
/// `bump-x` link curve is left out: the library's `link` with a curve draws
/// stubs rather than the link.)
fn e01() -> Plot {
    let paths: Vec<Vec<&str>> = GODS.iter().map(|path| path.split('/').collect()).collect();
    let layout = gpui_plot::tree::tidy(&paths);
    let nodes = Dataset::new(layout.nodes.iter().map(|node| {
        Table::empty()
            .with("x", Value::Number(node.depth as f64))
            .with("y", Value::Number(node.x))
            .with("name", Value::text(node.label.clone()))
    }));
    let links = Dataset::new(layout.links.iter().map(|&(child, parent)| {
        let (child, parent) = (&layout.nodes[child], &layout.nodes[parent]);
        Table::empty()
            .with("x1", Value::Number(parent.depth as f64))
            .with("y1", Value::Number(parent.x))
            .with("x2", Value::Number(child.depth as f64))
            .with("y2", Value::Number(child.x))
    }));
    Plot::plot()
        .no_axes()
        .height(100.)
        .margin(20.)
        .margin_right(120.)
        // Plot's tree counts its rows down from the top.
        .y(|y| y.reverse(true))
        .mark(
            link(links)
                .x1("x1")
                .y1("y1")
                .x2("x2")
                .y2("y2")
                .stroke_opacity(0.5),
        )
        .mark(dot(nodes.clone()).x("x").y("y"))
        .mark(
            text(nodes)
                .x("x")
                .y("y")
                .text("name")
                .text_anchor(TextAnchor::Start)
                .dx(6.),
        )
}
