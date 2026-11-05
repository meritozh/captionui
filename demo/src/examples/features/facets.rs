//! Facets: `features/facets.md`.

use std::collections::HashMap;
use std::rc::Rc;

use crate::prelude::*;
use gpui_plot::table::VALUE;

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            height: 820.,
            ..example("features", "facets", 1, e01)
        },
        Example {
            height: 880.,
            ..example("features", "facets", 2, e02)
        },
        example("features", "facets", 3, e03),
        example("features", "facets", 4, e04),
        example("features", "facets", 5, e05),
        Example {
            height: 320.,
            ..example("features", "facets", 6, e06)
        },
        example("features", "facets", 7, e07),
        example("features", "facets", 8, e08),
    ]
}

/// Plot's mark `sort: {scale: "-by", reduce}`.
fn sort_by(scale: &str, by: &str, reduce: GroupReducer, reverse: bool) -> DomainSort {
    DomainSort {
        scale: scale.to_string().into(),
        by: by.to_string().into(),
        reduce,
        reverse,
        limit: None,
    }
}

fn e01() -> Plot {
    Plot::plot()
        .height(800.)
        .margin_right(90.)
        .margin_left(110.)
        .grid(true)
        .x(|x| x.nice(true))
        .y(|y| y.inset(5.))
        .color(|c| c.type_name("categorical"))
        .mark(frame())
        .mark(
            dot(data::barley())
                .x("yield")
                .y("variety")
                .fy("site")
                .stroke("year")
                .sort_domain_by(sort_by("y", "x", GroupReducer::Median, true))
                .sort_domain_by(sort_by("fy", "x", GroupReducer::Median, true)),
        )
}

/// `Plot.groupY({x1: "first", x2: "last", stroke: ([x1, x2]) => x2 - x1})`:
/// the custom stroke reducer sees each group's yields.
fn e02() -> Plot {
    Plot::plot()
        .height(800.)
        .margin_left(110.)
        .margin_right(90.)
        .grid(true)
        .x(|x| x.nice(true))
        .y(|y| y.inset(5.))
        .color(|c| {
            c.scheme("spectral")
                .label("Change in yield")
                .tick_format("+f")
                .legend(true)
        })
        .mark(frame())
        .mark(
            arrow(data::barley())
                .x("yield")
                .y("variety")
                .fy("site")
                .stroke("yield")
                // Gap: a stroke field implies `z` even when stroke is a group
                // output (Plot's maybeSubgroup skips outputs), splitting every
                // group by yield; `z: null` undoes that.
                .z(Value::Null)
                .stroke_width(2.)
                .group_y()
                .group_output("x1", GroupReducer::First)
                .group_output("x2", GroupReducer::Last)
                .group_output(
                    "stroke",
                    GroupReducer::custom(|values, _| {
                        let first = values.first().and_then(Value::as_number);
                        let last = values.last().and_then(Value::as_number);
                        match (first, last) {
                            (Some(x1), Some(x2)) => Value::Number(x2 - x1),
                            _ => Value::Null,
                        }
                    }),
                )
                .sort_domain_by(sort_by("y", "x1", GroupReducer::Median, true))
                .sort_domain_by(sort_by("fy", "x1", GroupReducer::Median, true)),
        )
}

/// `olympians.filter((d) => d.height)`, faceted by 10cm of height.
fn e03() -> Plot {
    let olympians = Dataset::new(
        data::olympians()
            .rows()
            .iter()
            .filter(|d| d.number("height").is_some_and(|h| h != 0.))
            .cloned(),
    );
    Plot::plot()
        .fy_scale(|fy| {
            fy.grid(true)
                .tick_format(".1f")
                .interval(0.1) // 10cm
                .reverse(true)
        })
        .mark(box_x(olympians).x("weight").fy("height"))
}

fn e04() -> Plot {
    Plot::plot()
        .grid(true)
        .margin_right(60.)
        .fx_scale(|fx| fx.no_label())
        .fy_scale(|fy| fy.no_label())
        .mark(frame())
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fx("sex")
                .fy("species"),
        )
}

fn e05() -> Plot {
    Plot::plot()
        .grid(true)
        .margin_right(60.)
        .fx_scale(|fx| fx.no_label())
        .fy_scale(|fy| fy.no_label())
        .mark(frame())
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fill("#aaa")
                .r(1.),
        )
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .fx("sex")
                .fy("species"),
        )
}

/// A facet wrap: each industry's place in a three-column grid.
fn e06() -> Plot {
    let industries = data::bls_industry_unemployment();
    let n = 3;
    let mut keys: Vec<String> = Vec::new();
    for row in industries.rows() {
        let key = row.value("industry").label().to_string();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let index: Rc<HashMap<String, usize>> = Rc::new(
        keys.iter()
            .enumerate()
            .map(|(i, k)| (k.clone(), i))
            .collect(),
    );
    let fx = |field: &'static str, index: Rc<HashMap<String, usize>>| {
        Channel::number(move |d| (index[&*d.value(field).label()] % n) as f64)
    };
    let fy = |field: &'static str, index: Rc<HashMap<String, usize>>| {
        Channel::number(move |d| (index[&*d.value(field).label()] / n) as f64)
    };
    Plot::plot()
        .height(300.)
        .no_axes()
        .y(|mut y| {
            y.inset_end = Some(10.);
            y
        })
        .fx_scale(|fx| fx.padding(0.03))
        .mark(
            area_y(industries)
                .x("date")
                .y("unemployed")
                .z("industry")
                .normalize_y(NormalizeBasis::Extent)
                .fx(fx("industry", index.clone()))
                .fy(fy("industry", index.clone())),
        )
        .mark(
            text(Dataset::values_of(keys.iter().map(|k| k.as_str())))
                .text(Channel::identity())
                .fx(fx(VALUE, index.clone()))
                .fy(fy(VALUE, index))
                .frame_anchor(FrameAnchor::TopLeft)
                .dx(6.)
                .dy(6.),
        )
        .mark(frame())
}

fn e07() -> Plot {
    Plot::plot()
        .margin_left(60.)
        .margin_right(60.)
        .grid(true)
        .y(|y| y.no_label())
        .fy_scale(|fy| fy.no_label())
        .color(|c| c.legend(true))
        .mark(
            bar_x(data::penguins())
                .fy("species")
                .y("island")
                .fill("sex")
                .group_y(),
        )
        .mark(
            text(["While Chinstrap and Gentoo penguins were each observed on only one island, Adelie penguins were observed on all three islands."])
                .text(Channel::identity())
                .fy(Value::text("Adelie"))
                .frame_anchor(FrameAnchor::TopRight)
                .line_width(18.)
                .dx(-6.)
                .dy(6.),
        )
        .mark(frame())
}

/// `facet: {data: anscombe, x: "series"}`
fn e08() -> Plot {
    Plot::new(data::anscombe())
        .fx("series")
        .grid(true)
        .aspect_ratio(0.5)
        .mark(frame())
        .line(|m| m.x("x").y("y"))
        .dot(|m| m.x("x").y("y"))
}
