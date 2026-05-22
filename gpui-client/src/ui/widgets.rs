//! Small reusable UI primitives shared across the app: toggle switches,
//! pill buttons, and the difit logo glyph.

use gpui::{
    div, prelude::*, px, svg, AnyView, App, Context, ElementId, IntoElement, ParentElement,
    Rgba, SharedString, Styled, Window,
};

use crate::ui::theme::Theme;

/// Render an embedded SVG icon. `name` is the filename without
/// extension (e.g. `"chevron-right"`). Color follows `currentColor`
/// via `.text_color`.
pub fn icon(name: &str, size_px: f32, color: Rgba) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .w(px(size_px))
        .h(px(size_px))
        .text_color(color)
}

/// Icon + label inline, used when replacing text-only header buttons.
pub fn icon_label(name: &str, label: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(icon(name, 14.0, Theme::TEXT_MUTED))
        .child(SharedString::from(label))
}

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

/// "difit" wordmark, rendered via the embedded logo SVG (lifted from
/// the React `Logo.tsx`).
pub fn logo() -> impl IntoElement {
    svg()
        .path(SharedString::from("icons/difit-logo.svg"))
        .w(px(72.0))
        .h(px(19.0))
        .text_color(Theme::TEXT)
}
