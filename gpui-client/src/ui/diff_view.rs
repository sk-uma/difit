use gpui::{
    div, prelude::*, px, AnyElement, HighlightStyle, IntoElement, ParentElement, Rgba,
    SharedString, Styled, StyledText,
};

use crate::api::types::{
    DiffChunk, DiffCommentThread, DiffFile, DiffLine, DiffLineRange, DiffSide, LineType,
};
use crate::highlighting::{extension_for_path, highlight_line};
use crate::ui::comment_card::render_thread;
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
    mode: DiffViewMode,
    comments: &[DiffCommentThread],
) -> impl IntoElement {
    let mut container = div()
        .id("diff-scroll")
        .flex_1()
        .h_full()
        .min_h_0()
        .min_w_0()
        .bg(Theme::BG)
        .text_color(Theme::TEXT)
        .font_family(MONO_FONT)
        .text_size(px(12.5))
        .overflow_y_scroll();

    let Some(file) = file else {
        return container.child(empty_placeholder("Select a file to see its diff"));
    };

    container = container.child(file_header(file, comments.len()));

    if file.chunks.is_empty() {
        container = container.child(empty_placeholder(
            if file.is_generated.unwrap_or(false) {
                "Generated file — collapsed by default."
            } else {
                "No textual diff."
            },
        ));
        return container;
    }

    let extension = extension_for_path(&file.path);

    for chunk in &file.chunks {
        container = container.child(hunk_header(&chunk.header));
        match mode {
            DiffViewMode::Unified => {
                for line in &chunk.lines {
                    container = container.child(unified_row(line, &extension));
                    for thread in threads_anchored_to(line, comments) {
                        container = container.child(render_thread(thread));
                    }
                }
            }
            DiffViewMode::Split => {
                let mut pending_threads: Vec<&DiffCommentThread> = Vec::new();
                for row in pair_lines_for_split(chunk) {
                    // Threads on either side of the row should appear under it.
                    if let Some(line) = row.left_source {
                        pending_threads.extend(threads_anchored_to(line, comments));
                    }
                    if let Some(line) = row.right_source {
                        pending_threads.extend(threads_anchored_to(line, comments));
                    }
                    container = container.child(split_row(row, &extension));
                    for thread in pending_threads.drain(..) {
                        container = container.child(render_thread(thread));
                    }
                }
            }
        }
    }

    container
}

fn threads_anchored_to<'a>(
    line: &DiffLine,
    threads: &'a [DiffCommentThread],
) -> impl Iterator<Item = &'a DiffCommentThread> {
    let old = line.old_line_number;
    let new = line.new_line_number;
    let kind = line.kind.clone();
    threads.iter().filter(move |t| {
        let (side, anchor) = (t.position.side, anchor_line(&t.position.line));
        match (side, kind.clone()) {
            (DiffSide::Old, LineType::Delete | LineType::Remove) => Some(anchor) == old,
            (DiffSide::New, LineType::Add) => Some(anchor) == new,
            (DiffSide::Old, LineType::Normal | LineType::Context) => Some(anchor) == old,
            (DiffSide::New, LineType::Normal | LineType::Context) => Some(anchor) == new,
            _ => false,
        }
    })
}

fn anchor_line(range: &DiffLineRange) -> u32 {
    match range {
        DiffLineRange::Single(n) => *n,
        DiffLineRange::Range { end, .. } => *end,
    }
}

