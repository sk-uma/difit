//! Pre-computed visual rows for the diff viewer.
//!
//! `build_rows` walks a single `DiffFile` once and produces a flat
//! `Vec<DiffRow>` where every per-line cost — tab expansion, syntect
//! highlighting, comment anchoring, intra-line word diff — has already
//! been paid. The render pass only has to turn rows into elements, and
//! the virtualized `list` widget only materializes rows that are
//! actually on screen.

use std::ops::Range;
use std::sync::Arc;

use gpui::{combine_highlights, HighlightStyle, Rgba, SharedString};

use crate::api::types::{
    DiffCommentThread, DiffFile, DiffLine, DiffLineRange, DiffSide, LineType,
};
use crate::highlighting::{extension_for_path, highlight_line};
use crate::ui::diff_view::DiffViewMode;
use crate::ui::theme::Theme;
use crate::word_diff::{word_changes, word_highlight};

pub type HighlightSpans = Arc<Vec<(Range<usize>, HighlightStyle)>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommentAnchor {
    pub side: DiffSide,
    pub line: u32,
}

/// A fully-rendered single line ready for the GPUI layer.
#[derive(Clone)]
pub struct RenderedCell {
    pub bg: Rgba,
    pub line_number: Option<u32>,
    /// Where a comment posted from this row should be anchored. `None` for
    /// non-anchorable rows (e.g. empty buddy cell in split view).
    pub anchor: Option<CommentAnchor>,
    pub marker: &'static str,
    pub text: SharedString,
    pub highlights: HighlightSpans,
}

#[derive(Clone)]
pub enum DiffRow {
    /// `@@ -10,5 +10,7 @@` style header.
    HunkHeader(SharedString),
    /// One line in unified view.
    Unified(RenderedCell),
    /// One row in split view. Either side can be empty (the buddy line was
    /// shorter than the run of adds/deletes).
    Split {
        left: Option<RenderedCell>,
        right: Option<RenderedCell>,
    },
    /// A review comment thread anchored to the preceding diff row.
    Comment(Arc<DiffCommentThread>),
}

pub fn build_rows(
    file: &DiffFile,
    mode: DiffViewMode,
    comments: &[DiffCommentThread],
) -> Vec<DiffRow> {
    let extension = extension_for_path(&file.path);
    let line_count: usize = file.chunks.iter().map(|c| c.lines.len()).sum();
    let mut rows: Vec<DiffRow> =
        Vec::with_capacity(line_count + file.chunks.len() + comments.len());

    for chunk in &file.chunks {
        rows.push(DiffRow::HunkHeader(SharedString::from(chunk.header.clone())));
        match mode {
            DiffViewMode::Unified => {
                build_unified_chunk(&mut rows, &chunk.lines, &extension, comments)
            }
            DiffViewMode::Split => build_split_chunk(&mut rows, &chunk.lines, &extension, comments),
        }
    }

    rows
}

fn build_unified_chunk(
    rows: &mut Vec<DiffRow>,
    lines: &[DiffLine],
    extension: &str,
    comments: &[DiffCommentThread],
) {
    let mut del_buf: Vec<&DiffLine> = Vec::new();
    let mut add_buf: Vec<&DiffLine> = Vec::new();

    for line in lines {
        match line.kind {
            LineType::Delete | LineType::Remove => del_buf.push(line),
            LineType::Add => add_buf.push(line),
            LineType::Normal | LineType::Context => {
                flush_unified_pair(rows, &mut del_buf, &mut add_buf, extension, comments);
                let cell = render_unified_cell(line, extension);
                rows.push(DiffRow::Unified(cell));
                push_anchored_comments(rows, line, comments);
            }
            LineType::Hunk | LineType::Header => {}
        }
    }
    flush_unified_pair(rows, &mut del_buf, &mut add_buf, extension, comments);
}

