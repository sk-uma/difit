//! Pre-computed visual rows for the diff viewer.
//!
//! Every file in the current `DiffResponse` flattens into a single
//! `Vec<DiffRow>` so the virtualized list renders all files in one
//! continuous scroll — matching the React UI. `FileHeader` rows act as
//! sticky-feeling separators with the per-file metadata (status, +/-,
//! viewed / collapsed). Image, notebook and markdown-preview files
//! contribute a single tall block row in place of the per-line content.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use gpui::{combine_highlights, HighlightStyle, Rgba, SharedString};

use crate::api::types::{
    DiffChunk, DiffCommentThread, DiffFile, DiffLine, DiffLineRange, DiffResponse, DiffSide,
    FileStatus, LineType,
};
use crate::highlighting::{extension_for_path, highlight_line};
use crate::ui::actions::ExpandDirection;
use crate::ui::diff_view::DiffViewMode;
use crate::ui::image_viewer::is_image_ext;
use crate::ui::notebook_view::is_notebook_ext;
use crate::ui::theme::Theme;
use crate::word_diff::{word_changes, word_highlight};

pub type HighlightSpans = Arc<Vec<(Range<usize>, HighlightStyle)>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommentAnchor {
    pub side: DiffSide,
    pub line: u32,
}

#[derive(Clone)]
pub struct RenderedCell {
    pub bg: Rgba,
    /// Old (pre-change) line number. Shown in the left gutter column
    /// for Unified mode; the split renderer ignores it.
    pub old_line_number: Option<u32>,
    /// New (post-change) line number. Shown in the right gutter column
    /// for Unified mode; the split renderer uses the appropriate side.
    pub new_line_number: Option<u32>,
    pub anchor: Option<CommentAnchor>,
    pub marker: &'static str,
    pub text: SharedString,
    pub highlights: HighlightSpans,
}

#[derive(Clone)]
pub struct FileHeaderData {
    pub file_idx: usize,
    pub path: SharedString,
    pub old_path: Option<SharedString>,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
    pub thread_count: usize,
    pub viewed: bool,
    pub collapsed: bool,
    pub previewable: bool,
    pub preview_on: bool,
}

#[derive(Clone)]
pub enum DiffRow {
    /// One-line gap that visually separates files.
    Spacer,
    /// Per-file header with status badge, +/-, viewed/collapsed toggles.
    FileHeader(FileHeaderData),
    /// `@@ -10,5 +10,7 @@` style hunk header.
    HunkHeader { file_path: SharedString, text: SharedString },
    /// One line in unified view.
    Unified { file_path: SharedString, cell: RenderedCell },
    /// One row in split view.
    Split {
        file_path: SharedString,
        left: Option<RenderedCell>,
        right: Option<RenderedCell>,
    },
    /// A review comment thread anchored to the preceding diff row.
    Comment(Arc<DiffCommentThread>),
    /// Clickable affordance for fetching more context above (▲) or below (▼).
    Expand {
        file_path: SharedString,
        chunk_idx: usize,
        direction: ExpandDirection,
        /// How many lines are still hidden in this gap (0 = nothing to
        /// expand; the row is dropped before reaching the renderer).
        hidden_lines: u32,
    },
    /// Side-by-side image viewer for one image file.
    Image {
        file_path: SharedString,
        extension: SharedString,
        status: FileStatus,
        old: Option<Arc<Vec<u8>>>,
        new: Option<Arc<Vec<u8>>>,
    },
    /// Tall block containing the entire notebook for one .ipynb file.
    Notebook {
        file_path: SharedString,
        bytes: Arc<Vec<u8>>,
    },
    /// Empty marker row inserted directly under the anchor of an
    /// in-progress comment. The renderer in `diff_view.rs` consults
    /// `RenderedDiff::compose_renderer` to swap this row for the
    /// actual compose form.
    ComposeSlot,
    /// Tall block containing the markdown preview of one .md file.
    MarkdownPreview {
        file_path: SharedString,
        bytes: Arc<Vec<u8>>,
    },
}

/// Per-chunk how many lines of context have been expanded above / below.
pub type ExpansionMap = HashMap<usize, (u32, u32)>;

pub struct BuildContext<'a> {
    pub mode: DiffViewMode,
    pub comments: &'a [DiffCommentThread],
    pub viewed: &'a HashSet<String>,
    pub collapsed: &'a HashSet<String>,
    pub preview_paths: &'a HashSet<String>,
    pub expansions: &'a HashMap<String, ExpansionMap>,
    pub blob_bytes: &'a HashMap<(String, String), Arc<Vec<u8>>>,
    pub old_ref: Option<String>,
    pub new_ref: Option<String>,
    /// Where the in-progress compose form should be inlined, if any.
    pub compose_anchor: Option<ComposeAnchor>,
}

