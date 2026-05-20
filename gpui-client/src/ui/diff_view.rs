use std::sync::Arc;

use gpui::{
    div, list, prelude::*, px, AnyElement, ElementId, IntoElement, ListState, ParentElement,
    SharedString, Styled, StyledText,
};

use crate::api::types::{DiffCommentThread, DiffFile};
use crate::ui::actions::{DiffAction, DiffActions, ExpandDirection};
use crate::ui::comment_card::render_thread;
use crate::ui::diff_rows::{CommentAnchor, DiffRow, RenderedCell};
use crate::ui::theme::{Theme, MONO_FONT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewMode {
    Unified,
    Split,
}

impl DiffViewMode {
    pub fn toggle(self) -> Self {
        match self {
            DiffViewMode::Unified => DiffViewMode::Split,
            DiffViewMode::Split => DiffViewMode::Unified,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DiffViewMode::Unified => "Unified",
            DiffViewMode::Split => "Split",
        }
    }
}

pub fn render_diff(
    file: Option<&DiffFile>,
    rendered: Option<RenderedDiff>,
    thread_count_for_file: usize,
    collapsed: bool,
    font_size: f32,
    actions: DiffActions,
) -> impl IntoElement {
    let container = div()
        .flex_1()
        .h_full()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_col()
        .bg(Theme::BG)
        .text_color(Theme::TEXT)
        .font_family(MONO_FONT)
        .text_size(px(font_size));

    let Some(file) = file else {
        return container.child(empty_placeholder("Select a file to see its diff"));
    };

    let container = container.child(file_header(file, thread_count_for_file));

    if collapsed {
        return container.child(empty_placeholder(
            "Collapsed. Use the chevron in the sidebar to expand.",
        ));
    }

    let Some(rendered) = rendered else {
        return container.child(empty_placeholder(
            if file.is_generated.unwrap_or(false) {
                "Generated file — collapsed by default."
            } else {
                "No textual diff."
            },
        ));
    };

    if rendered.rows.is_empty() {
        return container.child(empty_placeholder(
            if file.is_generated.unwrap_or(false) {
                "Generated file — collapsed by default."
            } else {
                "No textual diff."
            },
        ));
    }

    container.child(virtualized_diff_body(rendered, actions))
}

#[derive(Clone)]
pub struct RenderedDiff {
    pub rows: Arc<Vec<DiffRow>>,
    pub list_state: ListState,
}

fn virtualized_diff_body(rendered: RenderedDiff, actions: DiffActions) -> impl IntoElement {
    let rows = rendered.rows.clone();
    list(rendered.list_state, move |ix, _window, _cx| {
        let row = &rows[ix];
        render_row(row, ix, &actions).into_any_element()
    })
    .flex_1()
    .min_h_0()
    .with_sizing_behavior(gpui::ListSizingBehavior::Infer)
}

fn render_row(row: &DiffRow, ix: usize, actions: &DiffActions) -> AnyElement {
    match row {
        DiffRow::HunkHeader(text) => hunk_header(text.clone()).into_any_element(),
        DiffRow::Unified(cell) => unified_row(cell, ix, actions).into_any_element(),
        DiffRow::Split { left, right } => {
            split_row(left.as_ref(), right.as_ref(), ix, actions).into_any_element()
        }
        DiffRow::Comment(thread) => render_thread(thread, actions).into_any_element(),
        DiffRow::Expand {
            chunk_idx,
            direction,
            label,
        } => expand_row(*chunk_idx, *direction, label.clone(), ix, actions).into_any_element(),
    }
}

fn expand_row(
    chunk_idx: usize,
    direction: ExpandDirection,
    label: SharedString,
    ix: usize,
    actions: &DiffActions,
) -> impl IntoElement {
    let actions = actions.clone();
    div()
        .id(ElementId::Name(SharedString::from(format!(
            "expand-{ix}-{}",
            match direction {
                ExpandDirection::Above => "above",
                ExpandDirection::Below => "below",
            }
        ))))
        .w_full()
        .px_3()
        .py_1()
        .bg(Theme::DIFF_HUNK_BG)
        .text_color(Theme::TEXT_LINK)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, window, cx| {
            actions(
                DiffAction::ExpandContext {
                    chunk_idx,
                    direction,
                },
                window,
                cx,
            )
        })
        .child(label)
}

fn hunk_header(header: SharedString) -> impl IntoElement {
    div()
        .w_full()
        .bg(Theme::DIFF_HUNK_BG)
        .px_3()
        .py_1()
        .text_color(Theme::DIFF_HUNK_TEXT)
        .child(header)
}

