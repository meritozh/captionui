//! Box mark: `marks/box.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "box", 1, e01),
        example("marks", "box", 2, e02),
        example("marks", "box", 3, e03),
        example("marks", "box", 4, e04),
        example("marks", "box", 5, e05),
    ]
}

/// `(x) => x.toFixed(1)`
fn fixed1(v: &Value) -> String {
    format!("{:.1}", v.as_number().unwrap_or(f64::NAN))
}

fn e01() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true).inset(6.))
        .mark(box_y(data::morley()).x("Expt").y("Speed"))
}

fn e02() -> Plot {
    Plot::plot()
        .x(|x| x.grid(true).inset(6.))
        .mark(box_x(data::morley()).x("Speed").y("Expt"))
}

/// `Plot.boxX([0, 3, 4.4, 4.5, 4.6, 5, 7]).plot()`
fn e03() -> Plot {
    // Plot's boxX defaults x to the value; the port names it.
    box_x([0., 3., 4.4, 4.5, 4.6, 5., 7.])
        .x(Channel::identity())
        .plot()
}

fn e04() -> Plot {
    Plot::plot()
        .margin_left(60.)
        .y(|y| y.grid(true).label("Price"))
        .x(|x| {
            x.interval(0.5)
                .label("Carats")
                .label_anchor(LabelAnchor::End)
                .tick_format(fixed1)
        })
        .mark(rule_y([0.]))
        .mark(
            box_y(data::diamonds())
                .x(Channel::number(|d| (d.num("carat") * 2.).floor() / 2.))
                .y("price"),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .margin_left(60.)
        .y(|y| y.grid(true).label("Price"))
        .fx_scale(|fx| {
            fx.interval(0.5)
                .label("Carats")
                .label_anchor(LabelAnchor::End)
                .tick_format(fixed1)
        })
        .mark(rule_y([0.]))
        .mark(box_y(data::diamonds()).fx("carat").y("price"))
}
