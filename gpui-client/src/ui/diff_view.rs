use std::sync::Arc;

use gpui::{
    canvas, div, list, prelude::*, px, AnyElement, ElementId, IntoElement, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, SharedString, Styled, StyledText,
};

use crate::api::types::FileStatus;
use crate::ui::actions::{DiffAction, DiffActions, ExpandDirection};
use crate::ui::comment_card::render_thread;
use crate::ui::diff_rows::{DiffRow, FileHeaderData, RenderedCell};
use crate::ui::image_viewer::render_image_diff;
use crate::ui::markdown_view::render_markdown;
use crate::ui::notebook_view::render_notebook;
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
            DiffViewMode::Split => "Side by Side",
        }
    }
}

#[derive(Clone)]
pub struct RenderedDiff {
    pub rows: Arc<Vec<DiffRow>>,
    pub list_state: ListState,
}

pub fn render_main_pane(
    rendered: Option<RenderedDiff>,
    font_size: f32,
    actions: DiffActions,
) -> impl IntoElement {
    // Generous line-height (~1.55× the font size) matches Zed's buffer
    // breathing room — diff lines are dense and otherwise feel cramped.
    let container = div()
        .flex_1()
        .h_full()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_col()
        .bg(Theme::BG)
        .text_color(Theme::TEXT)
        .font_family(MONO_FONT())
        .text_size(px(font_size))
        .line_height(px((font_size * 1.55).round()));

    let Some(rendered) = rendered else {
        return container.child(empty_placeholder("Loading diff…"));
    };

    if rendered.rows.is_empty() {
        return container.child(empty_placeholder("No files in this diff."));
    }

    container.child(virtualized_body(rendered, font_size, actions))
}

fn virtualized_body(
    rendered: RenderedDiff,
    font_size: f32,
    actions: DiffActions,
) -> impl IntoElement {
    let rows = rendered.rows.clone();
    let state_for_bar = rendered.list_state.clone();
    let actions_for_bar = actions.clone();
    let list_el = list(rendered.list_state, move |ix, _window, _cx| {
        render_row(&rows[ix], ix, font_size, &actions).into_any_element()
    })
    .flex_1()
    .min_h_0()
    .with_sizing_behavior(gpui::ListSizingBehavior::Infer);

    let row_count = rendered.rows.len();
    div()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_row()
        .child(list_el)
        .child(list_scrollbar(state_for_bar, row_count, actions_for_bar))
}

