//! Image mark <VersionBadge version="0.3.0" />: `marks/image.md`.

use crate::prelude::*;
use gpui_plot::{Dodge, DodgeAnchor};

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "image", 1, e01),
        example("marks", "image", 2, e02),
        Example {
            height: 300.,
            ..example("marks", "image", 3, e03)
        },
        Example {
            height: 640.,
            ..example("marks", "image", 4, e04)
        },
        example("marks", "image", 5, e05),
    ]
}

/// The stand-in portrait's size, in pixels: a 3:4 portrait, as most of the
/// docs' Wikipedia thumbnails are.
const PORTRAIT: [u32; 2] = [30, 40];

/// `src: "Portrait URL"`: the portraits are remote URLs with no local copy
/// in `demo/data`, and the image mark paints one in-memory image rather than
/// an `src` per row, so every president gets the same grey stand-in, sized,
/// fit and clipped as Plot would the portrait.
fn portrait(mark: ImageMark) -> ImageMark {
    let [w, h] = PORTRAIT;
    let pixels = (0..w * h).flat_map(|_| [0xa0, 0xa0, 0xa0, 0xff]).collect();
    mark.image(gpui_plot::image_from_rgba(w, h, pixels))
}

/// A column of the favorability table as a number.
fn pct(d: &Table, field: &str) -> f64 {
    d.number(field).unwrap_or(f64::NAN)
}

fn e01() -> Plot {
    Plot::plot()
        .inset(20.)
        .x(|x| x.label("First inauguration date"))
        .y(|y| y.grid(true).label("Net favorability (%)").tick_format("+f"))
        .mark(rule_y([0.]))
        .mark(portrait(
            image(data::us_president_favorability())
                .x("First Inauguration Date")
                .y(Channel::number(|d| {
                    pct(d, "Very Favorable %") + pct(d, "Somewhat Favorable %")
                        - pct(d, "Very Unfavorable %")
                        - pct(d, "Somewhat Unfavorable %")
                }))
                .width(40.)
                .tip_title("Name"),
        ))
}

fn e02() -> Plot {
    Plot::plot()
        .inset_top(4.)
        .x(|x| x.inset(20.).label("First inauguration date"))
        .y(|y| y.grid(true).label("Any opinion (%)").tick_format("+f"))
        .mark(rule_y([0.]))
        .mark(portrait(
            image(data::us_president_favorability())
                .x("First Inauguration Date")
                .y(Channel::number(|d| {
                    pct(d, "Very Favorable %")
                        + pct(d, "Somewhat Favorable %")
                        + pct(d, "Very Unfavorable %")
                        + pct(d, "Somewhat Unfavorable %")
                }))
                .r(20.)
                .preserve_aspect_ratio("xMidYMin slice")
                .tip_title("Name"),
        ))
}

fn e03() -> Plot {
    Plot::plot().inset(20.).height(280.).mark(portrait(
        image(data::us_president_favorability())
            .x("First Inauguration Date")
            .r(20.)
            .preserve_aspect_ratio("xMidYMin slice")
            .dodge(Dodge {
                anchor: DodgeAnchor::End,
                radius: 20.,
                padding: 1.,
            })
            .tip_title("Name"),
    ))
}

fn e04() -> Plot {
    Plot::plot()
        .aspect_ratio(1.)
        .grid(true)
        .x(|x| x.label("Favorable opinion (%)"))
        .y(|y| y.label("Unfavorable opinion (%)"))
        .mark(rule_y([0.]))
        .mark(rule_x([0.]))
        .mark(portrait(
            image(data::us_president_favorability())
                .x(Channel::number(|d| {
                    pct(d, "Very Favorable %") + pct(d, "Somewhat Favorable %")
                }))
                .y(Channel::number(|d| {
                    pct(d, "Very Unfavorable %") + pct(d, "Somewhat Unfavorable %")
                }))
                .tip_title("Name"),
        ))
}

fn e05() -> Plot {
    // `style: {padding: "10px", color: "black", background:
    // "url(../sea-ice.jpg)", backgroundSize: "cover"}` has no port: the plot
    // has no background image.
    Plot::plot()
        .margin(30.)
        .inset(10.)
        .grid(true)
        .mark(frame())
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fill("white")
                .stroke("black"),
        )
}
