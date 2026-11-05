//! Text mark: `marks/text.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    vec![
        example("marks", "text", 1, e01),
        example("marks", "text", 2, e02),
        example("marks", "text", 3, e03),
        example("marks", "text", 4, e04),
        Example {
            height: 620.,
            ..example("marks", "text", 5, e05)
        },
        Example {
            height: 200.,
            ..example("marks", "text", 6, e06)
        },
        Example {
            height: 320.,
            ..example("marks", "text", 7, e07)
        },
    ]
}

fn e01() -> Plot {
    Plot::plot()
        .no_labels()
        .y(|y| y.grid(true).label("Frequency (%)").percent(true))
        .mark(bar_y(data::alphabet()).x("letter").y("frequency"))
        .mark(
            text(data::alphabet())
                .x("letter")
                .y("frequency")
                .text(|d: &Table| Value::text(format!("{:.1}", d.num("frequency") * 100.)))
                .dy(-6.)
                .line_anchor(LineAnchor::Bottom),
        )
        .mark(rule_y([0.]))
}

fn e02() -> Plot {
    let every_five: Dataset = data::driving()
        .rows()
        .iter()
        .filter(|d| d.num("year") as i64 % 5 == 0)
        .cloned()
        .collect();
    Plot::plot()
        .inset(10.)
        .grid(true)
        .x(|x| x.label("Miles driven (per person-year)"))
        .y(|y| y.label("Cost of gasoline ($ per gallon)"))
        .mark(
            line(data::driving())
                .x("miles")
                .y("gas")
                .curve(Curve::CatmullRom)
                .marker("true"),
        )
        .mark(
            text(every_five)
                .x("miles")
                .y("gas")
                .text(|d: &Table| Value::text(format!("{}", d.num("year"))))
                .dy(-6.)
                .line_anchor(LineAnchor::Bottom),
        )
}

/// The y scale's `transform: (d) => d / 1e6` is applied in the channels.
fn e03() -> Plot {
    let millions = |field: &'static str| Channel::number(move |d| d.num(field) / 1e6);
    Plot::plot()
        .y(|y| y.grid(true).label("Travelers per day (millions)"))
        .mark(rule_y([0.]))
        .mark(
            line(data::travelers())
                .x("date")
                .y(millions("previous"))
                .stroke_opacity(0.5),
        )
        .mark(line(data::travelers()).x("date").y(millions("current")))
        .mark(
            text(data::travelers())
                .x("date")
                .y(millions("previous"))
                .text(Value::text("2019"))
                .fill_opacity(0.5)
                .line_anchor(LineAnchor::Bottom)
                .dy(-6.)
                .select(Selector::First, None::<&str>),
        )
        .mark(
            text(data::travelers())
                .x("date")
                .y(millions("current"))
                .text(Value::text("2020"))
                .line_anchor(LineAnchor::Top)
                .dy(6.)
                .select(Selector::First, None::<&str>),
        )
}

/// `width: 240` has no counterpart: the timetable spans the full width.
fn e04() -> Plot {
    Plot::plot()
        .no_axes()
        .x(|x| x.type_name("point"))
        .y(|y| y.type_name("point").domain((4..25).map(|h| h as f64)))
        .color(|c| {
            c.domain(["N", "L", "B"])
                .colors(["currentColor", "peru", "brown"])
                .legend(true)
        })
        .mark(
            text(Dataset::tuples([[0.5, 4.]]))
                .text(Value::text("Northbound"))
                .text_anchor(TextAnchor::Start)
                .dx(16.),
        )
        .mark(
            text(Dataset::tuples([[-0.5, 4.]]))
                .text(Value::text("Southbound"))
                .text_anchor(TextAnchor::End)
                .dx(-16.),
        )
        .mark(
            text(Dataset::values_of((5..25).map(|h| h as f64)))
                .x(Value::Number(0.))
                .y(Channel::identity())
                .text(|d: &Table| {
                    let y = d.num(gpui_plot::table::VALUE) as i64;
                    let hour = if y % 12 == 0 { 12 } else { y % 12 };
                    Value::text(format!("{hour}{}", if y % 24 >= 12 { "p" } else { "a" }))
                }),
        )
        .mark(
            text(data::caltrain())
                .x(Channel::number(|d| {
                    if d.text("orientation").is_some_and(|o| o.as_ref() == "N") {
                        1.
                    } else {
                        -1.
                    }
                }))
                .y("hours")
                .fill("type")
                .text("minutes")
                .stack_x2(),
        )
        .mark(rule_x([-0.5, 0.5]))
}

