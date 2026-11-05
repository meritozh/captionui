//! Capture a hover screenshot with GPUI's real offscreen renderer, so the
//! crosshair and tooltip positions can be inspected visually.
//!
//! Run with `cargo run -p gpui-plot-demo --example hover_capture`; writes
//! `demo/snapshots/hover-capture.png`.

use gpui_kit::base::StyledExt;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AnyWindowHandle, AppContext, Context, IntoElement, ParentElement, Render, Styled,
    VisualTestAppContext, Window, div, px, size,
};
use gpui_plot_demo::examples;

struct Host {
    build: fn() -> gpui_plot::Plot,
}

impl Render for Host {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Replicate the demo gallery's use: a custom tooltip formatter (a table
        // of every field) and an explicit plot id, both of which previously
        // bypassed the crosshair and left the box unanchored.
        let plot = (self.build)()
            .id("gallery-plot")
            .tooltip(|row, _window, _cx| {
                let mut table = div().v_flex().gap_1().text_xs();
                for name in row.field_names() {
                    let value = row.value(name);
                    table = table.child(
                        div()
                            .h_flex()
                            .justify_between()
                            .gap_4()
                            .child(div().text_color(gpui_kit::black()).child(name.clone()))
                            .child(div().child(value.label())),
                    );
                }
                table.into_any_element()
            });
        div()
            .size_full()
            .bg(cx.theme().colors.background)
            .child(plot)
    }
}

fn main() {
    // Use the real Mac renderer so the screenshot is accurate.
    let platform = gpui_kit::platform::current_platform(true);
    let mut cx = VisualTestAppContext::new(platform);
    cx.update(gpui_kit::init);

    // A stacked bar chart: the pointer should pick a bar anywhere inside its
    // body, not just at its top where the anchor point sits. `HOVER_EXAMPLE`
    // picks another example by id.
    let id = std::env::var("HOVER_EXAMPLE").unwrap_or_else(|_| "transforms-stack-02".into());
    let example = examples::examples()
        .into_iter()
        .find(|e| e.id() == id)
        .unwrap_or_else(|| panic!("no example {id}"));
    let build = example.build;

    let window: AnyWindowHandle = cx
        .open_offscreen_window(size(px(760.), px(520.)), |_, cx| cx.new(|_| Host { build }))
        .expect("open offscreen window")
        .into();

    // Sweep the pointer across a few bars at *mid-height*, so the pointer lands
    // inside a bar's body rather than on its top anchor — the case that used to
    // miss. Captures whether a bar is picked from its body.
    for (x, y) in [
        (180., 300.),
        (260., 340.),
        (340., 360.),
        (420., 310.),
        (500., 350.),
        (600., 290.),
    ] {
        cx.simulate_mouse_move(
            window,
            gpui_kit::point(px(x), px(y)),
            Option::<gpui_kit::MouseButton>::None,
            Default::default(),
        );
        cx.run_until_parked();
        cx.advance_clock(std::time::Duration::from_millis(400));
        cx.run_until_parked();
    }
    // Rest the pointer inside a bar's body so the tooltip shows that bar.
    cx.simulate_mouse_move(
        window,
        gpui_kit::point(px(400.), px(340.)),
        Option::<gpui_kit::MouseButton>::None,
        Default::default(),
    );
    cx.run_until_parked();
    cx.advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();

    match cx.capture_screenshot(window) {
        Ok(image) => {
            image.save("demo/snapshots/hover-capture.png").unwrap();
            println!(
                "saved demo/snapshots/hover-capture.png ({}x{})",
                image.width(),
                image.height()
            );
        }
        Err(e) => eprintln!("capture_screenshot failed: {e}"),
    }
}
