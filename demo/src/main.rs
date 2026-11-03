//! A gallery of gpui-plot charts, built on GPUI Kit.
//!
//! The window lists the examples down the left and shows the selected one in
//! the pane beside it, so every mark type, scale, transform and interaction is
//! one click away.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};
use gpui_kit::base::StyledExt;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::*;

use gpui_plot_demo::gallery::{self, Example};

/// The gallery's window.
struct Gallery {
    examples: Rc<Vec<Example>>,
    selected: usize,
    /// The height the chart pane gives each plot, which a caller can change
    /// with the buttons above it.
    chart_height: f32,
}

impl Gallery {
    fn new() -> Self {
        Self {
            examples: Rc::new(gallery::examples()),
            selected: 0,
            chart_height: 420.,
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        cx.notify();
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div()
            .v_flex()
            .gap_1()
            .p_2()
            .w(px(220.))
            .h_full()
            .overflow_y_scrollbar();

        for (index, example) in self.examples.iter().enumerate() {
            let selected = index == self.selected;
            let id = SharedString::from(format!("example-{index}"));
            let entity = cx.entity();

            list = list.child(
                div()
                    .id(id)
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(selected, |this| this.bg(cx.theme().colors.accent))
                    .hover(|this| this.bg(cx.theme().colors.secondary_hover))
                    .on_click(move |_, _, cx| {
                        let entity = entity.clone();
                        entity.update(cx, |gallery, cx| gallery.select(index, cx));
                    })
                    .child(
                        div()
                            .text_sm()
                            .when(selected, |this| {
                                this.text_color(cx.theme().colors.accent_foreground)
                            })
                            .child(example.title),
                    ),
            );
        }

        div()
            .v_flex()
            .h_full()
            .border_r_1()
            .border_color(cx.theme().colors.border)
            .child(
                div()
                    .px_3()
                    .py_3()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("gpui-plot"),
            )
            .child(list)
    }

    fn content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let example = &self.examples[self.selected];
        let plot = (example.build)();
        let height = self.chart_height;

        let entity = cx.entity();
        let plot = plot
            .id("gallery-plot")
            .height(height)
            .tooltip(|row, _window, _cx| {
                // A custom tooltip body: a small table of every field, which is
                // what Plot's `tip` option lets a caller draw.
                let mut table = div().v_flex().gap_1().text_xs();
                for name in row.field_names() {
                    let value = row.value(name);
                    table = table.child(
                        div()
                            .h_flex()
                            .justify_between()
                            .gap_4()
                            .child(div().text_color(gpui::black()).child(name.clone()))
                            .child(div().child(value.label())),
                    );
                }
                table.into_any_element()
            });

        div()
            .v_flex()
            .flex_1()
            .h_full()
            .p_6()
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(example.title),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().colors.muted_foreground)
                            .child(example.section),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().colors.muted_foreground)
                            .child(example.description),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(Button::new("shorten").ghost().label("Shorter").on_click(
                        move |_, _, cx| {
                            let entity = entity.clone();
                            entity.update(cx, |gallery, cx| {
                                gallery.chart_height = (gallery.chart_height - 80.).max(200.);
                                cx.notify();
                            });
                        },
                    ))
                    .child(Button::new("taller").ghost().label("Taller").on_click({
                        let entity = cx.entity();
                        move |_, _, cx| {
                            let entity = entity.clone();
                            entity.update(cx, |gallery, cx| {
                                gallery.chart_height = (gallery.chart_height + 80.).min(900.);
                                cx.notify();
                            });
                        }
                    })),
            )
            .child(
                div()
                    .id("chart-pane")
                    .flex_1()
                    .min_h(px(200.))
                    .w_full()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().colors.border)
                    .p_4()
                    .child(plot),
            )
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .size_full()
            .bg(cx.theme().colors.background)
            .child(self.sidebar(cx))
            .child(self.content(cx))
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);

        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| Gallery::new())
        })
        .expect("failed to open the gallery window");
    });
}
