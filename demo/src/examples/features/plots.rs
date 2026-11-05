//! Plots: `features/plots.md`.

use crate::prelude::*;

pub fn examples() -> Vec<Example> {
    let builds: [fn() -> Plot; 8] = [e01, e02, e03, e04, e05, e06, e07, e08];
    builds
        .into_iter()
        .enumerate()
        .map(|(i, build)| example("features", "plots", i + 1, build))
        .collect()
}

/// The page's `stocks`: AAPL's and GOOG's rows, each tagged with its
/// `Symbol`.
fn stocks() -> Dataset {
    let tag = |data: Dataset, symbol: &'static str| {
        data.into_iter()
            .map(move |row| row.with("Symbol", Value::text(symbol)))
    };
    Dataset::new(tag(data::aapl(), "AAPL").chain(tag(data::goog(), "GOOG")))
}

fn e01() -> Plot {
    Plot::plot().mark(frame()).mark(
        text(["Hello, world!"])
            .text(Channel::identity())
            .frame_anchor(FrameAnchor::Middle),
    )
}

fn e02() -> Plot {
    Plot::plot().mark(line_y(data::aapl()).x("Date").y("Close"))
}

fn e03() -> Plot {
    Plot::plot()
        .mark(rule_y([0.]))
        .mark(line_y(data::goog()).x("Date").y("Close").stroke("red"))
        .mark(line_y(data::aapl()).x("Date").y("Close").stroke("blue"))
}

fn e04() -> Plot {
    Plot::plot()
        .color(|c| c.legend(true))
        .mark(rule_y([0.]))
        .mark(line_y(stocks()).x("Date").y("Close").stroke("Symbol"))
}

fn e05() -> Plot {
    Plot::plot()
        .x(|x| x.padding(0.4))
        .mark(
            bar_y(data::alphabet())
                .x("letter")
                .y("frequency")
                .dx(2.)
                .dy(2.),
        )
        .mark(
            bar_y(data::alphabet())
                .x("letter")
                .y("frequency")
                .fill("green")
                .dx(-2.)
                .dy(-2.),
        )
}

/// The page's margin sliders start at 20, 20, 30 and 40. The outer frame's
/// insets are per side in the docs (`insetTop: -marginTop`, …); the frame
/// here takes one inset, so it uses the top margin's. Its stroke is the
/// docs' `var(--vp-c-text-2)`, a gray.
fn e06() -> Plot {
    let (margin_top, margin_right, margin_bottom, margin_left) = (20., 20., 30., 40.);
    Plot::plot()
        .margin_top(margin_top)
        .margin_right(margin_right)
        .margin_bottom(margin_bottom)
        .margin_left(margin_left)
        .grid(true)
        .mark(
            frame()
                .stroke("gray")
                .stroke_opacity(0.5)
                .inset(-margin_top),
        )
        .mark(
            bar_y(data::alphabet())
                .x("letter")
                .y("frequency")
                .fill("green"),
        )
        .mark(frame())
}

/// The page's `fixed` toggle starts on: `aspectRatio: 1`.
fn e07() -> Plot {
    let fixed = true;
    let plot = Plot::plot()
        .grid(true)
        .inset(10.)
        .color(|c| c.legend(true))
        .mark(frame())
        .mark(
            dot(data::penguins())
                .x("culmen_length_mm")
                .y("culmen_depth_mm")
                .stroke("species"),
        );
    if fixed { plot.aspect_ratio(1.) } else { plot }
}

fn e08() -> Plot {
    Plot::plot()
        .title("For charts, an informative title")
        .subtitle("Subtitle to follow with additional context")
        .caption("Figure 1. A chart with a title, subtitle, and caption.")
        .mark(frame())
        .mark(
            text(["Titles, subtitles, captions, and annotations assist inter\u{ad}pretation by telling the reader what’s interesting. Don’t make the reader work to find what you already know."])
                .text(Channel::identity())
                .line_width(30.)
                .frame_anchor(FrameAnchor::Middle),
        )
}
