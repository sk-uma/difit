use std::collections::HashSet;

use gpui::{
    div, prelude::*, px, App, ElementId, IntoElement, ParentElement, SharedString, Styled,
};

use crate::api::types::{DiffFile, FileStatus};
use crate::ui::theme::{Theme, UI_FONT};

pub fn render_file_list(
    files: &[DiffFile],
    selected: Option<usize>,
    viewed: &HashSet<String>,
    collapsed: &HashSet<String>,
    on_select: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_viewed: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_collapsed: impl Fn(usize, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let viewed_count = files.iter().filter(|f| viewed.contains(&f.path)).count();
    div()
        .w(px(280.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(Theme::BG_ELEVATED)
        .border_r_1()
        .border_color(Theme::BORDER)
        .font_family(UI_FONT)
        .text_color(Theme::TEXT)
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(Theme::BORDER)
                .text_color(Theme::TEXT_MUTED)
                .text_size(px(12.0))
                .child(SharedString::from(format!(
                    "{}/{} viewed",
                    viewed_count,
                    files.len()
                ))),
        )
        .child(
            div()
                .id("file-list-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(files.iter().enumerate().map(|(idx, file)| {
                    let is_selected = Some(idx) == selected;
                    let is_viewed = viewed.contains(&file.path);
                    let is_collapsed = collapsed.contains(&file.path);
                    let sel = on_select.clone();
                    let toggle_v = on_toggle_viewed.clone();
                    let toggle_c = on_toggle_collapsed.clone();
                    file_row(
                        idx,
                        file,
                        is_selected,
                        is_viewed,
                        is_collapsed,
                        move |cx| sel(idx, cx),
                        move |cx| toggle_v(idx, cx),
                        move |cx| toggle_c(idx, cx),
                    )
                })),
        )
}

#[allow(clippy::too_many_arguments)]
fn file_row(
    idx: usize,
    file: &DiffFile,
    selected: bool,
    viewed: bool,
    collapsed: bool,
    on_click: impl Fn(&mut App) + 'static,
    on_toggle_viewed: impl Fn(&mut App) + 'static,
    on_toggle_collapsed: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let bg = if selected {
        Theme::BG_SELECTED
    } else {
        Theme::BG_ELEVATED
    };
    let id: ElementId = ElementId::Integer(idx as u64);
    let toggle_v_id = ElementId::Name(SharedString::from(format!("file-viewed-{idx}")));
    let toggle_c_id = ElementId::Name(SharedString::from(format!("file-collapsed-{idx}")));
    let text_color = if viewed { Theme::TEXT_MUTED } else { Theme::TEXT };

    div()
        .id(id)
        .px_3()
        .py_2()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .bg(bg)
        .hover(|s| s.bg(Theme::BG_HOVER))
        .border_b_1()
        .border_color(Theme::BORDER)
        .cursor_pointer()
        .on_click(move |_event, _window, cx| on_click(cx))
        .child(collapse_chevron(toggle_c_id, collapsed, on_toggle_collapsed))
        .child(viewed_checkbox(toggle_v_id, viewed, on_toggle_viewed))
        .child(status_badge(&file.status))
        .child(
            div()
                .flex_1()
                .text_size(px(13.0))
                .text_color(text_color)
                .child(SharedString::from(file.path.clone())),
        )
        .child(
            div()
                .flex()
                .gap_1()
                .text_size(px(11.0))
                .child(
                    div()
                        .text_color(Theme::FILE_STATUS_ADD)
                        .child(SharedString::from(format!("+{}", file.additions))),
                )
                .child(
                    div()
                        .text_color(Theme::FILE_STATUS_DEL)
                        .child(SharedString::from(format!("-{}", file.deletions))),
                ),
        )
}

fn collapse_chevron(
    id: ElementId,
    collapsed: bool,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let label = if collapsed { "▶" } else { "▼" };
    div()
        .id(id)
        .w(px(14.0))
        .text_size(px(10.0))
        .text_color(Theme::TEXT_MUTED)
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::TEXT_LINK))
        .on_click(move |_e, _w, cx| on_toggle(cx))
        .child(SharedString::from(label))
}

fn viewed_checkbox(
    id: ElementId,
    checked: bool,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let (label, fg) = if checked {
        ("✓", Theme::FILE_STATUS_ADD)
    } else {
        ("·", Theme::TEXT_MUTED)
    };
    div()
        .id(id)
        .w(px(18.0))
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(fg)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_xs()
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, _w, cx| {
            // The outer row's on_click also fires, so a checkbox click both
            // selects the file and toggles its viewed state.
            on_toggle(cx);
        })
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
