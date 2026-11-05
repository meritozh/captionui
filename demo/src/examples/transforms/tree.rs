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

/// ```js
/// Plot.plot({axis: null, height: 100, margin: 20, marginRight: 120, marks: [
///   Plot.link(gods, Plot.treeLink()),
///   Plot.dot(gods, Plot.treeNode()),
///   Plot.text(gods, Plot.treeNode({text: "node:name", dx: 6}))
/// ]})
/// ```
fn e01() -> Plot {
    let gods = || Dataset::values_of(GODS.map(Value::text));
    Plot::plot()
        .no_axes()
        .height(100.)
        .margin(20.)
        .margin_right(120.)
        .mark(link(gods()).tree_link(TreeSpec::default()))
        .mark(dot(gods()).tree_node(TreeSpec::default()))
        .mark(
            text(gods())
                .tree_node(TreeSpec::default())
                .text(Channel::Field("node:name".into()))
                .dx(6.),
        )
}
