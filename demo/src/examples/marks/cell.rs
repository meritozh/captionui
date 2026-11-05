//! Cell mark: `marks/cell.md`.

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            height: 640.,
            ..example("marks", "cell", 1, e01)
        },
        Example {
            height: 1100.,
            ..example("marks", "cell", 2, e02)
        },
        example("marks", "cell", 3, e03),
        Example {
            height: 100.,
            ..example("marks", "cell", 4, e04)
        },
        Example {
            height: 100.,
            ..example("marks", "cell", 5, e05)
        },
        Example {
            height: 100.,
            ..example("marks", "cell", 6, e06)
        },
    ]
}

fn e01() -> Plot {
    let simpsons = data::simpsons();
    Plot::plot()
        .padding(0.)
        .grid(true)
        .x(|x| x.axis(AxisAnchor::Top).label("Season"))
        .y(|y| y.label("Episode"))
        .color(|c| c.type_name("linear").scheme("PiYG"))
        .mark(
            cell(simpsons.clone())
                .x("season")
                .y("number_in_season")
                .fill("imdb_rating")
                .inset(0.5),
        )
        .mark(
            text(simpsons)
                .x("season")
                .y("number_in_season")
                .text(|d: &Table| match d.number("imdb_rating") {
                    Some(rating) => Value::text(format!("{rating:.1}")),
                    None => Value::Null,
                })
                .fill("black")
                .tip_title("title"),
        )
}

fn e02() -> Plot {
    // `fill: (d, i) => (d.Close - dji[i - 1].Close) / dji[i - 1].Close`
    // reads the previous datum, so the change is computed into the data.
    let dji = data::dji();
    let rows = dji.rows();
    let dji: Dataset = rows
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let change = if i > 0 {
                let previous = rows[i - 1].num("Close");
                (d.num("Close") - previous) / previous
            } else {
                f64::NAN
            };
            d.clone()
                .with("change", Value::number(change))
                .with("title", Value::text(format!("{:.1}", change * 100.)))
        })
        .collect();
    // `d3.utcWeek.count(d3.utcYear(d.Date), d.Date)`: Sunday-based weeks
    // since the first of the year.
    let week = |d: &Table| {
        let Some(date) = d.date("Date") else {
            return f64::NAN;
        };
        let date = date.date_naive();
        let jan1 = NaiveDate::from_ymd_opt(date.year(), 1, 1).expect("January 1st");
        let offset = jan1.weekday().num_days_from_sunday() as i64;
        ((date - jan1).num_days() + offset).div_euclid(7) as f64
    };
    Plot::plot()
        .padding(0.)
        .x(|x| x.no_axis())
        .y(|y| {
            // `Plot.formatWeekday("en", "narrow")`
            y.tick_format(|v: &Value| {
                let day = v.as_number().unwrap_or(f64::NAN);
                ["S", "M", "T", "W", "T", "F", "S"]
                    .get(day as usize)
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            })
            .tick_size(0.)
        })
        .fy_scale(|fy| fy.tick_format(""))
        .color(|c| c.scheme("PiYG"))
        .mark(
            cell(dji)
                .x(Channel::number(week))
                .y(Channel::number(|d| {
                    d.date("Date")
                        .map(|date| date.weekday().num_days_from_sunday() as f64)
                        .unwrap_or(f64::NAN)
                }))
                .fy(Channel::number(|d| {
                    d.date("Date")
                        .map(|date| date.year() as f64)
                        .unwrap_or(f64::NAN)
                }))
                .fill("change")
                .tip_title("title")
                .inset(0.5),
        )
}

fn e03() -> Plot {
    // `Plot.group({fill: "max"}, {x, y})` groups on x and y together, which
    // the group transform can't yet: the maxima are computed here.
    let mut max: BTreeMap<(u32, u32), f64> = BTreeMap::new();
    for d in data::seattle().rows() {
        let Some(date) = d.date("date") else {
            continue;
        };
        let entry = max
            .entry((date.day(), date.month0()))
            .or_insert(f64::NEG_INFINITY);
        *entry = entry.max(d.num("temp_max"));
    }
    let grouped: Dataset = max
        .into_iter()
        .map(|((day, month), temp)| {
            Table::empty()
                .with("day", Value::Number(day as f64))
                .with("month", Value::Number(month as f64))
                .with("temp_max", Value::Number(temp))
        })
        .collect();
    Plot::plot()
        .padding(0.)
        // The docs' channels are functions, which carry no label.
        .no_labels()
        .y(|y| {
            // `Plot.formatMonth("en", "short")`
            y.tick_format(|v: &Value| {
                let month = v.as_number().unwrap_or(f64::NAN);
                [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ]
                .get(month as usize)
                .map(|s| s.to_string())
                .unwrap_or_default()
            })
        })
        .mark(
            cell(grouped)
                .x("day")
                .y("month")
                .fill("temp_max")
                .inset(0.5),
        )
}

fn e04() -> Plot {
    let simpsons = data::simpsons();
    // The id of each season's first episode, and its season.
    let firsts: Vec<(f64, f64)> = simpsons
        .rows()
        .iter()
        .filter(|d| d.num("number_in_season") == 1.)
        .map(|d| (d.num("id"), d.num("season")))
        .collect();
    let ticks: Vec<f64> = firsts.iter().map(|(id, _)| *id).collect();
    Plot::plot()
        .x(|x| {
            x.tick_values(ticks)
                .tick_format(move |v: &Value| {
                    let id = v.as_number().unwrap_or(f64::NAN);
                    firsts
                        .iter()
                        .find(|(first, _)| *first == id)
                        .map(|(_, season)| season.to_string())
                        .unwrap_or_default()
                })
                .label("Season")
                .label_anchor(LabelAnchor::End)
                .label_arrow(true)
        })
        .color(|c| c.type_name("linear").scheme("PiYG"))
        .mark(cell(simpsons).x("id").fill("imdb_rating"))
}

/// `Plot.cell(alphabet, {x: "letter", fill: "frequency"}).plot()`
fn e05() -> Plot {
    cell(data::alphabet()).x("letter").fill("frequency").plot()
}

fn e06() -> Plot {
    let hadcrut = data::hadcrut();
    let years: Vec<f64> = hadcrut.rows().iter().map(|d| d.num("year")).collect();
    let (lo, hi) = years
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &y| {
            (lo.min(y), hi.max(y))
        });
    // `d3.ticks(1850, 2023, 10)`: a step of 20 years.
    let ticks: Vec<f64> = (((lo / 20.).ceil() as i64)..=((hi / 20.).floor() as i64))
        .map(|i| i as f64 * 20.)
        .collect();
    Plot::plot()
        .x(|x| {
            x.tick_values(ticks)
                .tick_format("d")
                .interval(1.)
                .no_label()
        })
        .color(|c| c.scheme("BuRd"))
        .mark(cell(hadcrut).x("year").fill("anomaly"))
}
