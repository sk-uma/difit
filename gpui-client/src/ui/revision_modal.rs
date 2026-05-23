//! "Detailed Diff" modal — picks base / target revisions.
//!
//! Matches React's `RevisionDetailModal`:
//!   * modal chrome (sticky header with title + X close)
//!   * body with Base [...] Target picker pair on a single row
//!   * footer with Cancel + Apply buttons (Apply highlighted with the
//!     accent color)

use std::sync::Arc;

use gpui::{div, prelude::*, px, App, IntoElement, ParentElement, SharedString, Styled};

use crate::api::types::RevisionsResponse;
use crate::ui::revision_picker::{render_revision_picker, RevisionRole};
use crate::ui::theme::{Theme, UI_FONT};
use crate::ui::widgets::icon;

#[allow(clippy::too_many_arguments)]
pub fn render_revision_modal(
    revisions: Option<&Arc<RevisionsResponse>>,
    selected_base: Option<&str>,
    selected_target: Option<&str>,
    base_open: bool,
    target_open: bool,
    on_toggle_base: impl Fn(&mut App) + 'static + Clone,
    on_pick_base: impl Fn(String, &mut App) + 'static + Clone,
    on_close_base: impl Fn(&mut App) + 'static + Clone,
    on_toggle_target: impl Fn(&mut App) + 'static + Clone,
    on_pick_target: impl Fn(String, &mut App) + 'static + Clone,
    on_close_target: impl Fn(&mut App) + 'static + Clone,
    on_apply: impl Fn(&mut App) + 'static,
    on_close: impl Fn(&mut App) + 'static + Clone,
) -> impl IntoElement {
    let close_backdrop = on_close.clone();
    let close_button = on_close;
    div()
        .id("revision-modal")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.4))
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |_e, _w, cx| close_backdrop(cx),
        )
        .child(
            div()
                .id("revision-modal-card")
                .w(px(560.0))
                .bg(Theme::BG_ELEVATED)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(8.0))
                .shadow_lg()
                .font_family(UI_FONT())
                .text_color(Theme::TEXT)
                .on_mouse_down(gpui::MouseButton::Left, |_e, _w, _cx| {})
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(16.0))
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
                                .child(SharedString::from("Detailed Diff")),
                        )
                        .child(
                            div()
                                .id("revision-close")
                                .p_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::BG_HOVER))
                                .on_click(move |_e, _w, cx| close_button(cx))
                                .child(icon("x", 18.0, Theme::TEXT_MUTED)),
                        ),
                )
                .child(
                    div()
                        .px(px(16.0))
                        .py(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(16.0))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_center()
                                .gap(px(12.0))
                                .child(render_revision_picker(
                                    RevisionRole::Base,
                                    selected_base,
                                    revisions,
                                    base_open,
                                    on_toggle_base,
                                    on_pick_base,
                                    on_close_base,
                                ))
                                .child(
                                    div()
                                        .text_color(Theme::TEXT_MUTED)
                                        .child(SharedString::from("...")),
                                )
                                .child(render_revision_picker(
                                    RevisionRole::Target,
                                    selected_target,
                                    revisions,
                                    target_open,
                                    on_toggle_target,
                                    on_pick_target,
                                    on_close_target,
                                )),
                        )
                        .child(footer(on_apply)),
                ),
        )
}

fn footer(on_apply: impl Fn(&mut App) + 'static) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .justify_end()
        .gap(px(8.0))
        .child(
            // Cancel — neutral pill. The actual close happens via the
            // modal backdrop / X button; this is just an alias so the
            // user sees the standard pair.
            div()
                .id("rev-cancel")
                .px(px(12.0))
                .py(px(6.0))
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .rounded(px(4.0))
                .bg(Theme::BG)
                .text_color(Theme::TEXT_MUTED)
                .border_1()
                .border_color(Theme::BORDER)
                .cursor_pointer()
                .hover(|s| s.text_color(Theme::TEXT))
                .child(SharedString::from("Cancel")),
        )
        .child(
            div()
                .id("rev-apply")
                .px(px(12.0))
                .py(px(6.0))
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .rounded(px(4.0))
                .bg(Theme::TEXT_LINK)
                .text_color(Theme::TEXT)
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .on_click(move |_e, _w, cx| on_apply(cx))
                .child(SharedString::from("Apply")),
        )
}
