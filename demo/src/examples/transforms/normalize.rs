//! Normalize transform: `transforms/normalize.md`.

use crate::prelude::*;
use gpui_plot::{GroupConfig, GroupOutput};

pub fn examples() -> Vec<Example> {
    vec![
        example("transforms", "normalize", 1, e01),
        Example {
            height: 680.,
            ..example("transforms", "normalize", 2, e02)
        },
    ]
}

/// The docs' `stocks`: AAPL, AMZN, GOOG and IBM, each row tagged with its
/// `Symbol`.
pub fn stocks() -> Dataset {
    let sets = [
        ("AAPL", data::aapl()),
        ("AMZN", data::amzn()),
        ("GOOG", data::goog()),
        ("IBM", data::ibm()),
    ];
    Dataset::new(sets.into_iter().flat_map(|(symbol, data)| {
        data.rows()
            .iter()
            .map(|d| d.clone().with("Symbol", Value::text(symbol)))
            .collect::<Vec<_>>()
    }))
}

/// `Plot.line(stocks, Plot.normalizeY({x: "Date", y: "Close", stroke: "Symbol"}))`
/// with each series labelled at its end.
///
/// Gap: `selectLast(normalizeY(…))` normalizes before it selects, but the
/// library selects first (its `select` runs on the data), which would leave
/// each label's one row normalized to 1; the label's y is the normalized
/// close, computed up front.
fn e01() -> Plot {
    let stocks = stocks();
    let mut firsts: Vec<(Value, f64)> = Vec::new();
    for d in stocks.rows() {
        let symbol = d.value("Symbol");
        if !firsts.iter().any(|(s, _)| *s == symbol) {
            firsts.push((symbol, d.num("Close")));
        }
    }
    let labelled = Dataset::new(stocks.rows().iter().map(|d| {
        let symbol = d.value("Symbol");
        let first = firsts.iter().find(|(s, _)| *s == symbol).map(|(_, v)| *v);
        let normalized = first
            .map(|first| d.num("Close") / first)
            .unwrap_or(f64::NAN);
        d.clone().with("normalized", Value::number(normalized))
    }));
    Plot::plot()
        .y(|y| {
            y.type_name("log")
                .grid(true)
                .label("Change in price (%)")
                .tick_format(|v: &Value| {
                    let x = v.as_number().unwrap_or(1.);
                    format_number("+d", (x - 1.) * 100.)
                })
        })
        .mark(rule_y([1.]))
        .mark(
            line(stocks)
                .x("Date")
                .y("Close")
                .stroke("Symbol")
                .normalize_y(NormalizeBasis::First),
        )
        .mark(
            text(labelled)
                .x("Date")
                .y("normalized")
                .z("Symbol")
                .text("Symbol")
                .text_anchor(TextAnchor::Start)
                .dx(3.)
                .select(Selector::Last, Some("Symbol")),
        )
}

/// Each state's population by age, as a share of the state's total.
fn e02() -> Plot {
    let (stateage, ages) = data::stateage();
    // The docs' `xy`: `Plot.normalizeX("sum", {z: "state", x: "population", y: "state"})`.
    let xy = |mark: Mark| {
        mark.z("state")
            .x("population")
            .y("state")
            .normalize_x(NormalizeBasis::Sum)
    };
    let mut range = xy(rule_y(stateage.clone()).into_mark());
    range.group = Some(GroupConfig {
        both: false,
        axis: Some(Axis::Y),
        outputs: vec![
            GroupOutput::new("x1", GroupReducer::Min),
            GroupOutput::new("x2", GroupReducer::Max),
        ],
    });
    Plot::plot()
        .height(660.)
        .grid(true)
        .y(|y| y.no_axis())
        .x(|x| {
            x.axis(AxisAnchor::Top)
                .label("Population (%)")
                .percent(true)
        })
        .color(|c| {
            c.scheme("spectral")
                .domain(ages.iter().map(|age| Value::text(age.clone())))
                .legend(true)
        })
        .mark(rule_x([0.]))
        .mark(range.sort_domain("y", "x1", false))
        .mark(
            xy(dot(stateage.clone()).into_mark())
                .fill("age")
                .tip_title("age"),
        )
        .mark(
            xy(text(stateage).into_mark())
                .text_anchor(TextAnchor::End)
                .dx(-6.)
                .text("state")
                .select(Selector::Min("x".into()), Some("state")),
        )
}
