use gpui::{div, prelude::*, px, App, IntoElement, ParentElement, SharedString, Styled};

use crate::ui::theme::{Theme, UI_FONT};

const BINDINGS: &[(&str, &str)] = &[
    ("j / k", "Next / previous diff line"),
    ("n / p", "Next / previous file"),
    ("c", "Add a comment on the selected line"),
    ("v", "Toggle unified / split view"),
    ("w", "Toggle ignore whitespace"),
    ("m", "Toggle merge-base mode"),
    ("o", "Open active file in editor"),
    ("r", "Refresh diff"),
    ("?", "Show / hide this help"),
    ("Esc", "Cancel compose / close help"),
];

pub fn render_help_modal(on_close: impl Fn(&mut App) + 'static) -> impl IntoElement {
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
            move |_e, _w, cx| on_close(cx),
        )
        .child(
            div()
                .w(px(420.0))
                .max_h(px(560.0))
                .p_4()
                .bg(Theme::BG_ELEVATED)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded_md()
                .shadow_lg()
                .font_family(UI_FONT)
                .text_size(px(12.5))
                .text_color(Theme::TEXT)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(Theme::TEXT)
                        .child(SharedString::from("Keybindings")),
                )
                .children(BINDINGS.iter().copied().map(|(key, desc)| {
                    div()
                        .flex()
                        .flex_row()
                        .gap_3()
                        .child(
                            div()
                                .w(px(72.0))
                                .text_color(Theme::TEXT_LINK)
                                .child(SharedString::from(key)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(Theme::TEXT_MUTED)
                                .child(SharedString::from(desc)),
                        )
                }))
                .child(
                    div()
                        .mt_2()
                        .text_color(Theme::TEXT_MUTED)
                        .text_size(px(11.0))
                        .child(SharedString::from(
                            "Click outside or press Esc to close.",
                        )),
                ),
        )
}
