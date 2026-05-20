use gpui::{div, prelude::*, px, App, ElementId, IntoElement, ParentElement, SharedString, Styled};

use crate::api::types::{DiffFile, FileStatus};
use crate::ui::theme::{Theme, UI_FONT};

pub fn render_file_list(
    files: &[DiffFile],
    selected: Option<usize>,
    on_select: impl Fn(usize, &mut App) + 'static + Clone,
) -> impl IntoElement {
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
                    "{} changed file{}",
                    files.len(),
                    if files.len() == 1 { "" } else { "s" }
                ))),
        )
        .child(
            div()
                .id("file-list-scroll")
                .flex_1()
                .overflow_y_scroll()
                .children(files.iter().enumerate().map(|(idx, file)| {
                    let is_selected = Some(idx) == selected;
                    let cb = on_select.clone();
                    file_row(idx, file, is_selected, move |cx| cb(idx, cx))
                })),
        )
}

fn file_row(
    idx: usize,
    file: &DiffFile,
    selected: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let bg = if selected {
        Theme::BG_SELECTED
    } else {
        Theme::BG_ELEVATED
    };
    let id: ElementId = ElementId::Integer(idx as u64);

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
        .child(status_badge(&file.status))
        .child(
            div()
                .flex_1()
                .text_size(px(13.0))
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
