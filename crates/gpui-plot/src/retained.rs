//! A plot kept across frames, so its transform pipeline runs once.
//!
//! A [`Plot`] is a description: each frame it is turned into an element, and
//! its `prepaint` runs every mark's transforms, the scales and the layout
//! again. For a view that rebuilds its plots on every render, that is the
//! cost of a frame. A [`RetainedPlot`] holds one plot behind an `Rc` and
//! remembers what it resolved, so a frame at the same size and theme reuses
//! it: hovering, an animation, or a sibling's change only repaints.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, Window};
use gpui_kit::base::plot::{Plot as PlotTrait, PlotAppear, PlotElement, PlotHover, TooltipState};
use gpui_kit::component::ActiveTheme;

use crate::plot::Plot;
use crate::theme::PlotTheme;

/// A [`Plot`] that resolves its marks once and reuses them while it is drawn
/// at the same size and theme; build it with [`Plot::retained`].
///
/// Clones share the plot and what it resolved, so a view keeps one and
/// renders a clone each frame:
///
/// ```ignore
/// struct Chart { plot: RetainedPlot }
///
/// impl Render for Chart {
///     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
///         div().size_full().child(self.plot.clone())
///     }
/// }
/// ```
#[derive(Clone)]
pub struct RetainedPlot(Rc<RefCell<Retained>>);

struct Retained {
    plot: Plot,
    /// The bounds and theme the plot last resolved for.
    prepared: Option<(Bounds<Pixels>, PlotTheme)>,
    /// `natural_height` by width (as bits), which runs the pipeline too.
    heights: Vec<(u32, f32)>,
}

impl Plot {
    /// Keep this plot across frames: its pipeline runs once per size and
    /// theme instead of on every frame. See [`RetainedPlot`].
    pub fn retained(self) -> RetainedPlot {
        RetainedPlot(Rc::new(RefCell::new(Retained {
            plot: self,
            prepared: None,
            heights: Vec::new(),
        })))
    }
}

impl RetainedPlot {
    /// [`Plot::natural_height`], remembered per width.
    pub fn natural_height(&self, width: f32) -> f32 {
        let mut inner = self.0.borrow_mut();
        if let Some(&(_, height)) = inner.heights.iter().find(|(w, _)| *w == width.to_bits()) {
            return height;
        }
        let height = inner.plot.natural_height(width);
        inner.heights.push((width.to_bits(), height));
        height
    }

    /// Whether the plot has resolved its marks at least once.
    pub fn is_prepared(&self) -> bool {
        self.0.borrow().prepared.is_some()
    }
}

impl PlotTrait for RetainedPlot {
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let theme = PlotTheme::from_theme(cx.theme());
        let mut inner = self.0.borrow_mut();
        match &inner.prepared {
            // The same frame again: nothing to resolve.
            Some((last, last_theme)) if *last == bounds && *last_theme == theme => {
                return Vec::new();
            }
            // Moved but not resized (a scrolled list): shift the frame.
            Some((last, last_theme)) if last.size == bounds.size && *last_theme == theme => {
                let delta: Point<Pixels> = bounds.origin - last.origin;
                inner.plot.translate_prepared(delta);
                inner.prepared = Some((bounds, theme));
                return Vec::new();
            }
            _ => {}
        }
        let children = PlotTrait::prepaint(&mut inner.plot, bounds, window, cx);
        inner.prepared = Some((bounds, theme));
        children
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        PlotTrait::paint(&mut self.0.borrow_mut().plot, bounds, window, cx);
    }

    fn id(&self) -> Option<ElementId> {
        PlotTrait::id(&self.0.borrow().plot)
    }

    fn interactive(&self) -> bool {
        PlotTrait::interactive(&self.0.borrow().plot)
    }

    fn appear(&mut self, appear: PlotAppear, window: &mut Window, cx: &mut App) {
        PlotTrait::appear(&mut self.0.borrow_mut().plot, appear, window, cx);
    }

    fn appear_generation(&self) -> Option<u64> {
        PlotTrait::appear_generation(&self.0.borrow().plot)
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &App,
    ) -> Option<TooltipState> {
        PlotTrait::tooltip_state(&self.0.borrow().plot, position, bounds, cx)
    }

    fn hover(&mut self, hover: Option<&PlotHover>, window: &mut Window, cx: &mut App) {
        PlotTrait::hover(&mut self.0.borrow_mut().plot, hover, window, cx);
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        PlotTrait::tooltip(&self.0.borrow().plot, state, cursor, bounds, window, cx)
    }
}

impl IntoElement for RetainedPlot {
    type Element = PlotElement<Self>;

    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}