#[derive(Clone)]
pub struct ComposeAnchor {
    pub file_path: String,
    pub anchor: CommentAnchor,
}

/// Walks every file in the diff, producing a flat row list plus a map
/// of `file_path → starting_row_index` for sidebar navigation.
pub fn build_all_rows(
    diff: &DiffResponse,
    ctx: &BuildContext,
) -> (Vec<DiffRow>, HashMap<String, usize>) {
    let mut rows: Vec<DiffRow> = Vec::new();
    let mut starts: HashMap<String, usize> = HashMap::new();

    for (file_idx, file) in diff.files.iter().enumerate() {
        let thread_count = ctx
            .comments
            .iter()
            .filter(|t| t.file_path == file.path)
            .count();
        let viewed = ctx.viewed.contains(&file.path);
        let collapsed = ctx.collapsed.contains(&file.path);
        let ext = extension_for_path(&file.path);
        let preview_on = ctx.preview_paths.contains(&file.path)
            && matches!(ext.as_str(), "md" | "markdown");
        let previewable = matches!(ext.as_str(), "md" | "markdown");

        if file_idx > 0 {
            rows.push(DiffRow::Spacer);
        }

        // Record the start AFTER the spacer so sidebar nav lands on the
        // FileHeader row, not the blank gap before it.
        starts.insert(file.path.clone(), rows.len());

        rows.push(DiffRow::FileHeader(FileHeaderData {
            file_idx,
            path: SharedString::from(file.path.clone()),
            old_path: file.old_path.clone().map(SharedString::from),
            status: file.status.clone(),
            additions: file.additions,
            deletions: file.deletions,
            thread_count,
            viewed,
            collapsed,
            previewable,
            preview_on,
        }));

        if collapsed {
            continue;
        }

        // Dispatch on file type.
        if is_image_ext(&ext) {
            let old_bytes = ctx
                .old_ref
                .as_ref()
                .and_then(|r| ctx.blob_bytes.get(&(file.path.clone(), r.clone())).cloned());
            let new_bytes = ctx
                .new_ref
                .as_ref()
                .and_then(|r| ctx.blob_bytes.get(&(file.path.clone(), r.clone())).cloned());
            rows.push(DiffRow::Image {
                file_path: SharedString::from(file.path.clone()),
                extension: SharedString::from(ext.clone()),
                status: file.status.clone(),
                old: old_bytes,
                new: new_bytes,
            });
            continue;
        }
        if is_notebook_ext(&ext) {
            if let Some(new_ref) = ctx.new_ref.as_ref() {
                if let Some(bytes) = ctx.blob_bytes.get(&(file.path.clone(), new_ref.clone())) {
                    rows.push(DiffRow::Notebook {
                        file_path: SharedString::from(file.path.clone()),
                        bytes: bytes.clone(),
                    });
                    continue;
                }
            }
        }
        if preview_on {
            if let Some(new_ref) = ctx.new_ref.as_ref() {
                if let Some(bytes) = ctx.blob_bytes.get(&(file.path.clone(), new_ref.clone())) {
                    rows.push(DiffRow::MarkdownPreview {
                        file_path: SharedString::from(file.path.clone()),
                        bytes: bytes.clone(),
                    });
                    continue;
                }
            }
        }

        // Text diff content.
        let extension = ext;
        let path_shared = SharedString::from(file.path.clone());
        let expansions = ctx
            .expansions
            .get(&file.path)
            .cloned()
            .unwrap_or_default();
        let new_blob_lines = ctx.new_ref.as_ref().and_then(|r| {
            ctx.blob_bytes
                .get(&(file.path.clone(), r.clone()))
                .map(|bytes| {
                    String::from_utf8_lossy(bytes)
                        .split('\n')
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
        });

        let chunk_count = file.chunks.len();
        for (chunk_idx, chunk) in file.chunks.iter().enumerate() {
            let (above_count, below_count) = expansions.get(&chunk_idx).copied().unwrap_or((0, 0));

            if ctx.mode == DiffViewMode::Unified {
                // Hidden lines above this chunk = gap between prev chunk
                // end and this chunk start, minus what's already been
                // expanded into.
                let prev_end_new = if chunk_idx == 0 {
                    0
                } else {
                    let prev = &file.chunks[chunk_idx - 1];
                    prev.new_start + prev.new_lines - 1
                };
                let gap_above = chunk.new_start.saturating_sub(prev_end_new + 1);
                let hidden_above = gap_above.saturating_sub(above_count);
                if hidden_above > 0 {
                    rows.push(DiffRow::Expand {
                        file_path: path_shared.clone(),
                        chunk_idx,
                        direction: ExpandDirection::Above,
                        hidden_lines: hidden_above,
                    });
                }
                push_expanded_above(
                    &mut rows,
                    &path_shared,
                    chunk,
                    above_count,
                    new_blob_lines.as_deref(),
                    &extension,
                );
            }

            // The `@@ -10,5 +10,7 @@` header is intentionally not
            // emitted — the React UI doesn't render it either; the
            // Expand rows above/below carry the same "context here"
            // affordance.

            match ctx.mode {
                DiffViewMode::Unified => {
                    build_unified_chunk(&mut rows, &path_shared, &chunk.lines, &extension, ctx.comments)
                }
                DiffViewMode::Split => {
                    build_split_chunk(&mut rows, &path_shared, &chunk.lines, &extension, ctx.comments)
                }
            }

            if ctx.mode == DiffViewMode::Unified {
                push_expanded_below(
                    &mut rows,
                    &path_shared,
                    chunk,
                    below_count,
                    new_blob_lines.as_deref(),
                    &extension,
                );
                let chunk_end_new = chunk.new_start + chunk.new_lines - 1;
                // Hidden lines below = gap to next chunk; for the last
                // chunk we don't know the file length so just allow
                // expansion via a fixed step.
                let gap_below = if chunk_idx + 1 < chunk_count {
                    let next = &file.chunks[chunk_idx + 1];
                    next.new_start.saturating_sub(chunk_end_new + 1)
                } else {
                    expand_step() * 4 // sentinel; lets user expand further
                };
                let hidden_below = gap_below.saturating_sub(below_count);
                if hidden_below > 0 {
                    rows.push(DiffRow::Expand {
                        file_path: path_shared.clone(),
                        chunk_idx,
                        direction: ExpandDirection::Below,
                        hidden_lines: hidden_below,
                    });
                }
            }
        }
    }

    // Post-pass: insert a ComposeSlot directly under the row that owns
    // the in-progress compose anchor. Iterating in reverse keeps the
    // already-recorded indices in `starts` valid (only later rows
    // shift down).
    if let Some(ca) = &ctx.compose_anchor {
        for i in (0..rows.len()).rev() {
            let matches = match &rows[i] {
                DiffRow::Unified { file_path, cell } => {
                    file_path.as_ref() == ca.file_path && cell.anchor == Some(ca.anchor)
                }
                DiffRow::Split {
                    file_path,
                    left,
                    right,
                } => {
                    file_path.as_ref() == ca.file_path
                        && (left.as_ref().and_then(|c| c.anchor) == Some(ca.anchor)
                            || right.as_ref().and_then(|c| c.anchor) == Some(ca.anchor))
                }
                _ => false,
            };
            if matches {
                rows.insert(i + 1, DiffRow::ComposeSlot);
                // Bump file_starts entries that pointed past the
                // insertion site so sidebar navigation stays correct.
                for (_, start) in starts.iter_mut() {
                    if *start > i {
                        *start += 1;
                    }
                }
                break;
            }
        }
    }

    (rows, starts)
}

pub fn expand_step() -> u32 {
    10
}

fn build_unified_chunk(
    rows: &mut Vec<DiffRow>,
    file_path: &SharedString,
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
                flush_unified_pair(rows, file_path, &mut del_buf, &mut add_buf, extension, comments);
                let cell = render_unified_cell(line, extension);
                rows.push(DiffRow::Unified {
                    file_path: file_path.clone(),
                    cell,
                });
                push_anchored_comments(rows, line, comments);
            }
            LineType::Hunk | LineType::Header => {}
        }
    }
    flush_unified_pair(rows, file_path, &mut del_buf, &mut add_buf, extension, comments);
}

