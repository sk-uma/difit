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

/// Parsed segment of a comment body: either a plain-text run rendered as
/// inline markdown, or a fenced `\`\`\`suggestion ... \`\`\`` block that
/// the React UI renders as a +/- diff snippet.
enum BodyPart {
    Text(String),
    Suggestion(String),
}

// Yellow palette pulled from the React UI (Tailwind yellow-400 / 600 +
// the path / button shades it derives from CSS vars).
const YELLOW_400: Rgba = rgb(0xfacc15);
const YELLOW_600_50: Rgba = rgba(0xca8a04, 0.5);
const YELLOW_PATH_BG: Rgba = rgba(0xeab308, 0.18);
const YELLOW_PATH_TEXT: Rgba = rgb(0xfde047);
const YELLOW_BTN_BG: Rgba = rgba(0xeab308, 0.15);
const YELLOW_BTN_TEXT: Rgba = rgb(0xfde047);
const YELLOW_BTN_BORDER: Rgba = rgba(0xca8a04, 0.45);
const GREEN_700: Rgba = rgb(0x15803d);

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
    // Layout mirrors React's ThreadMessageItem:
    //   `flex items-start gap-3` with
    //     [ author badge? + body ]  on the left (flex-1)
    //     [ Edit ] [ Check or Trash ]  on the right (shrink-0)
    let anchor = CommentAnchor {
        side: thread.position.side,
        line: thread_line(thread),
    };
    let edit_actions = actions.clone();
    let resolve_actions = actions.clone();
    let delete_actions = actions.clone();
    let thread_id_e = thread.id.clone();
    let thread_id_r = thread.id.clone();
    let thread_id_d = thread.id.clone();
    let msg_id_e = msg.id.clone();
    let msg_id_d = msg.id.clone();
    let body_for_edit = msg.body.clone();
    let author_label = msg.author.clone();

    let mut left = div().flex_1().min_w_0().flex().flex_col().gap(px(8.0));
    if let Some(author) = author_label {
        left = left.child(author_badge(author));
    }
    left = left.child(render_comment_body(&msg.body));

    let right = div()
        .flex()
        .flex_row()
        .items_start()
        .flex_shrink_0()
        .gap(px(8.0))
        .pt(px(2.0))
        .child(square_icon_button(
            format!("edit-{}-{}", thread.id, msg.id),
            "edit",
            "Edit message",
            Theme::TEXT,
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
        .child(if is_root {
            square_icon_button(
                format!("resolve-{}-{}", thread.id, msg.id),
                "check",
                "Resolve thread",
                GREEN_700,
                move |w, cx| {
                    resolve_actions(
                        DiffAction::DeleteThread {
                            thread_id: thread_id_r.clone(),
                        },
                        w,
                        cx,
                    );
                },
            )
            .into_any_element()
        } else {
            square_icon_button(
                format!("delete-{}-{}", thread.id, msg.id),
                "trash",
                "Delete reply",
                Theme::FILE_STATUS_DEL,
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
            )
            .into_any_element()
        });

    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(12.0))
        .child(left)
        .child(right)
}

fn author_badge(author: String) -> impl IntoElement {
    div()
        .self_start()
        .px(px(8.0))
        .py(px(2.0))
        .rounded_full()
        .border_1()
        .border_color(Theme::BORDER)
        .bg(Theme::BG)
        .text_color(Theme::TEXT)
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(SharedString::from(author))
}

fn square_icon_button(
    id: String,
    icon_name: &'static str,
    tooltip: &'static str,
    color: Rgba,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .p(px(6.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(Theme::BORDER)
        .bg(Theme::BG_HOVER)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG))
        .on_click(move |_e, w, cx| on_click(w, cx))
        .tooltip(crate::ui::widgets::label_tooltip(tooltip))
        .child(icon(icon_name, 12.0, color))
}

fn render_comment_body(text: &str) -> impl IntoElement {
    let parts = parse_suggestion_blocks(text);
    let mut wrap = div().flex().flex_col().gap(px(8.0)).text_color(Theme::TEXT);
    for part in parts {
        match part {
            BodyPart::Text(s) => {
                if s.trim().is_empty() {
                    continue;
                }
                wrap = wrap.child(render_inline_text(&s));
            }
            BodyPart::Suggestion(code) => {
                wrap = wrap.child(render_suggestion_block(code));
            }
        }
    }
    wrap
}

fn render_inline_text(text: &str) -> impl IntoElement {
    let (rendered, highlights) = parse_inline(text);
    let styled = if highlights.is_empty() {
        StyledText::new(SharedString::from(rendered))
    } else {
        StyledText::new(SharedString::from(rendered)).with_highlights(highlights)
    };
    div().text_color(Theme::TEXT).child(styled)
}

fn render_suggestion_block(code: String) -> impl IntoElement {
    let lines: Vec<String> = if code.is_empty() {
        vec![String::new()]
    } else {
        code.split('\n').map(|s| s.to_string()).collect()
    };
    let mut block = div()
        .my(px(4.0))
        .rounded_md()
        .overflow_hidden()
        .border_1()
        .border_color(Theme::BORDER)
        .font_family(MONO_FONT())
        .text_size(px(12.0));
    for (i, line) in lines.iter().enumerate() {
        block = block.child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "sugg-line-{i}"
                ))))
                .px(px(8.0))
                .py(px(1.0))
                .bg(Theme::DIFF_ADD_BG)
                .text_color(Theme::TEXT)
                .whitespace_nowrap()
                .child(SharedString::from(format!("+ {line}"))),
        );
    }
    block
}

/// Split a comment body into alternating plain-text and ```suggestion
/// fenced-block parts. Matches the regex used in
/// `src/utils/suggestionUtils.ts::parseSuggestionBlocks`.
fn parse_suggestion_blocks(body: &str) -> Vec<BodyPart> {
    const OPEN: &str = "```suggestion\n";
    const CLOSE: &str = "```";
    let mut out: Vec<BodyPart> = Vec::new();
    let mut cursor = 0;
    while cursor < body.len() {
        let rest = &body[cursor..];
        let Some(open_rel) = rest.find(OPEN) else {
            out.push(BodyPart::Text(rest.to_string()));
            break;
        };
        if open_rel > 0 {
            out.push(BodyPart::Text(body[cursor..cursor + open_rel].to_string()));
        }
        let body_start = cursor + open_rel + OPEN.len();
        let Some(close_rel) = body[body_start..].find(CLOSE) else {
            // Unterminated fence — treat the rest as text so we don't lose it.
            out.push(BodyPart::Text(body[cursor + open_rel..].to_string()));
            break;
        };
        let mut code = body[body_start..body_start + close_rel].to_string();
        if code.ends_with('\n') {
            code.pop();
        }
        out.push(BodyPart::Suggestion(code));
        cursor = body_start + close_rel + CLOSE.len();
    }
    out
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
