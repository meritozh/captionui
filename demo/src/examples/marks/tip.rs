//! Tip mark <VersionBadge version="0.6.7" />: `marks/tip.md`.
//!
//! Plot's interactive tips (the `tip` mark option and `Plot.tip` with
//! `Plot.pointer`) map onto the plot's built-in hover tooltip here; the
//! static tips are `tip` marks.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    (1..=10)
        .map(|i| {
            example(
                "marks",
                "tip",
                i,
                [e01, e02, e03, e04, e05, e06, e07, e08, e09, e10][i - 1],
            )
        })
        .collect()
}

/// `Plot.lineY(aapl, {x: "Date", y: "Close", tip: true}).plot({y: {grid: true}})`
fn e01() -> Plot {
    line_y(data::aapl())
        .x("Date")
        .y("Close")
        .plot()
        .y(|y| y.grid(true))
}

/// A one-row tip dataset: the note and where it points. Plot passes the
/// position as constants (`x: new Date("2015-02-23"), y: 133`); a constant
/// date channel does not make the x scale temporal here, so the position
/// rides along as fields.
fn note(text: &str, (y, m, d): (i32, u32, u32), close: f64) -> Dataset {
    Dataset::new([Table::empty()
        .with("text", Value::text(text.to_string()))
        .with(
            "Date",
            Value::date(chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()),
        )
        .with("Close", Value::Number(close))])
}

fn e02() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(line_y(data::aapl()).x("Date").y("Close"))
        .mark(
            tip(note(
                "Apple stock reaches a new high of $133 on Feb. 23, 2015. The release of the first Apple Watch, slated for April, is hotly anticipated.",
                (2015, 2, 23),
                133.,
            ))
                .text("text")
                .x("Date")
                .y("Close")
                .dy(-3.)
                .tip_anchor(FrameAnchor::Bottom),
        )
        .mark(
            tip(note(
                "Apple stock drops 8% after the company misses Q2 revenue targets and reports declining iPhone sales. It reaches a two-year low of $90.34 on May 12.",
                (2016, 5, 12),
                90.34,
            ))
                .text("text")
                .x("Date")
                .y("Close")
                .dy(3.)
                .tip_anchor(FrameAnchor::Top),
        )
}

/// The `Plot.tip(olympians, Plot.pointer({…, filter, title}))` becomes the
/// dot's hover tooltip title; there is no pointer transform or filter.
fn e03() -> Plot {
    let has_info = |d: &Table| d.text("info").is_some_and(|s| !s.is_empty());
    Plot::plot().grid(true).mark(
        dot(data::olympians())
            .x("weight")
            .y("height")
            .fy("sex")
            .sort("info", false)
            .stroke(move |d: &Table| {
                if has_info(d) {
                    gpui_plot::css::parse("black").unwrap()
                } else {
                    gpui_plot::css::parse("#aaa").unwrap()
                }
            })
            .tip_title(|d: &Table| {
                Value::text(format!(
                    "{}\n\n{}",
                    d.value("name").label(),
                    d.value("info").label()
                ))
            }),
    )
}

/// The extra `channels: {name, sport}` become the tooltip title.
fn e04() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .stroke("sex")
        .tip_title(|d: &Table| {
            Value::text(format!(
                "name {}\nsport {}",
                d.value("name").label(),
                d.value("sport").label()
            ))
        })
        .plot()
}

/// `Plot.rectY(olympians, Plot.binX({y: "count"}, {x: "weight", fill: "sex", tip: true})).plot()`
fn e05() -> Plot {
    rect_y(data::olympians())
        .x("weight")
        .fill("sex")
        .bin(Thresholds::Auto)
        .bin_output("y", GroupReducer::Count)
        .plot()
}

fn e06() -> Plot {
    rect_y(data::olympians())
        .x("weight")
        .y(Channel::number(|d: &Table| {
            if d.value("sex").label() == "male" {
                1.
            } else {
                -1.
            }
        }))
        .fill("sex")
        .bin(Thresholds::Auto)
        .bin_output("y", GroupReducer::Sum)
        .plot()
        .y(|y| y.label("Frequency"))
}

/// The tip's `format` (channel order, `kg`/`m` suffixes, hidden stroke)
/// becomes the tooltip title.
fn e07() -> Plot {
    dot(data::olympians())
        .x("weight")
        .y("height")
        .stroke("sex")
        .tip_title(|d: &Table| {
            Value::text(format!(
                "name {}\nsport {}\ncountry {}\nheight {}m\nweight {}kg",
                d.value("name").label(),
                d.value("sport").label(),
                d.value("nationality").label(),
                d.value("height").label(),
                d.value("weight").label(),
            ))
        })
        .plot()
}

fn e08() -> Plot {
    let anchors = [
        ("top", FrameAnchor::Top),
        ("right", FrameAnchor::Right),
        ("bottom", FrameAnchor::Bottom),
        ("left", FrameAnchor::Left),
        ("top-left", FrameAnchor::TopLeft),
        ("top-right", FrameAnchor::TopRight),
        ("bottom-right", FrameAnchor::BottomRight),
        ("bottom-left", FrameAnchor::BottomLeft),
        ("middle", FrameAnchor::Middle),
    ];
    let mut plot = Plot::plot().height(160.).mark(frame().stroke_opacity(0.2));
    for (name, anchor) in anchors {
        plot = plot
            .mark(dot(Dataset::length(1)).frame_anchor(anchor).fill("blue"))
            .mark(
                tip([name])
                    .text(Channel::identity())
                    .frame_anchor(anchor)
                    .tip_anchor(anchor),
            );
    }
    plot
}

fn e09() -> Plot {
    Plot::plot()
        .projection("albers-usa")
        .mark(geo(data::states()))
        .mark(
            tip(data::states())
                .geo_centroid()
                .text("name")
                .tip_anchor(FrameAnchor::Bottom)
                .text_padding(3.),
        )
}

fn e10() -> Plot {
    tip(["Danger! This tip is red."])
        .text(Channel::identity())
        .frame_anchor(FrameAnchor::Bottom)
        .tip_anchor(FrameAnchor::Bottom)
        .fill("red")
        .plot()
}