fn flush_unified_pair(
    rows: &mut Vec<DiffRow>,
    file_path: &SharedString,
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
        rows.push(DiffRow::Unified {
            file_path: file_path.clone(),
            cell,
        });
        push_anchored_comments(rows, del_buf[i], comments);
    }
    for (i, cell) in add_cells.into_iter().enumerate() {
        rows.push(DiffRow::Unified {
            file_path: file_path.clone(),
            cell,
        });
        push_anchored_comments(rows, add_buf[i], comments);
    }
    del_buf.clear();
    add_buf.clear();
}

fn build_split_chunk(
    rows: &mut Vec<DiffRow>,
    file_path: &SharedString,
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
                flush_split_pair(rows, file_path, &mut del_buf, &mut add_buf, extension, comments);
                rows.push(DiffRow::Split {
                    file_path: file_path.clone(),
                    left: Some(render_split_cell(line, extension, Theme::BG, DiffSide::Old)),
                    right: Some(render_split_cell(line, extension, Theme::BG, DiffSide::New)),
                });
                push_anchored_comments(rows, line, comments);
            }
            LineType::Hunk | LineType::Header => {}
        }
    }
    flush_split_pair(rows, file_path, &mut del_buf, &mut add_buf, extension, comments);
}

fn flush_split_pair(
    rows: &mut Vec<DiffRow>,
    file_path: &SharedString,
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
        rows.push(DiffRow::Split {
            file_path: file_path.clone(),
            left,
            right,
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
}

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
    let words: Vec<(Range<usize>, HighlightStyle)> =
        ranges.iter().map(|r| (r.clone(), bg_style)).collect();
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
    // Both gutter columns get values for context lines; deletes only
    // populate `old`, adds only `new` — matching DiffLineRow's two
    // <td> columns.
    let (old_ln, new_ln) = match line.kind {
        LineType::Add => (None, line.new_line_number),
        LineType::Delete | LineType::Remove => (line.old_line_number, None),
        LineType::Normal | LineType::Context => (line.old_line_number, line.new_line_number),
        LineType::Hunk | LineType::Header => (None, None),
    };
    RenderedCell {
        bg,
        old_line_number: old_ln,
        new_line_number: new_ln,
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
    // Split mode shows one gutter per side; populate that side's slot
    // and leave the other None so the row renderer just emits the
    // single column.
    let (old_ln, new_ln) = match side {
        DiffSide::Old => (line_no, None),
        DiffSide::New => (None, line_no),
    };
    RenderedCell {
        bg,
        old_line_number: old_ln,
        new_line_number: new_ln,
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

fn push_expanded_above(
    rows: &mut Vec<DiffRow>,
    file_path: &SharedString,
    chunk: &DiffChunk,
    above_count: u32,
    blob_lines: Option<&[String]>,
    extension: &str,
) {
    if above_count == 0 {
        return;
    }
    let Some(blob) = blob_lines else { return };
    let chunk_first_new = chunk.new_start as usize;
    if chunk_first_new <= 1 {
        return;
    }
    let want = above_count as usize;
    let start_line = chunk_first_new.saturating_sub(want).max(1);
    let line_range = start_line..chunk_first_new;
    let old_offset = chunk.old_start as i64 - chunk.new_start as i64;
    for new_line_no in line_range {
        let blob_idx = new_line_no - 1;
        if blob_idx >= blob.len() {
            break;
        }
        let old_line_no = (new_line_no as i64 + old_offset).max(1) as u32;
        rows.push(DiffRow::Unified {
            file_path: file_path.clone(),
            cell: make_context_cell(&blob[blob_idx], Some(old_line_no), Some(new_line_no as u32), extension),
        });
    }
}

fn push_expanded_below(
    rows: &mut Vec<DiffRow>,
    file_path: &SharedString,
    chunk: &DiffChunk,
    below_count: u32,
    blob_lines: Option<&[String]>,
    extension: &str,
) {
    if below_count == 0 {
        return;
    }
    let Some(blob) = blob_lines else { return };
    let chunk_last_new = (chunk.new_start as usize) + (chunk.new_lines as usize) - 1;
    let start = chunk_last_new + 1;
    let end = (start + below_count as usize).min(blob.len() + 1);
    let old_offset = chunk.old_start as i64 - chunk.new_start as i64;
    for new_line_no in start..end {
        let blob_idx = new_line_no - 1;
        if blob_idx >= blob.len() {
            break;
        }
        let old_line_no = (new_line_no as i64 + old_offset).max(1) as u32;
        rows.push(DiffRow::Unified {
            file_path: file_path.clone(),
            cell: make_context_cell(&blob[blob_idx], Some(old_line_no), Some(new_line_no as u32), extension),
        });
    }
}

fn make_context_cell(
    content: &str,
    old_line_no: Option<u32>,
    new_line_no: Option<u32>,
    extension: &str,
) -> RenderedCell {
    let (text, highlights) = bake_text(content, extension, true);
    let anchor = new_line_no.map(|n| CommentAnchor {
        side: DiffSide::New,
        line: n,
    });
    RenderedCell {
        bg: Theme::BG_ELEVATED,
        old_line_number: old_line_no,
        new_line_number: new_line_no,
        anchor,
        marker: " ",
        text,
        highlights,
    }
}

fn expand_tabs(s: &str) -> String {
    if !s.contains('\t') {
        return s.to_string();
    }
    s.replace('\t', "    ")
}
