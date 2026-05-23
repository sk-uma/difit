//! Review-comment thread card.
//!
//! Visual style mirrors React's `CommentThreadCard`:
//!   `rounded-md border border-yellow-600/50 border-l-4 border-l-yellow-400
//!    bg-github-bg-tertiary p-3 shadow-sm`
//! — a yellow-accented panel with a `file:line` badge + Copy Prompt /
//! Reply icon buttons in the header, and replies indented with a left
//! border.

use gpui::{
    div, prelude::*, px, ElementId, IntoElement, ParentElement, Rgba, SharedString, Styled,
    StyledText,
};

use crate::api::types::{DiffCommentMessage, DiffCommentThread, DiffLineRange};
use crate::ui::actions::{DiffAction, DiffActions};
use crate::ui::diff_rows::CommentAnchor;
use crate::ui::markdown_view::parse_inline;
use crate::ui::theme::{Theme, MONO_FONT, UI_FONT};
use crate::ui::widgets::icon;

// Yellow palette pulled from the React UI (Tailwind yellow-400 / 600 +
// the path / button shades it derives from CSS vars).
const YELLOW_400: Rgba = rgb(0xfacc15);
const YELLOW_600_50: Rgba = rgba(0xca8a04, 0.5);
const YELLOW_PATH_BG: Rgba = rgba(0xeab308, 0.18);
const YELLOW_PATH_TEXT: Rgba = rgb(0xfde047);
const YELLOW_BTN_BG: Rgba = rgba(0xeab308, 0.15);
const YELLOW_BTN_TEXT: Rgba = rgb(0xfde047);
const YELLOW_BTN_BORDER: Rgba = rgba(0xca8a04, 0.45);

pub fn render_thread(thread: &DiffCommentThread, actions: &DiffActions) -> impl IntoElement {
    let anchor = CommentAnchor {
        side: thread.position.side,
        line: thread_line(thread),
    };
    let thread_id = thread.id.clone();
    let file_label = format!("{}:{}", thread.file_path, thread_line(thread));

    let mut card = div()
        .my_1()
        .mx(px(16.0))
        .p_3()
        .rounded_md()
        .bg(Theme::BG_HOVER)
        // Outer yellow tint…
        .border_1()
        .border_color(YELLOW_600_50)
        // …with a thicker yellow stripe on the left edge.
        .border_l(px(4.0))
        .border_color(YELLOW_400)
        .font_family(UI_FONT())
        .text_size(px(12.0))
        .text_color(Theme::TEXT)
        .child(
            div()
                .mb_3()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(file_badge(file_label))
                .child(header_actions(&thread_id, anchor, actions)),
        );

    for (idx, msg) in thread.messages.iter().enumerate() {
        let body = render_message(thread, msg, idx == 0, actions);
        if idx == 0 {
            card = card.child(body);
        } else {
            // Replies sit indented with a left border, matching
            // `ml-4 border-l border-github-border pl-3`.
            card = card.child(
                div()
                    .mt_3()
                    .ml(px(16.0))
                    .pl(px(12.0))
                    .border_l_1()
                    .border_color(Theme::BORDER)
                    .child(body),
            );
        }
    }

    card
}

fn thread_line(thread: &DiffCommentThread) -> u32 {
    match thread.position.line {
        DiffLineRange::Single(n) => n,
        DiffLineRange::Range { end, .. } => end,
    }
}

fn file_badge(label: String) -> impl IntoElement {
    div()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(3.0))
        .bg(YELLOW_PATH_BG)
        .text_color(YELLOW_PATH_TEXT)
        .text_size(px(11.0))
        .font_family(MONO_FONT())
        .overflow_hidden()
        .whitespace_nowrap()
        .child(SharedString::from(label))
}

fn header_actions(
    thread_id: &str,
    anchor: CommentAnchor,
    actions: &DiffActions,
) -> impl IntoElement {
    let copy_actions = actions.clone();
    let reply_actions = actions.clone();
    let copy_id = thread_id.to_string();
    let reply_id = thread_id.to_string();

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        // Copy Prompt — yellow pill with copy icon.
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!("copy-{thread_id}"))))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(YELLOW_BTN_BG)
                .text_color(YELLOW_BTN_TEXT)
                .border_1()
                .border_color(YELLOW_BTN_BORDER)
                .text_size(px(11.0))
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .on_click(move |_e, w, cx| {
                    copy_actions(
                        DiffAction::CopyPromptThread {
                            thread_id: copy_id.clone(),
                        },
                        w,
                        cx,
                    );
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(icon("copy", 12.0, YELLOW_BTN_TEXT))
                        .child(SharedString::from("Copy Prompt")),
                ),
        )
        // Reply — square icon button.
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!("reply-{thread_id}"))))
                .p(px(6.0))
                .rounded(px(4.0))
                .bg(Theme::BG_HOVER)
                .border_1()
                .border_color(Theme::BORDER)
                .cursor_pointer()
                .hover(|s| s.bg(Theme::BG))
                .on_click(move |_e, w, cx| {
                    reply_actions(
                        DiffAction::StartReply {
                            thread_id: reply_id.clone(),
                            anchor,
                        },
                        w,
                        cx,
                    );
                })
                .child(icon("reply", 14.0, Theme::TEXT)),
        )
}

fn render_message(
    thread: &DiffCommentThread,
    msg: &DiffCommentMessage,
    is_root: bool,
    actions: &DiffActions,
) -> impl IntoElement {
    let author = msg
        .author
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let meta = format!("{} • {}", author, short_timestamp(&msg.created_at));

    let anchor = CommentAnchor {
        side: thread.position.side,
        line: thread_line(thread),
    };
    let edit_actions = actions.clone();
    let delete_actions = actions.clone();
    let resolve_actions = actions.clone();
    let thread_id_e = thread.id.clone();
    let thread_id_d = thread.id.clone();
    let thread_id_r = thread.id.clone();
    let msg_id_e = msg.id.clone();
    let msg_id_d = msg.id.clone();
    let body_for_edit = msg.body.clone();

    // Root messages get a "Resolve thread" action; replies get a
    // plain "Delete".
    let trailing_action = if is_root {
        ("resolve", "Resolve")
    } else {
        ("delete", "Delete")
    };

    div()
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
                .child(action_link(
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
                .child(action_link(
                    format!(
                        "{}-{}-{}",
                        trailing_action.0, thread.id, msg.id
                    ),
                    trailing_action.1,
                    move |w, cx| {
                        if is_root {
                            resolve_actions(
                                DiffAction::DeleteThread {
                                    thread_id: thread_id_r.clone(),
                                },
                                w,
                                cx,
                            );
                        } else {
                            delete_actions(
                                DiffAction::DeleteMessage {
                                    thread_id: thread_id_d.clone(),
                                    message_id: msg_id_d.clone(),
                                },
                                w,
                                cx,
                            );
                        }
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

fn action_link(
    id: String,
    label: &'static str,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .px_1()
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(11.0))
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::TEXT))
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
