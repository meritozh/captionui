//! The docs window: Observable Plot's documentation, ported, every example
//! of the Features, Marks, Transforms and Interactions pages drawn by
//! gpui-plot.
//!
//! The sidebar lists the docs' sections and pages; a page shows its examples
//! in order, each with the docs' JavaScript beside the chart it ports.
//!
//! Each chart is its own view holding a [`RetainedPlot`], embedded cached:
//! hovering a chart re-renders that chart alone, its transforms run once,
//! and the rest of the page is drawn again from the last frame. A page's
//! charts are built a few per frame, so switching pages paints at once and
//! the charts fill in.

use std::rc::Rc;

use gpui_kit::gpui;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder;
use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, SharedString, StyleRefinement, Styled,
    Window, div, px,
};
use gpui_kit::base::StyledExt;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::*;

use gpui_plot::RetainedPlot;
use crate::examples::{self, Example};

/// The width every chart is drawn at, the docs' default.
const WIDTH: f32 = 640.;

/// How long a frame may spend building charts before it yields, so the
/// window stays responsive while a page's charts fill in.
const BUILD_BUDGET: Duration = Duration::from_millis(8);

/// The height a chart's slot takes until the chart is built.
const PLACEHOLDER_HEIGHT: f32 = 240.;

/// One chart: its plot, resolved once, at its natural height.
struct Chart {
    plot: RetainedPlot,
}

impl Render for Chart {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.plot.clone())
    }
}

/// A docs example on the current page, and its chart once built.
struct Slot {
    example: Option<usize>,
    chart: Option<(Entity<Chart>, f32)>,
}

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

pub struct Docs {
    examples: Rc<Vec<Example>>,
    pages: Vec<Page>,
    selected: usize,
    /// The current page's examples, in docs order.
    slots: Vec<Slot>,
    /// Whether a frame is already booked to build more charts.
    building: bool,
}

impl Default for Docs {
    fn default() -> Self {
        Self::new()
    }
}

impl Docs {
    pub fn new() -> Self {
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
        let mut docs = Self {
            examples: Rc::new(examples::examples()),
            pages,
            selected: 0,
            slots: Vec::new(),
            building: false,
        };
        docs.select(0);
        docs
    }

    /// Show page `index`: its slots, every chart still to build.
    pub fn select(&mut self, index: usize) {
        self.selected = index;
        let page = &self.pages[index];
        self.slots = examples::docs()
            .iter()
            .filter(|doc| doc.section == page.section && doc.page == page.page)
            .map(|doc| Slot {
                example: self.examples.iter().position(|e| {
                    e.section == doc.section && e.page == doc.page && e.index == doc.index
                }),
                chart: None,
            })
            .collect();
    }

    /// Build the page's next charts, in order, until the frame's budget is
    /// spent; book another frame while any remain.
    pub fn build_charts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.building = false;
        let start = Instant::now();
        let examples = self.examples.clone();
        for slot in self.slots.iter_mut() {
            if slot.chart.is_some() {
                continue;
            }
            let Some(index) = slot.example else {
                continue;
            };
            let example = &examples[index];
            let plot = (example.build)()
                .id(SharedString::from(example.id()))
                .retained();
            let height = plot.natural_height(WIDTH);
            slot.chart = Some((cx.new(|_| Chart { plot }), height));
            if start.elapsed() >= BUILD_BUDGET {
                break;
            }
        }
        cx.notify();
        self.schedule_build(window, cx);
    }

    /// Book a frame to build charts if any are still missing.
    fn schedule_build(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self
            .slots
            .iter()
            .any(|slot| slot.example.is_some() && slot.chart.is_none());
        if pending && !self.building {
            self.building = true;
            cx.on_next_frame(window, |docs, window, cx| docs.build_charts(window, cx));
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
                                docs.select(index);
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

        let docs = examples::docs()
            .iter()
            .filter(|doc| doc.section == page.section && doc.page == page.page);
        for (doc, slot) in docs.zip(&self.slots) {
            let chart = match (&slot.chart, slot.example) {
                // Cached: drawn again from the last frame until the chart
                // itself is notified (its hover) or its size changes.
                (Some((chart, height)), _) => div().child(
                    chart
                        .clone()
                        .cached(StyleRefinement::default().w(px(WIDTH)).h(px(*height))),
                ),
                (None, Some(_)) => div()
                    .w(px(WIDTH))
                    .h(px(PLACEHOLDER_HEIGHT))
                    .rounded_md()
                    .bg(cx.theme().colors.secondary)
                    .opacity(0.5),
                (None, None) => div()
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.schedule_build(window, cx);
        div()
            .h_flex()
            .size_full()
            .bg(cx.theme().colors.background)
            .child(self.sidebar(cx))
            .child(self.content(cx))
    }
}

impl Docs {
    /// The index of the page titled `section/page`, for tools that open one.
    pub fn page_index(&self, section: &str, page: &str) -> Option<usize> {
        self.pages
            .iter()
            .position(|p| p.section == section && p.page == page)
    }

    /// Whether every chart on the page is built.
    pub fn is_built(&self) -> bool {
        self.slots
            .iter()
            .all(|slot| slot.example.is_none() || slot.chart.is_some())
    }
}
