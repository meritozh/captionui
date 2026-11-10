//! Dumps what the pipeline resolves for every ported documentation example
//! to `$DUMP_DIR/<id>.txt`: the identity harness for pipeline rewrites.
//!
//! A dump holds [`gpui_plot::Plot::debug_dump`] (what `natural_height`
//! prepares at Plot's default width: the facet grid, every row's channels and
//! record in order, the values the scales read) and
//! [`gpui_plot::Plot::debug_dump_frame`] (what `prepaint` resolves at that
//! width and the natural height: the frame's scales and ticks, the color
//! scales and legend, every row after the dodge and hexbin with its position
//! and colors). The frame half runs inside a headless draw, as the snapshots
//! do, so its bounds snap and its labels measure as the painted plot's.
//! Two builds whose dumps match prepared the same thing, which a pixel
//! comparison only approximates.
//!
//! ```sh
//! DUMP_DIR=/tmp/dump cargo run --release -p gpui-plot-demo --example dump
//! DUMP_DIR=/tmp/dump DUMP_FILTER=marks-dot,transforms-bin cargo run --release -p gpui-plot-demo --example dump
//! ```

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::{
    AppContext, Bounds, Context, HeadlessAppContext, IntoElement, ParentElement, Render, Styled,
    Window, canvas, div, px, size,
};
use gpui_plot::Plot;
use gpui_plot_demo::examples;

/// Plot's default width, which the docs' examples are drawn at.
const WIDTH: f32 = 640.;

/// Draws one example's plot as a canvas whose prepaint runs the plot's
/// `prepaint` pipeline and dumps it, at the size the snapshots draw it, so
/// its legend labels measure in the text style the element tree sets.
struct Dumper {
    build: fn() -> Plot,
    height: f32,
    out: Rc<RefCell<String>>,
}

impl Render for Dumper {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let plot = (self.build)();
        let out = self.out.clone();
        div().size_full().child(
            div().w(px(WIDTH)).h(px(self.height)).child(
                canvas(
                    move |bounds, window, cx| {
                        *out.borrow_mut() = plot.debug_dump_frame(bounds, window, cx);
                    },
                    |_, _, _, _| {},
                )
                .size_full(),
            ),
        )
    }
}

/// A headless context with the platform's real text system, so the legend's
/// labels measure as they do in the snapshots and the app.
fn context() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        std::sync::Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    cx
}

fn main() {
    let dir = PathBuf::from(std::env::var("DUMP_DIR").expect("set DUMP_DIR to the output folder"));
    std::fs::create_dir_all(&dir).expect("failed to create the dump directory");
    let filter = std::env::var("DUMP_FILTER").ok();
    let all = examples::examples();
    let selected: Vec<_> = all
        .iter()
        .filter(|example| {
            filter
                .as_ref()
                .is_none_or(|filter| filter.split(',').any(|f| example.id().contains(f)))
        })
        .collect();
    if selected.is_empty() {
        println!("no examples match");
        return;
    }

    let mut cx = context();
    let out = Rc::new(RefCell::new(String::new()));
    let (handle, entity) = cx
        .update(|cx| {
            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(WIDTH), px(2400.)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| Dumper {
                        build: selected[0].build,
                        height: 400.,
                        out: out.clone(),
                    })
                },
            )
        })
        .expect("failed to open the dump window");

    let mut panicked = Vec::new();
    for example in &selected {
        let id = example.id();
        let dumped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let plot = (example.build)();
            let mut dump = plot.debug_dump(WIDTH);
            let height = plot.natural_height(WIDTH);
            out.borrow_mut().clear();
            cx.update_window(handle, |_, window, cx| {
                entity.update(cx, |dumper, _| {
                    dumper.build = example.build;
                    dumper.height = height;
                });
                window.draw(cx).clear(cx);
            })
            .expect("failed to draw the dump frame");
            dump.push_str("--- frame\n");
            dump.push_str(&out.borrow());
            dump
        }));
        let dump = dumped.unwrap_or_else(|_| {
            panicked.push(id.to_string());
            "panicked\n".to_string()
        });
        let path = dir.join(format!("{id}.txt"));
        std::fs::write(&path, dump)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    }
    println!(
        "dumped {} examples to {}{}",
        selected.len(),
        dir.display(),
        if panicked.is_empty() {
            String::new()
        } else {
            format!(", {} panicked: {}", panicked.len(), panicked.join(" "))
        }
    );
}
