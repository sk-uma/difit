use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, ElementId, IntoElement, ParentElement, SharedString, Styled,
};

use crate::api::types::{DiffCommentThread, DiffLineRange};
use crate::ui::theme::{Theme, UI_FONT};

pub fn render_comments_list_modal(
    threads: Arc<Vec<DiffCommentThread>>,
    on_jump: impl Fn(String, &mut App) + 'static + Clone,
    on_close: impl Fn(&mut App) + 'static + Clone,
) -> impl IntoElement {
    let close_backdrop = on_close.clone();
    let close_button = on_close;
    div()
        .id("comments-list-modal")
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
            div()
                .id("comments-list-card")
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
                                .child(SharedString::from(format!(
                                    "All comments ({})",
                                    threads.len()
                                ))),
                        )
                        .child(
                            div()
                                .id("comments-list-close")
                                .p_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::BG_HOVER))
                                .on_click(move |_e, _w, cx| close_button(cx))
                                .child(crate::ui::widgets::icon(
                                    "x",
                                    18.0,
                                    Theme::TEXT_MUTED,
                                )),
                        ),
                )
                .child(
                    div()
                        .id("comments-list-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .px(px(24.0))
                        .py(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .children(threads.iter().enumerate().map(|(idx, thread)| {
                            let jump = on_jump.clone();
                            let path = thread.file_path.clone();
                            row(idx, thread, move |cx| jump(path.clone(), cx))
                        })),
                ),
        )
}

fn row(
    idx: usize,
    thread: &DiffCommentThread,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let line_label = match thread.position.line {
        DiffLineRange::Single(n) => format!("L{n}"),
        DiffLineRange::Range { start, end } => format!("L{start}-L{end}"),
    };
    let header = format!("{}  {}", thread.file_path, line_label);
    let preview = thread
        .messages
        .first()
        .map(|m| truncate(&m.body, 200))
        .unwrap_or_default();
    let total = thread.messages.len();

    div()
        .id(ElementId::Name(SharedString::from(format!(
            "comments-list-row-{idx}"
        ))))
        .p_2()
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_sm()
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(
            div()
                .text_color(Theme::TEXT_LINK)
                .text_size(px(12.0))
                .child(SharedString::from(header)),
        )
        .child(
            div()
                .mt_1()
                .text_color(Theme::TEXT)
                .child(SharedString::from(preview)),
        )
        .child(
            div()
                .mt_1()
                .text_color(Theme::TEXT_MUTED)
                .text_size(px(11.0))
                .child(SharedString::from(format!(
                    "{total} message{}",
                    if total == 1 { "" } else { "s" }
                ))),
        )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}
