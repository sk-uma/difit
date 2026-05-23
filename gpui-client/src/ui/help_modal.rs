//! Keyboard-shortcuts modal.
//!
//! Matches React's `HelpModal`: a centered card with a sticky header
//! ("Keyboard Shortcuts" + close X), and a two-column body of
//! categorized shortcut sections.

use gpui::{div, prelude::*, px, App, IntoElement, ParentElement, SharedString, Styled};

use crate::ui::theme::{Theme, UI_FONT};
use crate::ui::widgets::icon;

struct Section {
    title: &'static str,
    items: &'static [(&'static str, &'static str)],
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Line Navigation",
        items: &[
            ("j", "Next diff line"),
            ("k", "Previous diff line"),
        ],
    },
    Section {
        title: "File Navigation",
        items: &[
            ("n", "Next file"),
            ("p", "Previous file"),
        ],
    },
    Section {
        title: "View Options",
        items: &[
            ("v", "Toggle Split / Unified view"),
            ("w", "Toggle Ignore Whitespace"),
            ("m", "Toggle merge-base mode"),
        ],
    },
    Section {
        title: "Actions",
        items: &[
            ("c", "Add comment on the selected line"),
            ("o", "Open active file in editor"),
            ("r", "Refresh diff"),
            ("?", "Show / hide this help"),
            ("Esc", "Cancel compose / close modal"),
        ],
    },
];

pub fn render_help_modal(on_close: impl Fn(&mut App) + 'static + Clone) -> impl IntoElement {
    let close_backdrop = on_close.clone();
    let close_button = on_close;

    div()
        .id("help-modal")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5))
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |_e, _w, cx| close_backdrop(cx),
        )
        .child(
            // Stop click propagation so clicking inside the modal
            // doesn't dismiss it.
            div()
                .id("help-modal-card")
                .w(px(720.0))
                .max_h(px(640.0))
                .bg(Theme::BG)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded_md()
                .shadow_lg()
                .font_family(UI_FONT())
                .text_color(Theme::TEXT)
                .on_mouse_down(gpui::MouseButton::Left, |_e, _w, _cx| {})
                .flex()
                .flex_col()
                // Header.
                .child(
                    div()
                        .px(px(24.0))
                        .py(px(16.0))
                        .border_b_1()
                        .border_color(Theme::BORDER)
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(SharedString::from("Keyboard Shortcuts")),
                        )
                        .child(
                            div()
                                .id("help-close")
                                .p_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::BG_HOVER))
                                .on_click(move |_e, _w, cx| close_button(cx))
                                .child(icon("x", 18.0, Theme::TEXT_MUTED)),
                        ),
                )
                // Body: two-column grid.
                .child(
                    div()
                        .px(px(24.0))
                        .py(px(16.0))
                        .flex()
                        .flex_row()
                        .gap(px(24.0))
                        .child(
                            // Left column = first half of sections.
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(20.0))
                                .children(SECTIONS.iter().take(2).map(section_block)),
                        )
                        .child(
                            // Right column = remaining sections.
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(20.0))
                                .children(SECTIONS.iter().skip(2).map(section_block)),
                        ),
                ),
        )
}

fn section_block(section: &Section) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(Theme::TEXT)
                .mb_1()
                .child(SharedString::from(section.title)),
        )
        .children(section.items.iter().copied().map(|(key, desc)| {
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .text_size(px(13.0))
                .child(
                    div()
                        .text_color(Theme::TEXT_MUTED)
                        .child(SharedString::from(desc)),
                )
                .child(kbd(key))
        }))
        .into_any_element()
}

/// A keyboard-key chip — small monospace pill with border.
fn kbd(key: &'static str) -> impl IntoElement {
    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(Theme::BORDER)
        .bg(Theme::BG_HOVER)
        .text_size(px(11.0))
        .font_family(crate::ui::theme::MONO_FONT())
        .text_color(Theme::TEXT)
        .child(SharedString::from(key))
}
