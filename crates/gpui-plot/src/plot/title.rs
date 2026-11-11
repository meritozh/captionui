//! The title, subtitle and caption painters.

use gpui::{App, Bounds, FontWeight, Pixels, TextAlign, Window, point, px};

use crate::paint::Label;
use crate::plot::Plot;
use crate::theme::PlotTheme;

impl Plot {
    /// Plot's `caption`, a figcaption below the plot, in the band
    /// [`Self::insets`] reserves at the figure's bottom.
    pub(super) fn paint_caption(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(caption) = &self.caption else {
            return;
        };
        let label = Label::new(
            caption.clone(),
            theme.label_size,
            theme.label,
            FontWeight::NORMAL,
            gpui::font(theme.font_family.clone()),
            window,
        );
        let band = theme.label_size.as_f32() + 8.;
        let y = bounds.size.height.as_f32() - band + 4.;
        label.paint_at(
            bounds.origin + point(px(0.), px(y)),
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }

    pub(super) fn paint_title(
        &self,
        bounds: Bounds<Pixels>,
        theme: &PlotTheme,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(title) = &self.title.text else {
            return;
        };
        let font = gpui::font(theme.font_family.clone());
        let label = Label::new(
            title.clone(),
            theme.title_size,
            theme.title,
            FontWeight::SEMIBOLD,
            font.clone(),
            window,
        );
        label.paint_at(
            bounds.origin + point(px(12.), px(6.)),
            TextAlign::Left,
            None,
            window,
            cx,
        );
        if let Some(subtitle) = &self.title.subtitle {
            let sub = Label::new(
                subtitle.clone(),
                theme.label_size,
                theme.label,
                FontWeight::NORMAL,
                font,
                window,
            );
            sub.paint_at(
                bounds.origin + point(px(12.), px(6. + theme.title_size.as_f32() + 6.)),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}
