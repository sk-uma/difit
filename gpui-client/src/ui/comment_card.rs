use gpui::{
    div, prelude::*, px, ElementId, IntoElement, ParentElement, SharedString, Styled, StyledText,
};

use crate::api::types::{DiffCommentMessage, DiffCommentThread, DiffLineRange};
use crate::ui::actions::{DiffAction, DiffActions};
use crate::ui::diff_rows::CommentAnchor;
use crate::ui::markdown_view::parse_inline;
use crate::ui::theme::{Theme, UI_FONT};

pub fn render_thread(thread: &DiffCommentThread, actions: &DiffActions) -> impl IntoElement {
    let header = format!(
        "Thread • {} message{}",
        thread.messages.len(),
        if thread.messages.len() == 1 { "" } else { "s" }
    );

    let anchor = CommentAnchor {
        side: thread.position.side,
        line: match thread.position.line {
            DiffLineRange::Single(n) => n,
            DiffLineRange::Range { end, .. } => end,
        },
    };
    let thread_id = thread.id.clone();

    let mut card = div()
        .my_1()
        .mx_4()
        .p_2()
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_sm()
        .bg(Theme::BG_ELEVATED)
        .font_family(UI_FONT())
        .text_size(px(12.0))
        .text_color(Theme::TEXT)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_color(Theme::TEXT_MUTED)
                        .text_size(px(11.0))
                        .child(SharedString::from(header)),
                )
                .child(thread_actions(&thread_id, anchor, actions)),
        );

    for msg in &thread.messages {
        card = card.child(render_message(thread, msg, actions));
    }

    card
}

fn thread_actions(
    thread_id: &str,
    anchor: CommentAnchor,
    actions: &DiffActions,
) -> impl IntoElement {
    let reply_actions = actions.clone();
    let copy_actions = actions.clone();
    let delete_actions = actions.clone();
    let reply_id = thread_id.to_string();
    let copy_id = thread_id.to_string();
    let delete_id = thread_id.to_string();

    div()
        .flex()
        .flex_row()
        .gap_1()
        .child(mini_button(
            format!("reply-{thread_id}"),
            "Reply",
            move |w, cx| {
                reply_actions(
                    DiffAction::StartReply {
                        thread_id: reply_id.clone(),
                        anchor,
                    },
                    w,
                    cx,
                );
            },
        ))
        .child(mini_button(
            format!("copy-{thread_id}"),
            "Copy",
            move |w, cx| {
                copy_actions(
                    DiffAction::CopyPromptThread {
                        thread_id: copy_id.clone(),
                    },
                    w,
                    cx,
                );
            },
        ))
        .child(mini_button(
            format!("delete-thread-{thread_id}"),
            "Delete",
            move |w, cx| {
                delete_actions(
                    DiffAction::DeleteThread {
                        thread_id: delete_id.clone(),
                    },
                    w,
                    cx,
                );
            },
        ))
}

fn render_message(
    thread: &DiffCommentThread,
    msg: &DiffCommentMessage,
    actions: &DiffActions,
) -> impl IntoElement {
    let author = msg
        .author
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let meta = format!("{} • {}", author, short_timestamp(&msg.created_at));

    let anchor = CommentAnchor {
        side: thread.position.side,
        line: match thread.position.line {
            DiffLineRange::Single(n) => n,
            DiffLineRange::Range { end, .. } => end,
        },
    };

    let edit_actions = actions.clone();
    let delete_actions = actions.clone();
    let thread_id_e = thread.id.clone();
    let thread_id_d = thread.id.clone();
    let msg_id_e = msg.id.clone();
    let msg_id_d = msg.id.clone();
    let body_for_edit = msg.body.clone();

    div()
        .mt_1()
        .pt_1()
        .border_t_1()
        .border_color(Theme::BORDER)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_color(Theme::TEXT_MUTED)
                        .text_size(px(11.0))
                        .child(SharedString::from(meta)),
                )
                .child(mini_button(
                    format!("edit-{}-{}", thread.id, msg.id),
                    "Edit",
                    move |w, cx| {
                        edit_actions(
                            DiffAction::StartEdit {
                                thread_id: thread_id_e.clone(),
                                message_id: msg_id_e.clone(),
                                body: body_for_edit.clone(),
                                anchor,
                            },
                            w,
                            cx,
                        );
                    },
                ))
                .child(mini_button(
                    format!("del-msg-{}-{}", thread.id, msg.id),
                    "Delete",
                    move |w, cx| {
                        delete_actions(
                            DiffAction::DeleteMessage {
                                thread_id: thread_id_d.clone(),
                                message_id: msg_id_d.clone(),
                            },
                            w,
                            cx,
                        );
                    },
                )),
        )
        .child(
            div()
                .mt_1()
                .text_color(Theme::TEXT)
                .child(render_comment_body(&msg.body)),
        )
}

fn render_comment_body(text: &str) -> StyledText {
    let (rendered, highlights) = parse_inline(text);
    if highlights.is_empty() {
        StyledText::new(SharedString::from(rendered))
    } else {
        StyledText::new(SharedString::from(rendered)).with_highlights(highlights)
    }
}

fn mini_button(
    id: String,
    label: &'static str,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .px_2()
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(11.0))
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::TEXT_LINK))
        .on_click(move |_e, w, cx| on_click(w, cx))
        .child(SharedString::from(label))
}

fn short_timestamp(iso: &str) -> String {
    if let Some(t_idx) = iso.find('T') {
        let date = &iso[..t_idx];
        let tail = &iso[t_idx + 1..];
        let time = tail.get(..5).unwrap_or(tail);
        format!("{date} {time}")
    } else {
        iso.to_string()
    }
}
