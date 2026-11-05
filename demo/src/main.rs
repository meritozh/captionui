//! Observable Plot's documentation, ported: every example of the Features,
//! Marks, Transforms and Interactions pages, drawn by gpui-plot.
//!
//! The sidebar lists the docs' sections and pages; a page shows its examples
//! in order, each with the docs' JavaScript beside the chart it ports.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};
use gpui_kit::base::StyledExt;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::*;

use gpui_plot_demo::examples::{self, Example};

/// The docs' sections, in sidebar order.
const SECTIONS: [(&str, &str); 4] = [
    ("features", "Features"),
    ("marks", "Marks"),
    ("transforms", "Transforms"),
    ("interactions", "Interactions"),
];

/// One docs page: its section, file name and title.
#[derive(Clone)]
struct Page {
    section: &'static str,
    page: String,
    title: String,
}

struct Docs {
    examples: Rc<Vec<Example>>,
    pages: Vec<Page>,
    selected: usize,
}

impl Docs {
    fn new() -> Self {
        let mut pages: Vec<Page> = Vec::new();
        for (section, _) in SECTIONS {
            for doc in examples::docs().iter().filter(|doc| doc.section == section) {
                if !pages
                    .iter()
                    .any(|page| page.section == section && page.page == doc.page)
                {
                    pages.push(Page {
                        section,
                        page: doc.page.clone(),
                        title: doc.page_title.clone(),
                    });
                }
            }
        }
        Self {
            examples: Rc::new(examples::examples()),
            pages,
            selected: 0,
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div()
            .v_flex()
            .gap_0p5()
            .p_2()
            .w(px(220.))
            .h_full()
            .overflow_y_scrollbar();

        for (section, label) in SECTIONS {
            list = list.child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_1()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().colors.muted_foreground)
                    .child(label),
            );
            for (index, page) in self
                .pages
                .iter()
                .enumerate()
                .filter(|(_, p)| p.section == section)
            {
                let selected = index == self.selected;
                let entity = cx.entity();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("page-{index}")))
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_sm()
                        .when(selected, |this| {
                            this.bg(cx.theme().colors.accent)
                                .text_color(cx.theme().colors.accent_foreground)
                        })
                        .hover(|this| this.bg(cx.theme().colors.secondary_hover))
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |docs, cx| {
                                docs.selected = index;
                                cx.notify();
                            });
                        })
                        .child(page.title.clone()),
                );
            }
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
                    .child("gpui-plot · Plot docs"),
            )
            .child(list)
    }

    fn content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let page = &self.pages[self.selected];
        let mut column = div()
            .id("page")
            .v_flex()
            .flex_1()
            .h_full()
            .p_6()
            .gap_6()
            .overflow_y_scrollbar()
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(page.title.clone()),
            );

        for doc in examples::docs()
            .iter()
            .filter(|doc| doc.section == page.section && doc.page == page.page)
        {
            let port = self
                .examples
                .iter()
                .find(|e| e.section == doc.section && e.page == doc.page && e.index == doc.index);
            let chart = match port {
                Some(example) => div()
                    .w(px(640.))
                    .h(px((example.build)().natural_height(640.)))
                    .child((example.build)().id(SharedString::from(example.id()))),
                None => div()
                    .text_sm()
                    .text_color(cx.theme().colors.muted_foreground)
                    .child("Not ported yet."),
            };
            column = column.child(
                div()
                    .v_flex()
                    .gap_2()
                    .when_some(doc.heading.clone(), |this, heading| {
                        this.child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(heading),
                        )
                    })
                    .child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().colors.secondary)
                            .text_xs()
                            .font_family("Menlo")
                            .child(doc.code.clone()),
                    )
                    .child(chart),
            );
        }
        column
    }
}

impl Render for Docs {
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
            cx.new(|_| Docs::new())
        })
        .expect("failed to open the docs window");
    });
}