/// A thin track + thumb scrollbar.
///
/// The geometry is row-index based rather than pixel-based: `ListState`
/// measures heights lazily, so reading `max_offset_for_scrollbar` while
/// scrolling makes the thumb shrink as new rows are sized. By using
/// `rows.len()` + `logical_scroll_top().item_ix` we get a stable thumb
/// size at the cost of pixel-perfect accuracy inside oversized rows
/// (image / notebook blocks).
fn list_scrollbar(state: ListState, total_items: usize, actions: DiffActions) -> impl IntoElement {
    const ESTIMATED_ROW_HEIGHT: f32 = 18.0;

    let viewport_h = f32::from(state.viewport_bounds().size.height);
    let total = total_items as f32;
    let top_item = state.logical_scroll_top().item_ix as f32;

    let (thumb_h, thumb_top, track_space, scroll_range) =
        if viewport_h > 0.0 && total > 1.0 {
            let visible_items = (viewport_h / ESTIMATED_ROW_HEIGHT).max(1.0).min(total);
            let scroll_range = (total - visible_items).max(1.0);
            let h = (viewport_h * (visible_items / total))
                .max(30.0)
                .min(viewport_h);
            let track_space = (viewport_h - h).max(0.0);
            let fraction = (top_item / scroll_range).clamp(0.0, 1.0);
            (h, track_space * fraction, track_space, scroll_range)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

    // Mouse handlers are registered through `window.on_mouse_event` in
    // the canvas paint phase so they fire for *all* events — even when
    // the cursor is outside the scrollbar (or the whole window) while a
    // drag is in progress.
    let actions_down = actions.clone();
    let actions_move = actions.clone();
    let actions_up = actions;

    div()
        .w(px(10.0))
        .h_full()
        .flex_shrink_0()
        .bg(Theme::BG_ELEVATED)
        .relative()
        .child(
            div()
                .absolute()
                .top(px(thumb_top))
                .left(px(2.0))
                .w(px(6.0))
                .h(px(thumb_h))
                .bg(Theme::TEXT_MUTED)
                .rounded_full()
                .child(
                    canvas(
                        |_, _, _| (),
                        move |thumb_bounds, _, window, _| {
                            let actions_down = actions_down.clone();
                            window.on_mouse_event(move |ev: &MouseDownEvent, _, w, cx| {
                                if ev.button != MouseButton::Left {
                                    return;
                                }
                                if !thumb_bounds.contains(&ev.position) {
                                    return;
                                }
                                actions_down(
                                    DiffAction::ScrollbarDragStart {
                                        mouse_y: f32::from(ev.position.y),
                                        start_top_item: top_item,
                                        scroll_range_items: scroll_range,
                                        track_space_px: track_space,
                                    },
                                    w,
                                    cx,
                                );
                            });
                            let actions_move = actions_move.clone();
                            window.on_mouse_event(move |ev: &MouseMoveEvent, _, w, cx| {
                                if !ev.dragging() {
                                    return;
                                }
                                actions_move(
                                    DiffAction::ScrollbarDragMove {
                                        mouse_y: f32::from(ev.position.y),
                                    },
                                    w,
                                    cx,
                                );
                            });
                            let actions_up = actions_up.clone();
                            window.on_mouse_event(move |_ev: &MouseUpEvent, _, w, cx| {
                                actions_up(DiffAction::ScrollbarDragEnd, w, cx);
                            });
                        },
                    )
                    .w_full()
                    .h_full(),
                ),
        )
}

fn render_row(row: &DiffRow, ix: usize, font_size: f32, actions: &DiffActions) -> AnyElement {
    match row {
        DiffRow::Spacer => div().h(px(8.0)).into_any_element(),
        DiffRow::FileHeader(data) => render_file_header(data, actions).into_any_element(),
        DiffRow::HunkHeader { text, .. } => hunk_header(text.clone()).into_any_element(),
        DiffRow::Unified { file_path, cell } => {
            unified_row(file_path, cell, ix, actions).into_any_element()
        }
        DiffRow::Split {
            file_path,
            left,
            right,
        } => split_row(file_path, left.as_ref(), right.as_ref(), ix, actions).into_any_element(),
        DiffRow::Comment(thread) => render_thread(thread, actions).into_any_element(),
        DiffRow::Expand {
            file_path,
            chunk_idx,
            direction,
            hidden_lines,
        } => expand_row(file_path, *chunk_idx, *direction, *hidden_lines, ix, actions)
            .into_any_element(),
        DiffRow::Image {
            file_path: _,
            extension,
            status,
            old,
            new,
        } => render_image_diff(
            &crate::api::types::DiffFile {
                path: String::new(), // not used by image_viewer header
                old_path: None,
                status: status.clone(),
                additions: 0,
                deletions: 0,
                chunks: Vec::new(),
                is_generated: None,
            },
            extension,
            old.clone(),
            new.clone(),
        )
        .into_any_element(),
        DiffRow::Notebook { bytes, .. } => render_notebook(bytes, font_size).into_any_element(),
        DiffRow::MarkdownPreview { bytes, .. } => {
            let text = String::from_utf8_lossy(bytes).to_string();
            render_markdown(&text, font_size).into_any_element()
        }
    }
}

fn render_file_header(data: &FileHeaderData, actions: &DiffActions) -> impl IntoElement {
    let path_display = match &data.old_path {
        Some(old) if old.as_ref() != data.path.as_ref() => {
            SharedString::from(format!("{} → {}", old, data.path))
        }
        _ => data.path.clone(),
    };
    let actions_collapse = actions.clone();
    let actions_viewed = actions.clone();
    let actions_open = actions.clone();
    let actions_preview = actions.clone();
    let actions_copy_all = actions.clone();
    let path_collapse = data.path.to_string();
    let path_viewed = data.path.to_string();
    let path_open = data.path.to_string();
    let path_preview = data.path.to_string();
    let path_copy = data.path.to_string();

    let chevron = if data.collapsed { "▶" } else { "▼" };

    let mut row = div()
        .w_full()
        .flex_shrink_0()
        .px_3()
        .py_2()
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_sm()
        .mt_2()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "fh-chevron-{}",
                    data.file_idx
                ))))
                .text_color(Theme::TEXT_MUTED)
                .cursor_pointer()
                .hover(|s| s.text_color(Theme::TEXT_LINK))
                .on_click(move |_e, w, cx| {
                    actions_collapse(
                        DiffAction::ToggleCollapsed {
                            file_path: path_collapse.clone(),
                        },
                        w,
                        cx,
                    );
                })
                .child(SharedString::from(chevron)),
        )
        .child(status_badge(&data.status))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(13.0))
                .text_color(if data.viewed {
                    Theme::TEXT_MUTED
                } else {
                    Theme::TEXT
                })
                .child(path_display),
        )
        .child(
            div()
                .flex_shrink_0()
                .whitespace_nowrap()
                .text_color(Theme::FILE_STATUS_ADD)
                .text_size(px(11.0))
                .child(SharedString::from(format!("+{}", data.additions))),
        )
        .child(
            div()
                .flex_shrink_0()
                .whitespace_nowrap()
                .text_color(Theme::FILE_STATUS_DEL)
                .text_size(px(11.0))
                .child(SharedString::from(format!("-{}", data.deletions))),
        );

    if data.thread_count > 0 {
        row = row.child(
            div()
                .text_color(Theme::TEXT_LINK)
                .text_size(px(11.0))
                .child(SharedString::from(format!(
                    "💬 {}",
                    data.thread_count
                ))),
        );
    }

    if data.previewable {
        let label = if data.preview_on { "Code" } else { "Preview" };
        row = row.child(small_button(
            format!("fh-preview-{}", data.file_idx),
            label,
            move |w, cx| {
                actions_preview(
                    DiffAction::TogglePreview {
                        file_path: path_preview.clone(),
                    },
                    w,
                    cx,
                );
            },
        ));
    }

    row = row
        .child(small_button(
            format!("fh-open-{}", data.file_idx),
            "Open",
            move |w, cx| {
                actions_open(
                    DiffAction::OpenFileInEditor {
                        file_path: path_open.clone(),
                    },
                    w,
                    cx,
                );
            },
        ))
        .child(small_button(
            format!("fh-copy-{}", data.file_idx),
            "Copy",
            move |w, cx| {
                actions_copy_all(
                    DiffAction::CopyAllPromptForFile {
                        file_path: path_copy.clone(),
                    },
                    w,
                    cx,
                );
            },
        ))
        .child(viewed_pill(data.file_idx, data.viewed, move |w, cx| {
            actions_viewed(
                DiffAction::ToggleViewed {
                    file_path: path_viewed.clone(),
                },
                w,
                cx,
            );
        }));

    row
}