fn flush_unified_pair(
    rows: &mut Vec<DiffRow>,
    del_buf: &mut Vec<&DiffLine>,
    add_buf: &mut Vec<&DiffLine>,
    extension: &str,
    comments: &[DiffCommentThread],
) {
    let mut del_cells: Vec<RenderedCell> = del_buf
        .iter()
        .map(|l| render_unified_cell(l, extension))
        .collect();
    let mut add_cells: Vec<RenderedCell> = add_buf
        .iter()
        .map(|l| render_unified_cell(l, extension))
        .collect();

    let pairs = del_cells.len().min(add_cells.len());
    for i in 0..pairs {
        apply_word_diff_pair(&mut del_cells[i], &mut add_cells[i]);
    }

    for (i, cell) in del_cells.into_iter().enumerate() {
        rows.push(DiffRow::Unified(cell));
        push_anchored_comments(rows, del_buf[i], comments);
    }
    for (i, cell) in add_cells.into_iter().enumerate() {
        rows.push(DiffRow::Unified(cell));
        push_anchored_comments(rows, add_buf[i], comments);
    }
    del_buf.clear();
    add_buf.clear();
}

fn build_split_chunk(
    rows: &mut Vec<DiffRow>,
    lines: &[DiffLine],
    extension: &str,
    comments: &[DiffCommentThread],
) {
    let mut del_buf: Vec<&DiffLine> = Vec::new();
    let mut add_buf: Vec<&DiffLine> = Vec::new();

    for line in lines {
        match line.kind {
            LineType::Delete | LineType::Remove => del_buf.push(line),
            LineType::Add => add_buf.push(line),
            LineType::Normal | LineType::Context => {
                flush_split_pair(rows, &mut del_buf, &mut add_buf, extension, comments);
                rows.push(DiffRow::Split {
                    left: Some(render_split_cell(line, extension, Theme::BG, DiffSide::Old)),
                    right: Some(render_split_cell(line, extension, Theme::BG, DiffSide::New)),
                });
                push_anchored_comments(rows, line, comments);
            }
            LineType::Hunk | LineType::Header => {}
        }
    }
    flush_split_pair(rows, &mut del_buf, &mut add_buf, extension, comments);
}

fn flush_split_pair(
    rows: &mut Vec<DiffRow>,
    del_buf: &mut Vec<&DiffLine>,
    add_buf: &mut Vec<&DiffLine>,
    extension: &str,
    comments: &[DiffCommentThread],
) {
    let pairs = del_buf.len().max(add_buf.len());
    for i in 0..pairs {
        let left_src = del_buf.get(i).copied();
        let right_src = add_buf.get(i).copied();
        let mut left = left_src
            .map(|l| render_split_cell(l, extension, Theme::DIFF_DEL_BG, DiffSide::Old));
        let mut right = right_src
            .map(|l| render_split_cell(l, extension, Theme::DIFF_ADD_BG, DiffSide::New));
        if let (Some(l), Some(r)) = (left.as_mut(), right.as_mut()) {
            apply_word_diff_pair(l, r);
        }
        rows.push(DiffRow::Split { left, right });
        if let Some(l) = left_src {
            push_anchored_comments(rows, l, comments);
        }
        if let Some(l) = right_src {
            push_anchored_comments(rows, l, comments);
        }
    }
    del_buf.clear();
    add_buf.clear();
}

/// Compute word-level diff between two cell contents and overlay
/// background highlights at the changed token ranges.
fn apply_word_diff_pair(del_cell: &mut RenderedCell, add_cell: &mut RenderedCell) {
    let (left_ranges, right_ranges) = word_changes(&del_cell.text, &add_cell.text);
    overlay_word_bg(del_cell, &left_ranges, Theme::DIFF_DEL_WORD_BG);
    overlay_word_bg(add_cell, &right_ranges, Theme::DIFF_ADD_WORD_BG);
}

fn overlay_word_bg(cell: &mut RenderedCell, ranges: &[Range<usize>], bg: Rgba) {
    if ranges.is_empty() {
        return;
    }
    let bg_style = word_highlight(bg.into());
    let syntax: Vec<(Range<usize>, HighlightStyle)> = (*cell.highlights).clone();
    let words: Vec<(Range<usize>, HighlightStyle)> = ranges
        .iter()
        .map(|r| (r.clone(), bg_style))
        .collect();
    let combined: Vec<_> = combine_highlights(syntax, words).collect();
    cell.highlights = Arc::new(combined);
}

