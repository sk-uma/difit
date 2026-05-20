use gpui::{div, px, IntoElement, ParentElement, SharedString, Styled};

use crate::api::types::{DiffCommentMessage, DiffCommentThread};
use crate::ui::theme::{Theme, UI_FONT};

pub fn render_thread(thread: &DiffCommentThread) -> impl IntoElement {
    let header = format!(
        "Thread • {} message{}",
        thread.messages.len(),
        if thread.messages.len() == 1 { "" } else { "s" }
    );

    let mut card = div()
        .my_1()
        .mx_4()
        .p_2()
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_sm()
        .bg(Theme::BG_ELEVATED)
        .font_family(UI_FONT)
        .text_size(px(12.0))
        .text_color(Theme::TEXT)
        .child(
            div()
                .text_color(Theme::TEXT_MUTED)
                .text_size(px(11.0))
                .child(SharedString::from(header)),
        );

    for msg in &thread.messages {
        card = card.child(render_message(msg));
    }

    card
}

fn render_message(msg: &DiffCommentMessage) -> impl IntoElement {
    let author = msg
        .author
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let meta = format!("{} • {}", author, short_timestamp(&msg.created_at));

    div()
        .mt_1()
        .pt_1()
        .border_t_1()
        .border_color(Theme::BORDER)
        .child(
            div()
                .text_color(Theme::TEXT_MUTED)
                .text_size(px(11.0))
                .child(SharedString::from(meta)),
        )
        .child(
            div()
                .mt_1()
                .text_color(Theme::TEXT)
                .child(SharedString::from(msg.body.clone())),
        )
}

fn short_timestamp(iso: &str) -> String {
    // ISO 8601 is verbose; show just the date + HH:MM if possible.
    if let Some(t_idx) = iso.find('T') {
        let date = &iso[..t_idx];
        let tail = &iso[t_idx + 1..];
        let time = tail.get(..5).unwrap_or(tail);
        format!("{date} {time}")
    } else {
        iso.to_string()
    }
}
