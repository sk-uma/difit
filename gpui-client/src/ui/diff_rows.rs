//! Pre-computed visual rows for the diff viewer.
//!
//! `build_rows` walks a single `DiffFile` once and produces a flat
//! `Vec<DiffRow>` where every per-line cost — tab expansion, syntect
//! highlighting, comment anchoring — has already been paid. The render
//! pass only has to turn rows into elements, and the virtualized `list`
//! widget only materializes rows that are actually on screen.

use std::ops::Range;
use std::sync::Arc;

use gpui::{HighlightStyle, Rgba, SharedString};

use crate::api::types::{
    DiffCommentThread, DiffFile, DiffLine, DiffLineRange, DiffSide, LineType,
};
use crate::highlighting::{extension_for_path, highlight_line};
use crate::ui::diff_view::DiffViewMode;
use crate::ui::theme::Theme;

pub type HighlightSpans = Arc<Vec<(Range<usize>, HighlightStyle)>>;

/// A fully-rendered single line ready for the GPUI layer.
#[derive(Clone)]
pub struct RenderedCell {
    pub bg: Rgba,
    pub line_number: Option<u32>,
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
    // Most files produce slightly more rows than lines (hunk headers + a
    // sprinkling of comment rows). Reserve a small extra margin.
    let line_count: usize = file.chunks.iter().map(|c| c.lines.len()).sum();
    let mut rows: Vec<DiffRow> = Vec::with_capacity(line_count + file.chunks.len() + comments.len());

    for chunk in &file.chunks {
        rows.push(DiffRow::HunkHeader(SharedString::from(chunk.header.clone())));
        match mode {
            DiffViewMode::Unified => build_unified_chunk(&mut rows, &chunk.lines, &extension, comments),
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
    for line in lines {
        let cell = render_unified_cell(line, extension);
        rows.push(DiffRow::Unified(cell));
        push_anchored_comments(rows, line, comments);
    }
}

fn build_split_chunk(
    rows: &mut Vec<DiffRow>,
    lines: &[DiffLine],
    extension: &str,
    comments: &[DiffCommentThread],
) {
    let mut del_buf: Vec<&DiffLine> = Vec::new();
    let mut add_buf: Vec<&DiffLine> = Vec::new();

    let flush = |rows: &mut Vec<DiffRow>,
                 del_buf: &mut Vec<&DiffLine>,
                 add_buf: &mut Vec<&DiffLine>,
                 extension: &str,
                 comments: &[DiffCommentThread]| {
        let pairs = del_buf.len().max(add_buf.len());
        for i in 0..pairs {
            let left_src = del_buf.get(i).copied();
            let right_src = add_buf.get(i).copied();
            rows.push(DiffRow::Split {
                left: left_src.map(|l| render_split_cell(l, extension, Theme::DIFF_DEL_BG)),
                right: right_src.map(|l| render_split_cell(l, extension, Theme::DIFF_ADD_BG)),
            });
            if let Some(l) = left_src {
                push_anchored_comments(rows, l, comments);
            }
            if let Some(l) = right_src {
                push_anchored_comments(rows, l, comments);
            }
        }
        del_buf.clear();
        add_buf.clear();
    };

    for line in lines {
        match line.kind {
            LineType::Delete | LineType::Remove => del_buf.push(line),
            LineType::Add => add_buf.push(line),
            LineType::Normal | LineType::Context => {
                flush(rows, &mut del_buf, &mut add_buf, extension, comments);
                let left = render_split_cell(line, extension, Theme::BG);
                // Context: same line both sides. Reuse the cell on the right
                // (cheap because text/highlights are Arc-shared anyway).
                let right = RenderedCell {
                    line_number: line.new_line_number,
                    ..left.clone()
                };
                rows.push(DiffRow::Split {
                    left: Some(RenderedCell {
                        line_number: line.old_line_number,
                        ..left
                    }),
                    right: Some(right),
                });
                // For context only anchor once (otherwise the same comment
                // would appear twice — once per side).
                push_anchored_comments(rows, line, comments);
            }
            LineType::Hunk | LineType::Header => {}
        }
    }
    flush(rows, &mut del_buf, &mut add_buf, extension, comments);
}

fn render_unified_cell(line: &DiffLine, extension: &str) -> RenderedCell {
    let (bg, marker, do_highlight) = match line.kind {
        LineType::Add => (Theme::DIFF_ADD_BG, "+", true),
        LineType::Delete | LineType::Remove => (Theme::DIFF_DEL_BG, "-", true),
        LineType::Hunk | LineType::Header => (Theme::DIFF_HUNK_BG, " ", false),
        LineType::Normal | LineType::Context => (Theme::BG, " ", true),
    };
    let (text, highlights) = bake_text(&line.content, extension, do_highlight);
    RenderedCell {
        bg,
        line_number: line.new_line_number.or(line.old_line_number),
        marker,
        text,
        highlights,
    }
}

fn render_split_cell(line: &DiffLine, extension: &str, bg: Rgba) -> RenderedCell {
    let do_highlight = !matches!(line.kind, LineType::Hunk | LineType::Header);
    let (text, highlights) = bake_text(&line.content, extension, do_highlight);
    RenderedCell {
        bg,
        line_number: line.new_line_number.or(line.old_line_number),
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
