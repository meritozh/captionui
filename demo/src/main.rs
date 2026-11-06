//! Observable Plot's documentation, ported: every example of the Features,
//! Marks, Transforms and Interactions pages, drawn by gpui-plot.

use gpui_kit::*;
use gpui_plot_demo::docs::Docs;

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| Docs::new())
        })
        .expect("failed to open the docs window");
    });
}