fn viewed_pill(
    file_idx: usize,
    viewed: bool,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let label = if viewed { "✓ Viewed" } else { "Viewed" };
    let (bg, fg) = if viewed {
        (Theme::FILE_STATUS_ADD, Theme::TEXT)
    } else {
        (Theme::BG_ELEVATED, Theme::TEXT_MUTED)
    };
    div()
        .id(ElementId::Name(SharedString::from(format!(
            "fh-viewed-{file_idx}"
        ))))
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(if viewed { Theme::FILE_STATUS_ADD } else { Theme::BORDER })
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.0))
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, w, cx| on_click(w, cx))
        .child(SharedString::from(label))
}

fn small_button(
    id: String,
    label: &'static str,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .px_2()
        .py_1()
        .text_size(px(11.0))
        .text_color(Theme::TEXT_MUTED)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_sm()
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER).text_color(Theme::TEXT))
        .on_click(move |_e, w, cx| on_click(w, cx))
        .child(SharedString::from(label))
}

fn status_badge(status: &FileStatus) -> impl IntoElement {
    let (letter, color) = match status {
        FileStatus::Added => ("A", Theme::FILE_STATUS_ADD),
        FileStatus::Deleted => ("D", Theme::FILE_STATUS_DEL),
        FileStatus::Modified => ("M", Theme::FILE_STATUS_MOD),
        FileStatus::Renamed => ("R", Theme::TEXT_LINK),
    };
    div()
        .w(px(18.0))
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(color)
        .border_1()
        .border_color(color)
        .rounded_xs()
        .child(SharedString::from(letter))
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

fn unified_row(
    file_path: &SharedString,
    cell: &RenderedCell,
    ix: usize,
    actions: &DiffActions,
) -> impl IntoElement {
    // items_start keeps the gutter / marker pinned to the first line
    // when the code column wraps to multiple lines. min_w_0 +
    // overflow_hidden on the row stops a long text column from
    // expanding the row past its container — without this, GPUI's
    // flex layout sizes the text column to its intrinsic (unwrapped)
    // width and the line escapes the viewport.
    div()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_start()
        .bg(cell.bg)
        .child(add_button(file_path, ix, "u", cell.anchor, actions))
        .child(gutter(line_number_label(cell)))
        .child(
            div()
                .w(px(18.0))
                .flex_shrink_0()
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(cell.marker)),
        )
        .child(cell_text(cell))
}

