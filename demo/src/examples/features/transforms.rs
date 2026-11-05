//! Transforms: `features/transforms.md`.

use crate::prelude::*;
use gpui_plot::transform::BinEdges;

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            height: 820.,
            ..example("features", "transforms", 1, e01)
        },
        Example {
            height: 860.,
            ..example("features", "transforms", 2, e02)
        },
        example("features", "transforms", 3, e03),
        example("features", "transforms", 4, e04),
        example("features", "transforms", 5, e05),
        example("features", "transforms", 6, e06),
    ]
}

/// The docs' `x: {transform: (x) => x / 1000}` has no counterpart (a scale's
/// transform is one of linear, log, pow, sqrt or symlog), so the marks'
/// `x` channels divide instead.
fn e01() -> Plot {
    let thousands = || Channel::number(|d| d.num("vehicles") / 1000.);
    Plot::plot()
        .margin_left(120.)
        .x(|x| x.label("Vehicles per hour (thousands)"))
        .y(|y| y.no_label())
        .mark(rule_x([0.]))
        .mark(
            tick_x(data::traffic())
                .x(thousands())
                .y("location")
                .stroke_opacity(0.3),
        )
        .mark(
            tick_x(data::traffic())
                .x(thousands())
                .y("location")
                .stroke("red")
                .stroke_width(4.)
                .group_y()
                .group_output("x", GroupReducer::Median)
                .sort_domain("y", "x", false),
        )
}

/// `Plot.group({fill: "median"}, {x: hour, y: "location", …})` groups on
/// both x and y; a group here reads one axis, so this groups by x.
fn e02() -> Plot {
    Plot::plot()
        .margin_left(120.)
        .padding(0.)
        .y(|y| y.no_label())
        .color(|c| c.scheme("YlGnBu").legend(true).zero(true))
        .mark(
            cell(data::traffic())
                .x(|d: &Table| {
                    d.date("date")
                        .map(|date| Value::Number(chrono::Timelike::hour(&date) as f64))
                        .unwrap_or(Value::Null)
                })
                .y("location")
                .fill("vehicles")
                .inset(0.5)
                .group_xy()
                .group_output("fill", GroupReducer::Median)
                .sort_domain("y", "fill", false),
        )
}

/// `d3.bin().thresholds(80).value((d) => d.weight)(olympians)`: each bin's
/// bounds `x0`, `x1` and its `length`.
fn bins() -> Dataset {
    let olympians = data::olympians();
    let weights: Vec<f64> = olympians
        .rows()
        .iter()
        .filter_map(|d| d.number("weight"))
        .collect();
    let Some(edges) = BinEdges::of(&weights, Thresholds::Count(80)) else {
        return Dataset::new([]);
    };
    let mut lengths = vec![0usize; edges.count];
    for &weight in &weights {
        lengths[edges.slot(weight)] += 1;
    }
    Dataset::new(lengths.into_iter().enumerate().map(|(i, length)| {
        let (x0, x1) = edges.bounds(i);
        Table::empty()
            .with("x0", Value::Number(x0))
            .with("x1", Value::Number(x1))
            .with("length", Value::Number(length as f64))
    }))
}

/// `Plot.rectY(bins, {x1: "x0", x2: "x1", y2: "length"}).plot()`
fn e03() -> Plot {
    rect_y(bins()).x1("x0").x2("x1").y2("length").plot()
}

fn e04() -> Plot {
    rect_y(data::olympians())
        .x("weight")
        .fill("sex")
        .bin(Thresholds::Auto)
        .bin_output("y", GroupReducer::Count)
        .plot()
        .color(|c| c.legend(true))
}

/// The docs blend the overlapping series with `mixBlendMode: "multiply"`;
/// there is no blend mode here.
fn e05() -> Plot {
    Plot::plot().y(|y| y.percent(true)).mark(
        rect_y(data::olympians())
            .x("weight")
            .fill("sex")
            .bin(Thresholds::Auto)
            .bin_output("y2", GroupReducer::Count)
            .normalize_y(NormalizeBasis::Sum),
    )
}

/// The docs' custom `transform: (data, facets) => …` keeps the Michigan
/// divisions; a mark takes no custom transform here, so the data is
/// filtered up front.
fn e06() -> Plot {
    let bls = Dataset::new(
        data::bls_metro_unemployment()
            .rows()
            .iter()
            .filter(|d| d.text("division").is_some_and(|d| d.contains(", MI ")))
            .cloned(),
    );
    Plot::plot()
        .y(|y| y.grid(true).label("Unemployment (%)"))
        .color(|c| c.domain([false, true]).colors(["#ccc", "red"]))
        .mark(rule_y([0.]))
        .mark(line(bls).x("date").y("unemployment").z("division"))
}
