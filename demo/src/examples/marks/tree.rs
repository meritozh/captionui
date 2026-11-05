//! Tree mark <VersionBadge version="0.4.3" />: `marks/tree.md`.
//!
//! The library's `tree` reads slash-separated paths only, so flare's
//! dot-delimited names are rewritten with slashes (Plot's `delimiter: "."`).

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            height: 100.,
            ..example("marks", "tree", 1, e01)
        },
        Example {
            height: 1800.,
            ..example("marks", "tree", 2, e02)
        },
        Example {
            height: 2400.,
            ..example("marks", "tree", 3, e03)
        },
        Example {
            height: 3600.,
            ..example("marks", "tree", 4, e04)
        },
    ]
}

/// The docs' `gods`.
fn gods() -> Dataset {
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

/// Plot's `path: "name", delimiter: "."`.
fn flare_path(d: &Table) -> Value {
    Value::text(d.value("name").label().replace('.', "/"))
}

fn e01() -> Plot {
    Plot::plot()
        .no_axes()
        .height(100.)
        .margin(10.)
        .margin_left(40.)
        .margin_right(120.)
        .mark(tree(gods()).path(Channel::identity()))
}

fn e02() -> Plot {
    Plot::plot()
        .no_axes()
        .margin(10.)
        .margin_left(30.)
        .margin_right(160.)
        .height(1800.)
        .mark(tree(data::flare()).path(flare_path))
}

/// `Plot.cluster` (d3.cluster, `treeSort: "node:height"`) has no
/// counterpart; this draws the tidy tree.
fn e03() -> Plot {
    Plot::plot()
        .no_axes()
        .margin(10.)
        .margin_left(30.)
        .margin_right(160.)
        .height(2400.)
        .mark(tree(data::flare()).path(flare_path))
}

/// The custom `treeLayout: indent` and `curve: "step-before"` have no
/// counterpart; this draws the tidy tree at the docs' size.
fn e04() -> Plot {
    Plot::plot().no_axes().inset(10.).height(3600.).mark(
        tree(data::flare())
            .path(flare_path)
            .stroke_width(1.)
            .curve(Curve::StepBefore),
    )
}