fn file_header(file: &DiffFile, thread_count: usize) -> impl IntoElement {
    let path_display = match &file.old_path {
        Some(old) if old != &file.path => format!("{old} → {}", file.path),
        _ => file.path.clone(),
    };

    let mut row = div()
        .w_full()
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

fn hunk_header(header: &str) -> impl IntoElement {
    div()
        .w_full()
        .bg(Theme::DIFF_HUNK_BG)
        .px_3()
        .py_1()
        .text_color(Theme::DIFF_HUNK_TEXT)
        .child(SharedString::from(header.to_string()))
}

// -- Unified --------------------------------------------------------------

fn unified_row(line: &DiffLine, extension: &str) -> impl IntoElement {
    let (bg, marker, do_highlight) = match line.kind {
        LineType::Add => (Theme::DIFF_ADD_BG, "+", true),
        LineType::Delete | LineType::Remove => (Theme::DIFF_DEL_BG, "-", true),
        LineType::Hunk | LineType::Header => (Theme::DIFF_HUNK_BG, " ", false),
        LineType::Normal | LineType::Context => (Theme::BG, " ", true),
    };

    div()
        .w_full()
        .flex()
        .flex_row()
        .bg(bg)
        .child(gutter(line.old_line_number))
        .child(gutter(line.new_line_number))
        .child(
            div()
                .w(px(18.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(marker)),
        )
        .child(
            div()
                .flex_1()
                .px_1()
                .whitespace_nowrap()
                .child(line_content(&line.content, extension, do_highlight)),
        )
}

// -- Split ----------------------------------------------------------------

/// One side of a split row. `None` means the row has no content on that side.
struct SplitCell {
    bg: Rgba,
    line_number: Option<u32>,
    text: Option<String>,
    highlight: bool,
}

struct SplitRow<'a> {
    left: SplitCell,
    right: SplitCell,
    /// Source DiffLine for the left cell, used to anchor comment threads.
    /// `None` for empty cells; for context rows we set only `left_source`
    /// (since the same line appears on both sides, it would otherwise
    /// double-report comments).
    left_source: Option<&'a DiffLine>,
    right_source: Option<&'a DiffLine>,
}

fn empty_cell(bg: Rgba) -> SplitCell {
    SplitCell {
        bg,
        line_number: None,
        text: None,
        highlight: false,
    }
}

fn pair_lines_for_split(chunk: &DiffChunk) -> Vec<SplitRow<'_>> {
    let mut rows: Vec<SplitRow<'_>> = Vec::with_capacity(chunk.lines.len());
    let mut del_buf: Vec<&DiffLine> = Vec::new();
    let mut add_buf: Vec<&DiffLine> = Vec::new();

    fn flush<'a>(
        rows: &mut Vec<SplitRow<'a>>,
        del_buf: &mut Vec<&'a DiffLine>,
        add_buf: &mut Vec<&'a DiffLine>,
    ) {
        let pairs = del_buf.len().max(add_buf.len());
        for i in 0..pairs {
            let left_src = del_buf.get(i).copied();
            let right_src = add_buf.get(i).copied();
            let left = left_src
                .map(|l| SplitCell {
                    bg: Theme::DIFF_DEL_BG,
                    line_number: l.old_line_number,
                    text: Some(l.content.clone()),
                    highlight: true,
                })
                .unwrap_or_else(|| empty_cell(Theme::BG_HOVER));
            let right = right_src
                .map(|l| SplitCell {
                    bg: Theme::DIFF_ADD_BG,
                    line_number: l.new_line_number,
                    text: Some(l.content.clone()),
                    highlight: true,
                })
                .unwrap_or_else(|| empty_cell(Theme::BG_HOVER));
            rows.push(SplitRow {
                left,
                right,
                left_source: left_src,
                right_source: right_src,
            });
        }
        del_buf.clear();
        add_buf.clear();
    }

    for line in &chunk.lines {
        match line.kind {
            LineType::Delete | LineType::Remove => del_buf.push(line),
            LineType::Add => add_buf.push(line),
            LineType::Normal | LineType::Context => {
                flush(&mut rows, &mut del_buf, &mut add_buf);
                rows.push(SplitRow {
                    left: SplitCell {
                        bg: Theme::BG,
                        line_number: line.old_line_number,
                        text: Some(line.content.clone()),
                        highlight: true,
                    },
                    right: SplitCell {
                        bg: Theme::BG,
                        line_number: line.new_line_number,
                        text: Some(line.content.clone()),
                        highlight: true,
                    },
                    left_source: Some(line),
                    right_source: None,
                });
            }
            LineType::Hunk | LineType::Header => {}
        }
    }
    flush(&mut rows, &mut del_buf, &mut add_buf);
    rows
}

fn split_row(row: SplitRow<'_>, extension: &str) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .child(split_side(row.left, extension))
        .child(div().w(px(1.0)).h_full().bg(Theme::BORDER))
        .child(split_side(row.right, extension))
}

fn split_side(cell: SplitCell, extension: &str) -> AnyElement {
    let mut content_box = div()
        .flex_1()
        .px_1()
        .whitespace_nowrap();
    if let Some(text) = &cell.text {
        content_box = content_box.child(line_content(text, extension, cell.highlight));
    }

    div()
        .w_1_2()
        .flex()
        .flex_row()
        .bg(cell.bg)
        .child(gutter(cell.line_number))
        .child(content_box)
        .into_any_element()
}

// -- Shared helpers ------------------------------------------------------

fn line_content(content: &str, extension: &str, highlight: bool) -> StyledText {
    let display = expand_tabs(content);

    if !highlight || extension.is_empty() {
        return StyledText::new(display);
    }

    let highlighted = highlight_line(&display, extension);
    let highlights: Vec<_> = highlighted
        .spans
        .into_iter()
        .map(|(range, color)| (range, HighlightStyle::from(color)))
        .collect();

    StyledText::new(display).with_highlights(highlights)
}

fn gutter(value: Option<u32>) -> impl IntoElement {
    div()
        .w(px(56.0))
        .px_2()
        .text_color(Theme::TEXT_MUTED)
        .child(SharedString::from(
            value.map(|n| n.to_string()).unwrap_or_default(),
        ))
}

fn empty_placeholder(msg: &'static str) -> impl IntoElement {
    div()
        .w_full()
        .p_8()
        .text_color(Theme::TEXT_MUTED)
        .child(SharedString::from(msg))
}

fn expand_tabs(s: &str) -> String {
    s.replace('\t', "    ")
}