fn e05() -> Plot {
    Plot::plot()
        .aspect_ratio(1.)
        .inset(10.)
        .grid(true)
        .mark(text(Dataset::tuples((0..151).map(|i| {
            let i = i as f64;
            [i.sqrt() * (i / 10.).sin(), i.sqrt() * (i / 10.).cos()]
        }))))
}

fn e06() -> Plot {
    Plot::plot().height(200.).mark(frame()).mark(
        text(Dataset::values_of([r#"This Is Just To Say
William Carlos Williams, 1934

I have eaten
the plums
that were in
the icebox

and which
you were probably
saving
for breakfast

Forgive me
they were delicious
so sweet
and so cold"#]))
        .text(Channel::identity())
        .frame_anchor(FrameAnchor::Middle),
    )
}

const MOBY_DICK: [&str; 3] = [
    "Call me Ishmael. Some years ago — never mind how long precisely — having little or no money in my purse, and nothing particular to interest me on shore, I thought I would sail about a little and see the watery part of the world. It is a way I have of driving off the spleen and regulating the circulation. Whenever I find myself growing grim about the mouth; whenever it is a damp, drizzly November in my soul; whenever I find myself involuntarily pausing before cof\u{ad}fin warehouses, and bringing up the rear of every funeral I meet; and especially whenever my hypos get such an upper hand of me, that it requires a strong moral principle to prevent me from deliberately stepping into the street, and methodically knocking people’s hats off — then, I account it high time to get to sea as soon as I can. This is my substitute for pistol and ball. With a philosophical flourish Cato throws himself upon his sword; I quietly take to the ship. There is nothing surprising in this. If they but knew it, almost all men in their degree, some time or other, cherish very nearly the same feelings towards the ocean with me.",
    "There now is your insular city of the Manhattoes, belted round by wharves as Indian isles by coral reefs — commerce surrounds it with her surf. Right and left, the streets take you waterward. Its extreme downtown is the battery, where that noble mole is washed by waves, and cooled by breezes, which a few hours previous were out of sight of land. Look at the crowds of water-gazers there.",
    "Circumambulate the city of a dreamy Sabbath afternoon. Go from Corlears Hook to Coenties Slip, and from thence, by Whitehall, northward. What do you see? — Posted like silent sentinels all around the town, stand thousands upon thousands of mortal men fixed in ocean reveries. Some leaning against the spiles; some seated upon the pier-heads; some looking over the bulwarks of ships from China; some high aloft in the rigging, as if striving to get a still better seaward peep. But these are all landsmen; of week days pent up in lath and plaster — tied to counters, nailed to benches, clinched to desks. How then is this? Are the green fields gone? What do they here?",
];

fn e07() -> Plot {
    // Each paragraph with its number, `(d, i) => 1 + i`.
    let paragraphs = Dataset::new(MOBY_DICK.iter().enumerate().map(|(i, p)| {
        Table::empty()
            .with(gpui_plot::table::VALUE, *p)
            .with("i", i as f64)
    }));
    Plot::plot()
        .height(320.)
        .x(|x| {
            x.type_name("point")
                .align(0.)
                .axis(AxisAnchor::Top)
                .tick_size(0.)
        })
        .mark(
            text(paragraphs)
                .x(Channel::number(|d| 1. + d.num("i"))) // paragraph number
                .text(Channel::identity())
                .line_width(20.)
                .frame_anchor(FrameAnchor::Top)
                .text_anchor(TextAnchor::Start),
        )
}
