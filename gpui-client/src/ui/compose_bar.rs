//! Comment compose bar.
//!
//! Visual layout mirrors React's `CommentForm`: a yellow-accented card
//! with a title row, a textarea-styled body input, and right-aligned
//! Cancel / Submit buttons (Submit uses the same yellow pill as the
//! "Copy Prompt" button on the thread card).

use gpui::{
    div, prelude::*, px, App, ElementId, Entity, IntoElement, ParentElement, Rgba, SharedString,
    Styled,
};

use crate::api::types::DiffSide;
use crate::ui::text_input::TextInput;
use crate::ui::theme::{Theme, UI_FONT};

// Yellow palette identical to the values in `comment_card.rs`.
const YELLOW_400: Rgba = rgb(0xfacc15);
const YELLOW_600_50: Rgba = rgba(0xca8a04, 0.5);
const YELLOW_PATH_TEXT: Rgba = rgb(0xfde047);
const YELLOW_BTN_BG: Rgba = rgba(0xeab308, 0.15);
const YELLOW_BTN_TEXT: Rgba = rgb(0xfde047);
const YELLOW_BTN_BORDER: Rgba = rgba(0xca8a04, 0.45);

pub fn render_compose_bar(
    file_path: SharedString,
    _side: DiffSide,
    _line_input: Entity<TextInput>,
    body_input: Entity<TextInput>,
    _on_toggle_side: impl Fn(DiffSide, &mut App) + 'static,
    on_submit: impl Fn(&mut App) + 'static,
    on_cancel: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .w_full()
        .flex_shrink_0()
        .my(px(8.0))
        .mx(px(12.0))
        .p(px(12.0))
        .rounded_md()
        .bg(Theme::BG_HOVER)
        .border_1()
        .border_color(YELLOW_600_50)
        .border_l(px(4.0))
        .border_color(YELLOW_400)
        .font_family(UI_FONT())
        .text_color(Theme::TEXT)
        .text_size(px(13.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_color(YELLOW_PATH_TEXT)
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(SharedString::from(format!("New comment — {file_path}"))),
                ),
        )
        .child(
            // Body: a textarea-styled box matching React's
            // `bg-bg-secondary border border-border rounded px-3 py-2`.
            div()
                .min_h(px(80.0))
                .bg(Theme::BG)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(6.0))
                .px(px(12.0))
                .py(px(8.0))
                .child(body_input.clone()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(8.0))
                .child(cancel_button(on_cancel))
                .child(submit_button(on_submit)),
        )
}

fn cancel_button(on_click: impl Fn(&mut App) + 'static) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from("compose-cancel")))
        .px(px(12.0))
        .py(px(6.0))
        .text_size(px(11.0))
        .rounded(px(4.0))
        .bg(Theme::BG_HOVER)
        .text_color(Theme::TEXT)
        .border_1()
        .border_color(Theme::BORDER)
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(SharedString::from("Cancel"))
}

fn submit_button(on_click: impl Fn(&mut App) + 'static) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from("compose-submit")))
        .px(px(12.0))
        .py(px(6.0))
        .text_size(px(11.0))
        .rounded(px(4.0))
        .bg(YELLOW_BTN_BG)
        .text_color(YELLOW_BTN_TEXT)
        .border_1()
        .border_color(YELLOW_BTN_BORDER)
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(SharedString::from("Submit"))
}

const fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

const fn rgba(hex: u32, alpha: f32) -> Rgba {
    let mut c = rgb(hex);
    c.a = alpha;
    c
}
