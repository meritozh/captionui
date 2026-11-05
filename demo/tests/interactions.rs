//! Headless interaction tests: render every gallery plot and sweep the pointer
//! across it, confirming the interaction rendering paths — hover, tooltip_state,
//! tooltip and the hover-bearing paint — run without erroring.
//!
//! The snapshot renderer draws the settled chart; this also moves the pointer
//! across it, so the crosshair, highlighted dots and tooltip overlay (which
//! only draw while hovering) are exercised.

use gpui_kit::{IntoElement, ParentElement, Render, Styled, TestAppContext, div, px};
use gpui_plot_demo::gallery;

/// A window holding one plot, sized like the gallery.
struct Host {
    build: fn() -> gpui_plot::Plot,
}

impl Render for Host {
    fn render(
        &mut self,
        _window: &mut gpui_kit::Window,
        _cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        div().size_full().child((self.build)())
    }
}

/// Build a headless test context on a thread-safe platform, so the sweep can
/// run from a `#[test]` thread (the native Mac renderer insists on main).
fn context() -> TestAppContext {
    let dispatcher = gpui_kit::TestDispatcher::new(0);
    TestAppContext::build(dispatcher, Some("interactions"))
}

#[test]
fn sweeping_the_pointer_across_every_example_does_not_error() {
    let mut cx = context();
    let examples = gallery::examples();
    let mut failures = Vec::new();

    // gpui-kit's init installs the Theme global the plots read; replicate what
    // its own tests do so a raw TestAppContext has a theme.
    cx.update(gpui_kit::init);

    for example in &examples {
        // Open a window holding just this example's plot.
        let build = example.build;
        let (_, window) = cx.add_window_view(|_, _| Host { build });

        // Sweep the pointer across the plot in a grid. Each move re-resolves a
        // hover; crossing a datum draws the crosshair, highlighted dots and
        // tooltip overlay.
        for row in 0..12 {
            for col in 0..16 {
                let x = 40. + col as f32 * 45.;
                let y = 40. + row as f32 * 38.;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    window.simulate_mouse_move(
                        gpui_kit::point(px(x), px(y)),
                        Option::<gpui_kit::MouseButton>::None,
                        Default::default(),
                    );
                    window.run_until_parked();
                }));
                if result.is_err() {
                    failures.push(format!("{}: panicked at ({x:.0},{y:.0})", example.title));
                    break;
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "interaction sweep failed: {failures:?}"
    );
    let _ = examples;
}