fn render_unified_cell(line: &DiffLine, extension: &str) -> RenderedCell {
    let (bg, marker, do_highlight) = match line.kind {
        LineType::Add => (Theme::DIFF_ADD_BG, "+", true),
        LineType::Delete | LineType::Remove => (Theme::DIFF_DEL_BG, "-", true),
        LineType::Hunk | LineType::Header => (Theme::DIFF_HUNK_BG, " ", false),
        LineType::Normal | LineType::Context => (Theme::BG, " ", true),
    };
    let (text, highlights) = bake_text(&line.content, extension, do_highlight);
    let anchor = unified_anchor(line);
    RenderedCell {
        bg,
        line_number: line.new_line_number.or(line.old_line_number),
        anchor,
        marker,
        text,
        highlights,
    }
}

fn unified_anchor(line: &DiffLine) -> Option<CommentAnchor> {
    match line.kind {
        LineType::Delete | LineType::Remove => line.old_line_number.map(|n| CommentAnchor {
            side: DiffSide::Old,
            line: n,
        }),
        LineType::Add | LineType::Normal | LineType::Context => {
            line.new_line_number.map(|n| CommentAnchor {
                side: DiffSide::New,
                line: n,
            })
        }
        LineType::Hunk | LineType::Header => None,
    }
}

fn render_split_cell(line: &DiffLine, extension: &str, bg: Rgba, side: DiffSide) -> RenderedCell {
    let do_highlight = !matches!(line.kind, LineType::Hunk | LineType::Header);
    let (text, highlights) = bake_text(&line.content, extension, do_highlight);
    let line_no = match side {
        DiffSide::Old => line.old_line_number,
        DiffSide::New => line.new_line_number,
    };
    let anchor = line_no.map(|n| CommentAnchor { side, line: n });
    RenderedCell {
        bg,
        line_number: line_no,
        anchor,
        marker: "",
        text,
        highlights,
    }
}

fn bake_text(content: &str, extension: &str, do_highlight: bool) -> (SharedString, HighlightSpans) {
    let display = expand_tabs(content);

    if !do_highlight || extension.is_empty() {
        return (SharedString::from(display), Arc::new(Vec::new()));
    }

    let highlighted = highlight_line(&display, extension);
    let highlights: Vec<(Range<usize>, HighlightStyle)> = highlighted
        .spans
        .into_iter()
        .map(|(range, color)| (range, HighlightStyle::from(color)))
        .collect();
    (SharedString::from(display), Arc::new(highlights))
}

fn push_anchored_comments(
    rows: &mut Vec<DiffRow>,
    line: &DiffLine,
    comments: &[DiffCommentThread],
) {
    for thread in comments {
        if thread_anchors_to(line, thread) {
            rows.push(DiffRow::Comment(Arc::new(thread.clone())));
        }
    }
}

fn thread_anchors_to(line: &DiffLine, thread: &DiffCommentThread) -> bool {
    let anchor = match thread.position.line {
        DiffLineRange::Single(n) => n,
        DiffLineRange::Range { end, .. } => end,
    };
    match (thread.position.side, line.kind) {
        (DiffSide::Old, LineType::Delete | LineType::Remove) => Some(anchor) == line.old_line_number,
        (DiffSide::New, LineType::Add) => Some(anchor) == line.new_line_number,
        (DiffSide::Old, LineType::Normal | LineType::Context) => {
            Some(anchor) == line.old_line_number
        }
        (DiffSide::New, LineType::Normal | LineType::Context) => {
            Some(anchor) == line.new_line_number
        }
        _ => false,
    }
}

fn expand_tabs(s: &str) -> String {
    // Hot path: avoid touching the allocator when there's nothing to expand.
    if !s.contains('\t') {
        return s.to_string();
    }
    s.replace('\t', "    ")
}
