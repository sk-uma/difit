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

pub type ComposeRenderer = Arc<dyn Fn() -> AnyElement + 'static>;

#[derive(Clone)]
pub struct RenderedDiff {
    pub rows: Arc<Vec<DiffRow>>,
    pub list_state: ListState,
}

pub fn render_main_pane(
    rendered: Option<RenderedDiff>,
    font_size: f32,
    actions: DiffActions,
    compose_renderer: Option<ComposeRenderer>,
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

    container.child(virtualized_body(rendered, font_size, actions, compose_renderer))
}

fn virtualized_body(
    rendered: RenderedDiff,
    font_size: f32,
    actions: DiffActions,
    compose_renderer: Option<ComposeRenderer>,
) -> impl IntoElement {
    let rows = rendered.rows.clone();
    let state_for_bar = rendered.list_state.clone();
    let actions_for_bar = actions.clone();
    let compose = compose_renderer;
    let list_el = list(rendered.list_state, move |ix, _window, _cx| {
        if matches!(rows[ix], DiffRow::ComposeSlot) {
            return compose
                .as_ref()
                .map(|f| f())
                .unwrap_or_else(|| div().into_any_element());
        }
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
        DiffRow::ComposeSlot => div().into_any_element(),
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
            paired_chunk_idx,
            hidden_lines,
        } => expand_row(
            file_path,
            *chunk_idx,
            *direction,
            *paired_chunk_idx,
            *hidden_lines,
            ix,
            actions,
        )
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
    // Layout exactly matches React's DiffLineRow + DiffCodeLine:
    //   [old# gutter][new# gutter][+/-/space prefix][code body]
    // The two gutters share the bg-secondary background and a right
    // border. The prefix column has its own line-coloured background.
    // The "+" affordance for adding a comment lives inside the new#
    // gutter as a click handler (React reveals it on hover; we keep
    // the gutter itself clickable for simplicity).
    let new_gutter_action = actions.clone();
    let new_gutter_path = file_path.to_string();
    let new_gutter_anchor = cell.anchor;
    div()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_start()
        .bg(cell.bg)
        .child(gutter_cell(
            optional_line_label(cell.old_line_number),
            None,
            None,
        ))
        .child(gutter_cell(
            optional_line_label(cell.new_line_number),
            Some(ElementId::Name(SharedString::from(format!(
                "ln-add-u-{ix}"
            )))),
            new_gutter_anchor.map(move |anchor| {
                let actions = new_gutter_action.clone();
                let path = new_gutter_path.clone();
                std::sync::Arc::new(move |w: &mut gpui::Window, cx: &mut gpui::App| {
                    actions(
                        DiffAction::StartComposeAt {
                            file_path: path.clone(),
                            anchor,
                        },
                        w,
                        cx,
                    );
                }) as std::sync::Arc<dyn Fn(&mut gpui::Window, &mut gpui::App)>
            }),
        ))
        .child(marker_cell(cell.marker, cell.bg))
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
        // Split shows one gutter per side; pick whichever line number
        // is populated (set by render_split_cell based on DiffSide).
        let label = optional_line_label(
            cell.old_line_number.or(cell.new_line_number),
        );
        let anchor = cell.anchor;
        let click = anchor.map(|a| {
            let actions = actions.clone();
            let path = file_path.to_string();
            std::sync::Arc::new(move |w: &mut gpui::Window, cx: &mut gpui::App| {
                actions(
                    DiffAction::StartComposeAt {
                        file_path: path.clone(),
                        anchor: a,
                    },
                    w,
                    cx,
                );
            }) as std::sync::Arc<dyn Fn(&mut gpui::Window, &mut gpui::App)>
        });
        side = side
            .child(gutter_cell(
                label,
                Some(ElementId::Name(SharedString::from(format!(
                    "ln-add-{side_tag}-{ix}"
                )))),
                click,
            ))
            .child(marker_cell(cell.marker, cell.bg))
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
    // px_3 matches React's `px-3` (12px each side) on the code column.
    div()
        .flex_1()
        .flex_basis(px(0.0))
        .min_w_0()
        .overflow_hidden()
        .px_3()
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

/// Single line-number column matching React's
/// `bg-github-bg-secondary border-r border-github-border px-2 text-right
/// text-github-text-muted` td. When `click` is Some, the cell becomes a
/// "+" affordance for starting a comment on that line.
fn gutter_cell(
    label: SharedString,
    id: Option<ElementId>,
    click: Option<std::sync::Arc<dyn Fn(&mut gpui::Window, &mut gpui::App)>>,
) -> AnyElement {
    let base = div()
        .w(px(56.0))
        .flex_shrink_0()
        .px_2()
        .py(px(0.0))
        .bg(Theme::BG_ELEVATED)
        .border_r_1()
        .border_color(Theme::BORDER)
        .text_align(gpui::TextAlign::Right)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(Theme::TEXT_MUTED);
    match (id, click) {
        (Some(id), Some(cb)) => base
            .id(id)
            .cursor_pointer()
            .hover(|s| s.bg(Theme::BG_HOVER).text_color(Theme::TEXT_LINK))
            .on_click(move |_e, w, cx| cb(w, cx))
            .child(label)
            .into_any_element(),
        _ => base.child(label).into_any_element(),
    }
}

/// "+/-/space" prefix column. 20px wide, center-aligned, with the
/// line's background colour and a right border that separates it from
/// the code text — matches React's DiffCodeLine span.
fn marker_cell(marker: &'static str, bg: gpui::Rgba) -> impl IntoElement {
    let fg = match marker {
        "+" => Theme::FILE_STATUS_ADD,
        "-" => Theme::FILE_STATUS_DEL,
        _ => Theme::TEXT_MUTED,
    };
    div()
        .w(px(20.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .border_r_1()
        .border_color(Theme::BORDER)
        .text_color(fg)
        .child(SharedString::from(marker))
}

fn optional_line_label(n: Option<u32>) -> SharedString {
    n.map(|v| SharedString::from(v.to_string()))
        .unwrap_or_default()
}

#[allow(dead_code)]
fn line_number_label(cell: &RenderedCell) -> SharedString {
    cell.new_line_number
        .or(cell.old_line_number)
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
    paired_chunk_idx: Option<usize>,
    hidden_lines: u32,
    ix: usize,
    actions: &DiffActions,
) -> impl IntoElement {
    const DEFAULT_EXPAND_COUNT: u32 = 20;
    let path = file_path.to_string();
    let label = SharedString::from(format!(
        "{hidden_lines} {}",
        if hidden_lines == 1 { "line" } else { "lines" }
    ));
    let show_unfold_all = hidden_lines <= DEFAULT_EXPAND_COUNT;

    // Build the gutter content based on direction. For Above/Below
    // it's a single icon button; for Both it's two stacked arrows
    // (or one "unfold all" icon when the remaining gap is small).
    let gutter = match direction {
        ExpandDirection::Above => single_expand_icon(
            "arrow-up-from-line",
            ElementId::Name(SharedString::from(format!("expand-up-{ix}"))),
            {
                let actions = actions.clone();
                let path = path.clone();
                move |w, cx| {
                    actions(
                        DiffAction::ExpandContext {
                            file_path: path.clone(),
                            chunk_idx,
                            direction: ExpandDirection::Above,
                        },
                        w,
                        cx,
                    );
                }
            },
        )
        .into_any_element(),
        ExpandDirection::Below if show_unfold_all => single_expand_icon(
            "unfold-vertical",
            ElementId::Name(SharedString::from(format!("expand-all-{ix}"))),
            {
                let actions = actions.clone();
                let path = path.clone();
                move |w, cx| {
                    actions(
                        DiffAction::ExpandContext {
                            file_path: path.clone(),
                            chunk_idx,
                            direction: ExpandDirection::Below,
                        },
                        w,
                        cx,
                    );
                }
            },
        )
        .into_any_element(),
        ExpandDirection::Below => single_expand_icon(
            "arrow-down-from-line",
            ElementId::Name(SharedString::from(format!("expand-down-{ix}"))),
            {
                let actions = actions.clone();
                let path = path.clone();
                move |w, cx| {
                    actions(
                        DiffAction::ExpandContext {
                            file_path: path.clone(),
                            chunk_idx,
                            direction: ExpandDirection::Below,
                        },
                        w,
                        cx,
                    );
                }
            },
        )
        .into_any_element(),
        ExpandDirection::Both if show_unfold_all => single_expand_icon(
            "unfold-vertical",
            ElementId::Name(SharedString::from(format!("expand-all-{ix}"))),
            {
                let actions = actions.clone();
                let path = path.clone();
                let paired = paired_chunk_idx;
                move |w, cx| {
                    // For "unfold all" on a middle gap, fire both
                    // directions so the gap is fully filled.
                    actions(
                        DiffAction::ExpandContext {
                            file_path: path.clone(),
                            chunk_idx,
                            direction: ExpandDirection::Below,
                        },
                        w,
                        cx,
                    );
                    if let Some(p) = paired {
                        actions(
                            DiffAction::ExpandContext {
                                file_path: path.clone(),
                                chunk_idx: p,
                                direction: ExpandDirection::Above,
                            },
                            w,
                            cx,
                        );
                    }
                }
            },
        )
        .into_any_element(),
        ExpandDirection::Both => stacked_arrows(
            ix,
            {
                let actions = actions.clone();
                let path = path.clone();
                move |w, cx| {
                    // Up arrow (top button) → grow content upward from
                    // the button: reveal lines just after the previous
                    // chunk (top of the gap).
                    actions(
                        DiffAction::ExpandContext {
                            file_path: path.clone(),
                            chunk_idx,
                            direction: ExpandDirection::Below,
                        },
                        w,
                        cx,
                    );
                }
            },
            {
                let actions = actions.clone();
                let path = path.clone();
                let paired = paired_chunk_idx;
                move |w, cx| {
                    // Down arrow (bottom button) → grow content
                    // downward: reveal lines just before the next
                    // chunk (bottom of the gap).
                    if let Some(p) = paired {
                        actions(
                            DiffAction::ExpandContext {
                                file_path: path.clone(),
                                chunk_idx: p,
                                direction: ExpandDirection::Above,
                            },
                            w,
                            cx,
                        );
                    }
                }
            },
        )
        .into_any_element(),
    };

    // React's ExpandButton has a row per direction: single-direction
    // buttons are ~24px tall, `Both` stacks two of them inside a 48px
    // row.
    let row_height = if matches!(direction, ExpandDirection::Both) && !show_unfold_all {
        48.0
    } else {
        24.0
    };

    div()
        .id(ElementId::Name(SharedString::from(format!(
            "expand-row-{ix}"
        ))))
        .w_full()
        .h(px(row_height))
        .flex()
        .flex_row()
        .items_stretch()
        .bg(Theme::BG_HOVER)
        .border_t_1()
        .border_b_1()
        .border_color(Theme::BORDER)
        .child(
            div()
                .w(px(64.0))
                .flex_shrink_0()
                .border_r_1()
                .border_color(Theme::BORDER)
                .child(gutter),
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

fn single_expand_icon(
    icon_name: &'static str,
    id: ElementId,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_SELECTED))
        .on_click(move |_e, w, cx| on_click(w, cx))
        .child(crate::ui::widgets::icon(icon_name, 14.0, Theme::TEXT))
}

fn stacked_arrows(
    ix: usize,
    on_up: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
    on_down: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "expand-up-{ix}"
                ))))
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(Theme::BG_SELECTED))
                .on_click(move |_e, w, cx| on_up(w, cx))
                .child(crate::ui::widgets::icon(
                    "arrow-up-from-line",
                    11.0,
                    Theme::TEXT,
                )),
        )
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "expand-down-{ix}"
                ))))
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(Theme::BG_SELECTED))
                .on_click(move |_e, w, cx| on_down(w, cx))
                .child(crate::ui::widgets::icon(
                    "arrow-down-from-line",
                    11.0,
                    Theme::TEXT,
                )),
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
