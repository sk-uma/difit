//! The "new comment" panel that sits below the diff list while the user
//! is composing a thread. Single-shot widget — the parent (DifitApp)
//! owns the `TextInput` entities and the lifecycle of this bar.

use gpui::{div, prelude::*, px, App, Entity, IntoElement, ParentElement, SharedString, Styled};

use crate::api::types::DiffSide;
use crate::ui::text_input::TextInput;
use crate::ui::theme::{Theme, UI_FONT};

pub struct ComposeBarProps<S, B, T, C>
where
    S: Fn(&mut App) + 'static,
    B: Fn(&mut App) + 'static,
    T: Fn(DiffSide, &mut App) + 'static,
    C: Fn(&mut App) + 'static,
{
    pub file_path: SharedString,
    pub side: DiffSide,
    pub line_input: Entity<TextInput>,
    pub body_input: Entity<TextInput>,
    pub on_toggle_side: T,
    pub on_submit: S,
    pub on_cancel: C,
    pub _phantom: std::marker::PhantomData<B>,
}

pub fn render_compose_bar(
    file_path: SharedString,
    side: DiffSide,
    line_input: Entity<TextInput>,
    body_input: Entity<TextInput>,
    on_toggle_side: impl Fn(DiffSide, &mut App) + 'static,
    on_submit: impl Fn(&mut App) + 'static,
    on_cancel: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .w_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .bg(Theme::BG_ELEVATED)
        .border_t_1()
        .border_color(Theme::BORDER)
        .font_family(UI_FONT())
        .text_color(Theme::TEXT)
        .text_size(px(12.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_color(Theme::TEXT_MUTED)
                        .child(SharedString::from(format!("New comment in {file_path}"))),
                )
                .child(side_toggle(side, on_toggle_side))
                .child(
                    div()
                        .text_color(Theme::TEXT_MUTED)
                        .child(SharedString::from("Line:")),
                )
                .child(div().w(px(80.0)).child(line_input.clone()))
                .child(div().flex_1())
                .child(button("compose-cancel", "Cancel", on_cancel))
                .child(button("compose-submit", "Submit", on_submit)),
        )
        .child(div().h(px(120.0)).child(body_input.clone()))
}

fn side_toggle(
    current: DiffSide,
    on_toggle: impl Fn(DiffSide, &mut App) + 'static,
) -> impl IntoElement {
    let on_toggle = std::sync::Arc::new(on_toggle);
    let on_old = on_toggle.clone();
    let on_new = on_toggle;
    div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_sm()
        .child(side_button(
            "compose-side-old",
            "Old",
            current == DiffSide::Old,
            move |cx| on_old(DiffSide::Old, cx),
        ))
        .child(side_button(
            "compose-side-new",
            "New",
            current == DiffSide::New,
            move |cx| on_new(DiffSide::New, cx),
        ))
}

fn side_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let bg = if active { Theme::BG_SELECTED } else { Theme::BG_ELEVATED };
    let fg = if active { Theme::TEXT } else { Theme::TEXT_MUTED };
    div()
        .id(id)
        .px_2()
        .py_1()
        .bg(bg)
        .text_color(fg)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(SharedString::from(label))
}

fn button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(Theme::BORDER)
        .text_color(Theme::TEXT)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(SharedString::from(label))
}
