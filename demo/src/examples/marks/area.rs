//! Area mark: `marks/area.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "area", 1, e01),
        example("marks", "area", 2, e02),
        example("marks", "area", 3, e03),
        example("marks", "area", 4, e04),
        example("marks", "area", 5, e05),
        example("marks", "area", 6, e06),
        example("marks", "area", 7, e07),
        example("marks", "area", 8, e08),
        example("marks", "area", 9, e09),
        example("marks", "area", 10, e10),
        example("marks", "area", 11, e11),
        example("marks", "area", 12, e12),
        example("marks", "area", 13, e13),
        example("marks", "area", 14, e14),
        Example {
            height: 740.,
            ..example("marks", "area", 15, e15)
        },
        Example {
            height: 740.,
            ..example("marks", "area", 16, e16)
        },
    ]
}

/// The month of a datum's `Date`, from zero: `d.Date.getUTCMonth()`.
fn month(d: &Table) -> u32 {
    d.date("Date").map(|date| date.month0()).unwrap_or(0)
}

/// `(d) => d.Date.getUTCMonth() < 3 ? NaN : d.Close`
fn close_after_march(d: &Table) -> f64 {
    if month(d) < 3 {
        f64::NAN
    } else {
        d.num("Close")
    }
}

/// `d3.shuffle(aapl.slice())`, seeded so the snapshot is stable.
fn shuffled_aapl() -> Dataset {
    let mut rows: Vec<Table> = data::aapl().into_iter().collect();
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in (1..rows.len()).rev() {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let j = (state.wrapping_mul(0x2545_F491_4F6C_DD1D) % (i as u64 + 1)) as usize;
        rows.swap(i, j);
    }
    Dataset::new(rows)
}

/// `unemployed` in thousands, standing in for the docs'
/// `y: {transform: (d) => d / 1000}`.
fn thousands(d: &Table) -> f64 {
    d.num("unemployed") / 1000.
}

/// `Plot.areaY(aapl, {x: "Date", y: "Close"}).plot()`
fn e01() -> Plot {
    area_y(data::aapl()).x("Date").y("Close").plot()
}

fn e02() -> Plot {
    // `line: true` strokes the topline: drawn here as a lineY over the area.
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(area_y(data::aapl()).x("Date").y("Close").fill_opacity(0.3))
        .mark(line_y(data::aapl()).x("Date").y("Close"))
        .mark(rule_y([0.]))
}

/// `Plot.areaY(d3.cumsum({length: 600}, d3.randomNormal())).plot()`
fn e03() -> Plot {
    let mut normal = data::random_normal(42);
    let mut sum = 0.;
    // Plot's areaY defaults x to the index and y to the value; the port
    // spells both out over [index, value] tuples.
    let walk = Dataset::tuples((0..600).map(|i| {
        sum += normal();
        [i as f64, sum]
    }));
    area_y(walk).x("0").y("1").plot().no_labels()
}

/// `Plot.areaY(d3.shuffle(aapl.slice()), {x: "Date", y: "Close"}).plot()`
fn e04() -> Plot {
    area_y(shuffled_aapl()).x("Date").y("Close").plot()
}

/// `Plot.areaY(d3.shuffle(aapl.slice()), {x: "Date", y: "Close", sort: "Date"}).plot()`
fn e05() -> Plot {
    area_y(shuffled_aapl())
        .x("Date")
        .y("Close")
        .sort("Date", false)
        .plot()
}

fn e06() -> Plot {
    Plot::plot()
        .y(|y| y.label("Temperature (°F)").grid(true))
        .mark(area_y(data::sftemp()).x("date").y1("low").y2("high"))
}

fn e07() -> Plot {
    Plot::plot()
        .x(|x| x.no_label())
        .y(|y| y.grid(true).reverse(true))
        .mark(area_y(data::aapl()).x("Date").y("Close").fill_opacity(0.3))
        .mark(line_y(data::aapl()).x("Date").y("Close"))
        .mark(rule_y([0.]))
}

