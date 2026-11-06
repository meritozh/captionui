//! Time the docs window's frames while the pointer moves over a page.
//!
//! Compares the docs view (a cached view per chart, each a `RetainedPlot`)
//! with the way it used to draw a page, every plot rebuilt and re-resolved
//! on every frame:
//!
//! ```sh
//! BENCH_PAGE=transforms/group cargo run --release -p gpui-plot-demo --example frame_bench
//! ```

use std::time::Instant;

use gpui_kit::{
    IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext,
    div, point, px,
};
use gpui_plot_demo::docs::Docs;
use gpui_plot_demo::examples::{self, Example};

/// The page drawn the old way: every chart built (twice) and resolved on
/// every render.
struct Legacy {
    examples: Vec<Example>,
}

impl Render for Legacy {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let mut column = div().flex().flex_col().gap_6().p_6().size_full();
        for example in &self.examples {
            column = column.child(
                div()
                    .w(px(640.))
                    .h(px((example.build)().natural_height(640.)))
                    .child((example.build)().id(example.id())),
            );
        }
        column
    }
}

/// Move the pointer across the window `moves` times; the mean and worst
/// frame, in milliseconds.
fn sweep(window: &mut VisualTestContext, moves: usize) -> (f64, f64) {
    let mut total = 0.;
    let mut worst: f64 = 0.;
    for i in 0..moves {
        let x = 60. + (i * 37 % 560) as f32;
        let y = 120. + (i * 53 % 700) as f32;
        let start = Instant::now();
        window.simulate_mouse_move(
            point(px(x), px(y)),
            Option::<gpui_kit::MouseButton>::None,
            Default::default(),
        );
        window.run_until_parked();
        let ms = start.elapsed().as_secs_f64() * 1e3;
        total += ms;
        worst = worst.max(ms);
    }
    (total / moves as f64, worst)
}

fn main() {
    let page = std::env::var("BENCH_PAGE").unwrap_or_else(|_| "transforms/group".into());
    let (section, name) = page.split_once('/').expect("BENCH_PAGE is section/page");
    let moves = 40;

    let dispatcher = gpui_kit::TestDispatcher::new(0);
    let mut cx = TestAppContext::build(dispatcher, Some("frame_bench"));
    cx.update(gpui_kit::init);

    // The old way.
    let page_examples: Vec<Example> = examples::examples()
        .into_iter()
        .filter(|e| e.section == section && e.page == name)
        .collect();
    let n = page_examples.len();
    let start = Instant::now();
    let (_, window) = cx.add_window_view(|_, _| Legacy {
        examples: page_examples,
    });
    window.run_until_parked();
    let first = start.elapsed().as_secs_f64() * 1e3;
    let (mean, worst) = sweep(window, moves);
    println!("{page} ({n} charts)");
    println!("  rebuilt every frame: first frame {first:7.1} ms, pointer move mean {mean:7.1} ms, worst {worst:7.1} ms");

    // The docs view: one cached view per chart, each resolved once.
    let start = Instant::now();
    let (docs, window) = cx.add_window_view(|_, _| {
        let mut docs = Docs::new();
        let index = docs.page_index(section, name).expect("no such page");
        docs.select(index);
        docs
    });
    window.run_until_parked();
    let first = start.elapsed().as_secs_f64() * 1e3;
    // Build every chart, as the frames after the first would.
    let start = Instant::now();
    while !docs.read_with(window, |docs, _| docs.is_built()) {
        window.update(|window, cx| docs.update(cx, |docs, cx| docs.build_charts(window, cx)));
        window.run_until_parked();
    }
    let built = start.elapsed().as_secs_f64() * 1e3;
    let (mean, worst) = sweep(window, moves);
    println!("  retained + cached:   first frame {first:7.1} ms, charts built in {built:7.1} ms, pointer move mean {mean:7.1} ms, worst {worst:7.1} ms");
}
