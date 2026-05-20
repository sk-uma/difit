use std::sync::Arc;

use gpui::{
    div, list, px, AnyElement, IntoElement, ListState, ParentElement, SharedString, Styled,
    StyledText,
};

use crate::api::types::{DiffCommentThread, DiffFile};
use crate::ui::comment_card::render_thread;
use crate::ui::diff_rows::{DiffRow, RenderedCell};
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
        .text_size(px(12.5));

    let Some(file) = file else {
        return container.child(empty_placeholder("Select a file to see its diff"));
    };

    let container = container.child(file_header(file, thread_count_for_file));

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

    container.child(virtualized_diff_body(rendered))
}

#[derive(Clone)]
pub struct RenderedDiff {
    pub rows: Arc<Vec<DiffRow>>,
    pub list_state: ListState,
}

fn virtualized_diff_body(rendered: RenderedDiff) -> impl IntoElement {
    let rows = rendered.rows.clone();
    list(rendered.list_state, move |ix, _window, _cx| {
        let row = &rows[ix];
        render_row(row).into_any_element()
    })
    .flex_1()
    .min_h_0()
    .with_sizing_behavior(gpui::ListSizingBehavior::Infer)
}

fn render_row(row: &DiffRow) -> AnyElement {
    match row {
        DiffRow::HunkHeader(text) => hunk_header(text.clone()).into_any_element(),
        DiffRow::Unified(cell) => unified_row(cell).into_any_element(),
        DiffRow::Split { left, right } => split_row(left.as_ref(), right.as_ref()).into_any_element(),
        DiffRow::Comment(thread) => render_thread(thread).into_any_element(),
    }
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

fn unified_row(cell: &RenderedCell) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .bg(cell.bg)
        .child(gutter(line_number_label(cell)))
        .child(
            div()
                .w(px(18.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(cell.marker)),
        )
        .child(cell_text(cell))
}

fn split_row(left: Option<&RenderedCell>, right: Option<&RenderedCell>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .child(split_side(left))
        .child(div().w(px(1.0)).h_full().bg(Theme::BORDER))
        .child(split_side(right))
}

fn split_side(cell: Option<&RenderedCell>) -> impl IntoElement {
    let bg = cell.map(|c| c.bg).unwrap_or(Theme::BG_HOVER);
    let mut side = div()
        .w_1_2()
        .min_w_0()
        .flex()
        .flex_row()
        .bg(bg);

    if let Some(cell) = cell {
        side = side
            .child(gutter(line_number_label(cell)))
            .child(cell_text(cell));
    }

    side
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
        .w(px(56.0))
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
