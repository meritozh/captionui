//! Filter transform: `transforms/filter.md`.
//!
//! Gap: marks have no `filter` option, so each port filters the mark's data
//! up front; the result is the same chart, though Plot's filter keeps the
//! unfiltered data's index for the scales and channels.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "filter", 1, e01),
        example("transforms", "filter", 2, e02),
        example("transforms", "filter", 3, e03),
        example("transforms", "filter", 4, e04),
    ]
}

/// The rows of `data` that `test` accepts, standing in for Plot's `filter`.
fn filtered(data: Dataset, test: impl Fn(&Table) -> bool) -> Dataset {
    Dataset::new(data.rows().iter().filter(|d| test(d)).cloned())
}

/// The docs' `/[aeiouy]/i.test(d.letter)`.
fn is_vowel(d: &Table) -> bool {
    d.value("letter")
        .label()
        .chars()
        .any(|c| "aeiouy".contains(c.to_ascii_lowercase()))
}

/// `Plot.text(metros, {filter: "highlight", x: "POP_2015", y: "R90_10_2015", text: "nyt_display", …})`
fn e01() -> Plot {
    let highlighted = filtered(data::metros(), |d| match d.value("highlight") {
        Value::Bool(b) => b,
        Value::Number(n) => n != 0.,
        Value::Text(t) => !t.is_empty() && &*t != "false",
        _ => false,
    });
    Plot::plot()
        .grid(true)
        .x(|x| x.type_name("log"))
        .mark(dot(data::metros()).x("POP_2015").y("R90_10_2015"))
        .mark(
            text(highlighted)
                .x("POP_2015")
                .y("R90_10_2015")
                .text("nyt_display")
                .frame_anchor(FrameAnchor::Bottom)
                .dy(-6.),
        )
}

/// A bar chart with the vowels outlined, `filter: (d) => /[aeiouy]/i.test(d.letter)`.
fn e02() -> Plot {
    Plot::plot()
        .mark(bar_y(data::alphabet()).x("letter").y("frequency"))
        .mark(
            bar_y(filtered(data::alphabet(), is_vowel))
                .x("letter")
                .y("frequency")
                .stroke("red")
                .stroke_width(3.)
                .inset(-3.),
        )
}

/// `Plot.barY(alphabet, {filter: (d) => /[aeiouy]/i.test(d.letter), x: "letter", y: "frequency"})`
fn e03() -> Plot {
    Plot::plot().mark(
        bar_y(filtered(data::alphabet(), is_vowel))
            .x("letter")
            .y("frequency"),
    )
}

/// `Plot.barY(alphabet.filter((d) => /[aeiouy]/i.test(d.letter)), {x: "letter", y: "frequency"})`
fn e04() -> Plot {
    Plot::plot().mark(
        bar_y(filtered(data::alphabet(), is_vowel))
            .x("letter")
            .y("frequency"),
    )
}