fn unified_row(cell: &RenderedCell, ix: usize, actions: &DiffActions) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .bg(cell.bg)
        .child(add_button(ix, "u", cell.anchor, actions))
        .child(gutter(line_number_label(cell)))
        .child(
            div()
                .w(px(18.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(cell.marker)),
        )
        .child(cell_text(cell))
}

fn split_row(
    left: Option<&RenderedCell>,
    right: Option<&RenderedCell>,
    ix: usize,
    actions: &DiffActions,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .child(split_side(left, ix, "l", actions))
        .child(div().w(px(1.0)).h_full().bg(Theme::BORDER))
        .child(split_side(right, ix, "r", actions))
}

fn split_side(
    cell: Option<&RenderedCell>,
    ix: usize,
    side_tag: &'static str,
    actions: &DiffActions,
) -> impl IntoElement {
    let bg = cell.map(|c| c.bg).unwrap_or(Theme::BG_HOVER);
    let mut side = div()
        .w_1_2()
        .min_w_0()
        .flex()
        .flex_row()
        .bg(bg);

    if let Some(cell) = cell {
        side = side
            .child(add_button(ix, side_tag, cell.anchor, actions))
            .child(gutter(line_number_label(cell)))
            .child(cell_text(cell));
    }

    side
}

/// A small "+" affordance that opens the compose form pre-filled for this
/// line. Disabled (no click handler) for rows without an anchor.
fn add_button(
    ix: usize,
    tag: &'static str,
    anchor: Option<CommentAnchor>,
    actions: &DiffActions,
) -> impl IntoElement {
    let id = ElementId::Name(SharedString::from(format!("add-{tag}-{ix}")));
    let mut btn = div()
        .id(id)
        .w(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(11.0));
    if let Some(anchor) = anchor {
        let actions = actions.clone();
        btn = btn
            .cursor_pointer()
            .hover(|s| s.bg(Theme::BG_HOVER).text_color(Theme::TEXT_LINK))
            .on_click(move |_e, window, cx| {
                actions(DiffAction::StartComposeAt(anchor), window, cx)
            })
            .child(SharedString::from("+"));
    }
    btn
}

fn cell_text(cell: &RenderedCell) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .px_1()
        .whitespace_nowrap()
        .child(styled_text(cell))
}

fn styled_text(cell: &RenderedCell) -> StyledText {
    if cell.highlights.is_empty() {
        StyledText::new(cell.text.clone())
    } else {
        StyledText::new(cell.text.clone()).with_highlights(cell.highlights.iter().cloned())
    }
}

fn gutter(label: SharedString) -> impl IntoElement {
    div()
        .w(px(48.0))
        .px_2()
        .text_color(Theme::TEXT_MUTED)
        .child(label)
}

fn line_number_label(cell: &RenderedCell) -> SharedString {
    cell.line_number
        .map(|n| SharedString::from(n.to_string()))
        .unwrap_or_default()
}

fn file_header(file: &DiffFile, thread_count: usize) -> impl IntoElement {
    let path_display = match &file.old_path {
        Some(old) if old != &file.path => format!("{old} → {}", file.path),
        _ => file.path.clone(),
    };

    let mut row = div()
        .w_full()
        .flex_shrink_0()
        .px_4()
        .py_2()
        .bg(Theme::BG_ELEVATED)
        .border_b_1()
        .border_color(Theme::BORDER)
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(13.0))
                .child(SharedString::from(path_display)),
        )
        .child(
            div()
                .text_color(Theme::FILE_STATUS_ADD)
                .text_size(px(11.0))
                .child(SharedString::from(format!("+{}", file.additions))),
        )
        .child(
            div()
                .text_color(Theme::FILE_STATUS_DEL)
                .text_size(px(11.0))
                .child(SharedString::from(format!("-{}", file.deletions))),
        );

    if thread_count > 0 {
        row = row.child(
            div()
                .text_color(Theme::TEXT_LINK)
                .text_size(px(11.0))
                .child(SharedString::from(format!(
                    "💬 {thread_count} thread{}",
                    if thread_count == 1 { "" } else { "s" }
                ))),
        );
    }

    row
}

fn empty_placeholder(msg: &'static str) -> impl IntoElement {
    div()
        .w_full()
        .p_8()
        .text_color(Theme::TEXT_MUTED)
        .child(SharedString::from(msg))
}

/// Count comment threads attached to a specific file, for the header badge.
pub fn count_threads_for_file<'a>(
    file_path: &str,
    threads: impl IntoIterator<Item = &'a DiffCommentThread>,
) -> usize {
    threads.into_iter().filter(|t| t.file_path == file_path).count()
}
