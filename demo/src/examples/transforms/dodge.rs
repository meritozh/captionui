//! Dodge transform: `transforms/dodge.md`.
//!
//! The library's dodge packs along the axis a mark varies on, so Plot's
//! `dot` with `dodgeY` is a `dotX` here and `dodgeX` a `dotY`.

use crate::prelude::*;
use gpui_plot::{Dodge, DodgeAnchor};

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            height: 160.,
            ..example("transforms", "dodge", 1, e01)
        },
        Example {
            height: 180.,
            ..example("transforms", "dodge", 2, e02)
        },
        example("transforms", "dodge", 3, e03),
        example("transforms", "dodge", 4, e04),
        example("transforms", "dodge", 5, e05),
        Example {
            height: 180.,
            ..example("transforms", "dodge", 6, e06)
        },
        Example {
            height: 180.,
            ..example("transforms", "dodge", 7, e07)
        },
        Example {
            height: 180.,
            ..example("transforms", "dodge", 8, e08)
        },
        Example {
            height: 790.,
            ..example("transforms", "dodge", 9, e09)
        },
        Example {
            height: 790.,
            ..example("transforms", "dodge", 10, e10)
        },
        Example {
            height: 180.,
            ..example("transforms", "dodge", 11, e11)
        },
        Example {
            height: 180.,
            ..example("transforms", "dodge", 12, e12)
        },
    ]
}

/// The IPOs, loaded as the docs do with `d3.csv("ipos.csv", d3.autoType)`.
fn ipos() -> Dataset {
    data::ipos()
}

/// Plot's `dodgeY` defaults: dots of radius 3, a padding of 1, packed up
/// from the bottom, which is the high end of the y axis's pixels.
fn dodge_y() -> Dodge {
    Dodge {
        anchor: DodgeAnchor::End,
        ..Dodge::default()
    }
}

/// `Plot.dotX(cars, Plot.dodgeY({x: "weight (lb)", title: "name", fill: "currentColor"}))`
fn e01() -> Plot {
    Plot::plot().height(160.).mark(
        dot_x(data::cars())
            .x("weight (lb)")
            .tip_title("name")
            .fill("currentColor")
            .dodge(dodge_y()),
    )
}

/// `Plot.rectY(cars, Plot.binX({y: "count"}, {x: "weight (lb)"}))`
fn e02() -> Plot {
    Plot::plot()
        .height(180.)
        .mark(
            rect_y(data::cars())
                .x("weight (lb)")
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `Plot.dot(penguins, Plot.dodgeX("middle", {fx: "species", y: "body_mass_g", fill: "sex"}))`
fn e03() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(
            dot_y(data::penguins())
                .fx("species")
                .y("body_mass_g")
                .fill("sex")
                .dodge(Dodge {
                    anchor: DodgeAnchor::Middle,
                    ..Dodge::default()
                }),
        )
}

/// `Plot.dotX(cars, {x: "weight (lb)"}).plot()`
fn e04() -> Plot {
    dot_x(data::cars()).x("weight (lb)").plot()
}

/// `Plot.ruleX(cars, {x: "weight (lb)"}).plot()`
fn e05() -> Plot {
    rule_x(data::cars()).x("weight (lb)").plot()
}

/// `Plot.dot(cars, Plot.dodgeY(anchor, {x: "weight (lb)", fill: "currentColor"}))`,
/// with the docs' default `anchor` of `"middle"`.
fn e06() -> Plot {
    Plot::plot().height(180.).mark(
        dot_x(data::cars())
            .x("weight (lb)")
            .fill("currentColor")
            .dodge(Dodge {
                anchor: DodgeAnchor::Middle,
                ..dodge_y()
            }),
    )
}

/// `Plot.dot(cars, Plot.dodgeY({x: "weight (lb)", r, fill: "currentColor"}))`,
/// with the docs' default `r` of 3.
fn e07() -> Plot {
    Plot::plot().height(180.).mark(
        dot_x(data::cars())
            .x("weight (lb)")
            .r(3.)
            .fill("currentColor")
            .dodge(Dodge {
                radius: 3.,
                ..dodge_y()
            }),
    )
}

/// `Plot.dot(cars, Plot.dodgeY({x: "weight (lb)", padding, fill: "currentColor"}))`,
/// with the docs' default `padding` of 2.
fn e08() -> Plot {
    Plot::plot().height(180.).mark(
        dot_x(data::cars())
            .x("weight (lb)")
            .fill("currentColor")
            .dodge(Dodge {
                padding: 2.,
                ..dodge_y()
            }),
    )
}

/// Gap: the dodge packs every dot at one radius, not each dot's own `r`, so
/// the variable-size IPO bubbles overlap; `insetRight` is missing; the text
/// mark's `filter` is done on the data.
fn e09() -> Plot {
    let big = Dataset::new(
        ipos()
            .rows()
            .iter()
            .filter(|d| d.num("rMVOP") > 5e3)
            .cloned(),
    );
    Plot::plot()
        .height(790.)
        .mark(
            dot_x(ipos())
                .x("date")
                .r("rMVOP")
                .tip_title(|d: &Table| {
                    Value::text(format!(
                        "{}\n{:.1}B",
                        d.value("NAME").label(),
                        d.num("rMVOP") / 1e3
                    ))
                })
                .fill("currentColor")
                .dodge(dodge_y()),
        )
        .mark(
            text_x(big)
                .x("date")
                .text(|d: &Table| Value::text(format!("{:.0}", d.num("rMVOP") / 1e3)))
                .fill("white")
                .dodge(dodge_y()),
        )
}

/// Gap: a per-datum `fontSize` channel and a dodge sized by `r` are missing;
/// the labels pack at one radius and one size.
fn e10() -> Plot {
    Plot::plot().height(790.).mark(
        text_x(ipos())
            .x("date")
            .r("rMVOP")
            .text(|d: &Table| Value::text(format!("{:.1}", d.num("rMVOP") / 1e3)))
            .tip_title("NAME")
            .dodge(dodge_y()),
    )
}

/// `Plot.dotX(cars, Plot.dodgeY({x: "weight (lb)", …, sort: "weight (lb)"}))`
fn e11() -> Plot {
    Plot::plot().height(180.).mark(
        dot_x(data::cars())
            .x("weight (lb)")
            .tip_title("name")
            .fill("currentColor")
            .sort("weight (lb)", false)
            .dodge(dodge_y()),
    )
}

/// `Plot.dotX(cars, Plot.dodgeY({…, sort: "weight (lb)", reverse: true}))`
fn e12() -> Plot {
    Plot::plot().height(180.).mark(
        dot_x(data::cars())
            .x("weight (lb)")
            .tip_title("name")
            .fill("currentColor")
            .sort("weight (lb)", false)
            .reverse()
            .dodge(dodge_y()),
    )
}
