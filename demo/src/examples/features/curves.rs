//! Curves: `features/curves.md`.

use crate::prelude::*;
use gpui_plot::table::VALUE;

pub fn examples() -> Vec<Example> {
    vec![example("features", "curves", 1, e01)]
}

/// d3's `d3.randomLcg(seed)`: a linear congruential generator in `[0, 1)`.
pub(super) fn random_lcg(seed: u32) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_mul(0x19660D).wrapping_add(0x3C6EF35F);
        state as f64 / 4_294_967_296.
    }
}

/// `d3.range(20).map(d3.randomLcg(42))`, each value with its index so a
/// channel can read `(d, i) => i`.
fn numbers() -> Dataset {
    let mut random = random_lcg(42);
    Dataset::new((0..20).map(|i| {
        Table::empty()
            .with(VALUE, Value::Number(random()))
            .with("$index", Value::Number(i as f64))
    }))
}

/// The page's curve control starts at `catmull-rom`.
///
/// ```js
/// Plot.plot({
///   marks: [
///     Plot.lineY(numbers, {curve: "catmull-rom"}),
///     Plot.dotY(numbers, {x: (d, i) => i})
///   ]
/// })
/// ```
fn e01() -> Plot {
    Plot::plot()
        .mark(
            line_y(numbers())
                .y(Channel::identity())
                .curve(Curve::CatmullRom),
        )
        .mark(dot_y(numbers()).y(Channel::identity()).x("$index"))
}
