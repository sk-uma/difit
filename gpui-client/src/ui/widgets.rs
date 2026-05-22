//! Small reusable UI primitives shared across the app: toggle switches,
//! pill buttons, and the difit logo glyph.

use gpui::{
    div, prelude::*, px, AnyView, App, Context, ElementId, IntoElement, ParentElement,
    SharedString, Styled, Window,
};

use crate::ui::theme::Theme;

/// Small hover-bubble view. Returned by `label_tooltip`.
pub struct TooltipBubble {
    pub text: SharedString,
}

impl gpui::Render for TooltipBubble {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(Theme::BG_ELEVATED)
            .border_1()
            .border_color(Theme::BORDER)
            .rounded_sm()
            .text_size(px(11.0))
            .text_color(Theme::TEXT)
            .child(self.text.clone())
    }
}

/// Returns a closure suitable for `Div::tooltip` that pops up a labelled
/// bubble on hover.
pub fn label_tooltip(label: &'static str) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_window, cx| {
        cx.new(|_cx| TooltipBubble {
            text: SharedString::from(label),
        })
        .into()
    }
}

/// A two-state toggle styled like the React UI (label + sliding pill).
pub fn toggle_switch(
    id: impl Into<SharedString>,
    label: &'static str,
    on: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let id = ElementId::Name(id.into());
    let track_bg = if on {
        Theme::TEXT_LINK
    } else {
        Theme::BG_HOVER
    };
    let knob_offset = if on { px(14.0) } else { px(2.0) };

    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(label)),
        )
        .child(
            div()
                .w(px(28.0))
                .h(px(16.0))
                .bg(track_bg)
                .rounded_full()
                .relative()
                .child(
                    div()
                        .absolute()
                        .top(px(2.0))
                        .left(knob_offset)
                        .w(px(12.0))
                        .h(px(12.0))
                        .bg(Theme::TEXT)
                        .rounded_full(),
                ),
        )
}

/// "difit" logo: a colored square + text.
pub fn logo() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_sm()
                .bg(Theme::TEXT_LINK),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(Theme::TEXT)
                .child(SharedString::from("difit")),
        )
}