fn e08() -> Plot {
    Plot::plot()
        .x(|x| x.grid(true))
        .mark(area_x(data::aapl()).y("Date").x("Close").fill_opacity(0.3))
        .mark(line_x(data::aapl()).y("Date").x("Close"))
        .mark(rule_x([0.]))
}

fn e09() -> Plot {
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(
            area_y(data::aapl())
                .x("Date")
                .y(Channel::number(close_after_march))
                .fill_opacity(0.3),
        )
        .mark(
            line_y(data::aapl())
                .x("Date")
                .y(Channel::number(close_after_march)),
        )
        .mark(rule_y([0.]))
}

fn e10() -> Plot {
    // `filter: (d) => d.Date.getUTCMonth() >= 3`, applied to the data.
    let filtered: Dataset = data::aapl().into_iter().filter(|d| month(d) >= 3).collect();
    Plot::plot()
        .y(|y| y.grid(true))
        .mark(area_y(filtered).x("Date").y("Close").fill_opacity(0.3))
        .mark(
            line_y(data::aapl())
                .x("Date")
                .y(Channel::number(close_after_march)),
        )
        .mark(rule_y([0.]))
}

fn e11() -> Plot {
    Plot::plot()
        .y(|y| y.label("Unemployed (thousands)"))
        .mark(
            area_y(data::bls_industry_unemployment())
                .x("date")
                .y(Channel::number(thousands))
                .fill("industry"),
        )
        .mark(rule_y([0.]))
}

fn e12() -> Plot {
    Plot::plot().y(|y| y.label("Unemployed (thousands)")).mark(
        area_y(data::bls_industry_unemployment())
            .x("date")
            .y(Channel::number(thousands))
            .fill("industry")
            .stack_offset(Offset::Wiggle),
    )
}

fn e13() -> Plot {
    Plot::plot()
        .mark(
            area_y(data::bls_industry_unemployment())
                .x("date")
                .y2("unemployed")
                .z("industry")
                .fill_opacity(0.1),
        )
        .mark(
            line_y(data::bls_industry_unemployment())
                .x("date")
                .y("unemployed")
                .z("industry")
                .stroke_width(1.),
        )
}

fn e14() -> Plot {
    Plot::plot()
        .color(|c| c.type_name("log").legend(true))
        .mark(
            area_y(data::aapl())
                .x("Date")
                .y("Close")
                .fill("Volume")
                .z(Value::Null),
        )
        .mark(rule_y([0.]))
}

fn e15() -> Plot {
    let industries = data::bls_industry_unemployment();
    Plot::plot()
        .height(720.)
        .no_axes()
        .mark(
            area_y(industries.clone())
                .x("date")
                .y("unemployed")
                .fy("industry"),
        )
        .mark(
            text(industries)
                .text("industry")
                .fy("industry")
                .select(Selector::First, Some("industry"))
                .frame_anchor(FrameAnchor::TopLeft)
                .dx(6.)
                .dy(6.),
        )
        .mark(frame())
}

fn e16() -> Plot {
    let industries = data::bls_industry_unemployment();
    let bands = 7;
    let max = industries
        .rows()
        .iter()
        .map(|d| d.num("unemployed"))
        .fold(f64::NEG_INFINITY, f64::max);
    let step = max / bands as f64;
    // `facet: {data: industries, y: "industry"}` facets every mark drawn
    // from the facet data; the port names the facet on each mark instead.
    let mut plot = Plot::plot()
        .height(720.)
        .no_axes()
        .y(|y| y.domain([0., step]))
        .color(|c| c.scheme("YlGnBu"));
    for i in 0..bands {
        let offset = i as f64 * step;
        plot = plot.mark(
            area_y(industries.clone())
                .x("date")
                .y(Channel::number(move |d| d.num("unemployed") - offset))
                .fy("industry")
                .fill(ColorChannel::value(move |_| Value::Number(i as f64)))
                .clip(true),
        );
    }
    plot.mark(
        text(industries)
            .text("industry")
            .fy("industry")
            .select(Selector::First, Some("industry"))
            .frame_anchor(FrameAnchor::TopLeft)
            .dx(6.)
            .dy(6.),
    )
    .mark(frame())
}
