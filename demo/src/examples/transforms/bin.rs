//! Bin transform: `transforms/bin.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "bin", 1, e01),
        example("transforms", "bin", 2, e02),
        example("transforms", "bin", 3, e03),
        example("transforms", "bin", 4, e04),
        example("transforms", "bin", 5, e05),
        example("transforms", "bin", 6, e06),
        example("transforms", "bin", 7, e07),
        example("transforms", "bin", 8, e08),
        example("transforms", "bin", 9, e09),
        example("transforms", "bin", 10, e10),
        example("transforms", "bin", 11, e11),
        example("transforms", "bin", 12, e12),
        example("transforms", "bin", 13, e13),
    ]
}

/// `Plot.rectY(olympians, Plot.binX({y: "count"}, {x: "weight"}))`
fn e01() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `Plot.rect(olympians, Plot.binX({fill: "count"}, {x: "weight"}))`
fn e02() -> Plot {
    rect(data::olympians())
        .x("weight")
        .bin_output("fill", GroupReducer::Count)
        .plot()
        .color(|c| c.scheme("YlGnBu"))
}

/// `Plot.rectY(olympians, Plot.binX({y: "count"}, {x: "weight", fill: "sex"}))`
fn e03() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .color(|c| c.legend(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .fill("sex")
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `Plot.binX({y2: "count"}, {x: "weight", fill: "sex", mixBlendMode: "multiply"})`
fn e04() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .fill("sex")
                .mix_blend_mode(BlendMode::Multiply)
                .bin_output("y2", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// `Plot.rectX(olympians, Plot.binY({x: "count"}, {y: "weight"}))`
fn e05() -> Plot {
    Plot::plot()
        .x(|x| x.grid(true))
        .mark(
            rect_x(data::olympians())
                .y("weight")
                .bin_output("x", GroupReducer::Count),
        )
        .mark(rule_x([0.]))
}

/// `Plot.rect(olympians, Plot.bin({fill: "count"}, {x: "weight", y: "height"}))`
fn e06() -> Plot {
    rect(data::olympians())
        .x("weight")
        .y("height")
        .bin_output("fill", GroupReducer::Count)
        .plot()
        .color(|c| c.scheme("YlGnBu"))
}

/// `Plot.dot(olympians, Plot.bin({r: "count"}, {x: "weight", y: "height"}))`
fn e07() -> Plot {
    Plot::plot().r(|r| r.range([0., 6.])).mark(
        dot(data::olympians())
            .x("weight")
            .y("height")
            .bin_output("r", GroupReducer::Count),
    )
}

/// `Plot.dot(olympians, Plot.bin({r: "count"}, {x: "weight", y: "height", stroke: "sex"}))`
fn e08() -> Plot {
    Plot::plot().r(|r| r.range([0., 6.])).mark(
        dot(data::olympians())
            .x("weight")
            .y("height")
            .stroke("sex")
            .bin_output("r", GroupReducer::Count),
    )
}

/// `Plot.dot(olympians, Plot.binX({r: "count"}, {x: "weight"}))`
fn e09() -> Plot {
    Plot::plot().r(|r| r.range([0., 14.])).mark(
        dot(data::olympians())
            .x("weight")
            .bin_output("r", GroupReducer::Count),
    )
}

/// `Plot.areaY(…, Plot.binX({y: "count", filter: null}, …))` and a `lineY`.
fn e10() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(
            area_y(data::olympians())
                .x("weight")
                .fill_opacity(0.2)
                .bin_output("y", GroupReducer::Count)
                .bin_keep_empty(),
        )
        .mark(
            line_y(data::olympians())
                .x("weight")
                .bin_output("y", GroupReducer::Count)
                .bin_keep_empty(),
        )
        .mark(rule_y([0.]))
}

/// `Plot.rectY(olympians, Plot.binX({y: "count"}, {x: "weight", cumulative}))`,
/// with the docs' `cumulative` toggle on.
fn e11() -> Plot {
    Plot::plot()
        .margin_left(60.)
        .y(|y| y.grid(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .bin_cumulative(1)
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}

/// The sports ordered by their athletes' median weight, the docs'
/// `d3.groupSort(…, (g) => d3.median(g, (d) => d.weight), (d) => d.sport)`.
fn sports_by_median_weight() -> Vec<Value> {
    let mut groups: Vec<(Value, Vec<f64>)> = Vec::new();
    for row in data::olympians().rows() {
        let Some(weight) = row.number("weight") else {
            continue;
        };
        let sport = row.value("sport");
        match groups.iter_mut().find(|(key, _)| *key == sport) {
            Some((_, weights)) => weights.push(weight),
            None => groups.push((sport, vec![weight])),
        }
    }
    let median = |values: &mut Vec<f64>| {
        values.sort_by(f64::total_cmp);
        let n = values.len();
        if n % 2 == 1 {
            values[n / 2]
        } else {
            (values[n / 2 - 1] + values[n / 2]) / 2.
        }
    };
    let mut keyed: Vec<(f64, Value)> = groups
        .into_iter()
        .map(|(sport, mut weights)| (median(&mut weights), sport))
        .collect();
    // d3.groupSort breaks ties by the key, ascending.
    keyed.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1.as_text().cmp(&b.1.as_text()))
    });
    keyed.into_iter().map(|(_, sport)| sport).collect()
}

/// `Plot.rect(olympians, Plot.binX({fill: "proportion-facet"}, {x: "weight", fy: "sport", inset: 0.5}))`
fn e12() -> Plot {
    Plot::plot()
        .margin_left(100.)
        .padding(0.)
        .x(|x| x.grid(true))
        .fy_scale(|fy| fy.domain(sports_by_median_weight()))
        .color(|c| c.scheme("YlGnBu"))
        .mark(
            rect(data::olympians())
                .x("weight")
                .fy("sport")
                .inset(0.5)
                .bin_output("fill", GroupReducer::parse("proportion-facet").unwrap()),
        )
}

/// `Plot.rectY(olympians, Plot.binX({y: "count"}, {x: "weight", inset: 0}))`
fn e13() -> Plot {
    Plot::plot()
        .x(|x| x.round(true))
        .y(|y| y.grid(true))
        .mark(
            rect_y(data::olympians())
                .x("weight")
                .inset(0.)
                .bin_output("y", GroupReducer::Count),
        )
        .mark(rule_y([0.]))
}
