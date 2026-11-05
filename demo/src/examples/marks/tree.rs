//! Tree mark: `marks/tree.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "tree", 1, e01),
        example("marks", "tree", 2, e02),
        example("marks", "tree", 3, e03),
        example("marks", "tree", 4, e04),
    ]
}

/// The docs' `gods`.
pub fn gods() -> Dataset {
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
}

/// `{path: "name", delimiter: "."}`.
fn flare_spec() -> TreeSpec {
    TreeSpec {
        path: Some("name".into()),
        delimiter: '.',
        ..TreeSpec::default()
    }
}

/// `Plot.tree(gods, {textStroke: "var(--vp-c-bg)"})`
fn e01() -> Plot {
    Plot::plot()
        .no_axes()
        .height(100.)
        .margin(10.)
        .margin_left(40.)
        .margin_right(120.)
        .mark(tree_of(gods(), TreeSpec::default()).text_stroke("white"))
}

/// `Plot.tree(flare, {path: "name", delimiter: ".", textStroke: …})`
fn e02() -> Plot {
    Plot::plot()
        .no_axes()
        .margin(10.)
        .margin_left(30.)
        .margin_right(160.)
        .width(688.)
        .height(1800.)
        .mark(tree_of(data::flare(), flare_spec()).text_stroke("white"))
}

/// `Plot.cluster(flare, {path: "name", treeSort: "node:height", delimiter: ".", …})`
fn e03() -> Plot {
    Plot::plot()
        .no_axes()
        .margin(10.)
        .margin_left(30.)
        .margin_right(160.)
        .width(688.)
        .height(2400.)
        .mark(
            cluster(
                data::flare(),
                TreeSpec {
                    sort: Some(TreeSort::Height),
                    ..flare_spec()
                },
            )
            .text_stroke("white"),
        )
}

/// `Plot.tree(flare, {path: "name", delimiter: ".", treeLayout: indent,
/// strokeWidth: 1, curve: "step-before", textStroke: "none"})`; the docs'
/// `indent` lays nodes out one row each in preorder, indented by depth.
fn e04() -> Plot {
    Plot::plot()
        .no_axes()
        .inset(10.)
        .margin_right(120.)
        .width(200.)
        .height(3600.)
        .mark(
            tree_of(
                data::flare(),
                TreeSpec {
                    layout: TreeLayout::Indent,
                    ..flare_spec()
                },
            )
            .stroke_width(1.)
            .curve("step-before")
            .text_stroke("none"),
        )
}