fn split_row(
    file_path: &SharedString,
    left: Option<&RenderedCell>,
    right: Option<&RenderedCell>,
    ix: usize,
    actions: &DiffActions,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_stretch()
        .child(split_side(file_path, left, ix, "l", actions))
        .child(div().w(px(1.0)).h_full().bg(Theme::BORDER))
        .child(split_side(file_path, right, ix, "r", actions))
}

fn split_side(
    file_path: &SharedString,
    cell: Option<&RenderedCell>,
    ix: usize,
    side_tag: &'static str,
    actions: &DiffActions,
) -> impl IntoElement {
    let bg = cell.map(|c| c.bg).unwrap_or(Theme::BG_HOVER);
    let mut side = div().w_1_2().min_w_0().flex().flex_row().items_start().bg(bg);

    if let Some(cell) = cell {
        side = side
            .child(add_button(file_path, ix, side_tag, cell.anchor, actions))
            .child(gutter(line_number_label(cell)))
            .child(cell_text(cell));
    }

    side
}

fn add_button(
    file_path: &SharedString,
    ix: usize,
    tag: &'static str,
    anchor: Option<crate::ui::diff_rows::CommentAnchor>,
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
        let path = file_path.to_string();
        btn = btn
            .cursor_pointer()
            .hover(|s| s.bg(Theme::BG_HOVER).text_color(Theme::TEXT_LINK))
            .on_click(move |_e, window, cx| {
                actions(
                    DiffAction::StartComposeAt {
                        file_path: path.clone(),
                        anchor,
                    },
                    window,
                    cx,
                )
            })
            .child(SharedString::from("+"));
    }
    btn
}

fn cell_text(cell: &RenderedCell) -> impl IntoElement {
    // Mirror React's `whitespace-pre-wrap break-all`: long code lines
    // wrap inside the column instead of overflowing horizontally.
    //
    // The combination is important — `whitespace_normal` is what makes
    // GPUI's text layer compute a wrap_width from the available flex
    // space, and `overflow_hidden` on a flex child with min_w_0 stops
    // the column from inflating to its intrinsic content size during
    // the flex layout pass. Without the overflow_hidden the row grows
    // wider than its parent and `wrap_width` ends up matching the
    // unwrapped natural width, so no wrap happens.
    div()
        .flex_1()
        .flex_basis(px(0.0))
        .min_w_0()
        .overflow_hidden()
        .px_1()
        .whitespace_normal()
        .child(styled_text(cell))
}

