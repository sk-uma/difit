use std::sync::Arc;

use gpui::{
    div, list, prelude::*, px, AnyElement, ElementId, IntoElement, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, SharedString, Styled, StyledText,
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

    div()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_row()
        .child(list_el)
        .child(list_scrollbar(state_for_bar, actions_for_bar))
}

/// A thin track + thumb scrollbar driven by `ListState`'s scrollbar
/// helpers. The thumb is draggable; drag state lives in `DifitApp` and
/// the move/end events fire through `DiffActions` so the math stays in
/// one place.
fn list_scrollbar(state: ListState, actions: DiffActions) -> impl IntoElement {
    let viewport_h = f32::from(state.viewport_bounds().size.height);
    let max_offset = f32::from(state.max_offset_for_scrollbar().y);
    let current = f32::from(-state.scroll_px_offset_for_scrollbar().y);

    let (thumb_h, thumb_top, track_space) = if viewport_h > 0.0 && max_offset > 0.0 {
        let total = viewport_h + max_offset;
        let h = (viewport_h * viewport_h / total).max(30.0).min(viewport_h);
        let track_space = viewport_h - h;
        let fraction = (current / max_offset).clamp(0.0, 1.0);
        (h, track_space * fraction, track_space)
    } else {
        (0.0, 0.0, 0.0)
    };

    // mouse_move / mouse_up handling lives on the app's root div so
    // dragging keeps working even when the cursor leaves the track.
    // The thumb only needs mouse_down to seed the drag snapshot.
    let actions_down = actions;

    div()
        .id(ElementId::Name(SharedString::from("scrollbar-track")))
        .w(px(10.0))
        .h_full()
        .flex_shrink_0()
        .bg(Theme::BG_ELEVATED)
        .relative()
        .child(
            div()
                .id(ElementId::Name(SharedString::from("scrollbar-thumb")))
                .absolute()
                .top(px(thumb_top))
                .left(px(2.0))
                .w(px(6.0))
                .h(px(thumb_h))
                .bg(Theme::TEXT_MUTED)
                .rounded_full()
                .cursor_pointer()
                .hover(|s| s.bg(Theme::TEXT))
                .on_mouse_down(
                    MouseButton::Left,
                    move |e: &MouseDownEvent, window, cx| {
                        actions_down(
                            DiffAction::ScrollbarDragStart {
                                mouse_y: f32::from(e.position.y),
                                current_offset_px: current,
                                max_offset_px: max_offset,
                                track_space_px: track_space,
                            },
                            window,
                            cx,
                        );
                    },
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
            label,
        } => expand_row(file_path, *chunk_idx, *direction, label.clone(), ix, actions)
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
                .text_color(Theme::FILE_STATUS_ADD)
                .text_size(px(11.0))
                .child(SharedString::from(format!("+{}", data.additions))),
        )
        .child(
            div()
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
    div()
        .w_full()
        .flex()
        .flex_row()
        .bg(cell.bg)
        .child(add_button(file_path, ix, "u", cell.anchor, actions))
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
    let mut side = div().w_1_2().min_w_0().flex().flex_row().bg(bg);

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

fn expand_row(
    file_path: &SharedString,
    chunk_idx: usize,
    direction: ExpandDirection,
    label: SharedString,
    ix: usize,
    actions: &DiffActions,
) -> impl IntoElement {
    let actions = actions.clone();
    let path = file_path.to_string();
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
                    file_path: path.clone(),
                    chunk_idx,
                    direction,
                },
                window,
                cx,
            )
        })
        .child(label)
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