fn styled_text(cell: &RenderedCell) -> StyledText {
    if cell.highlights.is_empty() {
        StyledText::new(cell.text.clone())
    } else {
        StyledText::new(cell.text.clone()).with_highlights(cell.highlights.iter().cloned())
    }
}

/// Line-number gutter. 64px is wide enough for 5-digit numbers at the
/// default mono font size; tight wrapping was making "1234" stack as
/// "123\n4" in tall files.
fn gutter(label: SharedString) -> impl IntoElement {
    div()
        .w(px(64.0))
        .flex_shrink_0()
        .px_2()
        .text_align(gpui::TextAlign::Right)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(Theme::TEXT_MUTED)
        .child(label)
}

fn line_number_label(cell: &RenderedCell) -> SharedString {
    cell.line_number
        .map(|n| SharedString::from(n.to_string()))
        .unwrap_or_default()
}

/// React's ExpandButton: gutter-width column with an icon, then a
/// muted "N lines" label. We pick the icon by `hidden_lines` — small
/// gaps get a single "unfold all" button, big gaps get a directional
/// arrow.
fn expand_row(
    file_path: &SharedString,
    chunk_idx: usize,
    direction: ExpandDirection,
    hidden_lines: u32,
    ix: usize,
    actions: &DiffActions,
) -> impl IntoElement {
    const DEFAULT_EXPAND_COUNT: u32 = 20;
    let actions = actions.clone();
    let path = file_path.to_string();
    let dir_tag = match direction {
        ExpandDirection::Above => "above",
        ExpandDirection::Below => "below",
    };
    let show_unfold_all = hidden_lines <= DEFAULT_EXPAND_COUNT;
    let icon_name = if show_unfold_all {
        "unfold-vertical"
    } else if direction == ExpandDirection::Above {
        "arrow-up-from-line"
    } else {
        "arrow-down-from-line"
    };
    let icon_size = if show_unfold_all { 16.0 } else { 12.0 };
    let label = SharedString::from(format!(
        "{hidden_lines} {}",
        if hidden_lines == 1 { "line" } else { "lines" }
    ));

    div()
        .id(ElementId::Name(SharedString::from(format!(
            "expand-{ix}-{dir_tag}"
        ))))
        .w_full()
        .h(px(24.0))
        .flex()
        .flex_row()
        .items_stretch()
        .bg(Theme::BG_HOVER)
        .border_t_1()
        .border_b_1()
        .border_color(Theme::BORDER)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_SELECTED))
        .on_click(move |_e, window, cx| {
            actions(
                DiffAction::ExpandContext {
                    file_path: path.clone(),
                    chunk_idx,
                    direction,
                },
                window,
                cx,
            )
        })
        .child(
            // Left gutter: matches the line-number column width so the
            // icon lines up with the diff numbers.
            div()
                .w(px(64.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .border_r_1()
                .border_color(Theme::BORDER)
                .child(crate::ui::widgets::icon(
                    icon_name,
                    icon_size,
                    Theme::TEXT,
                )),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .px_3()
                .text_color(Theme::TEXT_MUTED)
                .text_size(px(12.0))
                .font_family(MONO_FONT())
                .child(label),
        )
}

fn empty_placeholder(msg: &'static str) -> impl IntoElement {
    div()
        .w_full()
        .p_8()
        .text_color(Theme::TEXT_MUTED)
        .child(SharedString::from(msg))
}

/// Count comment threads attached to a specific file (used by app.rs).
pub fn count_threads_for_file<'a>(
    file_path: &str,
    threads: impl IntoIterator<Item = &'a crate::api::types::DiffCommentThread>,
) -> usize {
    threads.into_iter().filter(|t| t.file_path == file_path).count()
}
